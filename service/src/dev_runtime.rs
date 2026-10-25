//! Explicit local Docker adapters with exact-owned cleanup.
#[path = "dev_runtime_export.rs"]
mod export;
#[path = "dev_runtime_local.rs"]
mod local;
use crate::dev_worker::{self, Frame, OwnedChild};
pub use export::DockerSourceExporter;
pub(crate) use local::DockerLocalRuntime;
use rand::RngCore;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub(crate) trait DevRuntime: Send + Sync {
    fn run(
        &self,
        request: &RuntimeRequest,
        cancelled: &AtomicBool,
        event: &mut dyn FnMut(RuntimeEvent) -> crate::Result<()>,
    ) -> std::result::Result<Receipt, RuntimeFailure>;
    fn is_stopped(&self, request_id: &str) -> crate::Result<bool>;
}
pub(crate) struct RuntimeRequest {
    pub request_id: String,
    pub prompt: String,
}
pub(crate) enum RuntimeEvent {
    Ready { session_id: String },
    Dispatching,
}
pub(crate) struct Receipt {
    pub session_id: String,
}
#[derive(Clone, Copy)]
pub(crate) struct RuntimeFailure {
    pub diagnostic: RuntimeDiagnostic,
    pub may_have_sent: bool,
    pub cleanup_unknown: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeDiagnostic {
    OperatorUnsupported,
    CliMissing,
    CliUnavailable,
    CliUnsafe,
    RuntimeRootUnavailable,
    RuntimeRootUnsafe,
    SocketMissing,
    SocketUnavailable,
    SocketUnsafe,
    RootfulSocketMissing,
    RootfulSocketUnavailable,
    RootfulSocketUnsafe,
    ConfigIsolationUnavailable,
    DaemonCheckFailed,
    RootlessUnverified,
    RootfulDaemonCheckFailed,
    RootfulUnverified,
    ImageInspectionFailed,
    WorkerImageRejected,
    UnverifiedReceipt,
}
impl RuntimeDiagnostic {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::OperatorUnsupported => "Dev runtime requires an unprivileged operator; no automatic setup or replay",
            Self::CliMissing => "Required Docker CLI was not found; no automatic setup or replay",
            Self::CliUnavailable => "Required Docker CLI could not be accessed; no automatic setup or replay",
            Self::CliUnsafe => "Docker CLI identity did not pass safety checks; no automatic setup or replay",
            Self::RuntimeRootUnavailable => "Private runtime directory could not be verified; no automatic setup or replay",
            Self::RuntimeRootUnsafe => "Private runtime directory did not pass safety checks; no automatic setup or replay",
            Self::SocketMissing => "Required private rootless Docker socket was not found; no automatic setup or replay",
            Self::SocketUnavailable => "Private Docker socket could not be verified; no automatic setup or replay",
            Self::SocketUnsafe => "Private Docker socket did not pass safety checks; no automatic setup or replay",
            Self::RootfulSocketMissing => "Required rootful Docker socket was not found; no automatic setup or replay",
            Self::RootfulSocketUnavailable => "Rootful Docker socket could not be verified; no automatic setup or replay",
            Self::RootfulSocketUnsafe => "Rootful Docker socket did not pass safety checks; no automatic setup or replay",
            Self::ConfigIsolationUnavailable => "Isolated Docker configuration location could not be verified; no automatic setup or replay",
            Self::DaemonCheckFailed => "Rootless Docker daemon check could not be completed; no automatic setup or replay",
            Self::RootlessUnverified => "Docker endpoint was not verified rootless; no automatic setup or replay",
            Self::RootfulDaemonCheckFailed => "Rootful Docker daemon check could not be completed; no automatic setup or replay",
            Self::RootfulUnverified => "Docker endpoint was not verified rootful; no automatic setup or replay",
            Self::ImageInspectionFailed => "Required local worker image could not be verified; no automatic setup or replay",
            Self::WorkerImageRejected => "Required local worker image did not pass validation; no automatic setup or replay",
            Self::UnverifiedReceipt => "Runtime did not complete a verified receipt; no replay",
        }
    }

    fn during(self, operation: Self) -> Self {
        if self == Self::UnverifiedReceipt {
            operation
        } else {
            self
        }
    }
}
// Worker/event errors are untrusted. Only classifications made here can become specific copy.
impl From<String> for RuntimeDiagnostic {
    fn from(_: String) -> Self {
        Self::UnverifiedReceipt
    }
}
impl From<&str> for RuntimeDiagnostic {
    fn from(_: &str) -> Self {
        Self::UnverifiedReceipt
    }
}
type RuntimeResult<T> = std::result::Result<T, RuntimeDiagnostic>;

fn metadata_diagnostic(
    error: std::io::Error,
    missing: RuntimeDiagnostic,
    unavailable: RuntimeDiagnostic,
) -> RuntimeDiagnostic {
    if error.kind() == std::io::ErrorKind::NotFound {
        missing
    } else {
        unavailable
    }
}

fn docker_json(bytes: &[u8]) -> RuntimeResult<Value> {
    serde_json::from_slice(bytes).map_err(|_| RuntimeDiagnostic::UnverifiedReceipt)
}

fn verify_rootless(info: &Value) -> RuntimeResult<()> {
    if info.as_array().is_some_and(|v| {
        v.iter().any(|s| {
            s.as_str()
                .is_some_and(|s| s == "name=rootless" || s.starts_with("name=rootless,"))
        })
    }) {
        Ok(())
    } else {
        Err(RuntimeDiagnostic::RootlessUnverified)
    }
}

