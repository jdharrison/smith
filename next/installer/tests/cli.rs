//! Executed native CLI tests under private roots. No manager/runtime/provider
//! roots exist for dry-run/initial-decline cases, so lazy construction is required.
use smith_installer::cli::{self, Command as Parsed, Mode};
use smith_installer::manifest::APP;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn parse(args: &[&str]) -> smith_installer::Result<Parsed> {
    cli::parse(args.iter().map(OsString::from))
}
#[test]
fn recipient_flags_and_artifact_only_modes_preserve_their_options() {
    let Parsed::Run(options) = parse(&[
        "--bundle",
        "/package",
        "--model=tiny:exact",
        "--pull-model",
        "--verify-inference",
        "--dry-run",
    ])
    .unwrap() else {
        panic!()
    };
    assert!(options.dry_run);
    assert_eq!(
        options.mode,
        Mode::Recipient {
            bundle: Some(PathBuf::from("/package")),
            model: Some("tiny:exact".into()),
            pull_model: true,
            verify_inference: true
        }
    );
    let Parsed::Run(options) = parse(&[
        "--artifacts-only",
        "--client=/client",
        "--service",
        "/service",
        "--yes",
    ])
    .unwrap() else {
        panic!()
    };
    assert_eq!(
        options.mode,
        Mode::Artifacts {
            client: Some(PathBuf::from("/client")),
            service: Some(PathBuf::from("/service")),
            assume_yes: true
        }
    );
    assert_eq!(parse(&["-h"]).unwrap(), Parsed::Help);
    assert_eq!(parse(&["--help"]).unwrap(), Parsed::Help);
    assert_eq!(parse(&["-V"]).unwrap(), Parsed::Version);
    assert_eq!(parse(&["--version"]).unwrap(), Parsed::Version);
}
#[test]
fn mixed_modes_global_yes_bad_options_and_missing_selection_are_argument_errors() {
    for args in [
        vec!["--yes"],
        vec!["--client", "/client"],
        vec!["--service", "/service"],
        vec!["--artifacts-only"],
        vec!["--pull-model"],
        vec!["--verify-inference"],
        vec!["--model"],
        vec!["--bundle="],
        vec!["--service", "--yes"],
        vec!["--dry-run=yes"],
        vec!["--dry-run", "--dry-run"],
        vec!["unknown"],
        vec![
            "--artifacts-only",
            "--client",
            "/client",
            "--bundle",
            "/package",
        ],
        vec![
            "--artifacts-only",
            "--service",
            "/service",
            "--model",
            "tiny:exact",
        ],
        vec!["--artifacts-only", "--service", "/service", "--pull-model"],
        vec![
            "--artifacts-only",
            "--service",
            "/service",
            "--verify-inference",
        ],
    ] {
        assert!(parse(&args).is_err(), "accepted {args:?}");
    }
}

