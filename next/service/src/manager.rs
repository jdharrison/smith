use crate::{Result, Roots};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static CANCELLED: AtomicBool = AtomicBool::new(false);
extern "C" fn cancel(_: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

/// Only the short control helper installs these handlers, never the managed daemon.
pub fn install_control_cancellation() -> Result<()> {
    // SAFETY: zero-initialized sigaction is filled before use; handler only stores
    // a lock-free atomic. No allocation or I/O occurs in the signal handler.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = cancel as *const () as libc::sighandler_t;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGTERM, libc::SIGINT] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err("Cannot install control cancellation handlers".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn check_cancellation() -> Result<()> {
    if CANCELLED.load(Ordering::Relaxed) {
        Err("Control helper cancelled; action outcome may require fresh inspection".into())
    } else {
        Ok(())
    }
}

fn parent_death_guard(expected_parent: libc::pid_t) -> std::io::Result<()> {
    // SAFETY: prctl/getppid have no Rust memory dependencies and are safe after
    // fork. Check AFTER arming to close the fork-to-prctl parent-death race.
    unsafe {
        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::getppid() != expected_parent {
            return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagerFacts {
    pub load: String,
    pub active: String,
    pub sub: String,
    pub pid: u32,
    pub fragment: String,
    pub drop_ins: String,
    pub need_reload: bool,
    pub transient: bool,
    pub unit_file_state: String,
}

/// Injection is a Rust API for isolated tests; executable arguments/environment
/// can never select a manager executable.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceManagerFacts {
    pub service: ManagerFacts,
    pub unit_file_state: String,
    pub control_group: String,
}

pub trait Manager {
    fn inspect(&self, roots: &Roots) -> Result<ManagerFacts>;
    fn reload(&self, roots: &Roots) -> Result<()>;
    fn start(&self, roots: &Roots) -> Result<()>;
    fn enable(&self, _roots: &Roots) -> Result<()> {
        Err("Manager does not support service Enable".into())
    }
    fn disable(&self, _roots: &Roots) -> Result<()> {
        Err("Manager does not support service Disable".into())
    }
    fn maintenance_inspect(&self, _roots: &Roots) -> Result<MaintenanceManagerFacts> {
        Err("Manager does not support verified maintenance".into())
    }
    fn stop(&self, _roots: &Roots) -> Result<()> {
        Err("Manager does not support maintenance Stop".into())
    }
    fn quiescence(&self, _roots: &Roots, _control_group: &str) -> Result<()> {
        Err("Manager does not support kernel quiescence proof".into())
    }
}
const ENABLE_ARGS: &[&str] = &["enable", "smith-next.service"];
const DISABLE_ARGS: &[&str] = &["disable", "smith-next.service"];

pub struct SystemManager;
impl Manager for SystemManager {
    fn inspect(&self, roots: &Roots) -> Result<ManagerFacts> {
        let output = command(
            roots,
            &[
                "show",
                "smith-next.service",
                "--property=LoadState,ActiveState,SubState,MainPID,FragmentPath,DropInPaths,NeedDaemonReload,Transient,UnitFileState",
            ],
        )?;
        parse(&output)
    }
    fn reload(&self, roots: &Roots) -> Result<()> {
        command(roots, &["daemon-reload"]).map(|_| ())
    }
    fn start(&self, roots: &Roots) -> Result<()> {
        command(roots, &["start", "smith-next.service"]).map(|_| ())
    }
    fn enable(&self, roots: &Roots) -> Result<()> {
        command(roots, ENABLE_ARGS).map(|_| ())
    }
    fn disable(&self, roots: &Roots) -> Result<()> {
        command(roots, DISABLE_ARGS).map(|_| ())
    }
    fn maintenance_inspect(&self, roots: &Roots) -> Result<MaintenanceManagerFacts> {
        parse_maintenance(&command(roots, &[
            "show", "smith-next.service",
            "--property=LoadState,ActiveState,SubState,MainPID,FragmentPath,DropInPaths,NeedDaemonReload,Transient,UnitFileState,ControlGroup",
        ])?)
    }
    fn stop(&self, roots: &Roots) -> Result<()> {
        command(roots, &["stop", "smith-next.service"]).map(|_| ())
    }
    fn quiescence(&self, _roots: &Roots, control_group: &str) -> Result<()> {
        kernel_quiescence(control_group)
    }
}

fn parse_maintenance(text: &str) -> Result<MaintenanceManagerFacts> {
    let mut base = String::new();
    let mut group = None;
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or("Malformed maintenance properties")?;
        match key {
            "ControlGroup" if group.is_none() => group = Some(value.to_owned()),
            "ControlGroup" => return Err("Duplicate maintenance property".into()),
            _ => {
                base.push_str(line);
                base.push('\n');
            }
        }
    }
    let service = parse(&base)?;
    let facts = MaintenanceManagerFacts {
        unit_file_state: service.unit_file_state.clone(),
        service,
        control_group: group.ok_or("Missing ControlGroup")?,
    };
    validate_group(&facts.control_group)?;
    Ok(facts)
}

fn validate_group(group: &str) -> Result<()> {
    if !group.is_empty()
        && (!group.starts_with('/')
            || group == "/"
            || group.split('/').skip(1).any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.@\\x".contains(&b))
            }))
    {
        return Err("Unsafe ControlGroup path".into());
    }
    Ok(())
}

