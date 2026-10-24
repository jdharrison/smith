//! Own only the hook process group. Never signal a service PID or driver group.
use smith_installer::Result;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            fs::{MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const INPUT_CAP: usize = 64 * 1024;
const LOG_CAP: usize = 8192;
const LINE_CAP: usize = 160;
const GRACE: Duration = Duration::from_millis(250);

/// Parent creates this external private directory. No mkdir/permission repair,
/// truncation, or raw hook output is performed by the operational driver.
pub fn private_log() -> Result<File> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("Private log refused")?);
    let mut dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(home)
        .map_err(|_| "Private log refused")?;
    for name in [
        ".local",
        "state",
        "smith-qa-backups",
        "dev-coordinated-20261019",
    ] {
        check_dir(&dir, false)?;
        let name = std::ffi::CString::new(name).map_err(|_| "Private log refused")?;
        // SAFETY: valid directory FD and NUL-terminated fixed component; capture
        // the returned FD exactly once. No-follow applies to every component.
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err("Private log refused".into());
        }
        dir = unsafe { File::from_raw_fd(fd) };
    }
    check_dir(&dir, true)?;
    let name = std::ffi::CString::new(format!("driver-hook-{}.log", std::process::id()))
        .map_err(|_| "Private log refused")?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err("Private log refused".into());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    dir.sync_all().map_err(|_| "Private log refused")?;
    Ok(file)
}
fn check_dir(dir: &File, private: bool) -> Result<()> {
    let m = dir.metadata().map_err(|_| "Private log refused")?;
    super::ensure(
        m.is_dir()
            && m.uid() == unsafe { libc::geteuid() }
            && m.mode() & 0o022 == 0
            && (!private || m.mode() & 0o077 == 0),
    )
}

struct Diagnostics {
    file: File,
    lines: [Vec<u8>; 2],
    input: usize,
    written: usize,
    bad: bool,
}
impl Diagnostics {
    fn new(file: File) -> Self {
        Self {
            file,
            lines: [Vec::new(), Vec::new()],
            input: 0,
            written: 0,
            bad: false,
        }
    }
    fn record(&mut self, line: &[u8]) {
        // Normalize only these exact existing coordinator status lines. No
        // arbitrary prose, subprocess diagnostics or exception text is retained.
        let line: &[u8] = match line {
            b"Coordinated phase complete; no job submitted." => b"phase=complete errorcode=OK",
            b"Coordinated maintenance incomplete; no automatic retry, Start or rollback. Inspect private evidence." => b"phase=stopped-hook errorcode=INCOMPLETE",
            _ => line,
        };
        let valid = std::str::from_utf8(line).is_ok_and(|s| {
            let Some((phase, code)) = s.split_once(" errorcode=") else {
                return false;
            };
            matches!(
                phase,
                "phase=stopped-hook"
                    | "phase=snapshot"
                    | "phase=quiescence"
                    | "phase=reconciliation"
                    | "phase=worker-image"
                    | "phase=verification"
                    | "phase=complete"
                    | "phase=driver"
            ) && matches!(
                code,
                "OK" | "FAILED"
                    | "REFUSED"
                    | "TIMEOUT"
                    | "CANCELLED"
                    | "IO"
                    | "INCOMPLETE"
                    | "REDACTED"
                    | "OUTPUT_LIMIT"
            )
        });
        let line = if valid {
            line
        } else {
            self.bad = true;
            b"phase=driver errorcode=REDACTED"
        };
        if self.written + line.len() + 1 > LOG_CAP {
            self.bad = true;
            return;
        }
        if self
            .file
            .write_all(line)
            .and_then(|()| self.file.write_all(b"\n"))
            .is_err()
        {
            self.bad = true;
        }
        self.written += line.len() + 1;
    }
    fn drain<R: Read>(&mut self, stream: &mut R, index: usize) {
        // Bounded work per poll, even if a malicious/noisy child writes forever.
        let mut buf = [0; 4096];
        for _ in 0..4 {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    self.input = self.input.saturating_add(n);
                    if self.input > INPUT_CAP {
                        self.bad = true;
                        continue;
                    }
                    for &b in &buf[..n] {
                        if b == b'\n' {
                            let line = std::mem::take(&mut self.lines[index]);
                            self.record(&line);
                        } else if self.lines[index].len() < LINE_CAP {
                            self.lines[index].push(b);
                        } else {
                            self.bad = true;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.bad = true;
                    break;
                }
            }
        }
    }
    fn finish(&mut self) -> Result<()> {
        if self.lines.iter().any(|s| !s.is_empty()) {
            self.record(b"phase=driver errorcode=REDACTED");
            self.bad = true;
        }
        self.file.sync_all().map_err(|_| "Private log incomplete")?;
        super::ensure(!self.bad)
    }
}

