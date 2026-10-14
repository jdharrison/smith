//! Linux descriptor-relative publication. Paths are never canonicalized through links.

use crate::manifest::{digest, Entries, Entry, MANIFEST};
use crate::Result;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub(crate) const MAX_FILE: usize = 128 * 1024 * 1024;
pub(crate) const MAX_TOTAL: usize = 512 * 1024 * 1024;
pub(crate) const MAX_ENTRIES: usize = 10_000;
pub(crate) const MAX_DEPTH: usize = 32;
const DIR_FLAGS: i32 = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
const FILE_FLAGS: i32 = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;

pub(crate) type Effects = Rc<RefCell<Vec<String>>>;

// PathBuf equality ignores trailing separators, unlike Python's string trace
// keys. Separate namespaces prevent later ancestor traversal from discarding a
// tree directory's full size/mtime/ctime consent binding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Trace {
    paths: BTreeMap<PathBuf, Fact>,
    tree_directories: BTreeMap<PathBuf, Identity>,
}

impl Trace {
    fn new() -> Self {
        Self::default()
    }

    /// Account only for our known fresh child directory, preserving all other facts.
    pub(crate) fn directory_added(&mut self, parent: &Path) -> Result<()> {
        match self.paths.get_mut(parent) {
            Some(Fact::Directory(identity)) => {
                identity.links = identity
                    .links
                    .checked_add(1)
                    .ok_or("Directory link count overflow")?;
                Ok(())
            }
            _ => Err("Reviewed backup parent identity missing".into()),
        }
    }

    pub(crate) fn get(&self, path: &Path) -> Option<&Fact> {
        self.paths.get(path)
    }

    pub(crate) fn insert(&mut self, path: PathBuf, fact: Fact) {
        self.paths.insert(path, fact);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    links: u64,
    size: i64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    links: u64,
}

impl Identity {
    fn of(s: &libc::stat) -> Self {
        Self {
            dev: s.st_dev,
            ino: s.st_ino,
            uid: s.st_uid,
            mode: s.st_mode,
            links: s.st_nlink,
            size: s.st_size,
            mtime: (s.st_mtime, s.st_mtime_nsec),
            ctime: (s.st_ctime, s.st_ctime_nsec),
        }
    }

    fn directory(&self) -> DirectoryIdentity {
        DirectoryIdentity {
            dev: self.dev,
            ino: self.ino,
            uid: self.uid,
            mode: self.mode,
            links: self.links,
        }
    }