const FIXTURE_NAMES: &[&str] = &[
    "ancestor-drift",
    "help",
    "exit-codes",
    "partial",
    "read-only",
    "exe-alias",
    "bundle",
    "service-only",
    "artifact-read-only",
    "interleave-source",
    "interleave-peer",
];
static NAMESPACE: OnceLock<Namespace> = OnceLock::new();
struct Namespace {
    root: PathBuf,
    claimed: AtomicU64,
}
impl Namespace {
    fn initialize() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "smith-installer-cli-{}-{stamp}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        // Only namespace setup is one-time. All test bodies and child processes
        // remain parallel. Precreate private leaf roots so fixture creation and
        // cleanup cannot change another publisher's captured ancestor nlinks.
        for name in FIXTURE_NAMES {
            directory(&root.join(name));
        }
        // SAFETY: this process-only callback runs after the test harness has
        // joined its workers. It removes only this uniquely created temp namespace.
        assert_eq!(unsafe { libc::atexit(cleanup_namespace) }, 0);
        Self {
            root,
            claimed: AtomicU64::new(0),
        }
    }
    fn claim(&self, name: &str) -> PathBuf {
        let slot = FIXTURE_NAMES
            .iter()
            .position(|entry| *entry == name)
            .expect("Declare fixture leaf before parallel tests start");
        let mask = 1u64 << slot;
        assert_eq!(
            self.claimed.fetch_or(mask, Ordering::Relaxed) & mask,
            0,
            "Fixture leaf was already claimed: {name}"
        );
        self.root.join(name)
    }
}
extern "C" fn cleanup_namespace() {
    if let Some(namespace) = NAMESPACE.get() {
        if let Err(error) = fs::remove_dir_all(&namespace.root) {
            eprintln!("CLI fixture namespace cleanup failed: {error}");
            // SAFETY: cleanup failure must fail the test process, not turn a
            // retained fixture into a silently successful validation result.
            unsafe {
                libc::_exit(1);
            }
        }
    }
}
struct Fixture {
    base: PathBuf,
    package: PathBuf,
    executable: PathBuf,
    home: PathBuf,
    data: PathBuf,
    elsewhere: PathBuf,
}
fn directory(path: &Path) {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn file(path: &Path, contents: &[u8], mode: u32) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
impl Fixture {
    fn new(name: &str) -> Self {
        let base = NAMESPACE.get_or_init(Namespace::initialize).claim(name);
        let (package, home, data, elsewhere) = (
            base.join("package"),
            base.join("home"),
            base.join("data"),
            base.join("elsewhere"),
        );
        for path in [&package, &home, &data, &elsewhere] {
            directory(path);
        }
        for path in [
            "payload/client/lib",
            "payload/client/data/flutter_assets",
            "payload/client/smith_backend",
            "payload/client/share/pixmaps",
            "payload/client/share/applications",
            "payload/service",
        ] {
            directory(&package.join(path));
        }
        let executable = package.join("smith-installer");
        fs::copy(env!("CARGO_BIN_EXE_smith-installer"), &executable).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

        // Real native bytes for both helper copies; never executed as a service.
        let native = fs::read(&executable).unwrap();
        file(
            &package.join("payload/service/smith-service"),
            &native,
            0o755,
        );
        let client = package.join("payload/client");
        file(&client.join("smith_client"), &native, 0o755);
        file(&client.join("smith_backend/smith-service"), &native, 0o755);
        for name in [
            "lib/libflutter_linux_gtk.so",
            "lib/libapp.so",
            "data/icudtl.dat",
            "data/flutter_assets/asset",
        ] {
            file(&client.join(name), b"fixture contents, never run", 0o644);
        }
        file(
            &client.join(format!("share/pixmaps/{APP}.png")),
            b"fixture icon",
            0o644,
        );
        file(
            &client.join(format!("share/applications/{APP}.desktop")),
            b"fixture desktop",
            0o644,
        );
        Self {
            base,
            package,
            executable,
            home,
            data,
            elsewhere,
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(&self.elsewhere)
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_DATA_HOME", &self.data)
            // No config/runtime roots, no usable user bus or endpoint override.
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    fn run(&self, args: &[&str], input: Option<&[u8]>) -> Output {
        let mut command = self.command();
        command.args(args);
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().unwrap();
        if let Some(input) = input {
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(input).unwrap();
        }
        wait(child)
    }
    fn destinations(&self) -> BTreeMap<PathBuf, (u32, u64, Vec<u8>)> {
        fn walk(path: &Path, values: &mut BTreeMap<PathBuf, (u32, u64, Vec<u8>)>) {
            let meta = fs::symlink_metadata(path).unwrap();
            let contents = if meta.is_file() {
                fs::read(path).unwrap()
            } else if meta.file_type().is_symlink() {
                fs::read_link(path)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec()
            } else {
                Vec::new()
            };
            values.insert(path.into(), (meta.mode() & 0o7777, meta.ino(), contents));
            if meta.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    walk(&entry.unwrap().path(), values);
                }
            }
        }
        let mut values = BTreeMap::new();
        walk(&self.home, &mut values);
        walk(&self.data, &mut values);
        values
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Retain this leaf itself until process exit. Removing it now would
        // change the shared namespace's nlink while other publications run.
        for entry in fs::read_dir(&self.base).unwrap() {
            let path = entry.unwrap().path();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                fs::remove_dir_all(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
    }
}
fn wait(mut child: std::process::Child) -> Output {
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
        // SAFETY: these captured test pipes remain owned through the whole wait.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    loop {
        // Publication's truthful effect report can exceed a pipe buffer. Drain
        // both pipes while the process is alive, with no unbounded reader thread.
        for (stream, bytes) in [
            (&mut stdout as &mut dyn Read, &mut out),
            (&mut stderr as &mut dyn Read, &mut err),
        ] {
            let mut chunk = [0; 8192];
            match stream.read(&mut chunk) {
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("CLI fixture output failed: {error}");
                }
            }
        }
        if out.len() > 256 * 1024 || err.len() > 16 * 1024 {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI fixture output exceeded bound");
        }
        if let Some(status) = child.try_wait().unwrap() {
            // Child exit does not prove pipe EOF: a sibling fork can briefly
            // retain a CLOEXEC writer until its own exec. Drain only available
            // bytes, without adopting/waiting for that writer or changing flags.
            drain_available(&mut stdout, &mut out, 256 * 1024, deadline)
                .expect("CLI fixture final stdout drain failed");
            drain_available(&mut stderr, &mut err, 16 * 1024, deadline)
                .expect("CLI fixture final stderr drain failed");
            assert!(out.len() <= 256 * 1024 && err.len() <= 16 * 1024);
            return Output {
                status,
                stdout: out,
                stderr: err,
            };
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!(
                "CLI fixture exceeded its finite budget; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&out),
                String::from_utf8_lossy(&err)
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
/// Called only on nonblocking captured streams after the known child exited.
/// EOF or EAGAIN ends capture; EINTR retries inside the original time/byte bounds.
fn drain_available(
    stream: &mut dyn Read,
    bytes: &mut Vec<u8>,
    limit: usize,
    deadline: Instant,
) -> std::io::Result<()> {
    if bytes.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "CLI fixture output exceeded bound",
        ));
    }
    loop {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "CLI fixture output drain exceeded budget",
            ));
        }
        let mut chunk = [0; 8192];
        // Include one overflow-probe byte while preserving the existing total
        // cap (including bytes collected before exit), not a fresh tail allowance.
        let count = (limit - bytes.len()).min(chunk.len() - 1) + 1;
        match stream.read(&mut chunk[..count]) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() > limit {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "CLI fixture output exceeded bound",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

#[test]
fn known_child_exit_captures_status_and_available_output_without_waiting_for_other_writers() {
    fn pipe() -> (OwnedFd, OwnedFd) {
        let mut descriptors = [-1; 2];
        // SAFETY: pipe2 initializes two owned CLOEXEC descriptors on success.
        assert_eq!(
            unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        }
    }
    let (stdout_reader, stdout_writer) = pipe();
    let (stderr_reader, stderr_writer) = pipe();
    let mut child = Command::new(env!("CARGO_BIN_EXE_smith-installer"))
        .arg("--unknown-capture-regression")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_writer.try_clone().unwrap()))
        .stderr(Stdio::from(stderr_writer.try_clone().unwrap()))
        .spawn()
        .unwrap();
    // Retain sibling-like writer references after child exit so both empty
    // streams produce EAGAIN rather than EOF. No extra process is adopted.
    child.stdout = Some(stdout_reader.into());
    child.stderr = Some(stderr_reader.into());
    let started = Instant::now();
    let output = wait(child);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("Unknown argument; API keys are accepted only by hidden terminal input"));
    assert!(!error.contains("--unknown-capture-regression"));
    assert!(error.contains("Use --help for usage"));
    assert!(started.elapsed() < Duration::from_secs(2));
    // SAFETY: retained descriptors remain open through the completed capture.
    assert!(unsafe { libc::fcntl(stdout_writer.as_raw_fd(), libc::F_GETFD) } >= 0);
    assert!(unsafe { libc::fcntl(stderr_writer.as_raw_fd(), libc::F_GETFD) } >= 0);
}