fn verify_rootful(info: &Value) -> RuntimeResult<()> {
    let options = info
        .as_array()
        .filter(|v| v.len() == 3)
        .ok_or(RuntimeDiagnostic::RootfulUnverified)?;
    let mut seen = 0;
    for option in options {
        let bit = match option.as_str() {
            Some("name=apparmor" | "name=apparmor,profile=default") => 1,
            Some("name=seccomp,profile=builtin") => 2,
            Some("name=cgroupns") => 4,
            _ => return Err(RuntimeDiagnostic::RootfulUnverified),
        };
        if seen & bit != 0 {
            return Err(RuntimeDiagnostic::RootfulUnverified);
        }
        seen |= bit;
    }
    if seen == 7 {
        Ok(())
    } else {
        Err(RuntimeDiagnostic::RootfulUnverified)
    }
}

fn rootful_directory_safe(uid: u32, mode: u32) -> bool {
    uid == 0 && mode & libc::S_IFMT == libc::S_IFDIR && mode & 0o022 == 0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockerMode {
    Rootless,
    Rootful,
}
impl DockerMode {
    fn socket_path(self, runtime_root: &Path) -> PathBuf {
        match self {
            Self::Rootless => runtime_root.join("docker.sock"),
            Self::Rootful => PathBuf::from("/run/docker.sock"),
        }
    }

    fn socket_diagnostics(self) -> (RuntimeDiagnostic, RuntimeDiagnostic, RuntimeDiagnostic) {
        match self {
            Self::Rootless => (
                RuntimeDiagnostic::SocketMissing,
                RuntimeDiagnostic::SocketUnavailable,
                RuntimeDiagnostic::SocketUnsafe,
            ),
            Self::Rootful => (
                RuntimeDiagnostic::RootfulSocketMissing,
                RuntimeDiagnostic::RootfulSocketUnavailable,
                RuntimeDiagnostic::RootfulSocketUnsafe,
            ),
        }
    }

    fn socket_safe(self, owner_uid: u32, mode: u32, operator_uid: u32) -> bool {
        mode & libc::S_IFMT == libc::S_IFSOCK
            && match self {
                Self::Rootless => owner_uid == operator_uid && mode & 0o007 == 0,
                Self::Rootful => owner_uid == 0 && mode & 0o7777 == 0o660,
            }
    }

    fn daemon_check_failed(self) -> RuntimeDiagnostic {
        match self {
            Self::Rootless => RuntimeDiagnostic::DaemonCheckFailed,
            Self::Rootful => RuntimeDiagnostic::RootfulDaemonCheckFailed,
        }
    }

    fn verify_daemon_info(self, info: &Value) -> RuntimeResult<()> {
        match self {
            Self::Rootless => verify_rootless(info),
            Self::Rootful => verify_rootful(info),
        }
    }
}

fn worker_image(image: &Value) -> RuntimeResult<String> {
    let entries = image
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or(RuntimeDiagnostic::ImageInspectionFailed)?;
    if entries[0]["Config"]["Volumes"]
        .as_object()
        .is_some_and(|v| !v.is_empty())
    {
        return Err(RuntimeDiagnostic::WorkerImageRejected);
    }
    entries[0]["Id"]
        .as_str()
        .filter(|s| image_id(s))
        .map(str::to_owned)
        .ok_or(RuntimeDiagnostic::ImageInspectionFailed)
}
pub(crate) struct DockerRuntime {
    runtime_root: PathBuf,
    mode: DockerMode,
}
const IMAGE: &str = "smith-dev-worker:1.19.3";
const LABEL: &str = "org.smith.dev.request";
const KIND_LABEL: &str = "org.smith.dev.kind";
const OWNER_LABEL: &str = "org.smith.dev.owner";

struct Identity {
    id: String,
    running: bool,
    image: String,
    owner: String,
}
fn hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn image_id(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|s| hex(s, 64))
}
fn create_arguments<'a>(options: &[&'a str]) -> Vec<&'a str> {
    // The local driver requires compression off when retaining only one log file.
    let mut args = vec![
        "container",
        "create",
        "--log-driver=local",
        "--log-opt=max-size=1m",
        "--log-opt=max-file=1",
        "--log-opt=compress=false",
    ];
    args.extend_from_slice(options);
    args
}

fn name(request: &str) -> crate::Result<String> {
    if !dev_worker::valid_id(request) {
        return Err("Invalid runtime request identity".into());
    }
    Ok(format!("smith-dev-{request}"))
}
impl DockerRuntime {
    pub(crate) fn new(runtime_root: PathBuf) -> Self {
        Self {
            runtime_root,
            mode: DockerMode::Rootless,
        }
    }

    pub(crate) fn new_rootful(runtime_root: PathBuf) -> Self {
        Self {
            runtime_root,
            mode: DockerMode::Rootful,
        }
    }