    pub(crate) fn same_moved(&self, other: &Self) -> bool {
        self.directory() == other.directory()
            && self.size == other.size
            && self.mtime == other.mtime
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Selector {
    identity: Identity,
    pub(crate) target: String,
}

impl Selector {
    pub(crate) fn same_moved(&self, other: &Self) -> bool {
        // rename changes ctime; every other captured fact and the target must match.
        self.identity.same_moved(&other.identity) && self.target == other.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Fact {
    Missing,
    Directory(DirectoryIdentity),
    TreeEntry(Identity),
    File(Identity, String),
    Selector(Selector),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileData {
    pub(crate) bytes: Vec<u8>,
    pub(crate) mode: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Tree {
    pub(crate) entries: Entries,
    pub(crate) contents: BTreeMap<String, Vec<u8>>,
}

pub(crate) fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

pub(crate) fn path_parts(path: &Path) -> Result<Vec<&str>> {
    let bytes = path.as_os_str().as_bytes();
    require(
        bytes.first() == Some(&b'/')
            && bytes.len() > 1
            && bytes
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'/' | b'-')),
        "Paths must be restricted absolute paths",
    )?;
    // Validate raw spelling: Path::components would silently normalize repeated '/' and '.'.
    let parts: Vec<_> = std::str::from_utf8(&bytes[1..])
        .map_err(|_| "Paths must be ASCII".to_string())?
        .split('/')
        .collect();
    require(
        parts
            .iter()
            .all(|p| !p.is_empty() && *p != "." && *p != ".."),
        "Paths must be normalized without empty, dot or parent components",
    )?;
    Ok(parts)
}

fn c_name(name: &str) -> CString {
    CString::new(name).expect("validated filesystem names have no NUL")
}

fn io_error(operation: &str, path: &Path, error: io::Error) -> String {
    format!("{operation}: {}: {error}", path.display())
}

pub(crate) fn fstat(fd: RawFd) -> Result<libc::stat> {
    let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fd is held by the caller and s provides correctly sized output storage.
    if unsafe { libc::fstat(fd, s.as_mut_ptr()) } != 0 {
        return Err(format!("fstat: {}", io::Error::last_os_error()));
    }
    // SAFETY: successful fstat initialized s.
    Ok(unsafe { s.assume_init() })
}

fn stat_at(fd: RawFd, name: &str) -> io::Result<libc::stat> {
    let name = c_name(name);
    let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: name is NUL-terminated and s is valid output storage; no link is followed.
    if unsafe { libc::fstatat(fd, name.as_ptr(), s.as_mut_ptr(), libc::AT_SYMLINK_NOFOLLOW) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful fstatat initialized s.
    Ok(unsafe { s.assume_init() })
}

pub(crate) fn open_at(fd: RawFd, name: &str, flags: i32, mode: u32) -> io::Result<OwnedFd> {
    let name = c_name(name);
    // SAFETY: the parent is held, name is NUL-terminated, and mode is supplied for O_CREAT.
    let child = unsafe { libc::openat(fd, name.as_ptr(), flags, mode as libc::mode_t) };
    if child < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: this newly opened descriptor has one owner.
    Ok(unsafe { OwnedFd::from_raw_fd(child) })
}

fn safe_dir(s: &libc::stat, owned: bool, private: bool) -> Result<()> {
    let sticky = s.st_uid == 0 && s.st_mode & libc::S_ISVTX != 0;
    require(
        s.st_mode & libc::S_IFMT == libc::S_IFDIR
            && (s.st_uid == 0 || s.st_uid == effective_uid())
            && (s.st_mode & 0o022 == 0 || sticky),
        "Unsafe directory ancestor",
    )?;
    if owned {
        require(
            s.st_uid == effective_uid() && s.st_mode & 0o022 == 0,
            "Directory must be user-owned and not group/world writable",
        )?;
    }
    if private {
        require(
            s.st_mode & 0o7777 == 0o700,
            "Namespace must be exactly 0700",
        )?;
    }
    Ok(())
}

fn safe_file(s: &libc::stat) -> bool {
    s.st_mode & libc::S_IFMT == libc::S_IFREG
        && s.st_uid == effective_uid()
        && s.st_nlink == 1
        && s.st_mode & 0o7022 == 0
        && s.st_size >= 0
        && s.st_size as u64 <= MAX_FILE as u64
}

pub(crate) fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments and no failure mode.
    unsafe { libc::geteuid() }
}

pub(crate) fn sync(fd: RawFd, path: &Path) -> Result<()> {
    #[cfg(test)]
    testing::event(testing::Event::Sync(path.to_path_buf()))?;
    // SAFETY: fd is held until this synchronous call returns.
    if unsafe { libc::fsync(fd) } != 0 {
        return Err(io_error("fsync", path, io::Error::last_os_error()));
    }
    Ok(())
}

pub(crate) fn chmod(fd: RawFd, mode: u32, path: &Path) -> Result<()> {
    crate::consent::check()?;
    // SAFETY: fd is held; the mode came from a verified source or fixed managed policy.
    if unsafe { libc::fchmod(fd, mode as libc::mode_t) } != 0 {
        return Err(io_error("fchmod", path, io::Error::last_os_error()));
    }
    Ok(())
}

pub(crate) fn mkdir_at(fd: RawFd, name: &str, path: &Path) -> Result<()> {
    crate::consent::check()?;
    let name = c_name(name);
    // SAFETY: fd is held and name is NUL-terminated. No implicit overwrite is permitted.
    if unsafe { libc::mkdirat(fd, name.as_ptr(), 0o700) } != 0 {
        return Err(io_error("mkdirat", path, io::Error::last_os_error()));
    }
    Ok(())
}

pub(crate) fn stage_name(prefix: &str) -> Result<String> {
    let mut bytes = [0_u8; 12];
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: the remaining slice is writable for exactly the supplied length.
        let count = unsafe {
            libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
        };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("getrandom: {error}"));
        }
        require(count > 0, "getrandom returned no bytes")?;
        offset += count as usize;
    }
    use std::fmt::Write as _;
    let mut name = prefix.to_string();
    for byte in bytes {
        write!(name, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(name)
}

pub(crate) fn rename_at(
    src_fd: RawFd,
    src: &str,
    dst_fd: RawFd,
    dst: &str,
    flags: u32,
    path: &Path,
) -> Result<()> {
    crate::consent::check()?;
    #[cfg(test)]
    testing::event(testing::Event::BeforeRename {
        path: path.to_path_buf(),
        source: src.to_string(),
        flags,
    })?;
    let src = c_name(src);
    let dst = c_name(dst);
    // SAFETY: both directory descriptors are held, both names are valid C strings.
    // Linux renameat2 is required; there is deliberately no weaker fallback.
    if unsafe { libc::renameat2(src_fd, src.as_ptr(), dst_fd, dst.as_ptr(), flags) } != 0 {
        return Err(io_error("renameat2", path, io::Error::last_os_error()));
    }
    Ok(())
}

pub(crate) fn symlink_at(fd: RawFd, target: &str, name: &str, path: &Path) -> Result<()> {
    crate::consent::check()?;
    let target = c_name(target);
    let name = c_name(name);
    // SAFETY: fd is held and both strings are NUL-terminated.
    if unsafe { libc::symlinkat(target.as_ptr(), fd, name.as_ptr()) } != 0 {
        return Err(io_error("symlinkat", path, io::Error::last_os_error()));
    }
    Ok(())
}

pub(crate) fn selector_at(fd: RawFd, name: &str) -> Result<Option<Selector>> {
    let before = match stat_at(fd, name) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("stat selector {name}: {e}")),
    };
    require(
        before.st_mode & libc::S_IFMT == libc::S_IFLNK
            && before.st_uid == effective_uid()
            && before.st_nlink == 1,
        format!("Unknown selector object: {name}"),
    )?;
    let mut target = [0_u8; 4096];
    let c_name = c_name(name);
    // SAFETY: target is valid output storage and readlinkat writes at most its capacity.
    let size = unsafe {
        libc::readlinkat(
            fd,
            c_name.as_ptr(),
            target.as_mut_ptr().cast(),
            target.len(),
        )
    };
    if size < 0 {
        return Err(format!("readlinkat {name}: {}", io::Error::last_os_error()));
    }
    require(
        (size as usize) < target.len(),
        "Selector target exceeds limit",
    )?;
    let target = std::str::from_utf8(&target[..size as usize])
        .map_err(|_| "Selector target must be ASCII".to_string())?
        .to_string();
    let identity = Identity::of(&before);
    let after = stat_at(fd, name).map_err(|e| format!("stat selector {name}: {e}"))?;
    require(
        identity == Identity::of(&after),
        format!("Selector changed while reading: {name}"),
    )?;
    Ok(Some(Selector { identity, target }))
}

pub(crate) fn verify_lock(fd: RawFd) -> Result<Identity> {
    let s = fstat(fd)?;
    require(
        s.st_mode & libc::S_IFMT == libc::S_IFREG
            && s.st_uid == effective_uid()
            && s.st_nlink == 1
            && s.st_mode & 0o7777 == 0o600,
        "Unsafe installation lock",
    )?;
    Ok(Identity::of(&s))
}

pub(crate) fn verify_named_identity(fd: RawFd, name: &str, expected: &Identity) -> Result<()> {
    let actual = stat_at(fd, name).map_err(|e| format!("stat {name}: {e}"))?;
    require(
        *expected == Identity::of(&actual),
        format!("Object replaced: {name}"),
    )
}

pub(crate) struct Filesystem {
    pub(crate) trace: Trace,
    effects: Effects,
}

impl Filesystem {
    pub(crate) fn new(effects: Effects) -> Self {
        Self {
            trace: Trace::new(),
            effects,
        }
    }

