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
}

/// Injection is a Rust API for isolated tests; executable arguments/environment
/// can never select a manager executable.
pub trait Manager {
    fn inspect(&self, roots: &Roots) -> Result<ManagerFacts>;
    fn reload(&self, roots: &Roots) -> Result<()>;
    fn start(&self, roots: &Roots) -> Result<()>;
}
pub struct SystemManager;
impl Manager for SystemManager {
    fn inspect(&self, roots: &Roots) -> Result<ManagerFacts> {
        let output = command(
            roots,
            &[
                "show",
                "smith-next.service",
                "--property=LoadState,ActiveState,SubState,MainPID,FragmentPath,DropInPaths,NeedDaemonReload,Transient",
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
        ]
        .contains(&key)
            || values.insert(key, value).is_some()
        {
            return Err("Unknown or duplicate manager property".into());
        }
    }
    if values.len() != 8 {
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
        || facts.pid > i32::MAX as u32
    {
        return Err("Unsupported manager facts".into());
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
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
        let good = "LoadState=loaded\nActiveState=active\nSubState=running\nMainPID=42\nFragmentPath=/private/unit\nDropInPaths=\nNeedDaemonReload=no\nTransient=no\n";
        assert_eq!(parse(good).unwrap().pid, 42);
        assert!(parse(&format!("{good}MainPID=42\n")).is_err());
        assert!(parse(&good.replace("42", "-1")).is_err());
        assert!(parse(&good.replace("loaded", "invented")).is_err());
        assert!(parse("MainPID=42\n").is_err());
    }
}