#[test]
fn post_exit_drain_preserves_prefix_tail_caps_and_handles_interruption_and_eagain() {
    struct AvailableThenWouldBlock {
        data: std::io::Cursor<Vec<u8>>,
        interrupted: bool,
    }
    impl Read for AvailableThenWouldBlock {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            match self.data.read(output)? {
                0 => Err(std::io::ErrorKind::WouldBlock.into()),
                n => Ok(n),
            }
        }
    }
    let tail = vec![b'x'; 8192 + 17];
    let mut source = AvailableThenWouldBlock {
        data: std::io::Cursor::new(tail.clone()),
        interrupted: false,
    };
    let mut bytes = b"prefix".to_vec();
    let mut expected = bytes.clone();
    expected.extend_from_slice(&tail);
    drain_available(
        &mut source,
        &mut bytes,
        expected.len(),
        Instant::now() + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(bytes, expected);
    for extra in [0, 1] {
        let mut source = std::io::Cursor::new(vec![b'x'; 16 + extra]);
        let mut bytes = b"prefix".to_vec();
        let result = drain_available(
            &mut source,
            &mut bytes,
            6 + 16,
            Instant::now() + Duration::from_secs(1),
        );
        assert_eq!(result.is_ok(), extra == 0);
        if let Err(error) = result {
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        }
    }
}