    pub(crate) fn effect(&self, effect: impl Into<String>) {
        self.effects.borrow_mut().push(effect.into());
    }

    pub(crate) fn directory(
        &mut self,
        path: &Path,
        create: bool,
        private: bool,
    ) -> Result<Option<OwnedFd>> {
        crate::consent::check()?;
        let parts = path_parts(path)?;
        let mut fd = open_at(libc::AT_FDCWD, "/", DIR_FLAGS, 0)
            .map_err(|e| io_error("open root", path, e))?;
        let mut current = PathBuf::from("/");
        for (i, part) in parts.iter().enumerate() {
            current.push(part);
            #[cfg(test)]
            testing::event(testing::Event::BeforeOpen(current.clone()))?;
            let child = match open_at(fd.as_raw_fd(), part, DIR_FLAGS, 0) {
                Ok(child) => child,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if !create {
                        self.trace.insert(current, Fact::Missing);
                        return Ok(None);
                    }
                    mkdir_at(fd.as_raw_fd(), part, &current)?;
                    self.effect(format!("created directory {}", current.display()));
                    sync(fd.as_raw_fd(), current.parent().expect("absolute child"))?;
                    open_at(fd.as_raw_fd(), part, DIR_FLAGS, 0)
                        .map_err(|e| io_error("open created directory", &current, e))?
                }
                Err(e) => return Err(io_error("open directory (no-follow)", &current, e)),
            };
            fd = child;
            let s = fstat(fd.as_raw_fd())?;
            safe_dir(&s, i == parts.len() - 1, private && i == parts.len() - 1).map_err(|e| {
                format!(
                    "{e}: {} (mode={:04o}, uid={})",
                    current.display(),
                    s.st_mode & 0o7777,
                    s.st_uid
                )
            })?;
            self.trace.insert(
                current.clone(),
                Fact::Directory(Identity::of(&s).directory()),
            );
        }
        Ok(Some(fd))
    }

