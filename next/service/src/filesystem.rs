use crate::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

pub const MAX_FILE: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roots {
    pub data: PathBuf,
    pub config: PathBuf,
    pub runtime: PathBuf,
}

pub fn uid() -> u32 {
    // SAFETY: geteuid has no arguments or memory access.
    unsafe { libc::geteuid() }
}

impl Roots {
    pub fn environment() -> Result<Self> {
        let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
        let home = PathBuf::from(home);
        let root = Self {
            data: std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share")),
            config: std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config")),
            runtime: std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .ok_or("XDG_RUNTIME_DIR is required")?,
        };
        root.validate()?;
        Ok(root)
    }
    pub fn binary(&self) -> PathBuf {
        self.data.join("smith-next/bin/smith-service")
    }
    pub fn unit(&self) -> PathBuf {
        self.config.join("systemd/user/smith-next.service")
    }
    pub fn enablement_link(&self) -> PathBuf {
        self.config
            .join("systemd/user/default.target.wants/smith-next.service")
    }
    pub fn state(&self) -> PathBuf {
        self.data.join("smith-next/state.json")
    }
    pub fn app_socket(&self) -> PathBuf {
        self.runtime.join("smith-next/app.sock")
    }
    pub fn socket(&self) -> PathBuf {
        self.runtime.join("smith-next/service.sock")
    }
    pub fn validate(&self) -> Result<()> {
        for (root, private) in [
            (&self.data, false),
            (&self.config, false),
            (&self.runtime, true),
        ] {
            check_directory(root, private)?;
        }
        for path in [
            self.binary(),
            self.unit(),
            self.state(),
            self.socket(),
            self.app_socket(),
            self.data.join("smith-next/app-state.json"),
            self.data.join("smith-next/models"),
            self.runtime.join("smith-next/runner.lock"),
        ] {
            check_ancestors(&path)?;
        }
        for path in [
            self.data.join("smith-next"),
            self.data.join("smith-next/bin"),
            self.data.join("smith-next/models"),
            self.runtime.join("smith-next"),
        ] {
            if path.try_exists().map_err(|_| "Cannot inspect namespace")? {
                check_directory(&path, true)?;
            }
        }
        Ok(())
    }
    pub fn legacy_static_unit_text(&self) -> String {
        // Restricted path alphabet below excludes systemd specifiers/quoting ambiguities.
        format!("[Unit]\nDescription=Smith next debug service\n\n[Service]\nType=simple\nExecStart={} serve --data-root {} --config-root {} --runtime-root {}\nRestart=no\nUMask=0077\nTimeoutStopSec=5\n", self.binary().display(), self.data.display(), self.config.display(), self.runtime.display())
    }
    pub fn unit_text(&self) -> String {
        format!(
            "{}\n[Install]\nWantedBy=default.target\n",
            self.legacy_static_unit_text()
        )
    }
}

pub fn check_ancestors(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || !path.to_str().is_some_and(|s| {
            (s == "/"
                || s.split('/')
                    .skip(1)
                    .all(|part| !part.is_empty() && part != "." && part != ".."))
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
        })
    {
        return Err(
            "Roots must be absolute normalized paths using letters, digits, /, _, -, or .".into(),
        );
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err("Non-normalized path rejected".into());
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err("Symlink rejected".into());
                }
                if current != path && !meta.is_dir() {
                    return Err("Non-directory ancestor".into());
                }
                if meta.is_dir() {
                    // Root-owned sticky /tmp permits isolated tests, never a private root.
                    let sticky_root = meta.uid() == 0 && meta.mode() & libc::S_ISVTX != 0;
                    if (meta.uid() != uid() && meta.uid() != 0)
                        || (meta.mode() & 0o022 != 0 && !sticky_root)
                    {
                        return Err("Unsafe ancestor ownership or permissions".into());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err("Cannot inspect path".into()),
        }
    }
    Ok(())
}

pub fn check_directory(path: &Path, private: bool) -> Result<()> {
    check_ancestors(path)?;
    let m = fs::symlink_metadata(path).map_err(|_| "Root directory missing")?;
    if !m.is_dir()
        || m.uid() != uid()
        || m.mode() & 0o022 != 0
        || (private && m.mode() & 0o7777 != 0o700)
    {
        return Err("Unsafe root directory ownership, type or permissions".into());
    }
    Ok(())
}

pub fn mkdir(path: &Path) -> Result<()> {
    check_ancestors(path)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            mkdir(parent)?;
        }
    }
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Cannot create private directory".into()),
    }
    check_directory(path, false)
}

pub fn read_file(path: &Path, mode: u32, limit: u64) -> Result<Option<Vec<u8>>> {
    check_ancestors(path)?;
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Cannot safely open file".into()),
    };
    let m = file.metadata().map_err(|_| "Cannot inspect file")?;
    if !m.is_file()
        || m.uid() != uid()
        || m.mode() & 0o7777 != mode
        || m.nlink() != 1
        || m.len() > limit
    {
        return Err("Unsafe file type, ownership, permissions, links or size".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read file")?;
    if bytes.len() as u64 > limit {
        return Err("File exceeds limit".into());
    }
    Ok(Some(bytes))
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Same-directory synced publication. No replacement except the validated state file.
pub fn publish(path: &Path, bytes: &[u8], mode: u32, replace: bool) -> Result<()> {
    check_ancestors(path)?;
    let parent = path.parent().ok_or("Missing parent")?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid publication filename")?;
    // App and Core state can publish concurrently in the same managed PID.
    let temp = parent.join(format!(".smith-next-{name}-{}-tmp", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temp)
        .map_err(|_| "Cannot create staging file")?;
    let result = (|| {
        f.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|_| "Cannot secure staging file")?;
        f.write_all(bytes)
            .map_err(|_| "Cannot write staging file")?;
        f.sync_all().map_err(|_| "Cannot sync staging file")?;
        if replace {
            fs::rename(&temp, path).map_err(|_| "Cannot replace state")?;
        } else {
            fs::hard_link(&temp, path).map_err(|_| "Destination exists or cannot publish")?;
            fs::remove_file(&temp).map_err(|_| "Cannot remove staging link")?;
        }
        File::open(parent)
            .and_then(|d| d.sync_all())
            .map_err(|_| "Cannot sync directory")?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    setup_complete: bool,
}

pub fn state(roots: &Roots) -> Result<bool> {
    match read_file(&roots.state(), 0o600, 4096)? {
        None => Ok(false),
        Some(bytes) => serde_json::from_slice::<State>(&bytes)
            .map(|s| s.setup_complete)
            .map_err(|_| "State is corrupt; preserved without reset".into()),
    }
}

pub fn complete(roots: &Roots) -> Result<()> {
    state(roots)?;
    publish(&roots.state(), b"{\"setup_complete\":true}\n", 0o600, true)
}

pub struct Lock(File);
impl Lock {
    pub fn acquire(path: &Path) -> Result<Self> {
        check_directory(path, false)?;
        let f = File::open(path).map_err(|_| "Cannot open namespace lock")?;
        // SAFETY: flock operates on our open directory descriptor.
        if unsafe {
            libc::flock(
                std::os::fd::AsRawFd::as_raw_fd(&f),
                libc::LOCK_EX | libc::LOCK_NB,
            )
        } != 0
        {
            return Err("Another operation owns this namespace".into());
        }
        Ok(Self(f))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        // SAFETY: our descriptor remains alive until after Drop.
        unsafe {
            libc::flock(std::os::fd::AsRawFd::as_raw_fd(&self.0), libc::LOCK_UN);
        }
    }
}