// Walk held no-follow directory descriptors on cgroup2; never signal a PID.
fn kernel_quiescence(group: &str) -> Result<()> {
    use std::ffi::{CStr, CString};
    use std::fs::File;
    use std::os::fd::FromRawFd;
    validate_group(group)?;
    let fallback = format!(
        "/user.slice/user-{}.slice/user@{}.service/app.slice/smith-next.service",
        crate::filesystem::uid(),
        crate::filesystem::uid()
    );
    // The exact unit has no Slice override: Ubuntu's user-manager default is app.slice.
    // When systemd clears ControlGroup after Stop, require that known manager hierarchy
    // to exist; missing/unknown parent hierarchy is not proof of an empty service group.
    let cleared = group.is_empty();
    let group = if cleared { &fallback } else { group };
    fn open(parent: i32, name: &CStr, directory: bool) -> std::io::Result<File> {
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if directory { libc::O_DIRECTORY } else { 0 };
        // SAFETY: name is NUL-terminated and parent is a held directory descriptor.
        let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: openat returned a fresh owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    fn empty(dir: &File, budget: &mut usize) -> Result<()> {
        if *budget == 0 {
            return Err("Cgroup inspection budget exceeded".into());
        }
        *budget -= 1;
        let mut procs = open(dir.as_raw_fd(), c"cgroup.procs", false)
            .map_err(|_| "Cannot inspect cgroup.procs")?;
        let mut bytes = Vec::new();
        (&mut procs)
            .take(4097)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read cgroup.procs")?;
        if !bytes.is_empty() {
            return Err("Service cgroup still contains processes".into());
        }
        // SAFETY: dup creates an independent descriptor owned by fdopendir.
        let duplicate = unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
        if duplicate < 0 {
            return Err("Cannot enumerate cgroup".into());
        }
        let stream = unsafe { libc::fdopendir(duplicate) };
        if stream.is_null() {
            unsafe {
                libc::close(duplicate);
            }
            return Err("Cannot enumerate cgroup".into());
        }
        let result = (|| {
            loop {
                // SAFETY: stream remains live; errno distinguishes EOF from failure.
                unsafe {
                    *libc::__errno_location() = 0;
                }
                let entry = unsafe { libc::readdir(stream) };
                if entry.is_null() {
                    if unsafe { *libc::__errno_location() } != 0 {
                        return Err("Cgroup enumeration failed".into());
                    }
                    break;
                }
                let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
                if name == c"." || name == c".." {
                    continue;
                }
                // cgroupfs cannot contain symlinks; still refuse unknown entry types.
                match unsafe { (*entry).d_type } {
                    libc::DT_DIR => {
                        let child = open(dir.as_raw_fd(), name, true)
                            .map_err(|_| "Cgroup changed during inspection")?;
                        empty(&child, budget)?;
                    }
                    libc::DT_REG => {}
                    _ => return Err("Unknown cgroup entry type".into()),
                }
            }
            Ok(())
        })();
        unsafe {
            libc::closedir(stream);
        }
        result
    }
    let mut dir = open(libc::AT_FDCWD, c"/", true).map_err(|_| "Filesystem root unavailable")?;
    for component in [c"sys", c"fs", c"cgroup"] {
        dir = open(dir.as_raw_fd(), component, true).map_err(|_| "Cgroup2 root unavailable")?;
    }
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(dir.as_raw_fd(), &mut stat) } != 0 || stat.f_type != 0x63677270 {
        return Err("Unified kernel cgroup2 required".into());
    }
    let parts: Vec<_> = group.split('/').skip(1).collect();
    for (index, part) in parts.iter().enumerate() {
        let name = CString::new(*part).map_err(|_| "Invalid cgroup component")?;
        match open(dir.as_raw_fd(), &name, true) {
            Ok(child) => dir = child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if cleared && index + 1 != parts.len() {
                    return Err("Unknown user-manager cgroup hierarchy; quiescence unproven".into());
                }
                return Ok(());
            }
            Err(_) => return Err("Cannot safely inspect service cgroup".into()),
        }
    }
    empty(&dir, &mut 256)
}