    fn parent(&mut self, path: &Path, create: bool) -> Result<(Option<OwnedFd>, String)> {
        let parts = path_parts(path)?;
        let name = parts.last().expect("validated nonempty path").to_string();
        let parent = path.parent().expect("absolute child");
        Ok((self.directory(parent, create, false)?, name))
    }

    fn read_at(&self, fd: RawFd, name: &str, mode: Option<u32>) -> Result<(FileData, Identity)> {
        let file =
            open_at(fd, name, FILE_FLAGS, 0).map_err(|e| format!("open file {name}: {e}"))?;
        let mut file = File::from(file);
        let before = fstat(file.as_raw_fd())?;
        require(safe_file(&before), format!("Unsafe regular file: {name}"))?;
        let actual_mode = before.st_mode & 0o7777;
        if let Some(mode) = mode {
            require(actual_mode == mode, format!("Incorrect file mode: {name}"))?;
        }
        #[cfg(test)]
        testing::event(testing::Event::BeforeRead(name.to_string()))?;
        let mut bytes = Vec::new();
        let mut chunk = vec![0_u8; 1024 * 1024];
        loop {
            crate::consent::check()?;
            let size = file
                .read(&mut chunk)
                .map_err(|e| format!("read {name}: {e}"))?;
            if size == 0 {
                break;
            }
            require(bytes.len() + size <= MAX_FILE, "File exceeds limit")?;
            bytes.extend_from_slice(&chunk[..size]);
        }
        let identity = Identity::of(&before);
        require(
            identity == Identity::of(&fstat(file.as_raw_fd())?),
            "File changed while reading",
        )?;
        verify_named_identity(fd, name, &identity)?;
        Ok((
            FileData {
                bytes,
                mode: actual_mode,
            },
            identity,
        ))
    }

    /// Retain an exact reviewed object; a late conflict is preserved, never rolled back.
    pub(crate) fn preserve(&mut self, path: &Path, expected: &Fact) -> Result<PathBuf> {
        crate::consent::check()?;
        let (parent, name) = self.parent(path, false)?;
        let parent = parent.ok_or("Preservation parent missing")?;
        let stage = stage_name(".smith-next-maintained-")?;
        let retained = path.parent().ok_or("Missing parent")?.join(&stage);
        rename_at(
            parent.as_raw_fd(),
            &name,
            parent.as_raw_fd(),
            &stage,
            libc::RENAME_NOREPLACE,
            path,
        )?;
        self.effect(format!(
            "moved {} to {}; verification pending, no rollback",
            path.display(),
            retained.display()
        ));
        sync(parent.as_raw_fd(), path.parent().ok_or("Missing parent")?)?;
        self.verify_moved(&retained, expected)?;
        Ok(retained)
    }

    pub(crate) fn verify_moved(&mut self, path: &Path, expected: &Fact) -> Result<()> {
        match expected {
            Fact::File(identity, sha) => {
                self.file(path, Some(identity.mode & 0o777))?
                    .ok_or("Retained file missing")?;
                require(
                    matches!(self.trace.get(path), Some(Fact::File(actual, digest)) if actual.same_moved(identity) && digest == sha),
                    "Retained file identity conflict; no rollback",
                )
            }
            Fact::Selector(selector) => {
                let (parent, name) = self.parent(path, false)?;
                let parent = parent.ok_or("Retained selector parent missing")?;
                require(
                    selector_at(parent.as_raw_fd(), &name)?
                        .is_some_and(|actual| actual.same_moved(selector)),
                    "Retained selector identity conflict; no rollback",
                )
            }
            _ => Err("Only reviewed regular files/selectors can be preserved".into()),
        }
    }

