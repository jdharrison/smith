//! Unwired Ubuntu-only local launcher. No constructor performs I/O or launches work.
//!
//! The fixed CLI File argument and raw scalar codec are SOURCE contracts, not live
//! Dagger interoperability evidence. No engine, image, provider or egress authority
//! is supplied here. A process group is not an engine ownership boundary or sandbox.
//! Private snapshots prevent later source edits from changing dispatched bytes;
//! metadata checks also reject observed concurrent source drift. This is not a
//! defense against a hostile same-UID writer, nor a hard-real-time filesystem API.

use crate::artifact_dagger::{
    Invocation, Observation, Runner, RunnerError, MAX_REQUEST, MAX_STDERR, MAX_STDOUT,
};
use crate::artifact_pipeline::{AttemptId, ContentHash, Phase};
use std::collections::BTreeMap;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

const MAX_FILES: usize = 128;
const MAX_SOURCE: usize = 4 * 1024 * 1024;
const MAX_EXECUTABLE: usize = 256 * 1024 * 1024;
const MAX_HISTORY: usize = 256;
const POLL: Duration = Duration::from_millis(2);

/// Explicit owner-selected locations, never inferred from HOME, cwd or a project.
/// State/workspace must already be distinct, private, empty-or-retained directories.
/// Executable/module must be exact absolute trusted source paths. No creation,
/// permission repair, startup reconciliation or pruning is performed by `new`.
#[derive(Clone)]
pub struct Roots {
    pub state: PathBuf,
    pub workspace: PathBuf,
    pub executable: PathBuf,
    pub module: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quiescence {
    Verified,
    Active,
    Unknown,
}

/// Trusted independent engine observation. Never derive this from module JSON,
/// CLI exit, or CLI process-group death. Implementations must obey `deadline` and
/// cancellation; they receive no captured output and must not launch fallback work.
pub trait QuiescenceVerifier: Send {
    fn verify(
        &mut self,
        attempt: AttemptId,
        request: ContentHash,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Quiescence;
}

#[derive(Default)]
pub struct UnknownQuiescence;
impl QuiescenceVerifier for UnknownQuiescence {
    fn verify(&mut self, _: AttemptId, _: ContentHash, _: Instant, _: &AtomicBool) -> Quiescence {
        Quiescence::Unknown
    }
}

pub struct ProcessRunner {
    roots: Roots,
    verifier: Box<dyn QuiescenceVerifier>,
    cancelled: Arc<AtomicBool>,
    original_start: Instant,
    original_budget: Duration,
}

impl ProcessRunner {
    /// Explicit dependency injection only; no effects, defaults, probes or recovery.
    pub fn new(
        roots: Roots,
        verifier: Box<dyn QuiescenceVerifier>,
        cancelled: Arc<AtomicBool>,
        original_start: Instant,
        original_budget: Duration,
    ) -> Self {
        Self {
            roots,
            verifier,
            cancelled,
            original_start,
            original_budget,
        }
    }

    fn launch(&mut self, invocation: &Invocation) -> Result<Observation, RunnerError> {
        let now = Instant::now();
        let original_deadline = self
            .original_start
            .checked_add(self.original_budget)
            .ok_or(RunnerError::Unavailable)?;
        let deadline = now
            .checked_add(invocation.remaining())
            .ok_or(RunnerError::Unavailable)?
            .min(original_deadline);
        let budget = Budget {
            deadline,
            cancelled: &self.cancelled,
        };
        budget.check()?;
        if self.original_start > now
            || invocation.binding().executable() != self.roots.executable
            || invocation.binding().module() != self.roots.module
            || ContentHash::of(invocation.request()) != invocation.request_hash()
            || invocation.request().len() > MAX_REQUEST
            || invocation.environment() != &fixed_environment()
        {
            return Err(RunnerError::Unavailable);
        }
        let state = private_root(&self.roots.state)?;
        let workspace = private_root(&self.roots.workspace)?;
        if self.roots.state.starts_with(&self.roots.workspace)
            || self.roots.workspace.starts_with(&self.roots.state)
            || self.roots.module.starts_with(&self.roots.workspace)
            || self.roots.module.starts_with(&self.roots.state)
            || self.roots.workspace.starts_with(&self.roots.module)
            || self.roots.state.starts_with(&self.roots.module)
        {
            return Err(RunnerError::Unavailable);
        }
        let _lock = Lock::acquire(&self.roots.state.join("exclusion.lock"))?;
        state.unchanged()?;
        workspace.unchanged()?;
        let key = attempt_key(invocation.attempt());
        admit_history(&self.roots.state, &key)?;
        budget.check()?;
        let executable = held_file(&self.roots.executable, MAX_EXECUTABLE)?;
        if ContentHash::of(&executable.bytes) != invocation.binding().executable_hash()
            || executable.metadata.mode() & 0o111 == 0
        {
            return Err(RunnerError::Unavailable);
        }
        let source = snapshot(&self.roots.module)?;
        if source.hash() != invocation.binding().module_hash() {
            return Err(RunnerError::Unavailable);
        }
        budget.check()?;
        let work = self.roots.workspace.join(&key);
        private_mkdir(&work)?;
        let module = work.join("module");
        private_mkdir(&module)?;
        source.copy_to(&module)?;
        let cli = work.join("cli");
        publish(&cli, &executable.bytes, 0o700)?;
        let request = work.join("request.json");
        publish(&request, invocation.request(), 0o600)?;
        for name in ["home", "config", "cache", "data", "runtime", "tmp"] {
            private_mkdir(&work.join(name))?;
        }
        File::open(&work)
            .map_err(unavailable)?
            .sync_all()
            .map_err(unavailable)?;
        workspace.file.sync_all().map_err(unavailable)?;
        executable.unchanged()?;
        source.unchanged(&self.roots.module)?;
        state.unchanged()?;
        workspace.unchanged()?;
        budget.check()?;
        // Record identity is attempt-only: changed request/binding cannot evade replay.
        // Both immutable history and the global pending fence are durable BEFORE spawn.
        let marker = format!(
            "smith-launch-v1\nattempt={key}\nrequest={}\nexecutable={}\nmodule={}\n",
            hex(invocation.request_hash().bytes()),
            hex(invocation.binding().executable_hash().bytes()),
            hex(invocation.binding().module_hash().bytes()),
        );
        publish(
            &self.roots.state.join(format!("{key}.record")),
            marker.as_bytes(),
            0o600,
        )?;
        state.file.sync_all().map_err(unavailable)?;
        publish(&self.roots.state.join("pending"), marker.as_bytes(), 0o600)?;
        state.file.sync_all().map_err(unavailable)?;
        // Every error beyond this point intentionally retains unresolved history.
        budget.check()?;
        executable.unchanged()?;
        source.unchanged(&self.roots.module)?;
        state.unchanged().map_err(|_| RunnerError::OutcomeUnknown)?;
        workspace
            .unchanged()
            .map_err(|_| RunnerError::OutcomeUnknown)?;
        budget.check()?;
        let mut command = Command::new(&cli);
        command
            .args(["--mod"])
            .arg(&module)
            .args(["call", "run-fixture", "--request"])
            .arg(&request)
            .current_dir(&work)
            .env_clear()
            .env("HOME", work.join("home"))
            .env("XDG_CONFIG_HOME", work.join("config"))
            .env("XDG_CACHE_HOME", work.join("cache"))
            .env("XDG_DATA_HOME", work.join("data"))
            .env("XDG_RUNTIME_DIR", work.join("runtime"))
            .env("TMPDIR", work.join("tmp"))
            .env("PATH", "/usr/bin:/bin")
            .envs(invocation.environment())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let child = command.spawn().map_err(|_| RunnerError::OutcomeUnknown)?;
        let mut child = OwnedChild {
            child,
            reaped: false,
        };
        let capture = capture(&mut child, &budget);
        // Verify source drift even on normal transport success. Snapshots themselves
        // remain private/retained; source failure cannot be accepted as artifact proof.
        let stable = executable.unchanged().is_ok() && source.unchanged(&self.roots.module).is_ok();
        let engine_quiet = if budget.check().is_ok() {
            self.verifier.verify(
                invocation.attempt(),
                invocation.request_hash(),
                deadline,
                &self.cancelled,
            ) == Quiescence::Verified
        } else {
            false
        };
        let within_budget = budget.check().is_ok();
        let local_quiet = child.reaped && child.group_absent();
        let verified = engine_quiet && local_quiet && within_budget;
        if verified {
            budget.check()?;
            state.unchanged().map_err(|_| RunnerError::OutcomeUnknown)?;
            // Positive engine observation + local reap, never response claims.
            publish(
                &self.roots.state.join(format!("{key}.quiescent")),
                marker.as_bytes(),
                0o600,
            )
            .map_err(|_| RunnerError::OutcomeUnknown)?;
            state
                .file
                .sync_all()
                .map_err(|_| RunnerError::OutcomeUnknown)?;
            budget.check()?;
            fs::remove_file(self.roots.state.join("pending"))
                .map_err(|_| RunnerError::OutcomeUnknown)?;
            state
                .file
                .sync_all()
                .map_err(|_| RunnerError::OutcomeUnknown)?;
        }
        if !stable {
            return Err(RunnerError::OutcomeUnknown);
        }
        let capture = capture?;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(RunnerError::Interrupted);
        }
        Ok(Observation {
            exit_code: capture.exit_code,
            stdout: capture.stdout,
            stderr: capture.stderr,
            elapsed: self.original_start.elapsed(),
            owned_cleanup_verified: verified,
        })
    }
}

impl Runner for ProcessRunner {
    fn run(&mut self, invocation: &Invocation) -> Result<Observation, RunnerError> {
        self.launch(invocation)
    }
}

fn fixed_environment() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("DAGGER_CLOUD_TOKEN", ""),
        ("DAGGER_NO_NAG", "1"),
        ("DAGGER_PROGRESS", "plain"),
        ("DO_NOT_TRACK", "1"),
    ])
}