fn command(roots: &Roots, args: &[&str]) -> Result<String> {
    let mut command = Command::new("/usr/bin/systemctl");
    command
        .args(["--user", "--no-pager", "--no-ask-password"])
        .args(args)
        .env_clear()
        .env("XDG_RUNTIME_DIR", &roots.runtime)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    execute(command, Duration::from_secs(10))
}

fn execute(mut command: Command, budget: Duration) -> Result<String> {
    check_cancellation()?;
    // SAFETY: getpid has no arguments or memory access. Capture in the parent,
    // not the child: a parent already lost before prctl must prevent exec.
    let expected_parent = unsafe { libc::getpid() };
    command.process_group(0);
    // SAFETY: the closure invokes only libc syscalls/errno construction before
    // exec, without allocating, taking locks, or accessing other threads.
    unsafe {
        command.pre_exec(move || parent_death_guard(expected_parent));
    }
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|_| "User manager command unavailable")?;
    let pid = child.id() as i32;
    let result = (|| {
        let mut stdout = child.stdout.take().ok_or("Manager output unavailable")?;
        // SAFETY: fcntl changes flags on our valid pipe descriptor.
        let flags = unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                < 0
        {
            return Err("Cannot bound manager output".into());
        }
        // Reserve the final second for captured-child cleanup inside the total budget.
        let deadline = started + budget.saturating_sub(Duration::from_secs(1));
        let mut bytes = Vec::new();
        loop {
            check_cancellation()?;
            let mut chunk = [0; 4096];
            match stdout.read(&mut chunk) {
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => break Err("Manager output failed".into()),
            }
            if bytes.len() > 16384 {
                break Err("Manager output exceeds limit".into());
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Drain only bounded available bytes after exit; never wait on descendants.
                    let mut chunk = [0; 16385];
                    if let Ok(n) = stdout.read(&mut chunk) {
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    break if !status.success() {
                        Err("User manager command failed; intermediate files preserved".into())
                    } else if bytes.len() > 16384 {
                        Err("Manager output exceeds limit".into())
                    } else {
                        String::from_utf8(bytes).map_err(|_| "Invalid manager output".into())
                    };
                }
                Ok(None) => {}
                Err(_) => break Err("Cannot observe manager child".into()),
            }
            if Instant::now() >= deadline {
                break Err("User manager command timed out".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    // This group cleanup requires the helper to remain alive. Abrupt helper
    // death invokes PDEATHSIG only on the direct child, not forked descendants.
    // SAFETY: only the process group created for this captured child is killed.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let cleanup = started + budget;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return result,
            _ if Instant::now() >= cleanup => {
                return Err("Manager child cleanup unconfirmed".into())
            }
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn boolean(value: &str) -> Result<bool> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err("Invalid manager boolean".into()),
    }
}

fn parse(text: &str) -> Result<ManagerFacts> {
    let mut values = std::collections::BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or("Malformed manager properties")?;
        if ![
            "LoadState",
            "ActiveState",
            "SubState",
            "MainPID",
            "FragmentPath",
            "DropInPaths",
            "NeedDaemonReload",
            "Transient",
            "UnitFileState",
        ]
        .contains(&key)
            || values.insert(key, value).is_some()
        {
            return Err("Unknown or duplicate manager property".into());
        }
    }
    if values.len() != 9 {
        return Err("Missing manager property".into());
    }
    let pid_text = values["MainPID"];
    if pid_text.is_empty() || !pid_text.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid manager PID".into());
    }
    let facts = ManagerFacts {
        load: values["LoadState"].into(),
        active: values["ActiveState"].into(),
        sub: values["SubState"].into(),
        pid: pid_text.parse().map_err(|_| "Invalid manager PID")?,
        fragment: values["FragmentPath"].into(),
        drop_ins: values["DropInPaths"].into(),
        need_reload: boolean(values["NeedDaemonReload"])?,
        transient: boolean(values["Transient"])?,
        unit_file_state: values["UnitFileState"].into(),
    };
    if !["loaded", "not-found", "error", "masked", "bad-setting"].contains(&facts.load.as_str())
        || ![
            "active",
            "inactive",
            "failed",
            "activating",
            "deactivating",
            "reloading",
        ]
        .contains(&facts.active.as_str())
        || facts.sub.is_empty()
        || ![
            "",
            "disabled",
            "enabled",
            "enabled-runtime",
            "linked",
            "linked-runtime",
            "alias",
            "masked",
            "masked-runtime",
            "indirect",
            "static",
            "generated",
            "transient",
            "bad",
        ]
        .contains(&facts.unit_file_state.as_str())
        || facts.pid > i32::MAX as u32
    {
        return Err("Unsupported manager facts".into());
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
    #[test]
    fn enable_and_disable_are_fixed_and_never_change_runtime() {
        assert_eq!(ENABLE_ARGS, ["enable", "smith-next.service"]);
        assert_eq!(DISABLE_ARGS, ["disable", "smith-next.service"]);
        assert!(!ENABLE_ARGS.contains(&"--now"));
        assert!(!DISABLE_ARGS.contains(&"--now"));
    }

    #[test]
    fn maintenance_properties_require_exact_additional_facts_and_safe_groups() {
        let base = "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nFragmentPath=/private/unit\nDropInPaths=\nNeedDaemonReload=no\nTransient=no\nUnitFileState=static\n";
        let valid = format!("{base}ControlGroup=/user.slice/smith-next.service\n");
        assert!(super::parse_maintenance(&valid).is_ok());
        for bad in [
            base.to_owned(),
            format!("{valid}UnitFileState=static\n"),
            valid.replace("static", "unknown"),
            valid.replace("/user.slice/smith-next.service", "/../escape"),
            valid.replace("/user.slice/smith-next.service", "/"),
            format!("{valid}Other=value\n"),
        ] {
            assert!(super::parse_maintenance(&bad).is_err());
        }
        for group in ["../escape", "/../escape", "/a//b", "/a/./b", "/", "/a\0b"] {
            assert!(super::kernel_quiescence(group).is_err());
        }
    }
    use super::*;
    fn fake(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", script])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0);
        command
    }
    #[test]
    fn injected_fake_command_deadline_output_and_pipe_cleanup_are_bounded() {
        let budget = Duration::from_millis(1200);
        let started = Instant::now();
        assert!(execute(fake("sleep 5"), budget)
            .unwrap_err()
            .contains("timed out"));
        assert!(started.elapsed() < budget + Duration::from_millis(200));
        assert_eq!(
            execute(fake("printf 'fake manager'; sleep 5 &"), budget).unwrap(),
            "fake manager"
        );
        assert!(execute(
            fake("while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done"),
            budget
        )
        .unwrap_err()
        .contains("exceeds limit"));
        assert!(execute(fake("exit 7"), budget).is_err());
    }
    static NEXT_SANDBOX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn sandbox() -> std::path::PathBuf {
        use std::os::unix::fs::DirBuilderExt;
        let path = std::env::temp_dir().join(format!(
            "smith-manager-test-{}-{}",
            std::process::id(),
            NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        path
    }
    fn alive(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| {
                s.rsplit_once(") ")
                    .map(|(_, rest)| !rest.starts_with('Z') && !rest.starts_with('X'))
            })
            .unwrap_or(false)
    }
    struct CapturedHelper {
        child: std::process::Child,
        pids: Vec<u32>,
        directory: std::path::PathBuf,
    }
    impl Drop for CapturedHelper {
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
            }
            let deadline = Instant::now() + Duration::from_secs(1);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if let Some(pid) = self.pids.first() {
                // SAFETY: this is the isolated group recorded by our fake child.
                unsafe {
                    libc::kill(-(*pid as i32), libc::SIGKILL);
                }
            }
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
    fn captured_helper(kind: &str, expected_pids: usize) -> CapturedHelper {
        let directory = sandbox();
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "manager::tests::captured_helper_fixture",
                "--nocapture",
            ])
            .env("SMITH_TEST_HELPER_PID_FILE", directory.join("pids"))
            .env("SMITH_TEST_HELPER_KIND", kind)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut helper = CapturedHelper {
            child,
            pids: Vec::new(),
            directory,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = std::fs::read_to_string(helper.directory.join("pids")) {
                let pids: Vec<u32> = text
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if pids.len() == expected_pids && pids.iter().all(|p| *p > 0 && alive(*p)) {
                    helper.pids = pids;
                    return helper;
                }
            }
            assert!(
                helper.child.try_wait().unwrap().is_none(),
                "fake helper exited before child capture"
            );
            assert!(Instant::now() < deadline, "fake helper capture deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn wait_for_helper_exit(helper: &mut CapturedHelper) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = helper.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "captured helper cleanup deadline"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn assert_commands_exited(pids: &[u32]) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while pids.iter().any(|p| alive(*p)) {
            assert!(
                Instant::now() < deadline,
                "fake manager command still executing"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    #[ignore = "fixture invoked only by captured-helper subprocess tests"]
    fn captured_helper_fixture() {
        let path = std::env::var_os("SMITH_TEST_HELPER_PID_FILE").expect("fixture PID path");
        let kind = std::env::var("SMITH_TEST_HELPER_KIND").unwrap();
        install_control_cancellation().unwrap();
        let script = if kind == "sigkill" {
            "printf '%s' $$ > \"$1\"; exec /bin/sleep 30"
        } else {
            "sleep 30 & printf '%s %s' $$ $! > \"$1\"; wait"
        };
        let mut command = fake(script);
        command.arg("fake-manager").arg(path);
        let error = execute(command, Duration::from_secs(10)).unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
    }
    #[test]
    fn abrupt_helper_death_kills_captured_direct_fake_manager_child() {
        let mut helper = captured_helper("sigkill", 1);
        helper.child.kill().unwrap();
        assert!(!wait_for_helper_exit(&mut helper).success());
        assert_commands_exited(&helper.pids);
        // No claim about forked descendants: PDEATHSIG is not inherited by fork.
    }
    #[test]
    fn cooperative_helper_termination_cleans_captured_fake_manager_group() {
        let mut helper = captured_helper("sigterm", 2);
        // SAFETY: SIGTERM is sent only to our captured helper subprocess.
        assert_eq!(
            unsafe { libc::kill(helper.child.id() as i32, libc::SIGTERM) },
            0
        );
        assert!(wait_for_helper_exit(&mut helper).success());
        assert_commands_exited(&helper.pids);
    }
    #[test]
    fn parent_changed_before_guard_armed_prevents_exec() {
        let directory = sandbox();
        let marker = directory.join("must-not-exec");
        let mut command = fake("printf 'unexpected' > \"$1\"");
        command.arg("fake-manager").arg(&marker);
        // SAFETY: the child closure only calls the async-signal-safe guard.
        unsafe {
            command.pre_exec(|| parent_death_guard(-1));
        }
        assert_eq!(
            command.spawn().unwrap_err().raw_os_error(),
            Some(libc::ECANCELED)
        );
        assert!(!marker.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn properties_fail_closed() {
        let good = "LoadState=loaded\nActiveState=active\nSubState=running\nMainPID=42\nFragmentPath=/private/unit\nDropInPaths=\nNeedDaemonReload=no\nTransient=no\nUnitFileState=enabled\n";
        assert_eq!(parse(good).unwrap().pid, 42);
        assert!(parse(&format!("{good}MainPID=42\n")).is_err());
        assert!(parse(&good.replace("42", "-1")).is_err());
        assert!(parse(&good.replace("loaded", "invented")).is_err());
        assert!(parse("MainPID=42\n").is_err());
    }
}