struct Group {
    child: Child,
    pgid: libc::pid_t,
    joined: bool,
}
impl Group {
    fn exists(&self) -> bool {
        // ESRCH is the only acceptable absence proof. EPERM/unknown retains leases.
        (unsafe { libc::kill(-self.pgid, 0) == 0 })
            || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
    fn signal(&self, signal: libc::c_int) {
        // pgid is always the dedicated child PID returned by our own spawn.
        // No caller-provided PID and no signal to the live service/driver group.
        unsafe {
            libc::kill(-self.pgid, signal);
        }
    }
    fn reap(&self) {
        loop {
            let mut status = 0;
            let pid = unsafe { libc::waitpid(-self.pgid, &mut status, libc::WNOHANG) };
            if pid > 0 {
                continue;
            }
            if pid < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            break;
        }
    }
    fn join(&mut self) {
        if self.joined {
            return;
        }
        self.signal(libc::SIGTERM);
        let until = Instant::now() + GRACE;
        loop {
            self.reap();
            if !self.exists() {
                // A final group-scoped wait drains any already adopted zombies.
                self.reap();
                self.joined = true;
                return;
            }
            if Instant::now() >= until {
                self.signal(libc::SIGKILL);
            }
            // Intentionally no cleanup timeout: unknown/uninterruptible members
            // must not cause callback unwind and release the native leases.
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        self.join();
    }
}

fn nonblocking(fd: libc::c_int) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    super::ensure(
        flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0,
    )
}

/// Hook and all its subprocesses MUST retain this group (no setsid/setpgid or
/// daemonization). The outer coordinator must TERM the driver and wait, never
/// force-kill it while leases are held. Uncatchable driver death is not a
/// quiescence guarantee; PDEATHSIG protects the direct child only.
pub fn run(
    command: &mut Command,
    log: File,
    budget: Duration,
    cancelled: impl Fn() -> bool,
) -> Result<()> {
    // Reparent hook orphans here, but waitpid is ALWAYS scoped to -hook_pgid.
    super::ensure(unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } == 0)?;
    let parent = unsafe { libc::getpid() };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
            {
                return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    if cancelled() {
        return Err("Hook cancelled before spawn".into());
    }
    let child = command.spawn().map_err(|_| "Hook spawn refused")?;
    let pgid = child.id() as libc::pid_t;
    let mut group = Group {
        child,
        pgid,
        joined: false,
    };
    let mut diagnostics = Diagnostics::new(log);
    let mut stdout = group.child.stdout.take().ok_or("Hook pipe refused")?;
    let mut stderr = group.child.stderr.take().ok_or("Hook pipe refused")?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    let until = Instant::now() + budget;
    let success = loop {
        diagnostics.drain(&mut stdout, 0);
        diagnostics.drain(&mut stderr, 1);
        if cancelled() {
            diagnostics.record(b"phase=driver errorcode=CANCELLED");
            break false;
        }
        if Instant::now() >= until {
            diagnostics.record(b"phase=driver errorcode=TIMEOUT");
            break false;
        }
        if diagnostics.bad {
            diagnostics.record(b"phase=driver errorcode=OUTPUT_LIMIT");
            break false;
        }
        match group.child.try_wait() {
            Ok(Some(status)) => {
                // A successful Python exit with surviving descendants is NOT
                // success. Cleanup/join still runs before returning failure.
                let clean = !group.exists();
                if clean {
                    group.joined = true;
                }
                diagnostics.record(if status.success() && clean {
                    b"phase=driver errorcode=OK"
                } else {
                    b"phase=driver errorcode=INCOMPLETE"
                });
                break status.success() && clean;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                diagnostics.record(b"phase=driver errorcode=IO");
                break false;
            }
        }
    };
    group.join();
    diagnostics.drain(&mut stdout, 0);
    diagnostics.drain(&mut stderr, 1);
    diagnostics.finish()?;
    super::ensure(success && !cancelled())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!(
                    "hook-fixture-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn log(&self) -> File {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.0.join("log"))
                .unwrap()
        }
        fn command(&self, parent_exits: bool) -> Command {
            let mut cmd = Command::new("python3");
            cmd.arg("-B").arg("-c").arg("import os,signal,subprocess,sys,time\nsignal.signal(signal.SIGTERM,signal.SIG_IGN)\np=subprocess.Popen([sys.executable,'-B','-c','import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(60)'])\nwith open(sys.argv[1],'w') as f: f.write(str(os.getpid())+' '+str(p.pid))\nprint('phase=quiescence errorcode=OK',flush=True)\nif sys.argv[2]=='exit': sys.exit(0)\ntime.sleep(60)\n").arg(self.0.join("pids")).arg(if parent_exits { "exit" } else { "wait" });
            cmd
        }
        fn assert_gone(&self) {
            let pids = std::fs::read_to_string(self.0.join("pids")).unwrap();
            for pid in pids.split_whitespace() {
                let pid: i32 = pid.parse().unwrap();
                assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
                let mut status = 0;
                assert_eq!(
                    unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
                    -1
                );
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ECHILD)
                );
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn timeout_kills_and_reaps_actual_term_ignoring_grandchild() {
        let f = Fixture::new();
        assert!(run(
            &mut f.command(false),
            f.log(),
            Duration::from_millis(1000),
            || false
        )
        .is_err());
        f.assert_gone();
        let text = std::fs::read_to_string(f.0.join("log")).unwrap();
        assert!(text.contains("errorcode=TIMEOUT"));
        assert!(text.len() <= LOG_CAP);
    }
    #[test]
    fn leader_success_with_live_grandchild_is_failure_and_joins() {
        let f = Fixture::new();
        assert!(run(
            &mut f.command(true),
            f.log(),
            Duration::from_secs(5),
            || false
        )
        .is_err());
        f.assert_gone();
    }
    #[test]
    fn cancellation_joins_group_before_return() {
        let f = Fixture::new();
        let start = Instant::now();
        assert!(run(
            &mut f.command(false),
            f.log(),
            Duration::from_secs(5),
            || start.elapsed() >= Duration::from_millis(1000)
        )
        .is_err());
        f.assert_gone();
    }
    #[test]
    fn diagnostics_are_bounded_allowlisted_not_raw_paths() {
        let f = Fixture::new();
        let mut d = Diagnostics::new(f.log());
        d.record(b"phase=snapshot errorcode=OK");
        d.record(b"Traceback /private/secret-path: credential");
        for _ in 0..1000 {
            d.record(b"phase=driver errorcode=FAILED");
        }
        assert!(d.finish().is_err());
        let text = std::fs::read_to_string(f.0.join("log")).unwrap();
        assert!(text.len() <= LOG_CAP);
        assert!(!text.contains("secret"));
        assert!(text.contains("REDACTED"));
    }
    #[test]
    fn normal_success_requires_no_remaining_group() {
        let f = Fixture::new();
        let mut cmd = Command::new("python3");
        cmd.arg("-B")
            .arg("-c")
            .arg("print('phase=complete errorcode=OK',flush=True)");
        run(&mut cmd, f.log(), Duration::from_secs(5), || false).unwrap();
        assert!(std::fs::read_to_string(f.0.join("log"))
            .unwrap()
            .contains("phase=complete errorcode=OK"));
    }
    #[test]
    fn cleanup_never_signals_an_unrelated_owned_fixture_group() {
        let f = Fixture::new();
        let mut cmd = Command::new("python3");
        let child = cmd
            .arg("-B")
            .arg("-c")
            .arg("import time; time.sleep(60)")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = child.id() as libc::pid_t;
        let mut unrelated = Group {
            child,
            pgid,
            joined: false,
        };
        let mut hook = Command::new("python3");
        hook.arg("-B").arg("-c").arg("import time; time.sleep(60)");
        assert!(run(&mut hook, f.log(), Duration::from_millis(100), || false).is_err());
        assert!(unrelated.child.try_wait().unwrap().is_none());
        unrelated.join();
    }
    #[test]
    fn output_flood_is_refused_without_hang() {
        let f = Fixture::new();
        let mut cmd = Command::new("python3");
        cmd.arg("-B")
            .arg("-c")
            .arg("import os; os.write(1,b'x'*1000000)");
        assert!(run(&mut cmd, f.log(), Duration::from_secs(5), || false).is_err());
        assert!(std::fs::metadata(f.0.join("log")).unwrap().len() <= LOG_CAP as u64);
    }
}
