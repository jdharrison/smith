//! Explicit GUI-only dispatch. No service lifecycle, shell, PATH or payload execution.
use crate::artifact::{data_root, resolve_installed_client, InstalledClient};
use crate::Result;
use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Dispatch completion, not desktop readiness or desktop lifetime.
pub type LaunchResult = Receiver<Result<()>>;

/// Injection seam for the GTK shell. Starting must not block on validation or a child.
pub trait DesktopLauncher {
    fn start(&self) -> Result<LaunchResult>;
}

pub struct NativeLauncher {
    data: Result<PathBuf>,
}

impl NativeLauncher {
    /// Capture installer-compatible roots once, without inspecting or modifying them.
    pub fn from_environment() -> Self {
        Self { data: data_root() }
    }
}

impl DesktopLauncher for NativeLauncher {
    fn start(&self) -> Result<LaunchResult> {
        let data = self.data.clone()?;
        let (sender, receiver) = mpsc::sync_channel(1);
        // Create the waiter before spawning a child: thread creation failure must
        // never leave a successfully launched child without its owner/reaper.
        thread::Builder::new()
            .name("smith-desktop-launch".into())
            .spawn(move || {
                let result = resolve_installed_client(&data).and_then(|target| spawn(&target));
                match result {
                    Ok(mut child) => {
                        let _ = sender.send(Ok(()));
                        // The UI never joins this thread. On installer exit the
                        // detached desktop is reparented; Close never kills it.
                        let _ = child.wait();
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                    }
                }
            })
            .map_err(|e| format!("Cannot start desktop launch worker: {e}"))?;
        Ok(receiver)
    }
}

fn spawn(target: &InstalledClient) -> Result<Child> {
    crate::consent::check()?;
    let executable = CString::new(target.executable.as_os_str().as_bytes())
        .map_err(|_| "Invalid installed executable path")?;
    let mut command = Command::new(&target.executable);
    command
        .current_dir(&target.release_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: after fork use only async-signal-safe setsid/execv and errno
    // conversion. CString storage is prepared before fork and remains live.
    // Direct execv is deliberate: libc execvp (used by Command) can fall back
    // to /bin/sh on ENOEXEC even for an absolute path. Never take that fallback.
    // Command still owns cwd/stdio setup and its CLOEXEC spawn-error pipe.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            let argv = [executable.as_ptr(), std::ptr::null()];
            libc::execv(executable.as_ptr(), argv.as_ptr());
            Err(io::Error::last_os_error())
        });
    }
    command
        .spawn()
        .map_err(|e| format!("Cannot launch installed Smith client: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::fixture::{chmod, write, Fixture};
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn until(mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !predicate() {
            assert!(Instant::now() < deadline, "fixture timed out");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait(child: &mut Child) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "fixture process failed: {status}");
                return;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("fixture process timed out");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn installed_desktop(f: &Fixture) -> PathBuf {
        let source = f.root.join("desktop.rs");
        write(
            &source,
            include_bytes!("../tests/fixtures/desktop.rs"),
            0o600,
        );
        // Builder-only compiler; production launch never resolves an executable on PATH.
        let mut compiler = Command::new("rustc")
            .arg("--edition=2021")
            .arg(&source)
            .arg("-o")
            .arg(f.bundle.join("smith_client"))
            .spawn()
            .unwrap();
        wait(&mut compiler);
        chmod(&f.bundle.join("smith_client"), 0o755);
        f.install(true, true).unwrap();
        fs::canonicalize(f.current()).unwrap()
    }

    fn probe(release: &Path) -> u32 {
        until(|| release.join("launch-probe").exists());
        let content = fs::read_to_string(release.join("launch-probe")).unwrap();
        let fields: Vec<_> = content.lines().collect();
        let pid: u32 = fields[0].parse().unwrap();
        assert_eq!(fields[1], fields[0], "new session leader");
        assert_eq!(fields[2], fields[0], "separate process group");
        assert_eq!(fields[4], "1", "no client arguments");
        assert_eq!(fields[5], release.to_str().unwrap(), "resolved release cwd");
        assert_eq!(&fields[6..9], &["/dev/null"; 3]);
        pid
    }

    #[test]
    fn native_launch_reports_spawn_then_reaps_even_without_receiver() {
        for disconnect in [false, true] {
            let f = Fixture::new();
            let release = installed_desktop(&f);
            let launcher = NativeLauncher {
                data: Ok(f.data.clone()),
            };
            let result = launcher.start().unwrap();
            if !disconnect {
                result
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap()
                    .unwrap();
            }
            drop(result);
            let pid = probe(&release);
            assert!(
                PathBuf::from(format!("/proc/{pid}")).exists(),
                "dispatch did not wait for desktop exit"
            );
            fs::write(release.join("launch-exit"), b"exit").unwrap();
            until(|| !PathBuf::from(format!("/proc/{pid}")).exists());
            assert!(release.join("launch-done").exists());
            let mut status = 0;
            // SAFETY: valid status pointer, exact already-reaped fixture PID.
            assert_eq!(
                unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) },
                -1
            );
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
        }
    }

    #[test]
    fn desktop_survives_launcher_process_exit() {
        let f = Fixture::new();
        let release = installed_desktop(&f);
        let mut parent = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "launch::tests::launch_parent_fixture",
                "--ignored",
            ])
            .env("SMITH_LAUNCH_TEST_DATA", &f.data)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait(&mut parent);
        let pid = probe(&release);
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        assert!(
            !stat.split_once(") ").unwrap().1.starts_with('Z'),
            "desktop died with installer"
        );
        fs::write(release.join("launch-exit"), b"exit").unwrap();
        until(|| release.join("launch-done").exists());
    }

    #[test]
    #[ignore = "finite subprocess fixture, invoked only with private test roots"]
    fn launch_parent_fixture() {
        let data = PathBuf::from(std::env::var_os("SMITH_LAUNCH_TEST_DATA").expect("fixture root"));
        NativeLauncher { data: Ok(data) }
            .start()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
    }

    #[test]
    fn invalid_target_and_exec_failure_are_reported_without_effects() {
        let f = Fixture::new();
        let launcher = NativeLauncher {
            data: Ok(f.data.clone()),
        };
        let before = f.snapshot();
        assert!(launcher
            .start()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .is_err());
        assert_eq!(f.snapshot(), before);
        // Published fixture bytes are deliberately not an executable format.
        f.install(true, true).unwrap();
        let before = f.snapshot();
        let error = launcher
            .start()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap_err();
        assert!(
            error.contains("Cannot launch installed Smith client"),
            "{error}"
        );
        assert_eq!(f.snapshot(), before);
    }
}
