//! Explicit Ubuntu JSONL supervisor transport. Construction neither probes nor launches.
//! This is not an engine sandbox or live-readiness proof. Source validation, dependency
//! preparation, engine authority and independent cleanup observation remain caller-owned.
use super::{Acknowledgement, Action, Command, FrozenProject, JobType, Outcome, Runtime};
use crate::{app::valid_id, cloud::unique_json, filesystem};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, DirBuilder};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command as Process, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

const REQUEST_LIMIT: usize = 32768;
const RESPONSE_LIMIT: usize = 65536;
const STDERR_LIMIT: usize = 65536;
const TOTAL_OUTPUT: usize = 8 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(2);
const MAX_FILES: usize = 128;
const MAX_MODULE: usize = 4 * 1024 * 1024;

/// Owner-selected paths/digests only, never values from App3 or model output.
/// The launcher receives exactly `-B -m smith_agent.retained` in the copied module
/// directory. Its interpreter/dependency closure must be separately prepared.
#[derive(Clone)]
pub struct PreparedConfig {
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub module: PathBuf,
    /// Exact full file manifest, relative normalized paths; directories implicit.
    pub manifest: BTreeMap<PathBuf, String>,
    pub workspace: PathBuf,
    pub operation_timeout: Duration,
    pub cleanup_timeout: Duration,
    pub poll_interval: Duration,
    pub max_polls: usize,
}
#[derive(Clone)]
pub struct SourceBinding {
    pub source_id: String,
    pub manifest_sha256: String,
}
/// Must return a sanitized immutable snapshot token bound to this exact frozen
/// project/mode, registered in the prepared supervisor's trusted source registry,
/// enforcing EVERY repository's read-only capability. No URL/path
/// derivation or acquisition is implemented by the transport. Implementations must
/// honor deadline/cancellation and refuse unsupported mixed-capability layouts.
pub trait SourceResolver: Send + Sync {
    fn resolve(
        &self,
        project: &FrozenProject,
        mode: JobType,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<SourceBinding>;
}
pub struct CleanupTarget {
    pub job_id: String,
    pub source_id: String,
    pub source_sha256: String,
    pub finish_operation_id: String,
    pub supervisor_pid: u32,
}
/// Independent owned ENGINE/service proof, not JSON, process exit, or path absence.
/// Must honor deadline/cancellation; never receives raw stdout/stderr.
pub trait CleanupVerifier: Send + Sync {
    fn verify(&self, target: &CleanupTarget, deadline: Instant, cancelled: &AtomicBool) -> bool;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    InvalidConfiguration,
}
type TransportResult<T> = std::result::Result<T, ()>;
struct OwnedProcess {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    errors: ChildStderr,
    stderr_bytes: usize,
    output_bytes: usize,
    reaped: bool,
}
impl OwnedProcess {
    fn stop(&mut self, budget: Duration) -> bool {
        if self.reaped {
            return true;
        }
        // Kill the group BEFORE reaping its leader, keeping its numeric identity
        // reserved. Group death is deliberately NOT engine-cleanup evidence.
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let deadline = Instant::now() + budget;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.reaped = true;
                    return true;
                }
                Err(_) => return false,
                _ if Instant::now() >= deadline => return false,
                _ => std::thread::sleep(POLL),
            }
        }
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        self.stop(Duration::from_millis(250));
    }
}
struct Session {
    process: Option<OwnedProcess>,
    source: SourceBinding,
    project_hash: String,
    mode: JobType,
    sealed: bool,
    released: bool,
    results: HashMap<(String, Action), (String, Outcome)>,
}
pub struct Prepared {
    config: PreparedConfig,
    resolver: Arc<dyn SourceResolver>,
    verifier: Arc<dyn CleanupVerifier>,
    cancelled: AtomicBool,
    sessions: Mutex<HashMap<String, Session>>,
}
fn hash_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn relative(p: &Path) -> bool {
    !p.as_os_str().is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && p.to_str()
            .is_some_and(|s| s.split('/').all(|x| !x.is_empty() && x != "." && x != ".."))
}
fn wire_id(c: &Command) -> String {
    filesystem::digest(
        format!(
            "retained-jsonl-v1:{}:{}:{:?}",
            c.session_id, c.operation_id, c.action
        )
        .as_bytes(),
    )[..32]
        .into()
}
fn nonblocking(fd: i32) -> TransportResult<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        Err(())
    } else {
        Ok(())
    }
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> TransportResult<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
        Err(())
    } else {
        Ok(())
    }
}
fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
    total: &mut usize,
    depth: usize,
) -> TransportResult<()> {
    if depth > 8 {
        return Err(());
    }
    filesystem::check_directory(dir, true).map_err(|_| ())?;
    for entry in fs::read_dir(dir).map_err(|_| ())? {
        let path = entry.map_err(|_| ())?.path();
        let kind = fs::symlink_metadata(&path).map_err(|_| ())?.file_type();
        if kind.is_dir() {
            collect_files(root, &path, files, total, depth + 1)?;
        } else if kind.is_file() {
            let bytes = filesystem::read_file(&path, 0o600, MAX_MODULE as u64)
                .map_err(|_| ())?
                .ok_or(())?;
            *total = total.checked_add(bytes.len()).ok_or(())?;
            if *total > MAX_MODULE || files.len() >= MAX_FILES {
                return Err(());
            }
            files.insert(
                path.strip_prefix(root).map_err(|_| ())?.to_path_buf(),
                bytes,
            );
        } else {
            return Err(());
        }
    }
    Ok(())
}
impl Prepared {
    /// Shape validation only: not host readiness, source acceptance or live permission.
    pub fn new(
        config: PreparedConfig,
        resolver: Arc<dyn SourceResolver>,
        verifier: Arc<dyn CleanupVerifier>,
    ) -> std::result::Result<Self, PrepareError> {
        if !config.executable.is_absolute()
            || !config.module.is_absolute()
            || !config.workspace.is_absolute()
            || !hash_valid(&config.executable_sha256)
            || config.manifest.is_empty()
            || config.manifest.len() > MAX_FILES
            || config
                .manifest
                .iter()
                .any(|(p, h)| !relative(p) || !hash_valid(h))
            || config.operation_timeout.is_zero()
            || config.operation_timeout > Duration::from_secs(180)
            || config.cleanup_timeout.is_zero()
            || config.cleanup_timeout > Duration::from_secs(30)
            || config.poll_interval < POLL
            || config.poll_interval > Duration::from_secs(1)
            || config.max_polls == 0
            || config.max_polls > 1024
            || config.workspace.starts_with(&config.module)
            || config.module.starts_with(&config.workspace)
            || config.executable.starts_with(&config.workspace)
            || config.executable.starts_with(&config.module)
        {
            return Err(PrepareError::InvalidConfiguration);
        }
        Ok(Self {
            config,
            resolver,
            verifier,
            cancelled: AtomicBool::new(false),
            sessions: Mutex::new(HashMap::new()),
        })
    }
    fn launch(&self, c: &Command, deadline: Instant) -> TransportResult<Session> {
        check(deadline, &self.cancelled)?;
        filesystem::check_directory(&self.config.workspace, true).map_err(|_| ())?;
        // Exclusive directory admission is a durable no-replacement fence across
        // adapter reconstruction; never remove it on failures, Finish, or Drop.
        let work = self.config.workspace.join(&c.session_id);
        DirBuilder::new()
            .mode(0o700)
            .create(&work)
            .map_err(|_| ())?;
        let source = self
            .resolver
            .resolve(&c.project, c.job_type, deadline, &self.cancelled)
            .ok_or(())?;
        if !valid_id(&source.source_id) || !hash_valid(&source.manifest_sha256) {
            return Err(());
        }
        let executable = filesystem::read_file(&self.config.executable, 0o700, 128 * 1024 * 1024)
            .map_err(|_| ())?
            .ok_or(())?;
        if filesystem::digest(&executable) != self.config.executable_sha256 {
            return Err(());
        }
        let mut files = BTreeMap::new();
        collect_files(
            &self.config.module,
            &self.config.module,
            &mut files,
            &mut 0,
            0,
        )?;
        let actual: BTreeMap<_, _> = files
            .iter()
            .map(|(p, b)| (p.clone(), filesystem::digest(b)))
            .collect();
        if actual != self.config.manifest {
            return Err(());
        }
        let module = work.join("module");
        filesystem::mkdir(&module).map_err(|_| ())?;
        for (p, bytes) in files {
            let target = module.join(p);
            filesystem::mkdir(target.parent().ok_or(())?).map_err(|_| ())?;
            filesystem::publish(&target, &bytes, 0o600, false).map_err(|_| ())?;
        }
        let launcher = work.join("launcher");
        filesystem::publish(&launcher, &executable, 0o700, false).map_err(|_| ())?;
        for name in ["home", "config", "cache", "data", "runtime", "tmp"] {
            filesystem::mkdir(&work.join(name)).map_err(|_| ())?;
        }
        let marker = json!({"v":1,"job_id":c.session_id,"source_id":source.source_id,"source_sha256":source.manifest_sha256,
            "create_operation_id":wire_id(c),"executable_sha256":self.config.executable_sha256,"manifest":self.config.manifest});
        filesystem::publish(
            &work.join("consumed.json"),
            &serde_json::to_vec(&marker).map_err(|_| ())?,
            0o600,
            false,
        )
        .map_err(|_| ())?;
        // Persist the exclusive job-directory name itself before spawn too.
        fs::File::open(&self.config.workspace)
            .and_then(|d| d.sync_all())
            .map_err(|_| ())?;
        check(deadline, &self.cancelled)?;
        let child = Process::new(&launcher)
            .args(["-B", "-m", "smith_agent.retained"])
            .current_dir(&module)
            .env_clear()
            .env("HOME", work.join("home"))
            .env("XDG_CONFIG_HOME", work.join("config"))
            .env("XDG_CACHE_HOME", work.join("cache"))
            .env("XDG_DATA_HOME", work.join("data"))
            .env("XDG_RUNTIME_DIR", work.join("runtime"))
            .env("TMPDIR", work.join("tmp"))
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("PYTHONNOUSERSITE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|_| ())?;
        let mut child = child;
        // All three handles are guaranteed by piped stdio; own the child before
        // any further fallible syscall so setup failure kills/reaps it too.
        let mut owned = OwnedProcess {
            input: child.stdin.take().ok_or(())?,
            output: child.stdout.take().ok_or(())?,
            errors: child.stderr.take().ok_or(())?,
            child,
            stderr_bytes: 0,
            output_bytes: 0,
            reaped: false,
        };
        if nonblocking(owned.input.as_raw_fd())
            .and_then(|_| nonblocking(owned.output.as_raw_fd()))
            .and_then(|_| nonblocking(owned.errors.as_raw_fd()))
            .is_err()
        {
            owned.stop(Duration::from_millis(250));
            return Err(());
        }
        Ok(Session {
            process: Some(owned),
            source,
            project_hash: filesystem::digest(&serde_json::to_vec(&c.project).map_err(|_| ())?),
            mode: c.job_type,
            sealed: false,
            released: false,
            results: HashMap::new(),
        })
    }
    fn exchange(
        &self,
        p: &mut OwnedProcess,
        request: Value,
        deadline: Instant,
    ) -> TransportResult<Response> {
        let mut input = serde_json::to_vec(&request).map_err(|_| ())?;
        input.push(b'\n');
        if input.len() > REQUEST_LIMIT {
            return Err(());
        }
        let mut written = 0;
        let mut output = Vec::new();
        loop {
            check(deadline, &self.cancelled)?;
            if written < input.len() {
                match p.input.write(&input[written..]) {
                    Ok(0) => return Err(()),
                    Ok(n) => written += n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                    Err(_) => return Err(()),
                }
            }
            let mut buffer = [0u8; 4096];
            // Stderr is counted and discarded, never decoded, logged, returned or
            // allowed to fill its pipe while stdin/stdout are progressing.
            for _ in 0..32 {
                match p.errors.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        p.stderr_bytes += n;
                        if p.stderr_bytes > STDERR_LIMIT {
                            return Err(());
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => return Err(()),
                }
            }
            for _ in 0..32 {
                match p.output.read(&mut buffer) {
                    Ok(0) => return Err(()),
                    Ok(n) => {
                        output.extend_from_slice(&buffer[..n]);
                        p.output_bytes += n;
                        if output.len() > RESPONSE_LIMIT || p.output_bytes > TOTAL_OUTPUT {
                            return Err(());
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => return Err(()),
                }
            }
            if let Some(end) = output.iter().position(|b| *b == b'\n') {
                if written != input.len() || end + 1 != output.len() {
                    return Err(());
                }
                return Response::decode(&output[..end], &request);
            }
            if output.len() == RESPONSE_LIMIT {
                return Err(());
            }
            std::thread::sleep(POLL);
        }
    }
    fn transact(
        &self,
        session: &mut Session,
        c: &Command,
        deadline: Instant,
    ) -> TransportResult<Outcome> {
        let id = wire_id(c);
        let base = json!({"v":1,"op":match c.action {Action::Create=>"create",Action::Prompt=>"prompt",Action::Finish=>"finish"},"operation_id":id,"job_id":c.session_id});
        let mut request = base;
        match c.action {
            Action::Create => {
                request["mode"] = json!(c.job_type);
                request["source_id"] = json!(session.source.source_id);
            }
            Action::Prompt => {
                let text = match super::compose_prompt(&c.project, c.prompt.as_deref().ok_or(())?) {
                    Ok(text) => text,
                    Err(_) => return Ok(Outcome::Failed),
                };
                request["purpose"] = json!("work");
                request["text"] = json!(text);
            }
            Action::Finish => (),
        }
        let p = session.process.as_mut().ok_or(())?;
        let mut response = self.exchange(p, request, deadline)?;
        for poll in 0..=self.config.max_polls {
            let operation = response.operation.as_ref().ok_or(())?;
            if operation.operation_id != id || operation.op != action_name(c.action) {
                return Err(());
            }
            let job = response.job.as_ref().ok_or(())?;
            if job.mode != c.job_type {
                return Err(());
            }
            match operation.state.as_str() {
                "pending" if poll < self.config.max_polls => {
                    check(deadline, &self.cancelled)?;
                    std::thread::sleep(
                        self.config
                            .poll_interval
                            .min(deadline.saturating_duration_since(Instant::now())),
                    );
                    response = self.exchange(p,json!({"v":1,"op":"observe","operation_id":crate::app::new_id().map_err(|_| ())?,"job_id":c.session_id,"target_operation_id":id}),deadline)?;
                }
                "succeeded"
                    if c.action == Action::Create
                        && job.state == "active"
                        && job.retained
                        && job.conversation =>
                {
                    return Ok(Outcome::Ready)
                }
                "succeeded"
                    if c.action == Action::Prompt
                        && job.state == "active"
                        && job.retained
                        && job.conversation =>
                {
                    return Ok(Outcome::Reply(operation.text.clone()))
                }
                "succeeded"
                    if c.action == Action::Finish && job.state == "finished" && !job.retained =>
                {
                    let target = CleanupTarget {
                        job_id: c.session_id.clone(),
                        source_id: session.source.source_id.clone(),
                        source_sha256: session.source.manifest_sha256.clone(),
                        finish_operation_id: id,
                        supervisor_pid: p.child.id(),
                    };
                    let cleanup_deadline =
                        (Instant::now() + self.config.cleanup_timeout).min(deadline);
                    check(cleanup_deadline, &self.cancelled)?;
                    if !self
                        .verifier
                        .verify(&target, cleanup_deadline, &self.cancelled)
                    {
                        return Err(());
                    }
                    check(cleanup_deadline, &self.cancelled)?;
                    if !p.stop(Duration::from_millis(250)) {
                        return Err(());
                    }
                    session.sealed = true;
                    session.released = true;
                    return Ok(Outcome::Finished);
                }
                "failed"
                    if c.action == Action::Prompt
                        && job.state == "failed"
                        && job.retained
                        && job.conversation =>
                {
                    return Ok(Outcome::Failed)
                }
                "failed"
                    if c.action == Action::Create
                        && job.state == "blocked"
                        && !job.retained
                        && operation.code.as_deref() == Some("policy_denied") =>
                {
                    return Ok(Outcome::Failed)
                }
                _ => return Err(()),
            }
        }
        Err(())
    }
    fn run(&self, c: &Command) -> Outcome {
        if !valid_id(&c.session_id)
            || !valid_id(&c.operation_id)
            || !valid_id(&c.project.project_id)
        {
            return Outcome::Unknown;
        }
        let Ok(bytes) = serde_json::to_vec(c) else {
            return Outcome::Unknown;
        };
        if bytes.len() > 128 * 1024 {
            return Outcome::Unknown;
        }
        let fingerprint = filesystem::digest(&bytes);
        let deadline = Instant::now() + self.config.operation_timeout;
        let mut sessions = loop {
            if check(deadline, &self.cancelled).is_err() {
                return Outcome::Unknown;
            }
            match self.sessions.try_lock() {
                Ok(s) => break s,
                Err(std::sync::TryLockError::Poisoned(_)) => return Outcome::Unknown,
                Err(std::sync::TryLockError::WouldBlock) => std::thread::sleep(POLL),
            }
        };
        if let Some(session) = sessions.get(&c.session_id) {
            if let Some((previous, outcome)) =
                session.results.get(&(c.operation_id.clone(), c.action))
            {
                return if previous == &fingerprint {
                    outcome.clone()
                } else {
                    Outcome::Unknown
                };
            }
        }
        if !sessions.contains_key(&c.session_id) {
            if c.action != Action::Create
                || sessions.len() >= 32
                || sessions.values().filter(|s| !s.released).count() >= 3
            {
                return Outcome::Unknown;
            }
            let session = self.launch(c, deadline);
            match session {
                Ok(s) => {
                    sessions.insert(c.session_id.clone(), s);
                }
                Err(_) => return Outcome::Unknown,
            }
        }
        let session = sessions.get_mut(&c.session_id).expect("inserted session");
        if session.sealed
            || session.results.len() >= 256
            || session.mode != c.job_type
            || session.project_hash
                != filesystem::digest(&serde_json::to_vec(&c.project).unwrap_or_default())
        {
            return Outcome::Unknown;
        }
        let result = self.transact(session, c, deadline);
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(()) => {
                session.sealed = true;
                if let Some(p) = &mut session.process {
                    p.stop(Duration::from_millis(250));
                }
                Outcome::Unknown
            }
        };
        session.results.insert(
            (c.operation_id.clone(), c.action),
            (fingerprint, outcome.clone()),
        );
        outcome
    }
}
impl Runtime for Prepared {
    /// Transport preparation is not live acceptance; do not enable Application admission.
    fn admission_ready(&self) -> bool {
        false
    }
    fn execute(&self, command: &Command) -> Acknowledgement {
        Acknowledgement {
            session_id: command.session_id.clone(),
            operation_id: command.operation_id.clone(),
            action: command.action,
            outcome: self.run(command),
        }
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let mut sessions = match self.sessions.try_lock() {
            Ok(sessions) => sessions,
            Err(std::sync::TryLockError::Poisoned(poison)) => poison.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return,
        };
        for s in sessions.values_mut() {
            s.sealed = true;
            if let Some(p) = &mut s.process {
                p.stop(Duration::from_millis(250));
            }
        }
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn action_name(action: Action) -> &'static str {
    match action {
        Action::Create => "create",
        Action::Prompt => "prompt",
        Action::Finish => "finish",
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    v: u8,
    operation_id: String,
    job_id: String,
    ok: bool,
    code: Option<String>,
    job: Option<Job>,
    operation: Option<Operation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    state: String,
    mode: JobType,
    retained: bool,
    conversation: bool,
    ready: bool,
    blockers: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    operation_id: String,
    op: String,
    state: String,
    code: Option<String>,
    text: String,
}
fn exact(v: &Value, keys: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|m| m.len() == keys.len() && keys.iter().all(|k| m.contains_key(*k)))
}
impl Response {
    fn decode(raw: &[u8], request: &Value) -> TransportResult<Self> {
        let v = unique_json(raw).map_err(|_| ())?;
        if !exact(
            &v,
            &[
                "v",
                "operation_id",
                "job_id",
                "ok",
                "code",
                "job",
                "operation",
            ],
        ) || (!v["job"].is_null()
            && !exact(
                &v["job"],
                &[
                    "state",
                    "mode",
                    "retained",
                    "conversation",
                    "ready",
                    "blockers",
                ],
            ))
            || (!v["operation"].is_null()
                && !exact(
                    &v["operation"],
                    &["operation_id", "op", "state", "code", "text"],
                ))
        {
            return Err(());
        }
        let r: Self = serde_json::from_value(v).map_err(|_| ())?;
        if r.v != 1
            || r.operation_id != request["operation_id"].as_str().ok_or(())?
            || r.job_id != request["job_id"].as_str().ok_or(())?
            || !r.ok
            || r.code.is_some()
        {
            return Err(());
        }
        let j = r.job.as_ref().ok_or(())?;
        let o = r.operation.as_ref().ok_or(())?;
        if j.ready
            || j.blockers.len() > 16
            || j.blockers.iter().any(|b| {
                !matches!(
                    b.as_str(),
                    "default_provider_egress_unresolved"
                        | "runtime_resource_enforcement_unresolved"
                        | "owned_cleanup_verification_unresolved"
                        | "live_interoperability_unverified"
                        | "source_containment_unverified"
                        | "service_incarnation_live_verification_pending"
                )
            })
            || !matches!(
                j.state.as_str(),
                "absent" | "active" | "failed" | "unknown" | "finished" | "blocked"
            )
            || !valid_id(&o.operation_id)
            || !matches!(o.op.as_str(), "create" | "prompt" | "finish")
            || !matches!(
                o.state.as_str(),
                "pending" | "succeeded" | "failed" | "unknown"
            )
            || o.code
                .as_deref()
                .is_some_and(|c| !matches!(c, "policy_denied" | "runtime_unknown"))
            || (matches!(o.state.as_str(), "pending" | "succeeded") && o.code.is_some())
            || (o.state == "pending" && !o.text.is_empty())
            || (o.state == "succeeded" && o.op != "prompt" && !o.text.is_empty())
            || (o.state == "failed" && o.op == "prompt" && o.code.is_some())
            || o.text.len() > 8192
            || o.text.contains('\0')
        {
            return Err(());
        }
        Ok(r)
    }
}

#[cfg(test)]
#[path = "retained_runtime_tests.rs"]
mod tests;