struct Budget<'a> {
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Budget<'_> {
    fn check(&self) -> Result<(), RunnerError> {
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            Err(RunnerError::Interrupted)
        } else {
            Ok(())
        }
    }
}

fn unavailable(_: io::Error) -> RunnerError {
    RunnerError::Unavailable
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn attempt_key(attempt: AttemptId) -> String {
    let phase = match attempt.phase() {
        Phase::Setup => "setup",
        Phase::Execute => "execute",
        Phase::Submit => "submit",
    };
    format!(
        "{}-{phase}-{}",
        hex(attempt.run().bytes()),
        attempt.number()
    )
}

fn safe_ancestry(path: &Path) -> Result<(), RunnerError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(RunnerError::Unavailable);
    }
    let mut current = PathBuf::from("/");
    let uid = unsafe { libc::geteuid() };
    for part in path.components().skip(1) {
        current.push(part.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(unavailable)?;
        // The root-owned sticky /tmp boundary is allowed; private leaves are not
        // allowed to rely on sticky permissions instead of exclusive ownership.
        let sticky_root = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0 && metadata.is_dir();
        if metadata.file_type().is_symlink()
            || (metadata.uid() != uid && metadata.uid() != 0)
            || (metadata.mode() & 0o022 != 0 && !sticky_root)
        {
            return Err(RunnerError::Unavailable);
        }
    }
    Ok(())
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn same_directory(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino() && a.mode() == b.mode() && a.uid() == b.uid()
}
struct PrivateRoot {
    path: PathBuf,
    file: File,
    metadata: Metadata,
}
impl PrivateRoot {
    fn unchanged(&self) -> Result<(), RunnerError> {
        safe_ancestry(&self.path)?;
        let current = fs::symlink_metadata(&self.path).map_err(unavailable)?;
        if !same_directory(&self.metadata, &current) {
            return Err(RunnerError::Unavailable);
        }
        Ok(())
    }
}
fn private_root(path: &Path) -> Result<PrivateRoot, RunnerError> {
    safe_ancestry(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(path)
        .map_err(unavailable)?;
    let metadata = file.metadata().map_err(unavailable)?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(RunnerError::Unavailable);
    }
    Ok(PrivateRoot {
        path: path.to_path_buf(),
        file,
        metadata,
    })
}
fn private_mkdir(path: &Path) -> Result<(), RunnerError> {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(unavailable)
}
fn publish(path: &Path, bytes: &[u8], mode: u32) -> Result<(), RunnerError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(unavailable)?;
    file.write_all(bytes).map_err(unavailable)?;
    file.sync_all().map_err(unavailable)
}