#[test]
fn changed_ancestor_link_count_after_consent_is_rejected_without_destination_writes() {
    let f = Fixture::new("ancestor-drift");
    let service = f.package.join("payload/service/smith-service");
    let source = fs::read(&service).unwrap();
    let before = f.destinations();
    let links = fs::metadata(&f.base).unwrap().nlink();
    let mut publisher = smith_installer::artifact::Installer::with_roots(
        None,
        Some(service.clone()),
        None,
        f.data.clone(),
    )
    .unwrap();
    let result = publisher.run(false, false, &mut |_: &str| {
        directory(&f.base.join("unrelated-sibling"));
        assert_eq!(fs::metadata(&f.base).unwrap().nlink(), links + 1);
        Ok("y".into())
    });
    assert!(result
        .unwrap_err()
        .contains("Artifacts or destinations changed after confirmation"));
    assert_eq!(fs::read(service).unwrap(), source);
    assert_eq!(f.destinations(), before);
    println!("Confirmed controlled ancestor nlink drift: {links} -> {}; publication rejected before destination writes", links + 1);
}
#[test]
fn fixture_creation_and_cleanup_interleave_with_artifact_consent_without_changing_ancestors() {
    let f = Fixture::new("interleave-source");
    let mut publisher = smith_installer::artifact::Installer::with_roots(
        None,
        Some(f.package.join("payload/service/smith-service")),
        None,
        f.data.clone(),
    )
    .unwrap();
    let shared = f.base.parent().unwrap();
    let links = fs::metadata(shared).unwrap().nlink();
    let result = publisher.run(false, false, &mut |_: &str| {
        // A second thread must finish constructing AND dropping a fixture while
        // the first fixture is live and awaiting consent. A lifetime mutex would
        // block this interleaving; stable, distinct leaf roots do not.
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let other = Fixture::new("interleave-peer");
                    assert_eq!(other.base.parent(), Some(shared));
                    drop(other);
                })
                .join()
                .unwrap();
        });
        assert_eq!(fs::metadata(shared).unwrap().nlink(), links);
        Ok("y".into())
    });
    assert_eq!(result, Ok(smith_installer::artifact::Outcome::Installed));
    assert!(f.data.join("smith-next/bin/smith-service").is_file());
    assert!(!f.data.join("smith-next/state.json").exists());
}
#[test]
fn native_elf_help_version_are_finite_and_operationally_inert_even_with_no_environment() {
    let f = Fixture::new("help");
    let header = fs::read(&f.executable).unwrap();
    assert_eq!(&header[..7], b"\x7fELF\x02\x01\x01");
    assert_eq!(u16::from_le_bytes([header[18], header[19]]), 62);
    let before = f.destinations();
    for flag in ["--help", "--version"] {
        let mut command = f.command();
        command.env_clear().arg(flag);
        let output = wait(command.spawn().unwrap());
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("smith-installer"));
        assert_eq!(f.destinations(), before);
    }
}
#[test]
fn executed_cli_distinguishes_argument_semantic_initial_decline_and_eof_exits() {
    let f = Fixture::new("exit-codes");
    let before = f.destinations();
    for args in [
        vec!["--yes"],
        vec!["--pull-model"],
        vec!["--artifacts-only"],
        vec![
            "--artifacts-only",
            "--service=/missing",
            "--model=tiny:exact",
        ],
    ] {
        assert_eq!(f.run(&args, None).status.code(), Some(2));
    }
    assert_eq!(f.run(&["--model=tiny"], None).status.code(), Some(1));
    for input in [b"n\n".as_slice(), b"Y\n", b"yes\n", b"\n"] {
        assert_eq!(f.run(&[], Some(input)).status.code(), Some(0));
    }
    let eof = f.run(&[], None);
    assert_eq!(eof.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&eof.stderr).contains("ended before newline"));
    assert_eq!(f.destinations(), before);
}
#[test]
fn accepted_artifacts_then_missing_runtime_roots_report_partial_effects_without_manager() {
    let f = Fixture::new("partial");
    // No XDG_RUNTIME_DIR/config roots exist. Root validation fails before a
    // production manager can be constructed or contacted, after temp publication.
    let output = f.run(&[], Some(b" y \n"));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        f.data.join("smith-next/bin/smith-service").is_file(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("artifacts remain installed"));
    assert!(text.contains("not rolled back"));
    assert!(!text.contains("Prepared native effects"));
    assert!(!f.data.join("smith-next/state.json").exists());
    assert!(!f.data.join("systemd").exists());
}
#[test]
fn recipient_dry_run_and_decline_need_no_runtime_roots_and_never_write() {
    let f = Fixture::new("read-only");
    let before = f.destinations();
    let output = f.run(
        &[
            "--dry-run",
            "--model=tiny:exact",
            "--pull-model",
            "--verify-inference",
        ],
        None,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = f.run(
        &["--model=tiny:exact", "--pull-model", "--verify-inference"],
        Some(b"n\n"),
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Prepared native effects"));
    assert_eq!(f.destinations(), before);
}
#[test]
fn default_package_root_is_real_executable_parent_not_cwd_argv0_or_symlink_alias() {
    let f = Fixture::new("exe-alias");
    let alias = f.elsewhere.join("alias");
    symlink(&f.executable, &alias).unwrap();
    assert_eq!(cli::executable_parent(&alias).unwrap(), f.package);
    let mut command = Command::new(&alias);
    command
        .arg0("/irrelevant/argv0/smith-installer")
        .current_dir(&f.elsewhere)
        .env_clear()
        .env("HOME", &f.home)
        .env("XDG_DATA_HOME", &f.data)
        .arg("--dry-run")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let before = f.destinations();
    let output = wait(command.spawn().unwrap());
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        &f.package
            .join("payload/client")
            .to_string_lossy()
            .to_string()
    ));
    assert_eq!(f.destinations(), before);
}
#[test]
fn explicit_bundle_overrides_default_and_relative_explicit_inputs_are_supported() {
    let f = Fixture::new("bundle");
    let output = f.run(&["--bundle", "../package", "--dry-run"], None);
    // The publisher intentionally rejects '..' rather than silently normalizing
    // a path that would erase evidence of unsafe traversal.
    assert_eq!(output.status.code(), Some(1));
    let output = f.run(
        &["--bundle", f.package.to_str().unwrap(), "--dry-run"],
        None,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut command = f.command();
    command
        .current_dir(&f.base)
        .args(["--bundle", "package", "--dry-run"]);
    let output = wait(command.spawn().unwrap());
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn artifact_only_service_yes_uses_only_existing_data_root_and_never_registers_or_starts() {
    let f = Fixture::new("service-only");
    let mut command = f.command();
    command
        .env_remove("HOME")
        .args(["--artifacts-only", "--yes", "--service"])
        .arg(f.package.join("payload/service/smith-service"));
    let output = wait(command.spawn().unwrap());
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(f.data.join("smith-next/bin/smith-service")).unwrap(),
        fs::read(f.package.join("payload/service/smith-service")).unwrap()
    );
    assert!(!f.data.join("smith-next/state.json").exists());
    assert!(!f.data.join("systemd").exists());
    assert!(fs::read_dir(&f.home).unwrap().next().is_none());
}
#[test]
fn artifact_only_client_and_service_flags_decline_and_dryrun_are_read_only() {
    let f = Fixture::new("artifact-read-only");
    let before = f.destinations();
    for (dry, input) in [(true, None), (false, Some(b"n\n".as_slice()))] {
        let mut command = f.command();
        command
            .arg("--artifacts-only")
            .arg("--client")
            .arg(f.package.join("payload/client"))
            .arg("--service")
            .arg(f.package.join("payload/service/smith-service"));
        if dry {
            command.arg("--dry-run");
        }
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().unwrap();
        if let Some(input) = input {
            child.stdin.take().unwrap().write_all(input).unwrap();
        }
        let output = wait(child);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(f.destinations(), before);
    }
}
#[test]
fn root_or_unsupported_recipient_preflight_cannot_be_bypassed_by_dry_run() {
    // Preflight's injected pure fact seam covers unsupported/root facts without
    // sudo, changing the machine OS, or adding cross-platform validation.
    assert!(smith_installer::setup::supported_host(
        "linux",
        "x86_64",
        0,
        "ID=ubuntu\nVERSION_ID=24.04"
    )
    .is_err());
    assert!(smith_installer::setup::supported_host(
        "linux",
        "x86_64",
        1000,
        "ID=ubuntu\nVERSION_ID=22.04"
    )
    .is_err());
}