    pub(crate) fn exchange_binary(
        &mut self,
        path: &Path,
        bytes: &[u8],
        expected: &Fact,
    ) -> Result<PathBuf> {
        self.exchange_file(path, bytes, 0o755, expected, ".smith-next-upgrade-")
    }

    pub(crate) fn exchange_file(
        &mut self,
        path: &Path,
        bytes: &[u8],
        mode: u32,
        expected: &Fact,
        prefix: &str,
    ) -> Result<PathBuf> {
        let (parent, name) = self.parent(path, false)?;
        let parent = parent.ok_or("File parent missing")?;
        let stage = stage_name(prefix)?;
        let retained = path.parent().ok_or("Missing parent")?.join(&stage);
        self.publish(&retained, bytes, mode)?;
        self.file(&retained, Some(mode))?
            .ok_or("Staged file missing")?;
        let staged = self
            .trace
            .get(&retained)
            .ok_or("Staged identity missing")?
            .clone();
        crate::consent::check()?;
        rename_at(
            parent.as_raw_fd(),
            &stage,
            parent.as_raw_fd(),
            &name,
            libc::RENAME_EXCHANGE,
            path,
        )?;
        self.effect(format!(
            "exchanged file; old object retained at {}; verification pending, no rollback",
            retained.display()
        ));
        sync(parent.as_raw_fd(), path.parent().ok_or("Missing parent")?)?;
        self.verify_moved(&retained, expected)?;
        self.verify_moved(path, &staged)?;
        Ok(retained)
    }

    pub(crate) fn file(&mut self, path: &Path, mode: Option<u32>) -> Result<Option<FileData>> {
        let (fd, name) = self.parent(path, false)?;
        let Some(fd) = fd else {
            self.trace.insert(path.to_path_buf(), Fact::Missing);
            return Ok(None);
        };
        match stat_at(fd.as_raw_fd(), &name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.trace.insert(path.to_path_buf(), Fact::Missing);
                return Ok(None);
            }
            Err(e) => return Err(io_error("stat file", path, e)),
            Ok(_) => (),
        }
        let (data, identity) = self.read_at(fd.as_raw_fd(), &name, mode)?;
        self.trace.insert(
            path.to_path_buf(),
            Fact::File(identity, digest(&data.bytes)),
        );
        Ok(Some(data))
    }

    pub(crate) fn tree(&mut self, path: &Path, installed: bool) -> Result<Tree> {
        let fd = self.directory(path, false, false)?;
        let fd = fd.ok_or("Bundle/release directory missing")?;
        let mut tree = Tree {
            entries: Entries::new(),
            contents: BTreeMap::new(),
        };
        let mut total = 0;
        self.visit(
            fd.as_raw_fd(),
            path,
            "",
            0,
            installed,
            &mut tree,
            &mut total,
        )?;
        Ok(tree)
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        parent: RawFd,
        path: &Path,
        prefix: &str,
        depth: usize,
        installed: bool,
        tree: &mut Tree,
        total: &mut usize,
    ) -> Result<()> {
        require(depth <= MAX_DEPTH, "Bundle nesting exceeds limit")?;
        let before = Identity::of(&fstat(parent)?);
        for name in list_names(parent)? {
            if prefix.is_empty() && name == MANIFEST && installed {
                continue;
            }
            require(name != MANIFEST, "Reserved manifest filename in source")?;
            let rel = format!("{prefix}{name}");
            let s = stat_at(parent, &name).map_err(|e| format!("stat bundle entry {rel}: {e}"))?;
            require(
                s.st_uid == effective_uid() && s.st_mode & 0o7022 == 0,
                "Unsafe bundle ownership or mode",
            )?;
            require(
                tree.entries.len() < MAX_ENTRIES,
                "Bundle entry limit exceeded",
            )?;
            if s.st_mode & libc::S_IFMT == libc::S_IFDIR {
                tree.entries
                    .insert(rel.clone(), Entry::directory(s.st_mode & 0o7777));
                let child = open_at(parent, &name, DIR_FLAGS, 0)
                    .map_err(|e| format!("open bundle directory {rel}: {e}"))?;
                require(
                    Identity::of(&s) == Identity::of(&fstat(child.as_raw_fd())?),
                    "Bundle directory changed",
                )?;
                self.visit(
                    child.as_raw_fd(),
                    path,
                    &format!("{rel}/"),
                    depth + 1,
                    installed,
                    tree,
                    total,
                )?;
            } else {
                let (data, token) = self.read_at(parent, &name, None)?;
                require(Identity::of(&s) == token, "Bundle entry changed")?;
                add_bytes(total, data.bytes.len())?;
                tree.entries
                    .insert(rel.clone(), Entry::file(data.mode, &data.bytes));
                tree.contents.insert(rel.clone(), data.bytes);
                self.trace.insert(path.join(&rel), Fact::TreeEntry(token));
            }
        }
        require(
            before == Identity::of(&fstat(parent)?),
            "Bundle changed during traversal",
        )?;
        self.trace
            .tree_directories
            .insert(path.join(prefix.trim_end_matches('/')), before);
        Ok(())
    }

