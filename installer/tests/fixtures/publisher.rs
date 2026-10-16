//! Non-root, disposable publisher fixtures. No artifact, manager or provider is executed.
// Shared by library unit tests and the public integration harness.
#![allow(dead_code)]

use crate::artifact::{Installer, Outcome};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
// The reference snapshot includes ancestor directory link counts. Parallel
// fixture mkdir/rmdir would intentionally invalidate each other's consent.
static FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct Fixture {
    pub root: PathBuf,
    pub home: PathBuf,
    pub data: PathBuf,
    pub bundle: PathBuf,
    pub service: PathBuf,
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Content {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
    Special,
}

pub type Snapshot = BTreeMap<PathBuf, (u32, u64, u64, Content)>;

impl Fixture {
    pub fn new() -> Self {
        let guard = FIXTURE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // A skipped privileged run would conceal a required safety gate.
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "Run publisher tests as a non-root user"
        );
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "smith-publisher-{}-{tick}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        chmod(&root, 0o700);
        let home = root.join("home");
        let data = root.join("data");
        let bundle = root.join("bundle");
        for path in [&home, &data, &bundle] {
            fs::create_dir(path).unwrap();
            chmod(path, 0o700);
        }
        let service = root.join("smith-service");
        write(&service, b"service-v1", 0o755);
        for name in [
            "lib",
            "data/flutter_assets",
            "smith_backend",
            "share/pixmaps",
            "share/applications",
        ] {
            fs::create_dir_all(bundle.join(name)).unwrap();
        }
        for (name, content, mode) in [
            ("smith_client", b"client-v1".as_slice(), 0o755),
            ("smith_backend/smith-service", b"service-v1", 0o755),
            ("lib/libflutter_linux_gtk.so", b"flutter", 0o644),
            ("lib/libapp.so", b"app", 0o644),
            ("data/icudtl.dat", b"icu", 0o644),
            ("data/flutter_assets/asset", b"asset", 0o644),
            ("share/pixmaps/dev.smith.smith_client.png", b"icon", 0o644),
            (
                "share/applications/dev.smith.smith_client.desktop",
                b"desktop",
                0o644,
            ),
        ] {
            write(&bundle.join(name), content, mode);
        }
        fn directory_modes(path: &Path) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if fs::symlink_metadata(&path).unwrap().is_dir() {
                    chmod(&path, 0o755);
                    directory_modes(&path);
                }
            }
        }
        directory_modes(&bundle);
        Self {
            root,
            home,
            data,
            bundle,
            service,
            _guard: guard,
        }
    }

    pub fn installer(&self, client: bool, service: bool) -> Installer {
        Installer::with_roots(
            client.then(|| self.bundle.clone()),
            service.then(|| self.service.clone()),
            Some(self.home.clone()),
            self.data.clone(),
        )
        .unwrap()
    }

    pub fn install(&self, client: bool, service: bool) -> crate::Result<Outcome> {
        self.installer(client, service)
            .run(false, true, &mut |_| panic!("unexpected consent"))
    }

    pub fn current(&self) -> PathBuf {
        self.data.join("smith-next/client/current")
    }

    pub fn binary(&self) -> PathBuf {
        self.data.join("smith-next/bin/smith-service")
    }

    pub fn private_parents(&self, path: &Path) {
        fs::create_dir_all(path).unwrap();
        let mut current = path;
        while current != self.home && current != self.data && current != self.root {
            chmod(current, 0o700);
            current = current.parent().unwrap();
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        snapshot(&self.root)
    }

    pub fn named(&self, parent: &Path, prefix: &str) -> Vec<PathBuf> {
        let mut paths: Vec<_> = fs::read_dir(parent)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(prefix)
            })
            .collect();
        paths.sort();
        paths
    }

    pub fn cli(&self, executable: &Path) -> Command {
        let mut command = Command::new(executable);
        command
            .current_dir(&self.root)
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_DATA_HOME", &self.data)
            .env("XDG_CONFIG_HOME", self.root.join("unused-config"))
            .env("XDG_RUNTIME_DIR", self.root.join("unused-runtime"))
            .stdin(Stdio::null());
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the uniquely created test root is removed; links are not traversed.
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

pub fn write(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    chmod(path, mode);
}

pub fn snapshot(root: &Path) -> Snapshot {
    fn visit(root: &Path, path: &Path, entries: &mut Snapshot) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let content = if metadata.file_type().is_symlink() {
                Content::Link(fs::read_link(&path).unwrap())
            } else if metadata.is_file() {
                Content::File(fs::read(&path).unwrap())
            } else if metadata.is_dir() {
                Content::Directory
            } else {
                Content::Special
            };
            entries.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                (
                    metadata.mode() & 0o7777,
                    metadata.ino(),
                    metadata.nlink(),
                    content,
                ),
            );
            if metadata.is_dir() {
                visit(root, &path, entries);
            }
        }
    }
    let mut entries = Snapshot::new();
    visit(root, root, &mut entries);
    entries
}

pub fn bounded_output(mut command: Command, budget: Duration) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + budget;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            let _ = out.join();
            let _ = err.join();
            panic!("isolated publisher child exceeded {budget:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}