    fn command(&self) -> RuntimeResult<Command> {
        let uid = unsafe { libc::geteuid() };
        if uid == 0 || uid != unsafe { libc::getuid() } {
            return Err(RuntimeDiagnostic::OperatorUnsupported);
        }
        for path in ["/usr", "/usr/bin", "/usr/bin/docker"] {
            let m = fs::symlink_metadata(path).map_err(|error| {
                metadata_diagnostic(
                    error,
                    if path == "/usr/bin/docker" {
                        RuntimeDiagnostic::CliMissing
                    } else {
                        RuntimeDiagnostic::CliUnavailable
                    },
                    RuntimeDiagnostic::CliUnavailable,
                )
            })?;
            if m.file_type().is_symlink()
                || m.uid() != 0
                || m.mode() & 0o022 != 0
                || (path == "/usr/bin/docker"
                    && (!m.is_file() || m.mode() & 0o111 == 0 || m.mode() & 0o6000 != 0))
            {
                return Err(RuntimeDiagnostic::CliUnsafe);
            }
        }
        if !self.runtime_root.is_absolute() {
            return Err(RuntimeDiagnostic::RuntimeRootUnsafe);
        }
        let mut path = PathBuf::new();
        for component in self.runtime_root.components() {
            if !matches!(component, Component::RootDir | Component::Normal(_)) {
                return Err(RuntimeDiagnostic::RuntimeRootUnsafe);
            }
            path.push(component);
            let m = fs::symlink_metadata(&path)
                .map_err(|_| RuntimeDiagnostic::RuntimeRootUnavailable)?;
            if !m.is_dir()
                || m.file_type().is_symlink()
                || (m.uid() != 0 && m.uid() != uid)
                || m.mode() & 0o022 != 0
            {
                return Err(RuntimeDiagnostic::RuntimeRootUnsafe);
            }
        }
        let root = fs::symlink_metadata(&self.runtime_root)
            .map_err(|_| RuntimeDiagnostic::RuntimeRootUnavailable)?;
        if root.uid() != uid || root.mode() & 0o077 != 0 {
            return Err(RuntimeDiagnostic::RuntimeRootUnsafe);
        }
        let (missing, unavailable, unsafe_socket) = self.mode.socket_diagnostics();
        if self.mode == DockerMode::Rootful {
            for path in ["/", "/run"] {
                let m = fs::symlink_metadata(path).map_err(|_| unavailable)?;
                if !rootful_directory_safe(m.uid(), m.mode()) {
                    return Err(unsafe_socket);
                }
            }
        }
        let socket = self.mode.socket_path(&self.runtime_root);
        let m = fs::symlink_metadata(&socket)
            .map_err(|error| metadata_diagnostic(error, missing, unavailable))?;
        if !self.mode.socket_safe(m.uid(), m.mode(), uid) {
            return Err(unsafe_socket);
        }
        // A guaranteed missing config parent prevents loading host auth/config/plugin settings.
        // Do not create a directory or touch credentials, including in is_stopped().
        if !matches!(fs::symlink_metadata("/nonexistent"), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            return Err(RuntimeDiagnostic::ConfigIsolationUnavailable);
        }
        let mut command = Command::new("/usr/bin/docker");
        self.configure_command(&mut command)?;
        Ok(command)
    }

    fn configure_command(&self, command: &mut Command) -> RuntimeResult<()> {
        let socket = self.mode.socket_path(&self.runtime_root);
        let endpoint = socket.to_str().ok_or("Invalid Docker socket path")?;
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", "/nonexistent")
            .env("LANG", "C")
            .args([
                "--config",
                "/nonexistent/smith-docker-config",
                "--host",
                &format!("unix://{endpoint}"),
            ])
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        Ok(())
    }

    fn verify_daemon(&self, until: Instant, cancel: &dyn Fn() -> bool) -> RuntimeResult<()> {
        let info = self
            .json(
                &["info", "--format", "{{json .SecurityOptions}}"],
                until,
                cancel,
            )
            .map_err(|diagnostic| diagnostic.during(self.mode.daemon_check_failed()))?;
        self.mode.verify_daemon_info(&info)
    }