    pub(crate) fn publish(&mut self, path: &Path, data: &[u8], mode: u32) -> Result<()> {
        crate::consent::check()?;
        // Scopes own their FDs; even large bundles retain only traversal-depth descriptors.
        let mut scoped = Self::new(self.effects.clone());
        let (fd, name) = scoped.parent(path, true)?;
        let fd = fd.ok_or("Publication parent missing")?;
        #[cfg(test)]
        testing::event(testing::Event::BeforePublish(path.to_path_buf()))?;
        let stage = stage_name(".smith-next-stage-")?;
        let staged_path = path.parent().expect("absolute child").join(&stage);
        let file = open_at(
            fd.as_raw_fd(),
            &stage,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
        .map_err(|e| io_error("create file stage", &staged_path, e))?;
        self.effect(format!(
            "created file staging object {}",
            staged_path.display()
        ));
        let mut file = File::from(file);
        file.write_all(data)
            .map_err(|e| io_error("write file stage", &staged_path, e))?;
        chmod(file.as_raw_fd(), mode, &staged_path)?;
        sync(file.as_raw_fd(), &staged_path)?;
        let identity = Identity::of(&fstat(file.as_raw_fd())?);
        verify_named_identity(fd.as_raw_fd(), &stage, &identity)?;
        // NOREPLACE also works for regular files and leaves them single-link. Unlike
        // link/unlink, it needs no race-prone deletion of a potentially replaced stage.
        rename_at(
            fd.as_raw_fd(),
            &stage,
            fd.as_raw_fd(),
            &name,
            libc::RENAME_NOREPLACE,
            path,
        )?;
        self.effect(format!(
            "published {}; verification pending",
            path.display()
        ));
        #[cfg(test)]
        testing::event(testing::Event::AfterPublish(path.to_path_buf()))?;
        sync(fd.as_raw_fd(), path.parent().expect("absolute child"))?;
        let actual =
            stat_at(fd.as_raw_fd(), &name).map_err(|e| io_error("stat publication", path, e))?;
        require(
            identity.same_moved(&Identity::of(&actual)),
            "Published file identity changed",
        )?;
        require(
            identity.same_moved(&Identity::of(&fstat(file.as_raw_fd())?)),
            "Staged file changed during publication",
        )?;
        Ok(())
    }
}

fn add_bytes(total: &mut usize, count: usize) -> Result<()> {
    let next = total
        .checked_add(count)
        .ok_or("Bundle byte limit exceeded")?;
    require(next <= MAX_TOTAL, "Bundle byte limit exceeded")?;
    *total = next;
    Ok(())
}

struct DirectoryStream(*mut libc::DIR);

impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // SAFETY: fdopendir transferred a unique descriptor to this stream.
        unsafe { libc::closedir(self.0) };
    }
}

fn list_names(fd: RawFd) -> Result<Vec<String>> {
    // A separate open file description avoids changing the held directory's offset.
    let scan =
        open_at(fd, ".", DIR_FLAGS, 0).map_err(|e| format!("open directory listing: {e}"))?;
    let scan = scan.into_raw_fd();
    // SAFETY: scan is uniquely owned and transferred to fdopendir on success.
    let stream = unsafe { libc::fdopendir(scan) };
    if stream.is_null() {
        let error = io::Error::last_os_error();
        // SAFETY: fdopendir did not consume scan on failure.
        unsafe { libc::close(scan) };
        return Err(format!("fdopendir: {error}"));
    }
    let stream = DirectoryStream(stream);
    let mut names = Vec::new();
    loop {
        // SAFETY: errno is thread-local; the live stream is exclusively borrowed here.
        unsafe { *libc::__errno_location() = 0 };
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(format!("readdir: {error}"));
            }
            break;
        }
        // SAFETY: readdir supplied a terminated d_name valid until the next readdir.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        require(
            !name.is_empty()
                && name
                    .iter()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-')),
            "Unsafe bundle filename",
        )?;
        require(names.len() <= MAX_ENTRIES, "Bundle entry limit exceeded")?;
        names.push(
            std::str::from_utf8(name)
                .expect("checked ASCII")
                .to_string(),
        );
    }
    names.sort();
    Ok(names)
}