struct Lock(File);
impl Lock {
    fn acquire(path: &Path) -> Result<Self, RunnerError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(unavailable)?;
        let m = file.metadata().map_err(unavailable)?;
        if !m.is_file()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
        {
            return Err(RunnerError::Unavailable);
        }
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(RunnerError::OutcomeUnknown);
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn admit_history(root: &Path, key: &str) -> Result<(), RunnerError> {
    let mut records = Vec::new();
    let mut completed = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(root).map_err(unavailable)? {
        count += 1;
        if count > MAX_HISTORY * 2 + 2 {
            return Err(RunnerError::Unavailable);
        }
        let entry = entry.map_err(unavailable)?;
        let m = fs::symlink_metadata(entry.path()).map_err(unavailable)?;
        if !m.is_file()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
        {
            return Err(RunnerError::OutcomeUnknown);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| RunnerError::OutcomeUnknown)?;
        if name == "pending" {
            return Err(RunnerError::OutcomeUnknown);
        }
        if name == "exclusion.lock" {
            continue;
        }
        if let Some(key) = name.strip_suffix(".record") {
            records.push(key.to_owned());
        } else if let Some(key) = name.strip_suffix(".quiescent") {
            completed.push(key.to_owned());
        } else {
            return Err(RunnerError::OutcomeUnknown);
        }
    }
    if records.iter().any(|r| !completed.contains(r))
        || completed.iter().any(|r| !records.contains(r))
    {
        return Err(RunnerError::OutcomeUnknown);
    }
    for record in &records {
        let consumed = held_file(&root.join(format!("{record}.record")), 1024)
            .map_err(|_| RunnerError::OutcomeUnknown)?;
        let quiet = held_file(&root.join(format!("{record}.quiescent")), 1024)
            .map_err(|_| RunnerError::OutcomeUnknown)?;
        let text = std::str::from_utf8(&consumed.bytes).map_err(|_| RunnerError::OutcomeUnknown)?;
        let lines: Vec<_> = text.lines().collect();
        let valid_hash = |line: &str, prefix: &str| {
            line.strip_prefix(prefix).is_some_and(|h| {
                h.len() == 64
                    && h.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        };
        if consumed.bytes != quiet.bytes
            || lines.len() != 5
            || lines[0] != "smith-launch-v1"
            || lines[1] != format!("attempt={record}")
            || !text.ends_with('\n')
            || !valid_hash(lines[2], "request=")
            || !valid_hash(lines[3], "executable=")
            || !valid_hash(lines[4], "module=")
        {
            return Err(RunnerError::OutcomeUnknown);
        }
    }
    if records.len() >= MAX_HISTORY || records.iter().any(|r| r == key) {
        return Err(RunnerError::Unavailable);
    }
    Ok(())
}

struct HeldFile {
    path: PathBuf,
    file: File,
    metadata: Metadata,
    bytes: Vec<u8>,
}
impl HeldFile {
    fn unchanged(&self) -> Result<(), RunnerError> {
        let a = self.file.metadata().map_err(unavailable)?;
        let b = fs::symlink_metadata(&self.path).map_err(unavailable)?;
        if !same(&self.metadata, &a) || !same(&self.metadata, &b) {
            return Err(RunnerError::Unavailable);
        }
        Ok(())
    }
}
fn held_file(path: &Path, limit: usize) -> Result<HeldFile, RunnerError> {
    safe_ancestry(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(unavailable)?;
    let metadata = file.metadata().map_err(unavailable)?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > limit as u64 {
        return Err(RunnerError::Unavailable);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(unavailable)?;
    if bytes.len() > limit {
        return Err(RunnerError::Unavailable);
    }
    let result = HeldFile {
        path: path.to_path_buf(),
        file,
        metadata,
        bytes,
    };
    result.unchanged()?;
    Ok(result)
}

struct Source {
    directories: Vec<(PathBuf, Metadata)>,
    files: BTreeMap<PathBuf, HeldFile>,
}
impl Source {
    fn hash(&self) -> ContentHash {
        // Domain + sorted typed relative UTF-8 paths, each length-prefixed u64 BE;
        // file content length u64 BE and SHA256 bytes. Directories (including root)
        // have zero length and SHA256(empty). No ignored files or excluded subtrees.
        let mut entries = BTreeMap::new();
        for (p, _) in &self.directories {
            entries.insert(
                p.to_str().expect("validated UTF-8 path"),
                (b'd', 0, ContentHash::of(b"")),
            );
        }
        for (p, f) in &self.files {
            entries.insert(
                p.to_str().expect("validated UTF-8 path"),
                (b'f', f.bytes.len() as u64, ContentHash::of(&f.bytes)),
            );
        }
        let mut bytes = b"smith-module-manifest-v1\0".to_vec();
        for (path, (kind, length, hash)) in entries {
            bytes.push(kind);
            bytes.extend_from_slice(&(path.len() as u64).to_be_bytes());
            bytes.extend_from_slice(path.as_bytes());
            bytes.extend_from_slice(&length.to_be_bytes());
            bytes.extend_from_slice(hash.bytes());
        }
        ContentHash::of(&bytes)
    }
    fn copy_to(&self, root: &Path) -> Result<(), RunnerError> {
        for (path, _) in &self.directories {
            if !path.as_os_str().is_empty() {
                private_mkdir(&root.join(path))?;
            }
        }
        for (path, file) in &self.files {
            publish(&root.join(path), &file.bytes, 0o600)?;
        }
        for (path, _) in self.directories.iter().rev() {
            File::open(root.join(path))
                .map_err(unavailable)?
                .sync_all()
                .map_err(unavailable)?;
        }
        Ok(())
    }
    fn unchanged(&self, root: &Path) -> Result<(), RunnerError> {
        for file in self.files.values() {
            file.unchanged()?;
        }
        let current = snapshot(root)?;
        if self.hash() != current.hash()
            || self.directories.len() != current.directories.len()
            || self.files.len() != current.files.len()
        {
            return Err(RunnerError::Unavailable);
        }
        for ((p, before), (q, after)) in self.directories.iter().zip(&current.directories) {
            if p != q || !same(before, after) {
                return Err(RunnerError::Unavailable);
            }
        }
        Ok(())
    }
}

/// Canonical full-snapshot source identity, with no ignore rules. SHA256 of the
/// domain `smith-module-manifest-v1\0`, then bytewise sorted UTF-8 relative paths
/// (root directory is ""). Each entry: type byte (`d`/`f`), path length u64 BE,
/// path bytes (Unix '/' separators), content length u64 BE, 32-byte content SHA256.
/// Directory content is empty. File modes are not hashed; unsafe permissions,
/// symlinks, hardlinks, special files, non-UTF8 paths and observed drift are rejected.
/// Limits: 128 total entries, 4MiB total file bytes, 32 directory levels. The caller
/// must pin the expected digest from a trusted fixed module, never a project module.
/// Performs read-only filesystem I/O; it does not attest provenance or permissions.
pub fn module_source_hash(root: &Path) -> Result<ContentHash, RunnerError> {
    Ok(snapshot(root)?.hash())
}
fn snapshot(root: &Path) -> Result<Source, RunnerError> {
    safe_ancestry(root)?;
    let mut source = Source {
        directories: Vec::new(),
        files: BTreeMap::new(),
    };
    let mut total = 0;
    walk(root, Path::new(""), &mut source, &mut total, 0)?;
    if source.files.is_empty() {
        return Err(RunnerError::Unavailable);
    }
    for file in source.files.values() {
        file.unchanged()?;
    }
    for (path, before) in &source.directories {
        let after = fs::symlink_metadata(root.join(path)).map_err(unavailable)?;
        if !same(before, &after) {
            return Err(RunnerError::Unavailable);
        }
    }
    Ok(source)
}
fn walk(
    root: &Path,
    relative: &Path,
    source: &mut Source,
    total: &mut usize,
    depth: usize,
) -> Result<(), RunnerError> {
    if depth > 32 || source.files.len() + source.directories.len() >= MAX_FILES {
        return Err(RunnerError::Unavailable);
    }
    let path = root.join(relative);
    let before = fs::symlink_metadata(&path).map_err(unavailable)?;
    if !before.is_dir() || before.file_type().is_symlink() {
        return Err(RunnerError::Unavailable);
    }
    safe_ancestry(&path)?;
    source
        .directories
        .push((relative.to_path_buf(), before.clone()));
    let mut entries = fs::read_dir(&path)
        .map_err(unavailable)?
        .take(MAX_FILES + 1)
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    if entries.len() > MAX_FILES {
        return Err(RunnerError::Unavailable);
    }
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if source.files.len() + source.directories.len() >= MAX_FILES {
            return Err(RunnerError::Unavailable);
        }
        let name = entry.file_name();
        let text = name.to_str().ok_or(RunnerError::Unavailable)?;
        if text.len() > 255 || text.contains(['\n', '\r', '\0']) {
            return Err(RunnerError::Unavailable);
        }
        let child = relative.join(&name);
        let m = fs::symlink_metadata(entry.path()).map_err(unavailable)?;
        if m.is_dir() && !m.file_type().is_symlink() {
            walk(root, &child, source, total, depth + 1)?;
        } else {
            let file = held_file(&entry.path(), MAX_SOURCE.saturating_sub(*total))?;
            *total += file.bytes.len();
            source.files.insert(child, file);
        }
    }
    let after = fs::symlink_metadata(&path).map_err(unavailable)?;
    if !same(&before, &after) {
        return Err(RunnerError::Unavailable);
    }
    Ok(())
}

struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl OwnedChild {
    fn signal(&self, signal: i32) {
        // Only the process group created for this Child; never an ambient PID.
        unsafe {
            libc::kill(-(self.child.id() as i32), signal);
        }
    }
    fn group_absent(&self) -> bool {
        let result = unsafe { libc::kill(-(self.child.id() as i32), 0) };
        result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            self.signal(libc::SIGKILL);
            // No unbounded wait in Drop. Unreaped ownership remains durably fenced.
            if let Ok(Some(_)) = self.child.try_wait() {
                self.reaped = true;
            }
        }
    }
}
struct Capture {
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
fn nonblocking(file: &impl AsRawFd) -> Result<(), RunnerError> {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(RunnerError::OutcomeUnknown);
    }
    Ok(())
}
fn drain(
    pipe: &mut impl Read,
    out: &mut Vec<u8>,
    max: usize,
    eof: &mut bool,
) -> Result<(), RunnerError> {
    let mut bytes = [0u8; 4096];
    // Fair bounded rounds: an endless stdout writer cannot starve stderr/deadline.
    for _ in 0..8 {
        match pipe.read(&mut bytes) {
            Ok(0) => {
                *eof = true;
                return Ok(());
            }
            Ok(n) => {
                let remaining = max.saturating_sub(out.len());
                out.extend_from_slice(&bytes[..n.min(remaining)]);
                if n > remaining {
                    return Err(RunnerError::OutcomeUnknown);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(RunnerError::OutcomeUnknown),
        }
    }
    Ok(())
}
fn capture(child: &mut OwnedChild, budget: &Budget<'_>) -> Result<Capture, RunnerError> {
    let mut stdout = child
        .child
        .stdout
        .take()
        .ok_or(RunnerError::OutcomeUnknown)?;
    let mut stderr = child
        .child
        .stderr
        .take()
        .ok_or(RunnerError::OutcomeUnknown)?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut result = Capture {
        exit_code: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    let (mut out_eof, mut err_eof) = (false, false);
    let mut failure = None;
    let mut terminate = None;
    let reserve = budget.deadline.saturating_duration_since(Instant::now()) / 4;
    let run_until = budget.deadline - reserve.min(Duration::from_millis(200));
    loop {
        let now = Instant::now();
        if (budget.cancelled.load(Ordering::Acquire) || now >= run_until) && terminate.is_none() {
            failure = Some(RunnerError::Interrupted);
            child.signal(libc::SIGTERM);
            terminate = Some(now);
        }
        if let Some(start) = terminate {
            if now.duration_since(start) >= Duration::from_millis(25) {
                child.signal(libc::SIGKILL);
            }
        }
        let out = drain(&mut stdout, &mut result.stdout, MAX_STDOUT, &mut out_eof);
        let err = drain(&mut stderr, &mut result.stderr, MAX_STDERR, &mut err_eof);
        if (out.is_err() || err.is_err()) && terminate.is_none() {
            failure = Some(RunnerError::OutcomeUnknown);
            child.signal(libc::SIGTERM);
            terminate = Some(now);
        }
        if !child.reaped {
            match child.child.try_wait() {
                Ok(Some(status)) => {
                    child.reaped = true;
                    result.exit_code = status.code();
                    // The CLI can exit while descendants hold pipes. Kill ONLY its
                    // owned group immediately, then boundedly drain/reap/observe.
                    child.signal(libc::SIGKILL);
                }
                Ok(None) => {}
                Err(_) => {
                    failure = Some(RunnerError::OutcomeUnknown);
                    child.signal(libc::SIGKILL);
                }
            }
        }
        if child.reaped && out_eof && err_eof {
            break;
        }
        if Instant::now() >= budget.deadline {
            child.signal(libc::SIGKILL);
            if let Ok(Some(_)) = child.child.try_wait() {
                child.reaped = true;
            }
            return Err(failure.unwrap_or(RunnerError::OutcomeUnknown));
        }
        std::thread::sleep(POLL.min(budget.deadline.saturating_duration_since(Instant::now())));
    }
    if let Some(error) = failure {
        Err(error)
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_dagger::{Adapter, ModuleBinding};
    use crate::artifact_pipeline::{Pipeline, RunId};
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::sync::atomic::AtomicU64;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const TASK: &str = "private task; $(touch forbidden) ' never an argv token";

    struct Fixture {
        root: PathBuf,
        roots: Roots,
    }
    impl Fixture {
        fn new(script: &str) -> Self {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let root =
                PathBuf::from("/tmp").join(format!("smith-launch-test-{}-{n}", std::process::id()));
            private_mkdir(&root).expect("private controlled fixture");
            let roots = Roots {
                state: root.join("state"),
                workspace: root.join("workspace"),
                executable: root.join("fake-cli"),
                module: root.join("fixed-module"),
            };
            for path in [&roots.state, &roots.workspace, &roots.module] {
                private_mkdir(path).unwrap();
            }
            publish(&roots.module.join("dagger.json"), b"{}", 0o600).unwrap();
            private_mkdir(&roots.module.join("src")).unwrap();
            publish(
                &roots.module.join("src/main.py"),
                b"# controlled fixture\n",
                0o600,
            )
            .unwrap();
            publish(
                &roots.executable,
                format!("#!/bin/sh\n{script}\n").as_bytes(),
                0o700,
            )
            .unwrap();
            Self { root, roots }
        }
        fn binding(&self) -> ModuleBinding {
            ModuleBinding::new(
                self.roots.executable.clone(),
                self.roots.module.clone(),
                ContentHash::of(&fs::read(&self.roots.executable).unwrap()),
                module_source_hash(&self.roots.module).unwrap(),
            )
            .unwrap()
        }
        fn runner(
            &self,
            verifier: Box<dyn QuiescenceVerifier>,
            duration: Duration,
        ) -> ProcessRunner {
            ProcessRunner::new(
                self.roots.clone(),
                verifier,
                Arc::new(AtomicBool::new(false)),
                Instant::now(),
                duration,
            )
        }
        fn call(&self, runner: &mut ProcessRunner, run: u8) -> Result<Observation, RunnerError> {
            dispatch(runner, self.binding(), run)
        }
        fn assert_pending(&self) {
            assert!(self.roots.state.join("pending").is_file());
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only our generated private roots, never installed state or arbitrary paths.
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    // Invocation construction stays owned by Adapter. The tap observes transport
    // without supplying response/check evidence or altering private Invocation data.
    struct Tap<'a> {
        runner: &'a mut ProcessRunner,
        observed: Option<Result<Observation, RunnerError>>,
    }
    impl Runner for Tap<'_> {
        fn run(&mut self, invocation: &Invocation) -> Result<Observation, RunnerError> {
            self.observed = Some(self.runner.run(invocation));
            Err(RunnerError::Unavailable)
        }
    }
    fn dispatch(
        runner: &mut ProcessRunner,
        binding: ModuleBinding,
        run: u8,
    ) -> Result<Observation, RunnerError> {
        let mut adapter = Adapter::new(binding);
        let mut pipeline = Pipeline::new(
            RunId::new([run; 16]).unwrap(),
            b"fixed contract",
            Duration::from_secs(5),
        )
        .unwrap();
        let mut tap = Tap {
            runner,
            observed: None,
        };
        let _ = adapter.dispatch(&mut pipeline, TASK, Duration::ZERO, &mut tap);
        tap.observed.expect("adapter constructed invocation")
    }
    struct Positive(Arc<AtomicBool>);
    impl QuiescenceVerifier for Positive {
        fn verify(
            &mut self,
            _: AttemptId,
            _: ContentHash,
            _: Instant,
            _: &AtomicBool,
        ) -> Quiescence {
            self.0.store(true, Ordering::Release);
            Quiescence::Verified
        }
    }
    fn positive() -> Box<dyn QuiescenceVerifier> {
        Box::new(Positive(Arc::new(AtomicBool::new(false))))
    }
    fn error(result: Result<Observation, RunnerError>) -> RunnerError {
        match result {
            Err(e) => e,
            Ok(_) => panic!("unexpected launch observation"),
        }
    }

    #[test]
    fn constructor_is_effect_free_and_requires_explicit_dependencies() {
        let f = Fixture::new("exit 0");
        let mut roots = f.roots.clone();
        roots.state = f.root.join("absent");
        let _runner = ProcessRunner::new(
            roots,
            Box::new(UnknownQuiescence),
            Arc::new(AtomicBool::new(false)),
            Instant::now(),
            Duration::from_secs(1),
        );
        assert!(!f.root.join("absent").exists());
        assert_eq!(fs::read_dir(&f.roots.workspace).unwrap().count(), 0);
    }

    #[test]
    fn exact_argv_environment_and_private_request_without_task_in_argv() {
        let f = Fixture::new(
            r#"
[ "$#" = 6 ] || exit 81
[ "$1" = --mod ] && [ "$3" = call ] && [ "$4" = run-fixture ] && [ "$5" = --request ] || exit 82
[ "$2" = "$PWD/module" ] && [ "$6" = "$PWD/request.json" ] || exit 83
[ -f "$2/dagger.json" ] && [ -f "$2/src/main.py" ] || exit 84
[ -f "$6" ] && [ ! -L "$6" ] || exit 85
[ "$PATH" = /usr/bin:/bin ] && [ "$HOME" = "$PWD/home" ] || exit 86
[ "$XDG_CONFIG_HOME" = "$PWD/config" ] && [ "$XDG_CACHE_HOME" = "$PWD/cache" ] || exit 87
[ "$XDG_DATA_HOME" = "$PWD/data" ] && [ "$XDG_RUNTIME_DIR" = "$PWD/runtime" ] || exit 88
[ "$TMPDIR" = "$PWD/tmp" ] && [ "$DAGGER_PROGRESS" = plain ] || exit 89
[ "$DAGGER_NO_NAG" = 1 ] && [ "$DO_NOT_TRACK" = 1 ] && [ "${DAGGER_CLOUD_TOKEN+x}" = x ] && [ -z "$DAGGER_CLOUD_TOKEN" ] || exit 90
[ -z "${OPENAI_API_KEY+x}${ANTHROPIC_API_KEY+x}${DAGGER_RUNNER_HOST+x}${DOCKER_HOST+x}${SSH_AUTH_SOCK+x}${LD_PRELOAD+x}${PYTHONPATH+x}" ] || exit 91
/usr/bin/env
printf '\nREQUEST\n'
/bin/cat "$6"
"#,
        );
        let mut runner = f.runner(positive(), Duration::from_secs(3));
        let observation = f.call(&mut runner, 1).unwrap();
        assert_eq!(observation.exit_code, Some(0));
        assert!(observation.owned_cleanup_verified);
        let output = String::from_utf8(observation.stdout).unwrap();
        let (environment, request) = output.split_once("\nREQUEST\n").unwrap();
        for line in environment.lines() {
            let key = line.split('=').next().unwrap();
            assert!(
                [
                    "HOME",
                    "XDG_CONFIG_HOME",
                    "XDG_CACHE_HOME",
                    "XDG_DATA_HOME",
                    "XDG_RUNTIME_DIR",
                    "TMPDIR",
                    "PATH",
                    "DAGGER_CLOUD_TOKEN",
                    "DAGGER_NO_NAG",
                    "DAGGER_PROGRESS",
                    "DO_NOT_TRACK",
                    "PWD"
                ]
                .contains(&key),
                "unexpected environment key"
            );
        }
        let request: serde_json::Value = serde_json::from_str(request).unwrap();
        assert_eq!(request["input"], TASK);
        let work = fs::read_dir(&f.roots.workspace)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let m = fs::metadata(work.join("request.json")).unwrap();
        assert_eq!(m.mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(work).unwrap().mode() & 0o777, 0o700);
        assert!(!f.root.join("forbidden").exists());
    }

    #[test]
    fn raw_scalar_bytes_and_nonzero_transport_are_not_reinterpreted() {
        for (script, expected, code) in [
            (
                "printf '%s' '{\"outcome\":\"failed\"}'",
                b"{\"outcome\":\"failed\"}".as_slice(),
                0,
            ),
            (
                "printf '%s' '\"{\\\"outcome\\\":\\\"produced\\\"}\"'",
                b"\"{\\\"outcome\\\":\\\"produced\\\"}\"".as_slice(),
                0,
            ),
            (
                "printf 'prefix\\n{}\\n'; printf 'authentication' >&2; exit 17",
                b"prefix\n{}\n".as_slice(),
                17,
            ),
        ] {
            let f = Fixture::new(script);
            let mut runner = f.runner(positive(), Duration::from_secs(3));
            let observation = f.call(&mut runner, 1).unwrap();
            assert_eq!(observation.exit_code, Some(code));
            assert_eq!(observation.stdout, expected);
        }
    }

    #[test]
    fn marker_and_consumed_binding_exist_before_child_runs() {
        let f = Fixture::new("exit 0");
        // The only interpolated value is our own controlled ASCII fixture path.
        // The fake child never receives arbitrary tasks as code or command text.
        let script = format!(
            "#!/bin/sh\n[ -s '{0}/pending' ] || exit 71\nset -- '{0}'/*.record\n[ \"$#\" = 1 ] && [ -s \"$1\" ] || exit 72\n/usr/bin/cmp -s '{0}/pending' \"$1\" || exit 73\n/bin/cat '{0}/pending'\n",
            f.roots.state.display()
        );
        fs::write(&f.roots.executable, script).unwrap();
        let mut runner = f.runner(positive(), Duration::from_secs(3));
        let observation = f.call(&mut runner, 1).unwrap();
        assert_eq!(observation.exit_code, Some(0));
        let marker = String::from_utf8(observation.stdout).unwrap();
        assert!(marker.starts_with("smith-launch-v1\nattempt="));
        assert!(
            marker.contains("\nrequest=")
                && marker.contains("\nexecutable=")
                && marker.contains("\nmodule=")
        );
        assert!(!marker.contains(TASK));
        let record = fs::read_dir(&f.roots.state)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|x| x == "record"))
            .unwrap();
        assert_eq!(fs::read(record.path()).unwrap(), marker.as_bytes());
        assert!(!f.roots.state.join("pending").exists());
    }

    #[test]
    fn unknown_cleanup_overrides_module_claims_and_blocks_restart_other_identity() {
        let f = Fixture::new("printf '%s' '{\"owned_cleanup_verified\":true}'");
        let mut runner = f.runner(Box::new(UnknownQuiescence), Duration::from_secs(3));
        let observation = f.call(&mut runner, 1).unwrap();
        assert!(!observation.owned_cleanup_verified);
        f.assert_pending();
        drop(runner);
        let called = Arc::new(AtomicBool::new(false));
        let mut recreated = f.runner(Box::new(Positive(called.clone())), Duration::from_secs(3));
        assert_eq!(
            error(f.call(&mut recreated, 2)),
            RunnerError::OutcomeUnknown
        );
        assert!(!called.load(Ordering::Acquire)); // no automatic reconciliation
        assert_eq!(fs::read_dir(&f.roots.workspace).unwrap().count(), 1);
    }

    #[test]
    fn positive_independent_verifier_clears_pending_but_never_consumed_history() {
        let f = Fixture::new("printf '%s' '{\"owned_cleanup_verified\":false}'");
        let called = Arc::new(AtomicBool::new(false));
        let mut runner = f.runner(Box::new(Positive(called.clone())), Duration::from_secs(3));
        let observation = f.call(&mut runner, 1).unwrap();
        assert!(observation.owned_cleanup_verified && called.load(Ordering::Acquire));
        assert!(!f.roots.state.join("pending").exists());
        assert_eq!(error(f.call(&mut runner, 1)), RunnerError::Unavailable);
        drop(runner);
        let mut recreated = f.runner(positive(), Duration::from_secs(3));
        assert_eq!(error(f.call(&mut recreated, 1)), RunnerError::Unavailable);
        assert!(f.call(&mut recreated, 2).unwrap().owned_cleanup_verified);
    }

    #[test]
    fn active_verifier_never_reports_verified() {
        struct Active;
        impl QuiescenceVerifier for Active {
            fn verify(
                &mut self,
                _: AttemptId,
                _: ContentHash,
                _: Instant,
                _: &AtomicBool,
            ) -> Quiescence {
                Quiescence::Active
            }
        }
        let f = Fixture::new("exit 0");
        let mut runner = f.runner(Box::new(Active), Duration::from_secs(3));
        assert!(!f.call(&mut runner, 1).unwrap().owned_cleanup_verified);
        f.assert_pending();
    }

    #[test]
    fn missing_pending_still_blocks_unresolved_consumed_history_and_corruption() {
        let f = Fixture::new("exit 0");
        publish(&f.roots.state.join("other.record"), b"incomplete", 0o600).unwrap();
        let mut runner = f.runner(positive(), Duration::from_secs(3));
        assert_eq!(error(f.call(&mut runner, 1)), RunnerError::OutcomeUnknown);
        publish(&f.roots.state.join("other.quiescent"), b"incomplete", 0o600).unwrap();
        assert_eq!(error(f.call(&mut runner, 2)), RunnerError::OutcomeUnknown);
    }

    #[test]
    fn private_exclusion_lock_prevents_overlapping_admission() {
        let f = Fixture::new("exit 0");
        let _lock = Lock::acquire(&f.roots.state.join("exclusion.lock")).unwrap();
        let mut runner = f.runner(positive(), Duration::from_secs(3));
        assert_eq!(error(f.call(&mut runner, 1)), RunnerError::OutcomeUnknown);
        assert_eq!(fs::read_dir(&f.roots.workspace).unwrap().count(), 0);
    }

    #[test]
    fn cli_and_full_manifest_drift_reject_before_spawn() {
        for kind in 0..5 {
            let f = Fixture::new("exit 0");
            let binding = f.binding();
            match kind {
                0 => fs::write(&f.roots.executable, b"#!/bin/sh\nexit 99\n").unwrap(),
                1 => fs::write(f.roots.module.join("src/main.py"), b"changed").unwrap(),
                2 => publish(&f.roots.module.join("unexpected"), b"extra", 0o600).unwrap(),
                3 => {
                    fs::remove_file(f.roots.module.join("src/main.py")).unwrap();
                }
                _ => private_mkdir(&f.roots.module.join("unexpected-empty-directory")).unwrap(),
            }
            let mut runner = f.runner(positive(), Duration::from_secs(3));
            assert_eq!(
                error(dispatch(&mut runner, binding, 1)),
                RunnerError::Unavailable
            );
            assert!(!f.roots.state.join("pending").exists());
            assert_eq!(fs::read_dir(&f.roots.workspace).unwrap().count(), 0);
        }
    }

    #[test]
    fn source_hash_is_canonical_and_covers_paths_lengths_and_all_files() {
        let a = Fixture::new("exit 0");
        let b = Fixture::new("exit 0");
        let original = module_source_hash(&a.roots.module).unwrap();
        assert_eq!(original, module_source_hash(&b.roots.module).unwrap());
        fs::rename(
            b.roots.module.join("src/main.py"),
            b.roots.module.join("src/renamed.py"),
        )
        .unwrap();
        assert_ne!(original, module_source_hash(&b.roots.module).unwrap());
        fs::write(
            a.roots.module.join("src/main.py"),
            b"# controlled fixture\nmore",
        )
        .unwrap();
        assert_ne!(original, module_source_hash(&a.roots.module).unwrap());
        let held = snapshot(&a.roots.module).unwrap();
        fs::write(
            a.roots.module.join("src/main.py"),
            b"# controlled fixture\nmore",
        )
        .unwrap();
        assert_eq!(
            held.unchanged(&a.roots.module),
            Err(RunnerError::Unavailable)
        );
    }

    #[test]
    fn held_cli_detects_same_content_replacement_and_module_drift_while_running() {
        let f = Fixture::new("exit 0");
        let held = held_file(&f.roots.executable, MAX_SOURCE).unwrap();
        let copy = fs::read(&f.roots.executable).unwrap();
        fs::remove_file(&f.roots.executable).unwrap();
        publish(&f.roots.executable, &copy, 0o700).unwrap();
        assert_eq!(held.unchanged(), Err(RunnerError::Unavailable));
        let script = format!(
            "#!/bin/sh\nprintf drift > '{}/src/main.py'\nexit 0\n",
            f.roots.module.display()
        );
        fs::write(&f.roots.executable, script).unwrap();
        let mut runner = f.runner(Box::new(UnknownQuiescence), Duration::from_secs(3));
        assert_eq!(error(f.call(&mut runner, 1)), RunnerError::OutcomeUnknown);
        f.assert_pending();
    }

    #[test]
    fn symlink_special_file_and_unsafe_workspace_are_rejected() {
        for kind in 0..5 {
            let f = Fixture::new("exit 0");
            let binding = f.binding();
            match kind {
                0 => {
                    fs::rename(&f.roots.executable, f.root.join("real-cli")).unwrap();
                    symlink(f.root.join("real-cli"), &f.roots.executable).unwrap();
                }
                1 => {
                    symlink("main.py", f.roots.module.join("src/link.py")).unwrap();
                }
                2 => {
                    fs::set_permissions(&f.roots.workspace, fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
                3 => {
                    fs::rename(&f.roots.workspace, f.root.join("real-workspace")).unwrap();
                    symlink(f.root.join("real-workspace"), &f.roots.workspace).unwrap();
                }
                _ => {
                    use std::ffi::CString;
                    let path =
                        CString::new(f.roots.module.join("fifo").as_os_str().as_encoded_bytes())
                            .unwrap();
                    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                }
            }
            let mut runner = f.runner(positive(), Duration::from_secs(3));
            assert_eq!(
                error(dispatch(&mut runner, binding, 1)),
                RunnerError::Unavailable
            );
            assert!(!f.roots.state.join("pending").exists());
        }
    }

    #[test]
    fn stdout_and_stderr_floods_are_bounded_while_process_is_running() {
        for script in [
            "while :; do printf '0123456789abcdef0123456789abcdef'; done",
            "while :; do printf '0123456789abcdef0123456789abcdef' >&2; done",
            "(while :; do printf '0123456789abcdef0123456789abcdef'; done) &\nwhile :; do printf '0123456789abcdef0123456789abcdef' >&2; done",
        ] {
            let f = Fixture::new(script);
            let mut runner = f.runner(Box::new(UnknownQuiescence), Duration::from_secs(2));
            let start = Instant::now();
            assert_eq!(error(f.call(&mut runner, 1)), RunnerError::OutcomeUnknown);
            assert!(start.elapsed() < Duration::from_secs(3));
            f.assert_pending();
        }
    }

    #[test]
    fn timeout_and_injected_cancellation_kill_owned_process_and_preserve_uncertainty() {
        for cancel in [false, true] {
            let f =
                Fixture::new("printf '%s' \"$$\" > leader.pid\ntrap '' TERM\nwhile :; do :; done");
            let mut runner = f.runner(Box::new(UnknownQuiescence), Duration::from_secs(1));
            let token = runner.cancelled.clone();
            let workspace = f.roots.workspace.clone();
            let canceller = std::thread::spawn(move || {
                if cancel {
                    let until = Instant::now() + Duration::from_secs(1);
                    while Instant::now() < until {
                        let started = fs::read_dir(&workspace)
                            .unwrap()
                            .filter_map(Result::ok)
                            .any(|entry| entry.path().join("leader.pid").exists());
                        if started {
                            break;
                        }
                        std::thread::sleep(POLL);
                    }
                    token.store(true, Ordering::Release);
                }
            });
            let start = Instant::now();
            assert_eq!(error(f.call(&mut runner, 1)), RunnerError::Interrupted);
            canceller.join().unwrap();
            assert!(start.elapsed() < Duration::from_secs(2));
            let work = fs::read_dir(&f.roots.workspace)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let pid: i32 = fs::read_to_string(work.join("leader.pid"))
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(
                unsafe { libc::kill(pid, 0) },
                -1,
                "owned direct child must be reaped"
            );
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
            f.assert_pending();
        }
    }

    #[test]
    fn exited_cli_with_descendant_held_pipes_does_not_hang_or_prove_engine_cleanup() {
        let f = Fixture::new("(trap '' TERM; while :; do :; done) &\nprintf '%s' \"$!\" > descendant.pid\nprintf child-exit\nexit 0");
        let mut runner = f.runner(Box::new(UnknownQuiescence), Duration::from_millis(700));
        let start = Instant::now();
        let observation = f.call(&mut runner, 1).unwrap();
        assert_eq!(observation.stdout, b"child-exit");
        let work = fs::read_dir(&f.roots.workspace)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let pid: i32 = fs::read_to_string(work.join("descendant.pid"))
            .unwrap()
            .parse()
            .unwrap();
        // An orphan zombie can remain until the test host's reaper acts. It is
        // not executable work, but group absence still stays UNKNOWN in production.
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => assert!(
                stat.rsplit_once(") ").unwrap().1.starts_with('Z'),
                "owned descendant must not be live"
            ),
            Err(e) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
        }
        assert_eq!(observation.exit_code, Some(0));
        assert!(!observation.owned_cleanup_verified);
        assert!(start.elapsed() < Duration::from_secs(2));
        f.assert_pending();
    }

    #[test]
    fn canonical_manifest_domain_and_entry_encoding_are_explicit() {
        let f = Fixture::new("exit 0");
        let mut reference = b"smith-module-manifest-v1\0".to_vec();
        for (kind, path, content) in [
            (b'd', "", b"".as_slice()),
            (b'f', "dagger.json", b"{}".as_slice()),
            (b'd', "src", b"".as_slice()),
            (b'f', "src/main.py", b"# controlled fixture\n".as_slice()),
        ] {
            reference.push(kind);
            reference.extend_from_slice(&(path.len() as u64).to_be_bytes());
            reference.extend_from_slice(path.as_bytes());
            reference.extend_from_slice(&(content.len() as u64).to_be_bytes());
            reference.extend_from_slice(ContentHash::of(content).bytes());
        }
        assert_eq!(
            module_source_hash(&f.roots.module).unwrap(),
            ContentHash::of(&reference)
        );
    }

    #[test]
    fn finite_drain_never_retains_more_than_capture_limit() {
        let mut input = io::Cursor::new(vec![b'x'; MAX_STDOUT + 1]);
        let mut captured = Vec::new();
        let mut eof = false;
        loop {
            if drain(&mut input, &mut captured, MAX_STDOUT, &mut eof).is_err() {
                break;
            }
            assert!(!eof);
        }
        assert_eq!(captured.len(), MAX_STDOUT);
    }

    #[test]
    fn cleanup_verifier_cannot_extend_original_deadline_or_release_uncertainty() {
        struct Late;
        impl QuiescenceVerifier for Late {
            fn verify(
                &mut self,
                _: AttemptId,
                _: ContentHash,
                deadline: Instant,
                _: &AtomicBool,
            ) -> Quiescence {
                // Deliberately violate the injected verifier contract to verify the
                // release gate checks time again, rather than trusting a late Yes.
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()) + POLL);
                Quiescence::Verified
            }
        }
        let f = Fixture::new("exit 0");
        let mut runner = f.runner(Box::new(Late), Duration::from_secs(1));
        let observed = f.call(&mut runner, 1).unwrap();
        assert!(!observed.owned_cleanup_verified);
        assert!(observed.elapsed >= Duration::from_secs(1));
        f.assert_pending();
    }

    #[test]
    fn qa_cancellation_during_verification_retains_pending_and_refuses_success() {
        struct CancelDuringVerification;
        impl QuiescenceVerifier for CancelDuringVerification {
            fn verify(
                &mut self,
                _: AttemptId,
                _: ContentHash,
                _: Instant,
                cancelled: &AtomicBool,
            ) -> Quiescence {
                cancelled.store(true, Ordering::Release);
                Quiescence::Verified
            }
        }
        let f = Fixture::new("exit 0");
        let mut runner = f.runner(Box::new(CancelDuringVerification), Duration::from_secs(3));
        assert!(matches!(
            f.call(&mut runner, 1),
            Err(RunnerError::Interrupted)
        ));
        f.assert_pending();
        assert!(!fs::read_dir(&f.roots.state).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".quiescent")
        }));
        let mut recreated = f.runner(positive(), Duration::from_secs(3));
        assert_eq!(
            error(f.call(&mut recreated, 2)),
            RunnerError::OutcomeUnknown
        );
    }

    #[test]
    fn cancelled_before_dispatch_does_not_publish_or_spawn() {
        let f = Fixture::new("exit 0");
        let mut runner = f.runner(positive(), Duration::from_secs(3));
        runner.cancelled.store(true, Ordering::Release);
        assert_eq!(error(f.call(&mut runner, 1)), RunnerError::Interrupted);
        assert_eq!(fs::read_dir(&f.roots.workspace).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&f.roots.state).unwrap().count(), 0);
    }

    #[test]
    fn original_instant_budget_is_not_reset_and_elapsed_includes_prior_admission() {
        let f = Fixture::new("exit 0");
        let mut runner = f.runner(positive(), Duration::from_secs(3));
        runner.original_start = Instant::now() - Duration::from_millis(100);
        assert!(f.call(&mut runner, 1).unwrap().elapsed >= Duration::from_millis(100));
        let mut expired = f.runner(positive(), Duration::from_millis(1));
        expired.original_start = Instant::now() - Duration::from_secs(1);
        assert_eq!(error(f.call(&mut expired, 2)), RunnerError::Interrupted);
        assert_eq!(fs::read_dir(&f.roots.workspace).unwrap().count(), 1);
    }
}