    fn capture(
        &self,
        args: &[&str],
        until: Instant,
        cancel: &dyn Fn() -> bool,
    ) -> RuntimeResult<Vec<u8>> {
        if cancel() || Instant::now() >= until {
            return Err("Docker command cancelled or timed out".into());
        }
        let mut command = self.command()?;
        command.args(args);
        let mut child = OwnedChild(
            command
                .spawn()
                .map_err(|_| RuntimeDiagnostic::CliUnavailable)?,
        );
        let mut out = child.0.stdout.take().ok_or("Missing Docker output")?;
        dev_worker::nonblocking(out.as_raw_fd())?;
        let mut bytes = Vec::new();
        let mut eof = false;
        loop {
            if cancel() || Instant::now() >= until {
                return Err("Docker command cancelled or timed out".into());
            }
            if !eof {
                let mut buf = [0; 4096];
                match out.read(&mut buf) {
                    Ok(0) => eof = true,
                    Ok(n) => {
                        bytes.extend_from_slice(&buf[..n]);
                        if bytes.len() > dev_worker::MAX_FRAME {
                            return Err("Docker output limit exceeded".into());
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => return Err("Cannot read Docker output".into()),
                }
            }
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| "Cannot observe Docker CLI")?
            {
                if !status.success() {
                    return Err("Docker command failed; daemon state may be unknown".into());
                }
                if eof {
                    return Ok(bytes);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn json(
        &self,
        args: &[&str],
        until: Instant,
        cancel: &dyn Fn() -> bool,
    ) -> RuntimeResult<Value> {
        docker_json(&self.capture(args, until, cancel)?)
    }
    fn owned(
        &self,
        request: &str,
        until: Instant,
        cancel: &dyn Fn() -> bool,
    ) -> RuntimeResult<Option<Identity>> {
        let name = name(request)?;
        // Successful listing distinguishes absence from an unavailable daemon. Never interpret an inspect error as absent.
        let bytes = self.capture(
            &[
                "container",
                "ls",
                "--all",
                "--no-trunc",
                "--filter",
                &format!("name=^/{name}$"),
                "--format",
                "{{json .}}",
            ],
            until,
            cancel,
        )?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "Invalid container listing")?;
        let lines: Vec<_> = text.lines().filter(|s| !s.is_empty()).collect();
        if lines.is_empty() {
            return Ok(None);
        }
        if lines.len() != 1 {
            return Err("Ambiguous container identity".into());
        }
        let listing: Value =
            serde_json::from_str(lines[0]).map_err(|_| "Invalid container listing")?;
        if listing["Names"] != name {
            return Err("Container name mismatch".into());
        }
        let id = listing["ID"]
            .as_str()
            .filter(|s| hex(s, 64))
            .ok_or("Invalid container ID")?;
        let value = self.json(&["container", "inspect", id], until, cancel)?;
        let entries = value
            .as_array()
            .filter(|v| v.len() == 1)
            .ok_or("Invalid container inspect shape")?;
        let v = &entries[0];
        if v["Id"] != id
            || v["Name"] != format!("/{name}")
            || v["Config"]["Labels"][LABEL] != request
            || v["Config"]["Labels"][KIND_LABEL] != dev_worker::MARKER
        {
            return Err("Container ownership mismatch; no mutation permitted".into());
        }
        let owner = v["Config"]["Labels"][OWNER_LABEL]
            .as_str()
            .filter(|s| hex(s, 32))
            .ok_or("Missing immutable runtime owner")?
            .to_string();
        let running = v["State"]["Running"]
            .as_bool()
            .ok_or("Invalid container state")?;
        // Paused/restarting/removing/dead are not a verified stopped result.
        let status = v["State"]["Status"]
            .as_str()
            .ok_or("Missing container status")?;
        if !running && !matches!(status, "created" | "exited") {
            return Err("Container stop state uncertain".into());
        }
        let image = v["Image"]
            .as_str()
            .filter(|s| image_id(s))
            .ok_or("Invalid container image identity")?
            .to_string();
        Ok(Some(Identity {
            id: id.into(),
            running,
            image,
            owner,
        }))
    }
    fn cleanup(
        &self,
        request: &str,
        owner: &str,
        expected: Option<&str>,
        until: Instant,
    ) -> RuntimeResult<()> {
        let Some(identity) = self.owned(request, until, &|| false)? else {
            // Once creation was attempted, absence is not proof of completion: a timed-out daemon request may still create it.
            return Err("Attempted container creation has no verified identity".into());
        };
        if identity.owner != owner || expected.is_some_and(|id| id != identity.id) {
            return Err("Container identity changed; stop refused".into());
        }
        if identity.running {
            let _ = self.capture(
                &["container", "stop", "--time", "1", &identity.id],
                until,
                &|| false,
            );
        }
        match self.owned(request, until, &|| false)? {
            Some(after) if after.id == identity.id && after.owner == owner && !after.running => {
                Ok(())
            }
            _ => Err("Owned container stop unverified".into()),
        }
    }
}

// The guard is armed before create. Unwinding/early-return also attempts exact-owned cleanup.
struct ContainerGuard<'a> {
    runtime: &'a DockerRuntime,
    request: &'a str,
    id: Option<String>,
    owner: String,
    ambiguous: bool,
    armed: bool,
}
impl ContainerGuard<'_> {
    fn stop(&mut self, until: Instant) -> RuntimeResult<()> {
        let result = self
            .runtime
            .cleanup(self.request, &self.owner, self.id.as_deref(), until);
        self.armed = false;
        result?;
        if self.ambiguous {
            Err("Create/start acknowledgement ambiguous; reconciliation required".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for ContainerGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.runtime.cleanup(
                self.request,
                &self.owner,
                self.id.as_deref(),
                Instant::now() + Duration::from_secs(3),
            );
        }
    }
}
impl DevRuntime for DockerRuntime {
    fn is_stopped(&self, request_id: &str) -> crate::Result<bool> {
        self.owned(request_id, Instant::now() + Duration::from_secs(5), &|| {
            false
        })
        .map(|owned| owned.is_none_or(|v| !v.running))
        .map_err(|diagnostic| diagnostic.message().to_owned())
    }
    fn run(
        &self,
        request: &RuntimeRequest,
        cancelled: &AtomicBool,
        event: &mut dyn FnMut(RuntimeEvent) -> crate::Result<()>,
    ) -> std::result::Result<Receipt, RuntimeFailure> {
        let started = Instant::now();
        let total = started + Duration::from_secs(90);
        // Reserve cleanup budget inside the total host deadline.
        let work = total - Duration::from_secs(8);
        let startup = started + Duration::from_secs(60);
        let cancel = || cancelled.load(Ordering::Acquire);
        let mut may_have_sent = false;
        let mut guard = ContainerGuard {
            runtime: self,
            request: &request.request_id,
            id: None,
            owner: String::new(),
            ambiguous: false,
            armed: false,
        };
        let result = (|| -> RuntimeResult<Receipt> {
            let name = name(&request.request_id)?;
            if request.prompt.is_empty() || request.prompt.len() > dev_worker::MAX_PROMPT {
                return Err("Invalid prompt size".into());
            }
            self.verify_daemon(
                startup.min(Instant::now() + Duration::from_secs(5)),
                &cancel,
            )?;
            if self.owned(&request.request_id, startup, &cancel)?.is_some() {
                return Err("Request container already exists; refusing retry".into());
            }
            let image = self
                .json(&["image", "inspect", IMAGE], startup, &cancel)
                .map_err(|diagnostic| {
                    diagnostic.during(RuntimeDiagnostic::ImageInspectionFailed)
                })?;
            let image = worker_image(&image)?;
            let mut nonce = [0u8; 16];
            rand::rngs::OsRng
                .try_fill_bytes(&mut nonce)
                .map_err(|_| "Cannot allocate runtime ownership nonce")?;
            guard.owner = nonce.iter().map(|b| format!("{b:02x}")).collect();
            guard.armed = true;
            guard.ambiguous = true;
            let bytes = self.capture(&create_arguments(&["--pull=never", "--name", &name,
                "--label", &format!("{LABEL}={}", request.request_id), "--label", &format!("{KIND_LABEL}={}", dev_worker::MARKER),
                                "--label", &format!("{OWNER_LABEL}={}", guard.owner), "--no-healthcheck",
                "--network=none", "--user=1000:1000", "--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges:true",
                "--cpus=2", "--memory=2g", "--memory-swap=2g", "--pids-limit=128", "--restart=no",

                "--tmpfs=/tmp:rw,nosuid,nodev,noexec,size=268435456,mode=1777",
                "--tmpfs=/workspace:rw,nosuid,nodev,noexec,size=268435456,mode=0700,uid=1000,gid=1000",
                "--workdir=/workspace", "--interactive", "--env", &format!("SMITH_DEV_WORKER={}", dev_worker::MARKER),
                "--entrypoint=/usr/local/bin/smith-service", &image, "dev-worker", &request.request_id]), startup, &cancel)?;
            let id = std::str::from_utf8(&bytes)
                .map_err(|_| "Invalid created identity")?
                .trim();
            if !hex(id, 64) {
                return Err("Ambiguous create response; no start permitted".into());
            }
            guard.id = Some(id.to_string());
            let owned = self
                .owned(&request.request_id, startup, &cancel)?
                .ok_or("Created container unavailable")?;
            if owned.id != id || owned.image != image || owned.owner != guard.owner || owned.running
            {
                return Err("Created container identity/state mismatch".into());
            }
            guard.ambiguous = false;
            let mut command = self.command()?;
            command
                .args(["container", "start", "--attach", "--interactive", id])
                .stdin(Stdio::piped());
            guard.ambiguous = true;
            let mut child = OwnedChild(
                command
                    .spawn()
                    .map_err(|_| "Cannot start attached Docker CLI")?,
            );
            let mut input = child.0.stdin.take().ok_or("Missing owned worker input")?;
            let mut output = child.0.stdout.take().ok_or("Missing owned worker output")?;
            dev_worker::nonblocking(input.as_raw_fd())?;
            dev_worker::nonblocking(output.as_raw_fd())?;
            let ready = dev_worker::receive(&mut output, startup, &cancel)?;
            let Frame::Ready {
                request_id,
                session_id,
            } = ready
            else {
                return Err("Expected correlated worker readiness".into());
            };
            if request_id != request.request_id {
                return Err("Worker readiness correlation mismatch".into());
            }
            dev_worker::session_id(&session_id)?;
            guard.ambiguous = false;
            event(RuntimeEvent::Ready {
                session_id: session_id.clone(),
            })?;
            if cancel() || Instant::now() >= work {
                return Err("Dispatch cancelled or timed out".into());
            }
            // The caller must durably persist dispatching here. A failing callback sends nothing.
            event(RuntimeEvent::Dispatching)?;
            if cancel() || Instant::now() >= work {
                return Err("Dispatch cancelled or timed out".into());
            }
            may_have_sent = true;
            let ack = work.min(Instant::now() + Duration::from_secs(10));
            dev_worker::send(
                &mut input,
                &Frame::Prompt {
                    request_id: request.request_id.clone(),
                    session_id: session_id.clone(),
                    text: request.prompt.clone(),
                },
                ack,
                &cancel,
            )?;
            match dev_worker::receive(&mut output, ack, &cancel)? {
                Frame::Recorded {
                    request_id,
                    session_id: recorded_session,
                } if request_id == request.request_id && recorded_session == session_id => {}
                _ => return Err("Worker receipt correlation mismatch".into()),
            }
            drop(input);
            // Require clean wrapper exit and reject trailing/non-protocol output.
            let mut trailing = [0; 1];
            loop {
                dev_worker::wait_io(output.as_raw_fd(), libc::POLLIN, work, &cancel)?;
                match output.read(&mut trailing) {
                    Ok(0) => break,
                    Ok(_) => return Err("Unexpected trailing worker output".into()),
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => return Err("Worker output close failed".into()),
                }
            }
            loop {
                if cancel() || Instant::now() >= work {
                    return Err("Worker exit unverified".into());
                }
                if let Some(status) = child.0.try_wait().map_err(|_| "Worker exit unverified")? {
                    if !status.success() {
                        return Err("Worker exited unsuccessfully".into());
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(Receipt { session_id })
        })();
        let cleanup = if guard.armed {
            guard.stop(total - Duration::from_secs(2))
        } else {
            Ok(())
        };
        match (result, cleanup) {
            (Ok(receipt), Ok(())) => Ok(receipt),
            (result, cleanup) => Err(RuntimeFailure {
                diagnostic: match result {
                    Err(diagnostic) => diagnostic,
                    Ok(_) => RuntimeDiagnostic::UnverifiedReceipt,
                },
                may_have_sent,
                cleanup_unknown: cleanup.is_err(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MESSAGES: &[(RuntimeDiagnostic, &str)] = &[
        (RuntimeDiagnostic::OperatorUnsupported, "Dev runtime requires an unprivileged operator; no automatic setup or replay"),
        (RuntimeDiagnostic::CliMissing, "Required Docker CLI was not found; no automatic setup or replay"),
        (RuntimeDiagnostic::CliUnavailable, "Required Docker CLI could not be accessed; no automatic setup or replay"),
        (RuntimeDiagnostic::CliUnsafe, "Docker CLI identity did not pass safety checks; no automatic setup or replay"),
        (RuntimeDiagnostic::RuntimeRootUnavailable, "Private runtime directory could not be verified; no automatic setup or replay"),
        (RuntimeDiagnostic::RuntimeRootUnsafe, "Private runtime directory did not pass safety checks; no automatic setup or replay"),
        (RuntimeDiagnostic::SocketMissing, "Required private rootless Docker socket was not found; no automatic setup or replay"),
        (RuntimeDiagnostic::SocketUnavailable, "Private Docker socket could not be verified; no automatic setup or replay"),
        (RuntimeDiagnostic::SocketUnsafe, "Private Docker socket did not pass safety checks; no automatic setup or replay"),
        (RuntimeDiagnostic::RootfulSocketMissing, "Required rootful Docker socket was not found; no automatic setup or replay"),
        (RuntimeDiagnostic::RootfulSocketUnavailable, "Rootful Docker socket could not be verified; no automatic setup or replay"),
        (RuntimeDiagnostic::RootfulSocketUnsafe, "Rootful Docker socket did not pass safety checks; no automatic setup or replay"),
        (RuntimeDiagnostic::ConfigIsolationUnavailable, "Isolated Docker configuration location could not be verified; no automatic setup or replay"),
        (RuntimeDiagnostic::DaemonCheckFailed, "Rootless Docker daemon check could not be completed; no automatic setup or replay"),
        (RuntimeDiagnostic::RootlessUnverified, "Docker endpoint was not verified rootless; no automatic setup or replay"),
        (RuntimeDiagnostic::RootfulDaemonCheckFailed, "Rootful Docker daemon check could not be completed; no automatic setup or replay"),
        (RuntimeDiagnostic::RootfulUnverified, "Docker endpoint was not verified rootful; no automatic setup or replay"),
        (RuntimeDiagnostic::ImageInspectionFailed, "Required local worker image could not be verified; no automatic setup or replay"),
        (RuntimeDiagnostic::WorkerImageRejected, "Required local worker image did not pass validation; no automatic setup or replay"),
        (RuntimeDiagnostic::UnverifiedReceipt, "Runtime did not complete a verified receipt; no replay"),
    ];

    #[test]
    fn diagnostic_messages_are_exact_static_bounded_and_single_line() {
        for &(diagnostic, expected) in MESSAGES {
            let message = diagnostic.message();
            assert_eq!(message, expected);
            assert!(!message.is_empty() && message.len() <= 256);
            assert!(!message.chars().any(char::is_control));
            assert!(message.contains("no ") && message.ends_with("replay"));
        }
    }

    #[test]
    fn metadata_absence_is_distinct_from_inaccessibility() {
        use std::io::{Error, ErrorKind};
        for (missing, unavailable) in [
            (
                RuntimeDiagnostic::CliMissing,
                RuntimeDiagnostic::CliUnavailable,
            ),
            (
                RuntimeDiagnostic::SocketMissing,
                RuntimeDiagnostic::SocketUnavailable,
            ),
            (
                RuntimeDiagnostic::RootfulSocketMissing,
                RuntimeDiagnostic::RootfulSocketUnavailable,
            ),
        ] {
            assert_eq!(
                metadata_diagnostic(Error::from(ErrorKind::NotFound), missing, unavailable),
                missing
            );
            for kind in [
                ErrorKind::PermissionDenied,
                ErrorKind::Other,
                ErrorKind::TimedOut,
            ] {
                assert_eq!(
                    metadata_diagnostic(
                        Error::new(kind, "/private/canary token=secret"),
                        missing,
                        unavailable
                    ),
                    unavailable
                );
            }
        }
    }

    #[test]
    fn untrusted_errors_use_fallback_and_context_preserves_prerequisites() {
        for raw in [
            "/private/canary token=secret prompt=hidden\n",
            "Explicit rootless Docker socket unavailable",
            "Required Docker CLI was not found; no automatic setup or replay",
            "Worker image must not declare volumes",
            "Docker command failed; daemon state may be unknown",
            "Docker command cancelled or timed out",
        ]
        .into_iter()
        .chain(MESSAGES.iter().map(|(_, message)| *message))
        {
            assert_eq!(
                RuntimeDiagnostic::from(raw),
                RuntimeDiagnostic::UnverifiedReceipt
            );
            assert_eq!(
                RuntimeDiagnostic::from(raw.to_owned()),
                RuntimeDiagnostic::UnverifiedReceipt
            );
        }
        for operation in [
            RuntimeDiagnostic::DaemonCheckFailed,
            RuntimeDiagnostic::RootfulDaemonCheckFailed,
            RuntimeDiagnostic::ImageInspectionFailed,
        ] {
            for &(diagnostic, _) in MESSAGES {
                let expected = if diagnostic == RuntimeDiagnostic::UnverifiedReceipt {
                    operation
                } else {
                    diagnostic
                };
                assert_eq!(diagnostic.during(operation), expected);
            }
            assert_eq!(
                docker_json(b"invalid /private/canary token=secret")
                    .unwrap_err()
                    .during(operation),
                operation
            );
            assert!(!operation.message().contains("not found"));
            assert!(!operation.message().contains("stopped"));
        }
    }

    #[test]
    fn adapter_modes_fix_endpoints_and_isolate_each_lifecycle_command() {
        for (runtime, mode, endpoint) in [
            (
                DockerRuntime::new(PathBuf::from("/private/runtime")),
                DockerMode::Rootless,
                "unix:///private/runtime/docker.sock",
            ),
            (
                DockerRuntime::new_rootful(PathBuf::from("/private/runtime")),
                DockerMode::Rootful,
                "unix:///run/docker.sock",
            ),
        ] {
            assert_eq!(runtime.mode, mode);
            for args in [
                &["info", "--format", "{{json .SecurityOptions}}"][..],
                &["container", "create", "--pull=never"][..],
                &["container", "start", "--attach", "--interactive"][..],
                &["container", "ls", "--all"][..],
                &["container", "inspect"][..],
                &["container", "stop", "--time", "1"][..],
                &["image", "inspect"][..],
            ] {
                // Seed only this Command; never mutate the process environment or spawn it.
                let mut command = Command::new("/usr/bin/docker");
                for key in [
                    "DOCKER_HOST",
                    "DOCKER_CONTEXT",
                    "DOCKER_CONFIG",
                    "DOCKER_TLS_VERIFY",
                    "DOCKER_CERT_PATH",
                    "DOCKER_API_VERSION",
                    "DOCKER_AUTH_CONFIG",
                    "DOCKER_CUSTOM_HEADERS",
                    "HTTP_PROXY",
                    "HTTPS_PROXY",
                    "ALL_PROXY",
                    "NO_PROXY",
                    "XDG_RUNTIME_DIR",
                    "PATH",
                    "HOME",
                    "LANG",
                ] {
                    command.env(key, "untrusted-canary");
                }
                runtime.configure_command(&mut command).unwrap();
                command.args(args);
                assert_eq!(command.get_program(), "/usr/bin/docker");
                assert_eq!(command.get_current_dir(), Some(Path::new("/")));
                let argv: Vec<_> = command
                    .get_args()
                    .map(|arg| arg.to_str().unwrap())
                    .collect();
                assert_eq!(
                    &argv[..4],
                    &[
                        "--config",
                        "/nonexistent/smith-docker-config",
                        "--host",
                        endpoint
                    ]
                );
                assert_eq!(&argv[4..], args);
                let env: std::collections::BTreeMap<_, _> = command
                    .get_envs()
                    .map(|(key, value)| {
                        (
                            key.to_str().unwrap(),
                            value.map(|value| value.to_str().unwrap()),
                        )
                    })
                    .collect();
                assert_eq!(
                    env,
                    std::collections::BTreeMap::from([
                        ("HOME", Some("/nonexistent")),
                        ("LANG", Some("C")),
                        ("PATH", Some("/usr/bin:/bin")),
                    ])
                );
            }
        }
        assert_eq!(
            DockerMode::Rootful.socket_path(Path::new("/different/runtime")),
            PathBuf::from("/run/docker.sock")
        );
    }

    #[test]
    fn both_worker_commands_disable_compression_with_one_bounded_log_file() {
        for (runtime, endpoint) in [
            (
                DockerRuntime::new(PathBuf::from("/private/runtime")),
                "unix:///private/runtime/docker.sock",
            ),
            (
                DockerRuntime::new_rootful(PathBuf::from("/private/runtime")),
                "unix:///run/docker.sock",
            ),
        ] {
            for worker in ["dev-worker", "dev-local-worker"] {
                let options = [
                    "--pull=never",
                    "--name",
                    "smith-dev-fixture",
                    "--network=none",
                    "--user=1000:1000",
                    "--read-only",
                    "--entrypoint=/usr/local/bin/smith-service",
                    "sha256:fixture",
                    worker,
                    "11111111111111111111111111111111",
                ];
                let mut command = Command::new("/usr/bin/docker");
                runtime.configure_command(&mut command).unwrap();
                command.args(create_arguments(&options));
                let args: Vec<_> = command
                    .get_args()
                    .map(|arg| arg.to_str().unwrap())
                    .collect();
                assert_eq!(
                    &args[..4],
                    &[
                        "--config",
                        "/nonexistent/smith-docker-config",
                        "--host",
                        endpoint
                    ]
                );
                assert_eq!(
                    &args[4..10],
                    &[
                        "container",
                        "create",
                        "--log-driver=local",
                        "--log-opt=max-size=1m",
                        "--log-opt=max-file=1",
                        "--log-opt=compress=false",
                    ]
                );
                assert_eq!(&args[10..], options);
                assert_eq!(
                    args.iter()
                        .filter(|a| a.starts_with("--log-driver="))
                        .count(),
                    1
                );
                assert_eq!(
                    args.iter().filter(|a| a.starts_with("--log-opt=")).count(),
                    3
                );
                let image = args.iter().position(|a| *a == "sha256:fixture").unwrap();
                assert!(args
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.starts_with("--log-"))
                    .all(|(i, _)| i < image));
            }
        }
    }

    #[test]
    fn rootful_directory_metadata_requires_native_root_owned_safe_ancestors() {
        for permissions in 0..=0o7777 {
            assert_eq!(
                rootful_directory_safe(0, libc::S_IFDIR | permissions),
                permissions & 0o022 == 0,
                "permissions {permissions:o}"
            );
        }
        for owner in [1, 1000, u32::MAX] {
            assert!(!rootful_directory_safe(owner, libc::S_IFDIR | 0o755));
        }
        for kind in [
            libc::S_IFREG,
            libc::S_IFLNK,
            libc::S_IFSOCK,
            libc::S_IFIFO,
            libc::S_IFCHR,
            libc::S_IFBLK,
            0,
        ] {
            assert!(!rootful_directory_safe(0, kind | 0o755));
        }
    }

    #[test]
    fn socket_metadata_keeps_rootless_rules_and_requires_rootful_native_0660() {
        for permissions in 0..=0o7777 {
            let mode = libc::S_IFSOCK | permissions;
            assert_eq!(
                DockerMode::Rootful.socket_safe(0, mode, 1000),
                permissions == 0o660,
                "rootful permissions {permissions:o}"
            );
            assert_eq!(
                DockerMode::Rootless.socket_safe(1000, mode, 1000),
                permissions & 0o007 == 0,
                "rootless permissions {permissions:o}"
            );
        }
        for owner in [1, 1000, u32::MAX] {
            assert!(!DockerMode::Rootful.socket_safe(owner, libc::S_IFSOCK | 0o660, 1000));
        }
        for owner in [0, 1, 1001, u32::MAX] {
            assert!(!DockerMode::Rootless.socket_safe(owner, libc::S_IFSOCK | 0o660, 1000));
        }
        for kind in [
            libc::S_IFREG,
            libc::S_IFDIR,
            libc::S_IFLNK,
            libc::S_IFIFO,
            libc::S_IFCHR,
            libc::S_IFBLK,
            0,
        ] {
            assert!(!DockerMode::Rootful.socket_safe(0, kind | 0o660, 1000));
            assert!(!DockerMode::Rootless.socket_safe(1000, kind | 0o660, 1000));
        }
        assert_eq!(
            DockerMode::Rootless.socket_diagnostics(),
            (
                RuntimeDiagnostic::SocketMissing,
                RuntimeDiagnostic::SocketUnavailable,
                RuntimeDiagnostic::SocketUnsafe,
            )
        );
        assert_eq!(
            DockerMode::Rootful.socket_diagnostics(),
            (
                RuntimeDiagnostic::RootfulSocketMissing,
                RuntimeDiagnostic::RootfulSocketUnavailable,
                RuntimeDiagnostic::RootfulSocketUnsafe,
            )
        );
    }

    #[test]
    fn rootful_daemon_accepts_only_supported_security_set_in_any_order() {
        for apparmor in ["name=apparmor", "name=apparmor,profile=default"] {
            let options = [apparmor, "name=seccomp,profile=builtin", "name=cgroupns"];
            for order in [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ] {
                let info = json!(order.map(|index| options[index]));
                assert_eq!(DockerMode::Rootful.verify_daemon_info(&info), Ok(()));
                assert_eq!(
                    DockerMode::Rootless.verify_daemon_info(&info),
                    Err(RuntimeDiagnostic::RootlessUnverified)
                );
            }
        }
        assert_eq!(
            DockerMode::Rootless.daemon_check_failed(),
            RuntimeDiagnostic::DaemonCheckFailed
        );
        assert_eq!(
            DockerMode::Rootful.daemon_check_failed(),
            RuntimeDiagnostic::RootfulDaemonCheckFailed
        );
    }

    #[test]
    fn rootful_daemon_rejects_missing_malformed_duplicate_unknown_and_rootless_data() {
        for info in [
            json!(null),
            json!(true),
            json!(42),
            json!({"SecurityOptions": ["name=apparmor", "name=seccomp,profile=builtin", "name=cgroupns"]}),
            json!("name=apparmor"),
            json!([]),
            json!(["name=apparmor"]),
            json!(["name=apparmor", "name=seccomp,profile=builtin"]),
            json!(["name=apparmor", "name=seccomp,profile=builtin", null]),
            json!(["name=apparmor", "name=seccomp,profile=builtin", 1]),
            json!(["name=apparmor", "name=seccomp,profile=builtin", {"name":"cgroupns"}]),
            json!([
                "name=apparmor",
                "name=seccomp,profile=builtin",
                ["name=cgroupns"]
            ]),
            json!([
                "name=apparmor",
                "name=seccomp,profile=builtin",
                "name=unknown"
            ]),
            json!([
                "name=apparmor",
                "name=seccomp,profile=builtin",
                "name=seccomp,profile=builtin"
            ]),
            json!([
                "name=apparmor",
                "name=apparmor,profile=default",
                "name=cgroupns"
            ]),
            json!(["name=apparmor", "name=cgroupns", "name=cgroupns"]),
            json!([
                "name=apparmor",
                "name=seccomp,profile=builtin",
                "name=cgroupns",
                "name=cgroupns"
            ]),
            json!([
                "name=apparmor",
                "name=seccomp,profile=builtin",
                "name=cgroupns",
                "name=unknown"
            ]),
            json!(["name=rootless"]),
            json!(["name=rootless,supplemental=true"]),
            json!([
                "name=rootless",
                "name=seccomp,profile=builtin",
                "name=cgroupns"
            ]),
            json!([
                "name=apparmor",
                "name=seccomp,profile=builtin",
                "name=cgroupns",
                "name=rootless"
            ]),
        ] {
            assert_eq!(
                DockerMode::Rootful.verify_daemon_info(&info),
                Err(RuntimeDiagnostic::RootfulUnverified),
                "{info}"
            );
        }
        for (index, malformed) in [
            (0, "name=apparmor,"),
            (0, "name=apparmor,profile=unconfined"),
            (0, "name=apparmor,profile=default,extra=true"),
            (0, "name=apparmor,profile=default,profile=default"),
            (0, "profile=default,name=apparmor"),
            (1, "name=seccomp"),
            (1, "name=seccomp,profile=default"),
            (1, "name=seccomp,profile=builtin,extra=true"),
            (2, "name=cgroupns,"),
            (2, "name=cgroupns,profile=default"),
            (2, " name=cgroupns"),
            (2, "name=cgroupns\n"),
            (2, "name=cgroupns\0"),
            (2, "name=rootless,"),
            (2, "name=rootful"),
            (2, ""),
        ] {
            let mut options = [
                "name=apparmor",
                "name=seccomp,profile=builtin",
                "name=cgroupns",
            ];
            options[index] = malformed;
            assert_eq!(
                DockerMode::Rootful.verify_daemon_info(&json!(options)),
                Err(RuntimeDiagnostic::RootfulUnverified),
                "{malformed:?}"
            );
        }
    }

    #[test]
    fn rootless_and_image_validation_keep_existing_acceptance_rules() {
        for info in [
            json!(["name=rootless"]),
            json!(["name=rootless,supplemental=true"]),
            json!(["name=rootless,"]),
            json!([null, "name=unknown", "name=rootless"]),
        ] {
            assert_eq!(verify_rootless(&info), Ok(()));
            assert_eq!(DockerMode::Rootless.verify_daemon_info(&info), Ok(()));
            assert_eq!(
                DockerMode::Rootful.verify_daemon_info(&info),
                Err(RuntimeDiagnostic::RootfulUnverified)
            );
        }
        for info in [
            json!(null),
            json!({}),
            json!([]),
            json!(["name=rootful"]),
            json!(["name=rootless-lookalike"]),
        ] {
            assert_eq!(
                verify_rootless(&info),
                Err(RuntimeDiagnostic::RootlessUnverified)
            );
        }
        for image in [
            json!(null),
            json!({}),
            json!([]),
            json!([{}, {}]),
            json!([{}]),
            json!([{"Id":"not-an-image-id"}]),
        ] {
            assert_eq!(
                worker_image(&image),
                Err(RuntimeDiagnostic::ImageInspectionFailed)
            );
        }
        let id = format!("sha256:{}", "a".repeat(64));
        for config in [json!({}), json!({"Volumes": null}), json!({"Volumes": {}})] {
            assert_eq!(
                worker_image(&json!([{"Id":id, "Config":config}])),
                Ok(id.clone())
            );
        }
        assert_eq!(
            worker_image(&json!([{"Id":id, "Config":{"Volumes":{"/private/canary":{}}}}])),
            Err(RuntimeDiagnostic::WorkerImageRejected)
        );
    }
}