// No hooks, altered identity/limits, endpoint overrides or environment bypasses
// exist in the public library. These seams are only compiled into its unit tests.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    #[derive(Clone, Debug)]
    pub(crate) enum Event {
        BeforeOpen(PathBuf),
        BeforeRead(String),
        BeforePublish(PathBuf),
        AfterPublish(PathBuf),
        BeforeRename {
            path: PathBuf,
            source: String,
            flags: u32,
        },
        AfterRename {
            path: PathBuf,
            retained: PathBuf,
        },
        SelectorStaged(PathBuf),
        LockAcquired,
        BeforeApply,
        Sync(PathBuf),
    }
    type Hook = Box<dyn FnMut(&Event) -> Result<()>>;
    thread_local! {
        static HOOK: RefCell<Option<Hook>> = RefCell::new(None);
    }

    pub(crate) fn event(event: Event) -> Result<()> {
        HOOK.with(|slot| {
            if let Some(hook) = slot.borrow_mut().as_mut() {
                hook(&event)
            } else {
                Ok(())
            }
        })
    }

    pub(crate) fn with_hook<T>(
        hook: impl FnMut(&Event) -> Result<()> + 'static,
        body: impl FnOnce() -> T,
    ) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                HOOK.with(|slot| *slot.borrow_mut() = None);
            }
        }
        HOOK.with(|slot| {
            assert!(slot.borrow().is_none(), "test hooks cannot be nested");
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        let _reset = Reset;
        body()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::fixture::{chmod, write, Fixture};
    use std::fs as stdfs;

    fn filesystem() -> Filesystem {
        Filesystem::new(Rc::new(RefCell::new(Vec::new())))
    }

    #[test]
    fn ordinary_ancestor_traversal_preserves_independent_full_tree_directory_facts() {
        let f = Fixture::new();
        let mut fs = filesystem();
        fs.tree(&f.bundle, false).unwrap();
        let expected = fs.trace.tree_directories.clone();
        fs.file(&f.bundle.join("smith_backend/smith-service"), Some(0o755))
            .unwrap()
            .unwrap();
        assert_eq!(fs.trace.tree_directories, expected);
        for path in [&f.bundle, &f.bundle.join("smith_backend")] {
            let full = fs.trace.tree_directories.get(path).unwrap();
            assert_eq!(
                fs.trace.paths.get(path),
                Some(&Fact::Directory(full.directory()))
            );
        }
    }

    #[test]
    fn paths_require_raw_normalized_restricted_absolute_spelling() {
        for path in [
            "",
            "/",
            "relative",
            "/tmp/../root",
            "/tmp/./root",
            "/tmp//root",
            "/tmp/root/",
            "/tmp/with space",
            "/tmp/x\n",
            "/tmp/é",
            "/tmp/x;touch",
        ] {
            assert!(path_parts(Path::new(path)).is_err(), "accepted {path:?}");
        }
        assert_eq!(
            path_parts(Path::new("/tmp/smith-_09.x")).unwrap(),
            ["tmp", "smith-_09.x"]
        );
    }

    #[test]
    fn owner_modes_type_links_and_file_size_predicates_are_fail_closed() {
        let f = Fixture::new();
        let file = stdfs::File::open(&f.service).unwrap();
        let original = fstat(file.as_raw_fd()).unwrap();
        assert!(safe_file(&original));
        for change in [
            "owner", "links", "suid", "sgid", "sticky", "group", "world", "fifo", "size",
            "negative",
        ] {
            let mut s = original;
            match change {
                "owner" => s.st_uid = effective_uid() + 1,
                "links" => s.st_nlink = 2,
                "suid" => s.st_mode |= 0o4000,
                "sgid" => s.st_mode |= 0o2000,
                "sticky" => s.st_mode |= 0o1000,
                "group" => s.st_mode |= 0o020,
                "world" => s.st_mode |= 0o002,
                "fifo" => s.st_mode = libc::S_IFIFO | 0o600,
                "size" => s.st_size = MAX_FILE as i64 + 1,
                _ => s.st_size = -1,
            }
            assert!(!safe_file(&s), "accepted {change}");
        }
        let dir = filesystem()
            .directory(&f.data, false, true)
            .unwrap()
            .unwrap();
        let mut s = fstat(dir.as_raw_fd()).unwrap();
        assert!(safe_dir(&s, true, true).is_ok());
        s.st_mode = libc::S_IFDIR | 0o755;
        assert!(safe_dir(&s, true, false).is_ok());
        assert!(safe_dir(&s, true, true).is_err());
        s.st_mode = libc::S_IFDIR | 0o777;
        assert!(safe_dir(&s, true, false).is_err());
        s.st_uid = 0;
        s.st_mode = libc::S_IFDIR | 0o1777;
        assert!(safe_dir(&s, false, false).is_ok());
        assert!(safe_dir(&s, true, false).is_err());
        s.st_uid = effective_uid() + 1;
        s.st_mode = libc::S_IFDIR | 0o700;
        assert!(safe_dir(&s, false, false).is_err());
    }

    #[test]
    fn oversized_sparse_file_is_rejected_without_reading_or_writing_destinations() {
        let f = Fixture::new();
        let file = stdfs::OpenOptions::new()
            .write(true)
            .open(&f.service)
            .unwrap();
        file.set_len(MAX_FILE as u64 + 1).unwrap();
        assert!(filesystem()
            .file(&f.service, Some(0o755))
            .unwrap_err()
            .contains("Unsafe regular file"));
        assert!(!f.data.join("smith-next").exists());
        assert_eq!(MAX_FILE, 128 * 1024 * 1024);
    }

    #[test]
    fn total_bytes_bound_and_overflow_are_checked_without_large_allocations() {
        let mut total = MAX_TOTAL - 1;
        assert_eq!(add_bytes(&mut total, 1), Ok(()));
        assert_eq!(total, MAX_TOTAL);
        assert!(add_bytes(&mut total, 1).is_err());
        let mut total = usize::MAX;
        assert!(add_bytes(&mut total, 1).is_err());
        assert_eq!(MAX_TOTAL, 512 * 1024 * 1024);
    }

    #[test]
    fn nesting_limit_is_enforced() {
        let f = Fixture::new();
        let mut path = f.bundle.clone();
        for _ in 0..=MAX_DEPTH {
            path.push("d");
            stdfs::create_dir(&path).unwrap();
            chmod(&path, 0o700);
        }
        assert!(filesystem()
            .tree(&f.bundle, false)
            .unwrap_err()
            .contains("nesting exceeds limit"));
    }

    #[test]
    fn entry_limit_is_enforced() {
        let f = Fixture::new();
        let flat = f.root.join("flat");
        stdfs::create_dir(&flat).unwrap();
        chmod(&flat, 0o700);
        for i in 0..=MAX_ENTRIES {
            write(&flat.join(format!("entry-{i:05}")), b"", 0o600);
        }
        assert!(filesystem()
            .tree(&flat, false)
            .unwrap_err()
            .contains("entry limit"));
        assert_eq!(MAX_ENTRIES, 10_000);
    }

    #[test]
    fn unsafe_names_and_reserved_manifests_are_rejected() {
        for name in [
            "bad name",
            "bad\nname",
            "é",
            MANIFEST,
            "data/flutter_assets/.smith-next-manifest.json",
        ] {
            let f = Fixture::new();
            write(&f.bundle.join(name), b"unknown", 0o600);
            assert!(
                filesystem().tree(&f.bundle, false).is_err(),
                "accepted {name:?}"
            );
        }
    }

    #[test]
    fn moved_identity_ignores_only_rename_ctime() {
        let f = Fixture::new();
        let file = stdfs::File::open(&f.service).unwrap();
        let original = Identity::of(&fstat(file.as_raw_fd()).unwrap());
        let mut moved = original.clone();
        moved.ctime.0 += 1;
        assert!(moved.same_moved(&original));
        assert_ne!(moved, original);
        for change in ["inode", "size", "mode", "links", "mtime", "owner", "device"] {
            let mut moved = original.clone();
            match change {
                "inode" => moved.ino += 1,
                "size" => moved.size += 1,
                "mode" => moved.mode ^= 0o100,
                "links" => moved.links += 1,
                "mtime" => moved.mtime.1 += 1,
                "owner" => moved.uid += 1,
                _ => moved.dev += 1,
            }
            assert!(!moved.same_moved(&original), "ignored {change}");
        }
    }
}
