//! Private native llama.cpp runner. The service app worker thread (or the
//! installer's orchestration thread) owns at most one child at a time.
//!
//! Trust notes: the curated catalog is compiled in; model and runtime files are
//! checked through no-follow descriptors. The per-runner API key lives only in
//! the child environment and is weak against same-UID processes; the private
//! 0700 socket directory and `SO_PEERCRED` pid checks are the real boundary.
use crate::acceleration::{self, ActualMode, Backend};
use crate::app::{new_id, AppError, AppResult, Message, MAX_CONTEXT};
pub use crate::catalog::{CatalogEntry, CATALOG, DEFAULT_MODEL};
use crate::filesystem::{self, uid};
use crate::{Result, Roots};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MODEL_LIMIT: usize = 64;
/// Production context window and generation cap (architecture amendment 1).
pub const CONTEXT: u32 = 4096;
pub const MAX_TOKENS: u32 = 1024;
/// One end-to-end service budget from dispatch (amendment 2).
pub const BUDGET: Duration = Duration::from_secs(180);
pub const IDLE: Duration = Duration::from_secs(600);
const MAX_MESSAGES: usize = 33;
const TOKEN_MARGIN: u32 = 32;
const SCAN_LIMIT: usize = 1024;
const MARKER_LIMIT: u64 = 4096;
const MANIFEST: &str = ".smith-next-manifest.json";
const MANIFEST_LIMIT: u64 = 4 * 1024 * 1024;
const RUNTIME_FILES: usize = 64;
const HEADER_LIMIT: usize = 16 * 1024;
const BODY_LIMIT: usize = 1024 * 1024;
const WIRE_LIMIT: usize = 2 * BODY_LIMIT + HEADER_LIMIT;
const STDERR_LIMIT: usize = 16 * 1024;
const EVIDENCE_LIMIT: usize = acceleration::MAX_EVIDENCE_BYTES + 1;
const SOCKET_PATH_LIMIT: usize = 107;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRecord {
    pub name: String,
    pub sha256: String,
    pub size: u64,
    pub license: String,
    pub source: String,
}

/// Field order is the canonical sorted key order.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    file: String,
    license: String,
    name: String,
    schema_version: u32,
    sha256: String,
    size: u64,
    source: String,
}

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}
fn coded(e: AppError) -> String {
    format!("{}: {}", e.code, e.message)
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn valid_hex32(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub fn lock_path(roots: &Roots) -> PathBuf {
    roots.runtime.join("smith-next/runner.lock")
}
pub fn runner_parent(roots: &Roots) -> PathBuf {
    roots.runtime.join("smith-next")
}
pub fn models_dir(roots: &Roots) -> PathBuf {
    roots.data.join("smith-next/models")
}

/// Canonical compact sorted marker JSON plus newline.
pub fn marker_bytes(record: &ModelRecord) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&Marker {
        file: format!("{}.gguf", record.sha256),
        license: record.license.clone(),
        name: record.name.clone(),
        schema_version: 1,
        sha256: record.sha256.clone(),
        size: record.size,
        source: record.source.clone(),
    })
    .expect("marker contains only strings and integers");
    bytes.push(b'\n');
    bytes
}

/// Installed curated models: canonical markers, exact catalog matches and
/// cheap file facts. Hashing happens once per runner start in `verify_model`.
pub fn list_models(dir: &Path) -> Result<Vec<ModelRecord>> {
    list_models_with(dir, CATALOG)
}

/// Rust-only catalog injection for isolated tests; production uses `CATALOG`.
pub fn list_models_with(dir: &Path, catalog: &[CatalogEntry]) -> Result<Vec<ModelRecord>> {
    filesystem::check_ancestors(dir)?;
    match fs::symlink_metadata(dir) {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err("Cannot inspect model directory".into()),
    }
    filesystem::check_directory(dir, true)?;
    let mut markers = Vec::new();
    for (i, entry) in fs::read_dir(dir)
        .map_err(|_| "Cannot list model directory")?
        .enumerate()
    {
        if i >= SCAN_LIMIT {
            return Err("Model directory scan limit exceeded".into());
        }
        let entry = entry.map_err(|_| "Cannot list model directory")?;
        // Dotfiles include installer staging files; non-UTF-8 names are never ours.
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if let Some(sha) = name.strip_suffix(".json") {
            if valid_sha(sha) {
                markers.push(sha.to_string());
            }
        }
    }
    markers.sort();
    let mut parsed = Vec::new();
    let mut names = HashSet::new();
    for sha in markers {
        let bytes = filesystem::read_file(&dir.join(format!("{sha}.json")), 0o600, MARKER_LIMIT)?
            .ok_or("Model marker disappeared")?;
        let marker: Marker =
            serde_json::from_slice(&bytes).map_err(|_| "Model marker is invalid")?;
        let record = ModelRecord {
            name: marker.name,
            sha256: marker.sha256,
            size: marker.size,
            license: marker.license,
            source: marker.source,
        };
        if marker.schema_version != 1
            || record.sha256 != sha
            || marker.file != format!("{sha}.gguf")
            || marker_bytes(&record) != bytes
        {
            return Err("Model marker is not canonical".into());
        }
        // Ambiguity is checked across all markers, including non-catalog ones.
        if !names.insert(record.name.clone()) {
            return Err("Duplicate installed model name".into());
        }
        parsed.push(record);
    }
    let mut records = Vec::new();
    for record in parsed {
        if !catalog.iter().any(|c| c.matches(&record)) {
            continue;
        }
        if records.len() == MODEL_LIMIT {
            return Err("Installed model limit exceeded".into());
        }
        model_file(dir, &record)?;
        records.push(record);
    }
    records.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(records)
}

fn model_file(dir: &Path, record: &ModelRecord) -> Result<(File, Metadata, PathBuf)> {
    if !valid_sha(&record.sha256) {
        return Err("Invalid model identity".into());
    }
    let path = dir.join(format!("{}.gguf", record.sha256));
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|_| "Model file missing or unsafe")?;
    let m = file.metadata().map_err(|_| "Cannot inspect model file")?;
    if !m.is_file()
        || m.uid() != uid()
        || m.mode() & 0o7777 != 0o600
        || m.nlink() != 1
        || m.len() != record.size
    {
        return Err("Unsafe model file type, ownership, permissions, links or size".into());
    }
    Ok((file, m, path))
}

fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
}

fn hash_file(
    file: &mut File,
    limit: u64,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1024 * 1024];
    let mut total = 0u64;
    loop {
        verification_budget(deadline, cancelled)?;
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err("Cannot read file for verification".into()),
        };
        total += n as u64;
        if total > limit {
            return Err("File grew during verification".into());
        }
        hasher.update(&buf[..n]);
    }
    verification_budget(deadline, cancelled)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Full SHA-256 through a held no-follow descriptor; file identity must be
/// unchanged before/after and the path must still name the hashed inode.
pub fn verify_model(dir: &Path, record: &ModelRecord) -> Result<PathBuf> {
    let held = verify_model_held(dir, record, Instant::now() + BUDGET, &|| false)?;
    Ok(held.path.clone())
}

pub type Cancellation = Arc<dyn Fn() -> bool + Send + Sync>;

fn verification_budget(deadline: Instant, cancelled: &dyn Fn() -> bool) -> Result<()> {
    if cancelled() {
        return Err(coded(Http::Cancelled.app()));
    }
    remaining(deadline).map(|_| ()).map_err(coded)
}

/// Keeps the hashed inode alive through child exec. This prevents pathname
/// replacement, not writes by a hostile same-UID process to the held inode.
pub struct VerifiedModel {
    file: File,
    path: PathBuf,
    sha256: String,
    lease: Option<(PathBuf, Arc<Mutex<Option<File>>>)>,
}
impl Drop for VerifiedModel {
    fn drop(&mut self) {
        if let Some((_, lease)) = self.lease.as_ref() {
            if let Ok(mut slot) = lease.lock() {
                if let Some(file) = slot.take() {
                    // SAFETY: the retained lease descriptor is owned by this
                    // verified model and no runner can hold it while it is in the slot.
                    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
                }
            }
        }
    }
}

/// Acquire the shared runner lease before inspecting or hashing model bytes.
/// Installer direct inference must use this API, then launch_verified/start_verified.
pub fn verify_model_leased(
    roots: &Roots,
    record: &ModelRecord,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<VerifiedModel> {
    verification_budget(deadline, cancelled)?;
    filesystem::mkdir(&runner_parent(roots))?;
    filesystem::check_directory(&runner_parent(roots), true)?;
    let path = lock_path(roots);
    let lease = acquire_lock(&path).map_err(coded)?;
    let mut model = verify_model_held(&models_dir(roots), record, deadline, cancelled)?;
    model.lease = Some((path, Arc::new(Mutex::new(Some(lease)))));
    Ok(model)
}

/// Legacy held-descriptor verification without a preverification lease.
/// Production service and installer inference must use `verify_model_leased`.
/// Launching this legacy wrapper obtains the runner lease at launch time.
pub fn verify_model_held(
    dir: &Path,
    record: &ModelRecord,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<VerifiedModel> {
    verification_budget(deadline, cancelled)?;
    filesystem::check_directory(dir, true)?;
    let (mut file, before, path) = model_file(dir, record)?;
    let digest = hash_file(&mut file, record.size, deadline, cancelled)?;
    let after = file.metadata().map_err(|_| "Cannot inspect model file")?;
    let named = fs::symlink_metadata(&path).map_err(|_| "Model file disappeared")?;
    if !same_file(&before, &after) || !same_file(&before, &named) {
        return Err("Model file changed during verification".into());
    }
    if digest != record.sha256 {
        return Err("Model file integrity verification failed".into());
    }
    Ok(VerifiedModel {
        file,
        path,
        sha256: digest,
        lease: None,
    })
}

/// Remove only an exact catalog-bound marker and GGUF, under both shared locks.
/// A failure after marker unlink preserves an orphan; no rollback or recursive cleanup.
pub fn remove_model_with(
    roots: &Roots,
    model: &str,
    sha256: &str,
    catalog: &[CatalogEntry],
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> AppResult<()> {
    verification_budget(deadline, cancelled).map_err(|_| {
        error(
            "REMOVAL_STOPPED",
            "Removal cancelled or its observation budget elapsed; model preserved",
        )
    })?;
    let record = catalog
        .iter()
        .find(|c| c.name == model && c.sha256 == sha256)
        .ok_or_else(|| {
            error(
                "MODEL_UNAVAILABLE",
                "Exact catalog model identity is unavailable",
            )
        })?
        .record();
    let unsafe_store = || {
        error(
            "MODEL_STORE_UNSAFE",
            "Model store changed or is unsafe; preserved",
        )
    };
    filesystem::check_directory(&roots.data, false).map_err(|_| unsafe_store())?;
    let install_path = roots.data.join(".smith-next-install.lock");
    let install = acquire_lock(&install_path).map_err(|e| {
        if e.code == "RUNNER_BUSY" {
            error(
                "INSTALL_BUSY",
                "Installation owns the model store; no wait or retry",
            )
        } else {
            unsafe_store()
        }
    })?;
    filesystem::mkdir(&runner_parent(roots)).map_err(|_| unsafe_store())?;
    filesystem::check_directory(&runner_parent(roots), true).map_err(|_| unsafe_store())?;
    let _lease = acquire_lock(&lock_path(roots))?;
    let path = models_dir(roots);
    filesystem::check_directory(&path, true).map_err(|_| unsafe_store())?;
    let dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|_| unsafe_store())?;
    let directory = dir.metadata().map_err(|_| unsafe_store())?;
    if !directory.is_dir() || directory.uid() != uid() || directory.mode() & 0o7777 != 0o700 {
        return Err(unsafe_store());
    }
    let marker_name = format!("{sha256}.json");
    let model_name = format!("{sha256}.gguf");
    let mut marker = removal_open(&dir, &marker_name).map_err(|_| unsafe_store())?;
    let marker_meta =
        removal_file(&marker, marker_bytes(&record).len() as u64).map_err(|_| unsafe_store())?;
    let mut bytes = Vec::new();
    (&mut marker)
        .take(MARKER_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unsafe_store())?;
    if bytes != marker_bytes(&record) {
        return Err(unsafe_store());
    }
    let mut file = removal_open(&dir, &model_name).map_err(|_| unsafe_store())?;
    let model_meta = removal_file(&file, record.size).map_err(|_| unsafe_store())?;
    if hash_file(&mut file, record.size, deadline, cancelled).map_err(|message| {
        if message.starts_with("RUNNER_CANCELLED:") || message.starts_with("PROVIDER_TIMEOUT:") {
            error(
                "REMOVAL_STOPPED",
                "Removal cancelled or verification budget elapsed; model preserved",
            )
        } else {
            unsafe_store()
        }
    })? != sha256
    {
        return Err(unsafe_store());
    }
    let revalidate = || -> AppResult<()> {
        verification_budget(deadline, cancelled).map_err(|_| {
            error(
                "REMOVAL_STOPPED",
                "Removal stopped; any earlier unlink remains",
            )
        })?;
        filesystem::check_directory(&path, true).map_err(|_| unsafe_store())?;
        let named_dir = fs::symlink_metadata(&path).map_err(|_| unsafe_store())?;
        let named_lock = fs::symlink_metadata(&install_path).map_err(|_| unsafe_store())?;
        let lock_meta = install.metadata().map_err(|_| unsafe_store())?;
        if named_dir.dev() != directory.dev()
            || named_dir.ino() != directory.ino()
            || !same_file(&lock_meta, &named_lock)
            || named_lock.nlink() != 1
            || named_lock.mode() & 0o7777 != 0o600
        {
            return Err(unsafe_store());
        }
        let current = removal_open(&dir, &model_name).map_err(|_| unsafe_store())?;
        let m = removal_file(&current, record.size).map_err(|_| unsafe_store())?;
        if !same_file(&model_meta, &m) {
            return Err(unsafe_store());
        }
        Ok(())
    };
    revalidate()?;
    let current = removal_open(&dir, &marker_name).map_err(|_| unsafe_store())?;
    let m = removal_file(&current, marker_meta.len()).map_err(|_| unsafe_store())?;
    if !same_file(&marker_meta, &m) {
        return Err(unsafe_store());
    }
    removal_unlink(&dir, &marker_name)
        .map_err(|_| error("REMOVE_FAILED", "Marker removal failed; model preserved"))?;
    let partial = || {
        error(
            "REMOVE_PARTIAL",
            "Marker unlinked; model file may remain or directory sync is uncertain; no rollback",
        )
    };
    dir.sync_all().map_err(|_| partial())?;
    revalidate().map_err(|_| partial())?;
    removal_unlink(&dir, &model_name).map_err(|_| partial())?;
    dir.sync_all().map_err(|_| partial())?;
    Ok(())
}

fn removal_open(dir: &File, name: &str) -> Result<File> {
    let name = std::ffi::CString::new(name).map_err(|_| "Invalid removal filename")?;
    // SAFETY: directory descriptor and nul-terminated filename stay alive.
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("Cannot open removal file".into());
    }
    // SAFETY: openat returned a fresh owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn removal_file(file: &File, size: u64) -> Result<Metadata> {
    let m = file.metadata().map_err(|_| "Cannot inspect removal file")?;
    if !m.is_file()
        || m.uid() != uid()
        || m.mode() & 0o7777 != 0o600
        || m.nlink() != 1
        || m.len() != size
    {
        return Err("Unsafe removal file".into());
    }
    Ok(m)
}
fn removal_unlink(dir: &File, name: &str) -> Result<()> {
    let name = std::ffi::CString::new(name).map_err(|_| "Invalid removal filename")?;
    // SAFETY: descriptor and filename stay alive; flags prohibit directory removal.
    if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), 0) } != 0 {
        return Err("Cannot unlink model entry".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseManifest {
    entries: BTreeMap<String, ReleaseEntry>,
    schema: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseEntry {
    mode: u32,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(rename = "type")]
    kind: String,
}

fn verification_error(message: &str) -> AppError {
    if message.starts_with("RUNNER_CANCELLED:") {
        Http::Cancelled.app()
    } else if message.starts_with("PROVIDER_TIMEOUT:") {
        timeout()
    } else {
        unsafe_runtime("Cannot verify runtime file")
    }
}

fn missing(message: &str) -> AppError {
    error("RUNTIME_MISSING", message)
}
fn unsafe_runtime(message: &str) -> AppError {
    error("RUNTIME_UNSAFE", message)
}

/// Owner directory below the Smith data namespace: no symlink, no group/world write.
fn owned_dir(path: &Path) -> AppResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() && m.uid() == uid() && m.mode() & 0o022 == 0 => Ok(true),
        Ok(_) => Err(unsafe_runtime("Unsafe runtime directory")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(unsafe_runtime("Cannot inspect runtime directory")),
    }
}

/// Resolve `client/current` once, then verify every runtime file in the
/// selected immutable release against its content-addressed manifest.
pub fn verify_runtime(roots: &Roots) -> AppResult<PathBuf> {
    verify_runtime_bounded(roots, Instant::now() + BUDGET, &|| false)
}

pub fn verify_runtime_bounded(
    roots: &Roots,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> AppResult<PathBuf> {
    verify_runtime_backend_bounded(roots, Backend::Cpu, deadline, cancelled)
}

pub fn verify_runtime_backend_bounded(
    roots: &Roots,
    backend: Backend,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> AppResult<PathBuf> {
    verification_budget(deadline, cancelled).map_err(|e| verification_error(&e))?;
    let base = roots.data.join("smith-next");
    let client = base.join("client");
    filesystem::check_ancestors(&client).map_err(|_| unsafe_runtime("Unsafe client path"))?;
    for dir in [&base, &client] {
        if !owned_dir(dir)? {
            return Err(missing(
                "No client release with a bundled runtime is installed",
            ));
        }
    }
    let current = client.join("current");
    let before = match fs::symlink_metadata(&current) {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(missing("No client release is selected"))
        }
        Err(_) => return Err(unsafe_runtime("Cannot inspect client selector")),
    };
    if !before.file_type().is_symlink() || before.uid() != uid() || before.nlink() != 1 {
        return Err(unsafe_runtime("Unmanaged client selector"));
    }
    let target = fs::read_link(&current).map_err(|_| unsafe_runtime("Cannot read selector"))?;
    let after = fs::symlink_metadata(&current).map_err(|_| unsafe_runtime("Selector changed"))?;
    if before.dev() != after.dev() || before.ino() != after.ino() {
        return Err(unsafe_runtime("Client selector changed during resolution"));
    }
    let release_id = target
        .to_str()
        .and_then(|t| t.strip_prefix("releases/"))
        .filter(|id| valid_sha(id))
        .ok_or_else(|| unsafe_runtime("Unmanaged client selector target"))?
        .to_string();
    let releases = client.join("releases");
    let release = releases.join(&release_id);
    let runtime = release.join("smith_backend/runtime");
    let backend_name = match backend {
        Backend::Cpu => "cpu",
        Backend::Vulkan => "vulkan",
    };
    let selected = runtime.join(backend_name);
    for dir in [&releases, &release] {
        if !owned_dir(dir)? {
            return Err(unsafe_runtime("Selected client release is missing"));
        }
    }
    for dir in [
        release.join("smith_backend"),
        runtime.clone(),
        selected.clone(),
    ] {
        if !owned_dir(&dir)? {
            return Err(missing(
                "Selected client release has no bundled runtime backend",
            ));
        }
    }
    let bytes = filesystem::read_file(&release.join(MANIFEST), 0o600, MANIFEST_LIMIT)
        .map_err(|_| unsafe_runtime("Unsafe release manifest"))?
        .ok_or_else(|| unsafe_runtime("Release has no managed manifest"))?;
    if filesystem::digest(&bytes) != release_id {
        return Err(unsafe_runtime("Release manifest identity mismatch"));
    }
    let manifest: ReleaseManifest =
        serde_json::from_slice(&bytes).map_err(|_| unsafe_runtime("Invalid release manifest"))?;
    if manifest.schema != 1 {
        return Err(unsafe_runtime("Unsupported release manifest"));
    }
    for directory in [
        "smith_backend/runtime".to_string(),
        format!("smith_backend/runtime/{backend_name}"),
    ] {
        match manifest.entries.get(&directory) {
            Some(e) if e.kind == "directory" && e.sha256.is_none() => {}
            Some(_) => return Err(unsafe_runtime("Invalid runtime manifest entry")),
            None => {
                return Err(missing(
                    "Selected client release has no bundled runtime backend",
                ))
            }
        }
    }
    let prefix = format!("smith_backend/runtime/{backend_name}/");
    let mut expected = BTreeMap::new();
    for (path, entry) in &manifest.entries {
        let Some(name) = path.strip_prefix(&prefix) else {
            continue;
        };
        let sha = entry.sha256.as_deref().unwrap_or_default();
        if name.is_empty()
            || name.contains('/')
            || entry.kind != "file"
            || !valid_sha(sha)
            || ![0o644, 0o755].contains(&entry.mode)
        {
            return Err(unsafe_runtime("Invalid runtime manifest entry"));
        }
        expected.insert(name.to_string(), (entry.mode, sha.to_string()));
    }
    if expected.len() > RUNTIME_FILES {
        return Err(unsafe_runtime("Runtime file limit exceeded"));
    }
    match expected.get("llama-server") {
        Some((0o755, _)) => {}
        Some(_) => return Err(unsafe_runtime("Runtime executable mode mismatch")),
        None => return Err(missing("Selected client release has no runner executable")),
    }
    let mut present = BTreeSet::new();
    for (i, entry) in fs::read_dir(&selected)
        .map_err(|_| unsafe_runtime("Cannot list runtime"))?
        .enumerate()
    {
        if i > RUNTIME_FILES {
            return Err(unsafe_runtime("Runtime file limit exceeded"));
        }
        let entry = entry.map_err(|_| unsafe_runtime("Cannot list runtime"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| unsafe_runtime("Unexpected runtime file"))?;
        present.insert(name);
    }
    if present.len() != expected.len() || !present.iter().all(|n| expected.contains_key(n)) {
        return Err(unsafe_runtime(
            "Runtime files differ from the release manifest",
        ));
    }
    for (name, (mode, sha)) in &expected {
        let path = selected.join(name);
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| unsafe_runtime("Runtime file missing or unsafe"))?;
        let before = file
            .metadata()
            .map_err(|_| unsafe_runtime("Cannot inspect runtime"))?;
        if !before.is_file()
            || before.uid() != uid()
            || before.mode() & 0o7777 != *mode
            || before.nlink() != 1
            || before.len() > filesystem::MAX_FILE
        {
            return Err(unsafe_runtime(
                "Unsafe runtime file type, ownership, mode, links or size",
            ));
        }
        let digest = hash_file(&mut file, filesystem::MAX_FILE, deadline, cancelled)
            .map_err(|e| verification_error(&e))?;
        let after = file
            .metadata()
            .map_err(|_| unsafe_runtime("Cannot inspect runtime"))?;
        if !same_file(&before, &after) || &digest != sha {
            return Err(unsafe_runtime(
                "Runtime file differs from the release manifest",
            ));
        }
    }
    Ok(selected.join("llama-server"))
}

/// Frozen string-error form of [`verify_runtime`]; errors are `CODE: message`.
pub fn runtime_executable(roots: &Roots) -> Result<PathBuf> {
    verify_runtime(roots).map_err(coded)
}

pub fn default_threads() -> u32 {
    let n = std::thread::available_parallelism().map_or(2, |n| n.get());
    (n / 2).clamp(1, 8) as u32
}

#[derive(Clone, Debug)]
pub struct RunnerConfig {
    pub executable: PathBuf,
    pub runtime_parent: PathBuf,
    pub lock: PathBuf,
    pub context: u32,
    pub threads: u32,
    pub backend: Backend,
}
impl RunnerConfig {
    pub fn production(roots: &Roots, executable: PathBuf) -> Self {
        Self {
            executable,
            runtime_parent: runner_parent(roots),
            lock: lock_path(roots),
            context: CONTEXT,
            threads: default_threads(),
            backend: Backend::Cpu,
        }
    }
}

fn parent_death_guard(expected_parent: libc::pid_t) -> std::io::Result<()> {
    // SAFETY: prctl/getppid have no Rust memory dependencies and are safe after
    // fork. Check AFTER arming to close the fork-to-prctl parent-death race.
    // PDEATHSIG follows the spawning thread: only the long-lived worker spawns.
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

fn remaining(deadline: Instant) -> AppResult<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(timeout)
}
fn timeout() -> AppError {
    error(
        "PROVIDER_TIMEOUT",
        "Local runner budget exhausted; runner stopped, no retry",
    )
}
fn unavailable(message: &str) -> AppError {
    error("LLM_UNAVAILABLE", message)
}
fn protocol(message: &str) -> AppError {
    error("PROVIDER_PROTOCOL", message)
}

fn acquire_lock(path: &Path) -> AppResult<File> {
    filesystem::check_ancestors(path).map_err(|_| unavailable("Unsafe runner lock path"))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| unavailable("Cannot open runner lock"))?;
    let m = file
        .metadata()
        .map_err(|_| unavailable("Cannot inspect runner lock"))?;
    if !m.is_file() || m.uid() != uid() || m.mode() & 0o7777 != 0o600 || m.nlink() != 1 {
        return Err(unavailable("Unsafe runner lock"));
    }
    // SAFETY: flock operates on our open descriptor, which outlives the lock.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(error(
            "RUNNER_BUSY",
            "Another local runner is active; no wait or retry",
        ));
    }
    Ok(file)
}

/// Remove leftovers of crashed runners. Caller must hold the runner lock.
fn cleanup_stale(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.take(256).flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.strip_prefix("runner-").is_some_and(valid_hex32) {
            continue;
        }
        let _ = remove_runner_dir(&parent.join(name), true);
    }
}

fn remove_runner_dir(dir: &Path, probe: bool) -> Result<()> {
    let before = fs::symlink_metadata(dir).map_err(|_| "Runner directory missing")?;
    if !before.is_dir() || before.uid() != uid() || before.mode() & 0o7777 != 0o700 {
        return Err("Unsafe runner directory preserved".into());
    }
    let names: Vec<_> = fs::read_dir(dir)
        .map_err(|_| "Cannot list runner directory")?
        .take(4)
        .collect::<std::io::Result<_>>()
        .map_err(|_| "Cannot list runner directory")?;
    if names.iter().any(|e| e.file_name() != "llama.sock") {
        return Err("Unexpected runner directory content preserved".into());
    }
    let socket = dir.join("llama.sock");
    if let Ok(s) = fs::symlink_metadata(&socket) {
        if !s.file_type().is_socket() || s.uid() != uid() {
            return Err("Unexpected runner socket preserved".into());
        }
        if probe {
            match connect_socket(&socket) {
                Err(e) if e.kind() == ErrorKind::ConnectionRefused => {}
                _ => return Err("Live or unknown runner socket preserved".into()),
            }
        }
        let again = fs::symlink_metadata(&socket).map_err(|_| "Runner socket changed")?;
        if again.dev() != s.dev() || again.ino() != s.ino() {
            return Err("Runner socket changed".into());
        }
        fs::remove_file(&socket).map_err(|_| "Cannot remove runner socket")?;
    }
    let again = fs::symlink_metadata(dir).map_err(|_| "Runner directory changed")?;
    if again.dev() != before.dev() || again.ino() != before.ino() {
        return Err("Runner directory changed".into());
    }
    fs::remove_dir(dir).map_err(|_| "Cannot remove runner directory".into())
}

fn connect_socket(path: &Path) -> std::io::Result<UnixStream> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() > SOCKET_PATH_LIMIT || bytes.contains(&0) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Invalid runner socket path",
        ));
    }
    // SAFETY: address is initialized, fd is uniquely owned, and the socket is
    // nonblocking so a saturated listen backlog cannot stall cancellation.
    unsafe {
        let raw = libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        );
        if raw < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let stream = UnixStream::from_raw_fd(raw);
        let mut address: libc::sockaddr_un = std::mem::zeroed();
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (dest, byte) in address.sun_path.iter_mut().zip(bytes) {
            *dest = *byte as libc::c_char;
        }
        if libc::connect(
            raw,
            (&address as *const libc::sockaddr_un).cast(),
            std::mem::size_of_val(&address) as libc::socklen_t,
        ) != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        stream.set_nonblocking(false)?;
        Ok(stream)
    }
}

/// Leader exit without reaping: the pid stays reserved, so the process group
/// cannot be recycled before the final group SIGKILL.
fn exited(pid: i32) -> bool {
    // SAFETY: zeroed siginfo is valid output storage for waitid.
    unsafe {
        let mut info: libc::siginfo_t = std::mem::zeroed();
        let rc = libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        );
        rc != 0 || info.si_pid() != 0
    }
}
fn wait_exit(pid: i32, budget: Duration) -> bool {
    let end = Instant::now() + budget;
    loop {
        if exited(pid) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn signal_group(pid: i32, signal: libc::c_int) {
    // SAFETY: kill with a negative pid targets our child's own process group.
    unsafe {
        libc::kill(-pid, signal);
    }
}

#[derive(Debug)]
enum Http {
    Connect,
    Unsafe,
    Timeout,
    Io,
    Limit,
    Protocol,
    Cancelled,
}
impl Http {
    fn app(self) -> AppError {
        match self {
            Http::Connect | Http::Io => unavailable("Local runner connection failed; no retry"),
            Http::Unsafe => error(
                "RUNNER_UNSAFE",
                "Local runner socket identity mismatch; runner stopped",
            ),
            Http::Timeout => timeout(),
            Http::Limit => error("PROVIDER_BODY_LIMIT", "Local runner response exceeds limit"),
            Http::Protocol => protocol("Invalid local runner HTTP response"),
            Http::Cancelled => error(
                "RUNNER_CANCELLED",
                "Local runner cancelled; accepted work is not retried",
            ),
        }
    }
}

struct Wire<'a> {
    stream: &'a mut UnixStream,
    buf: Vec<u8>,
    start: usize,
    total: usize,
    deadline: Instant,
    cancelled: &'a dyn Fn() -> bool,
}
impl Wire<'_> {
    fn fill(&mut self) -> std::result::Result<bool, Http> {
        if (self.cancelled)() {
            return Err(Http::Cancelled);
        }
        let left = remaining(self.deadline).map_err(|_| Http::Timeout)?;
        self.stream
            .set_read_timeout(Some(left.min(Duration::from_millis(100))))
            .map_err(|_| Http::Io)?;
        let mut chunk = [0; 16384];
        match self.stream.read(&mut chunk) {
            Ok(0) => Ok(false),
            Ok(n) => {
                self.total += n;
                if self.total > WIRE_LIMIT {
                    return Err(Http::Limit);
                }
                if self.start > 64 * 1024 {
                    self.buf.drain(..self.start);
                    self.start = 0;
                }
                self.buf.extend_from_slice(&chunk[..n]);
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => Ok(true),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if (self.cancelled)() {
                    Err(Http::Cancelled)
                } else if Instant::now() >= self.deadline {
                    Err(Http::Timeout)
                } else {
                    Ok(true)
                }
            }
            Err(_) => Err(Http::Io),
        }
    }
    fn line(&mut self, limit: usize) -> std::result::Result<Vec<u8>, Http> {
        loop {
            if let Some(i) = self.buf[self.start..].windows(2).position(|w| w == b"\r\n") {
                if i > limit {
                    return Err(Http::Limit);
                }
                let line = self.buf[self.start..self.start + i].to_vec();
                self.start += i + 2;
                return Ok(line);
            }
            if self.buf.len() - self.start > limit {
                return Err(Http::Limit);
            }
            if !self.fill()? {
                return Err(Http::Protocol);
            }
        }
    }
    fn take(&mut self, n: usize) -> std::result::Result<Vec<u8>, Http> {
        while self.buf.len() - self.start < n {
            if !self.fill()? {
                return Err(Http::Protocol);
            }
        }
        let out = self.buf[self.start..self.start + n].to_vec();
        self.start += n;
        Ok(out)
    }
    fn rest(&mut self) -> std::result::Result<Vec<u8>, Http> {
        while self.fill()? {
            if self.buf.len() - self.start > BODY_LIMIT {
                return Err(Http::Limit);
            }
        }
        if self.buf.len() - self.start > BODY_LIMIT {
            return Err(Http::Limit);
        }
        Ok(self.buf[self.start..].to_vec())
    }
}

/// Minimal bounded HTTP/1.1 response reader (Content-Length, chunked or EOF).
#[cfg(test)]
fn read_response(
    stream: &mut UnixStream,
    deadline: Instant,
) -> std::result::Result<(u16, Vec<u8>), Http> {
    read_response_cancelled(stream, deadline, &|| false)
}
fn read_response_cancelled(
    stream: &mut UnixStream,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> std::result::Result<(u16, Vec<u8>), Http> {
    let mut wire = Wire {
        stream,
        buf: Vec::new(),
        start: 0,
        total: 0,
        deadline,
        cancelled,
    };
    let status = wire.line(HEADER_LIMIT)?;
    let status = std::str::from_utf8(&status).map_err(|_| Http::Protocol)?;
    let code = status
        .strip_prefix("HTTP/1.1 ")
        .or_else(|| status.strip_prefix("HTTP/1.0 "))
        .and_then(|rest| rest.get(..3))
        .filter(|c| c.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or(Http::Protocol)?;
    let mut length = None;
    let mut chunked = false;
    let mut header_bytes = status.len();
    loop {
        let line = wire.line(HEADER_LIMIT)?;
        header_bytes += line.len() + 2;
        if header_bytes > HEADER_LIMIT {
            return Err(Http::Limit);
        }
        if line.is_empty() {
            break;
        }
        let line = std::str::from_utf8(&line).map_err(|_| Http::Protocol)?;
        let (name, value) = line.split_once(':').ok_or(Http::Protocol)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Http::Protocol);
            }
            let n: usize = value.parse().map_err(|_| Http::Limit)?;
            if n > BODY_LIMIT {
                return Err(Http::Limit);
            }
            length = Some(n);
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !value.eq_ignore_ascii_case("chunked") {
                return Err(Http::Protocol);
            }
            chunked = true;
        }
    }
    let body = match (length, chunked) {
        (Some(_), true) => return Err(Http::Protocol),
        (Some(n), false) => wire.take(n)?,
        (None, true) => {
            let mut body = Vec::new();
            loop {
                let line = wire.line(1024)?;
                let line = std::str::from_utf8(&line).map_err(|_| Http::Protocol)?;
                let size = line.split(';').next().unwrap_or_default().trim();
                if size.is_empty() || size.len() > 8 {
                    return Err(Http::Protocol);
                }
                let size = usize::from_str_radix(size, 16).map_err(|_| Http::Protocol)?;
                if size == 0 {
                    for _ in 0..32 {
                        if wire.line(1024)?.is_empty() {
                            return Ok((code, body));
                        }
                    }
                    return Err(Http::Limit);
                }
                if body.len() + size > BODY_LIMIT {
                    return Err(Http::Limit);
                }
                body.extend(wire.take(size)?);
                if !wire.line(2)?.is_empty() {
                    return Err(Http::Protocol);
                }
            }
        }
        (None, false) => wire.rest()?,
    };
    Ok((code, body))
}

/// Validated content-only completion; reasoning is ignored and tools rejected.
fn chat_result(alias: &str, v: &Value) -> AppResult<String> {
    if v.get("model").and_then(Value::as_str) != Some(alias) {
        return Err(protocol("Local runner model identity mismatch"));
    }
    let choices = v
        .get("choices")
        .and_then(Value::as_array)
        .filter(|c| c.len() == 1)
        .ok_or_else(|| protocol("Local runner returned no single choice"))?;
    let choice = &choices[0];
    let message = choice
        .get("message")
        .filter(|m| m.is_object())
        .ok_or_else(|| protocol("Local runner returned no message"))?;
    let tools = [message.get("tool_calls"), choice.get("tool_calls")]
        .into_iter()
        .flatten()
        .any(|t| !t.is_null() && t.as_array().is_none_or(|a| !a.is_empty()));
    if tools
        || v.get("error").is_some_and(|e| !e.is_null())
        || choice.get("index").and_then(Value::as_u64) != Some(0)
        || !matches!(
            choice.get("finish_reason").and_then(Value::as_str),
            Some("stop" | "length")
        )
        || message.get("function_call").is_some_and(|f| !f.is_null())
        || choice.get("finish_reason").and_then(Value::as_str) == Some("tool_calls")
        || message.get("role").and_then(Value::as_str) != Some("assistant")
    {
        return Err(protocol("Tool-free assistant response rejected"));
    }
    if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
        return Err(error(
            "MODEL_TRUNCATED",
            "Generation reached its token cap; final answer is not verified",
        ));
    }
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol("Missing assistant text"))?;
    if content.trim().is_empty() || content.len() > MAX_CONTEXT {
        return Err(error(
            "MODEL_RESULT_LIMIT",
            "Assistant text is empty or exceeds bounds",
        ));
    }
    Ok(content.into())
}

fn check_messages(messages: &[Message]) -> AppResult<()> {
    let alternating = messages
        .iter()
        .enumerate()
        .all(|(i, m)| m.role == if i % 2 == 0 { "user" } else { "assistant" });
    if messages.is_empty()
        || messages.len() > MAX_MESSAGES
        || messages.len() % 2 == 0
        || !alternating
        || messages.iter().map(|m| m.content.len()).sum::<usize>() > MAX_CONTEXT
    {
        return Err(error("CONTEXT_LIMIT", "Chat context exceeds bounds"));
    }
    Ok(())
}

/// Byte-token upper estimate with template overhead when tokenization is unavailable.
fn estimate_tokens(messages: &[Message]) -> u32 {
    let bytes: usize = messages.iter().map(|m| m.content.len() + 64).sum();
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

#[derive(Default)]
struct StartupEvidence {
    bytes: Vec<u8>,
    stream: EvidenceStream,
}

#[derive(Default)]
enum EvidenceStream {
    #[default]
    Open,
    Closed,
    Failed,
}

impl StartupEvidence {
    fn extend(&mut self, chunk: &[u8]) {
        let keep = EVIDENCE_LIMIT
            .saturating_sub(self.bytes.len())
            .min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..keep]);
    }

    fn actual_mode(&self) -> AppResult<ActualMode> {
        let message = match self.stream {
            EvidenceStream::Closed => Some("Vulkan startup evidence stream closed"),
            EvidenceStream::Failed => Some("Cannot read Vulkan startup evidence"),
            EvidenceStream::Open => None,
        };
        if let Some(message) = message {
            return Err(error("GPU_PREPARATION", message));
        }
        let stderr = std::str::from_utf8(&self.bytes).map_err(|_| {
            error(
                "GPU_PREPARATION",
                "Vulkan startup evidence is not valid UTF-8",
            )
        })?;
        acceleration::actual_mode(Backend::Vulkan, stderr)
            .map_err(|failure| error("GPU_PREPARATION", failure.message()))
    }
}

fn capture_stderr(mut pipe: impl Read, sink: &Mutex<Vec<u8>>, evidence: &Mutex<StartupEvidence>) {
    let mut chunk = [0; 4096];
    loop {
        let n = match pipe.read(&mut chunk) {
            Ok(0) => {
                if let Ok(mut captured) = evidence.lock() {
                    captured.stream = EvidenceStream::Closed;
                }
                break;
            }
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => {
                if let Ok(mut captured) = evidence.lock() {
                    captured.stream = EvidenceStream::Failed;
                }
                break;
            }
        };
        if let Ok(mut captured) = evidence.lock() {
            captured.extend(&chunk[..n]);
        }
        // Drain even after the sticky startup-prefix limit; never block the child
        // or forget earlier evidence by substituting the rolling diagnostic tail.
        if let Ok(mut buf) = sink.lock() {
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() > STDERR_LIMIT {
                let excess = buf.len() - STDERR_LIMIT;
                buf.drain(..excess);
            }
        }
    }
}

pub struct Runner {
    child: Option<Child>,
    pid: i32,
    dir: Option<PathBuf>,
    socket: PathBuf,
    key: String,
    alias: String,
    context: u32,
    lock: Option<File>,
    lease_return: Option<Arc<Mutex<Option<File>>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    evidence_stderr: Arc<Mutex<StartupEvidence>>,
    cancelled: Cancellation,
}

impl Runner {
    /// Frozen string-error form of [`Runner::launch`]; errors are `CODE: message`.
    pub fn start(config: &RunnerConfig, model_file: &Path, deadline: Instant) -> Result<Runner> {
        Self::launch(config, model_file, deadline).map_err(coded)
    }

    pub fn launch(
        config: &RunnerConfig,
        model_file: &Path,
        deadline: Instant,
    ) -> AppResult<Runner> {
        Self::launch_with_cancel(config, model_file, deadline, Arc::new(|| false), None)
    }
    pub fn start_cancelled(
        config: &RunnerConfig,
        model_file: &Path,
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<Runner> {
        Self::launch_with_cancel(config, model_file, deadline, Arc::new(cancelled), None)
            .map_err(coded)
    }
    pub fn launch_verified(
        config: &RunnerConfig,
        model: &VerifiedModel,
        deadline: Instant,
        cancelled: Cancellation,
    ) -> AppResult<Runner> {
        Self::launch_with_cancel(config, &model.path, deadline, cancelled, Some(model))
    }

    pub fn start_verified(
        config: &RunnerConfig,
        model: &VerifiedModel,
        deadline: Instant,
        cancelled: Cancellation,
    ) -> Result<Runner> {
        Self::launch_verified(config, model, deadline, cancelled).map_err(coded)
    }

    fn launch_with_cancel(
        config: &RunnerConfig,
        model_file: &Path,
        deadline: Instant,
        cancelled: Cancellation,
        verified: Option<&VerifiedModel>,
    ) -> AppResult<Runner> {
        if cancelled() {
            return Err(Http::Cancelled.app());
        }
        remaining(deadline)?;
        if !(1..=64).contains(&config.threads) || !(512..=32768).contains(&config.context) {
            return Err(unavailable("Runner configuration out of bounds"));
        }
        let alias = if let Some(model) = verified {
            model.sha256.clone()
        } else {
            let alias = model_file
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".gguf"))
                .filter(|s| valid_sha(s))
                .ok_or_else(|| error("MODEL_UNAVAILABLE", "Model file must be <sha256>.gguf"))?
                .to_string();
            filesystem::check_ancestors(model_file)
                .map_err(|_| error("MODEL_UNAVAILABLE", "Unsafe model path"))?;
            let m = fs::symlink_metadata(model_file)
                .map_err(|_| error("MODEL_UNAVAILABLE", "Model file missing"))?;
            if !m.is_file() || m.uid() != uid() || m.nlink() != 1 || m.mode() & 0o077 != 0 {
                return Err(error("MODEL_UNAVAILABLE", "Unsafe model file"));
            }
            alias
        };
        let model_fd = verified.map(|model| model.file.as_raw_fd());
        let inherited_path = model_fd.map(|fd| PathBuf::from(format!("/proc/self/fd/{fd}")));
        let load_path = inherited_path.as_deref().unwrap_or(model_file);
        filesystem::check_ancestors(&config.executable)
            .map_err(|_| unsafe_runtime("Unsafe runtime path"))?;
        let e = fs::symlink_metadata(&config.executable)
            .map_err(|_| missing("Runner executable missing"))?;
        if !e.is_file() || e.uid() != uid() || e.nlink() != 1 || e.mode() & 0o7777 != 0o755 {
            return Err(unsafe_runtime("Unsafe runner executable"));
        }
        let cwd = config
            .executable
            .parent()
            .ok_or_else(|| unsafe_runtime("Runner executable has no directory"))?;
        filesystem::mkdir(&config.runtime_parent)
            .and_then(|_| filesystem::check_directory(&config.runtime_parent, true))
            .map_err(|_| unavailable("Unsafe private runtime namespace"))?;
        let lease_return = verified
            .and_then(|model| model.lease.as_ref())
            .map(|(_, lease)| lease.clone());
        let lock = match verified.and_then(|model| model.lease.as_ref()) {
            Some((path, lease)) if *path == config.lock => lease
                .lock()
                .map_err(|_| unavailable("Cannot carry runner lease"))?
                .take()
                .ok_or_else(|| unavailable("Verified model lease already consumed"))?,
            Some(_) => return Err(unavailable("Verified model lease path mismatch")),
            None => acquire_lock(&config.lock)?,
        };
        cleanup_stale(&config.runtime_parent);
        let id = new_id().map_err(|_| unavailable("Cannot create runner identity"))?;
        let dir = config.runtime_parent.join(format!("runner-{id}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|_| unavailable("Cannot create private runner directory"))?;
        let mut runner = Runner {
            child: None,
            pid: 0,
            socket: dir.join("llama.sock"),
            dir: Some(dir),
            key: new_id().map_err(|_| unavailable("Cannot create runner key"))?,
            alias,
            context: config.context,
            lock: Some(lock),
            lease_return,
            stderr: Arc::new(Mutex::new(Vec::new())),
            evidence_stderr: Arc::new(Mutex::new(StartupEvidence::default())),
            cancelled,
        };
        if runner.socket.as_os_str().len() > SOCKET_PATH_LIMIT {
            return Err(unavailable(
                "Runner socket path exceeds the UNIX socket limit",
            ));
        }
        let mut command = Command::new(&config.executable);
        command
            .arg("--model")
            .arg(load_path)
            .arg("--host")
            .arg(&runner.socket)
            .args([
                "--offline",
                "--no-webui",
                "--no-slots",
                "--no-agent",
                "--no-mmproj-auto",
                "--no-cache-idle-slots",
                "--cache-ram",
                "0",
                "--ctx-checkpoints",
                "0",
                "--ctx-size",
            ])
            .arg(config.context.to_string())
            .args(["--parallel", "1", "--threads"])
            .arg(config.threads.to_string())
            .args(["--threads-http", "2", "--timeout", "180", "--alias"])
            .arg(&runner.alias)
            .args(acceleration::launch_args(config.backend))
            .env_clear()
            .env("LLAMA_API_KEY", &runner.key)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .process_group(0);
        // SAFETY: getpid has no arguments; captured in the parent before fork.
        let expected_parent = unsafe { libc::getpid() };
        // SAFETY: the closure invokes only libc syscalls/errno construction
        // before exec, without allocating, taking locks or touching threads.
        let lease_fd = runner
            .lock
            .as_ref()
            .expect("runner lease acquired")
            .as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                parent_death_guard(expected_parent)?;
                // Keep the same lock description in the child until it exits.
                // Unknown cleanup must not permit a second inference process.
                if let Some(fd) = model_fd {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                if libc::fcntl(lease_fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|_| unavailable("Cannot start local runner"))?;
        runner.pid = child.id() as i32;
        let stderr = child.stderr.take();
        runner.child = Some(child);
        if let Some(pipe) = stderr {
            let sink = runner.stderr.clone();
            let evidence = runner.evidence_stderr.clone();
            if std::thread::Builder::new()
                .name("smith-runner-stderr".into())
                .spawn(move || capture_stderr(pipe, &sink, &evidence))
                .is_err()
            {
                let error = unavailable("Cannot capture runner diagnostics");
                return Err(match runner.shutdown() {
                    Ok(()) => error,
                    Err(_) => unsafe_runtime("Runner cleanup could not be confirmed"),
                });
            }
        }
        if let Err(mut failure) = runner.wait_ready(deadline) {
            if config.backend == Backend::Vulkan && failure.code == "LLM_UNAVAILABLE" {
                failure = error("GPU_PREPARATION", "Vulkan runner exited before readiness");
            }
            return Err(match runner.shutdown() {
                Ok(()) => failure,
                Err(_) => unsafe_runtime("Runner cleanup could not be confirmed"),
            });
        }
        if config.backend == Backend::Vulkan {
            if let Err(failure) = runner.wait_full_offload(deadline) {
                return Err(match runner.shutdown() {
                    Ok(()) => failure,
                    Err(_) => unsafe_runtime("GPU runner cleanup could not be confirmed"),
                });
            }
        }
        Ok(runner)
    }

    fn wait_full_offload(&self, deadline: Instant) -> AppResult<()> {
        // Health is not a stderr-flush barrier. A valid candidate stays provisional
        // through this fixed window so later captured duplicates/overflow count.
        // This describes observed startup bytes, not all future runner output.
        let evidence_deadline = deadline.min(Instant::now() + Duration::from_secs(1));
        loop {
            if (self.cancelled)() {
                return Err(Http::Cancelled.app());
            }
            remaining(deadline)?;
            if !self.is_running() {
                return Err(error(
                    "GPU_PREPARATION",
                    "Vulkan runner exited before offload confirmation",
                ));
            }
            let wait = evidence_deadline.saturating_duration_since(Instant::now());
            if !wait.is_zero() {
                std::thread::sleep(wait.min(Duration::from_millis(10)));
                continue;
            }
            let mode = self
                .evidence_stderr
                .lock()
                .map_err(|_| error("GPU_PREPARATION", "Cannot read Vulkan startup evidence"))?
                .actual_mode();
            // Budget/cancellation still take precedence over any evidence failure.
            if (self.cancelled)() {
                return Err(Http::Cancelled.app());
            }
            remaining(deadline)?;
            if !self.is_running() {
                return Err(error(
                    "GPU_PREPARATION",
                    "Vulkan runner exited before offload confirmation",
                ));
            }
            return match mode? {
                ActualMode::Gpu => Ok(()),
                _ => Err(error(
                    "GPU_PREPARATION",
                    "Vulkan runner did not confirm full model offload",
                )),
            };
        }
    }

    fn wait_ready(&mut self, deadline: Instant) -> AppResult<()> {
        loop {
            if (self.cancelled)() {
                return Err(Http::Cancelled.app());
            }
            if !self.is_running() {
                return Err(unavailable("Local runner exited during startup"));
            }
            remaining(deadline)?;
            let attempt = deadline.min(Instant::now() + Duration::from_secs(2));
            match self.exchange("GET", "/health", None, attempt) {
                Ok((200, _)) => return Ok(()),
                Err(Http::Unsafe) => return Err(Http::Unsafe.app()),
                Err(Http::Timeout) if Instant::now() >= deadline => return Err(timeout()),
                _ => std::thread::sleep(Duration::from_millis(25)),
            }
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid as u32
    }

    /// Bounded tail of runner stderr for operator diagnostics; never sent to the app.
    pub fn diagnostics(&self) -> String {
        self.stderr
            .lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some() && !exited(self.pid)
    }

    fn connect(&self) -> std::result::Result<UnixStream, Http> {
        let s = match fs::symlink_metadata(&self.socket) {
            Ok(s) => s,
            Err(_) => return Err(Http::Connect),
        };
        if !s.file_type().is_socket() || s.uid() != uid() {
            return Err(Http::Unsafe);
        }
        let stream = connect_socket(&self.socket).map_err(|_| Http::Connect)?;
        match crate::transport::peer(&stream) {
            Ok(c) if c.pid == self.pid && c.uid == uid() => Ok(stream),
            _ => Err(Http::Unsafe),
        }
    }

    fn exchange(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        deadline: Instant,
    ) -> std::result::Result<(u16, Vec<u8>), Http> {
        let mut stream = self.connect()?;
        let body = body.unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nAccept: application/json\r\nConnection: close\r\n",
            self.key
        )
        .into_bytes();
        if method == "POST" {
            request.extend(
                format!(
                    "Content-Type: application/json\r\nContent-Length: {}\r\n",
                    body.len()
                )
                .bytes(),
            );
        }
        request.extend_from_slice(b"\r\n");
        request.extend_from_slice(body);
        let mut offset = 0;
        while offset < request.len() {
            if (self.cancelled)() {
                return Err(Http::Cancelled);
            }
            let left = remaining(deadline).map_err(|_| Http::Timeout)?;
            stream
                .set_write_timeout(Some(left.min(Duration::from_millis(100))))
                .map_err(|_| Http::Io)?;
            match stream.write(&request[offset..]) {
                Ok(0) => return Err(Http::Io),
                Ok(n) => offset += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut
                    ) => {}
                Err(_) => return Err(Http::Io),
            }
        }
        read_response_cancelled(&mut stream, deadline, self.cancelled.as_ref())
    }

    /// POST JSON; `Ok(None)` means the endpoint does not exist (HTTP 404).
    fn post(&mut self, path: &str, body: &Value, deadline: Instant) -> AppResult<Option<Value>> {
        let bytes = serde_json::to_vec(body).map_err(|_| protocol("Cannot encode request"))?;
        let (status, body) = match self.exchange("POST", path, Some(&bytes), deadline) {
            Ok(r) => r,
            Err(e) => {
                // EOF can be observed just before the dying child becomes waitable.
                let crashed = matches!(e, Http::Io | Http::Protocol | Http::Connect)
                    && self.child.is_some()
                    && wait_exit(self.pid, Duration::from_millis(200));
                self.fail();
                return Err(if crashed {
                    unavailable(
                        "Local runner exited during the request; it restarts on the next request",
                    )
                } else {
                    e.app()
                });
            }
        };
        match status {
            200 => serde_json::from_slice(&body).map(Some).map_err(|_| {
                self.fail();
                protocol("Invalid local runner JSON")
            }),
            404 => Ok(None),
            400 if serde_json::from_slice::<Value>(&body).is_ok_and(|v| {
                v.pointer("/error/type").and_then(Value::as_str)
                    == Some("exceed_context_size_error")
            }) =>
            {
                Err(error("CONTEXT_LIMIT", "Prompt exceeds the local context"))
            }
            503 => Err(unavailable("Local runner is not ready")),
            _ => Err(error("PROVIDER_HTTP", "Local runner refused request")),
        }
    }

    fn count(&mut self, window: &[Message], deadline: Instant) -> AppResult<u32> {
        let Some(rendered) =
            self.post("/apply-template", &json!({"messages": window}), deadline)?
        else {
            return Ok(estimate_tokens(window));
        };
        let prompt = rendered
            .get("prompt")
            .and_then(Value::as_str)
            .ok_or_else(|| protocol("Invalid rendered prompt"))?;
        let Some(tokens) = self.post(
            "/tokenize",
            &json!({"content": prompt, "add_special": true}),
            deadline,
        )?
        else {
            return Ok(estimate_tokens(window));
        };
        tokens
            .get("tokens")
            .and_then(Value::as_array)
            .filter(|t| t.iter().all(|token| token.as_u64().is_some()))
            .map(|t| u32::try_from(t.len()).unwrap_or(u32::MAX))
            .ok_or_else(|| protocol("Invalid token count"))
    }

    /// Trim oldest complete pairs in this request only; durable history is untouched.
    fn fit(
        &mut self,
        messages: &[Message],
        max_tokens: u32,
        deadline: Instant,
    ) -> AppResult<Vec<Message>> {
        let budget = self
            .context
            .saturating_sub(max_tokens)
            .saturating_sub(TOKEN_MARGIN);
        let mut start = 0;
        loop {
            let window = &messages[start..];
            if self.count(window, deadline)? <= budget {
                return Ok(window.to_vec());
            }
            if window.len() <= 1 {
                return Err(error(
                    "CONTEXT_LIMIT",
                    "Message does not fit the local model context",
                ));
            }
            start += 2;
        }
    }

    pub fn chat(
        &mut self,
        messages: &[Message],
        max_tokens: u32,
        deadline: Instant,
    ) -> AppResult<String> {
        check_messages(messages)?;
        if max_tokens == 0 || max_tokens >= self.context {
            return Err(error("CONTEXT_LIMIT", "Generation limit out of bounds"));
        }
        if !self.is_running() {
            self.fail();
            return Err(unavailable(
                "Local runner exited; it restarts on the next request",
            ));
        }
        let result = (|| {
            remaining(deadline)?;
            let window = self.fit(messages, max_tokens, deadline)?;
            let request = json!({"model": self.alias, "messages": window, "stream": false, "max_tokens": max_tokens});
            let v = self
                .post("/v1/chat/completions", &request, deadline)?
                .ok_or_else(|| error("PROVIDER_HTTP", "Local runner refused request"))?;
            chat_result(&self.alias, &v).inspect_err(|e| {
                if e.code == "PROVIDER_PROTOCOL" {
                    self.fail();
                }
            })
        })();
        if result.as_ref().is_err_and(|e| {
            matches!(
                e.code.as_str(),
                "PROVIDER_TIMEOUT" | "RUNNER_CANCELLED" | "PROVIDER_PROTOCOL" | "RUNNER_UNSAFE"
            )
        }) || !self.is_running()
        {
            self.fail();
        }
        result
    }

    /// Never reuse a runner after a mid-request failure or expired deadline.
    fn fail(&mut self) {
        let _ = self.shutdown();
    }

    pub fn stop(mut self) -> Result<()> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<()> {
        let mut result = Ok(());
        if let Some(mut child) = self.child.take() {
            let pid = self.pid;
            if !exited(pid) {
                signal_group(pid, libc::SIGTERM);
                if !wait_exit(pid, Duration::from_secs(2)) {
                    signal_group(pid, libc::SIGKILL);
                    if !wait_exit(pid, Duration::from_secs(1)) {
                        result = Err("Local runner did not exit after SIGKILL".into());
                    }
                }
            }
            if result.is_err() {
                self.child = Some(child);
                return result;
            }
            if result.is_ok() {
                // The unreaped leader keeps the group id reserved for this sweep.
                signal_group(pid, libc::SIGKILL);
                let _ = child.wait();
            }
        }
        if result.is_ok() {
            if let Some(dir) = self.dir.take() {
                result = remove_runner_dir(&dir, false);
            }
        }
        if let Some(lock) = self.lock.take() {
            if let Some(lease) = self.lease_return.take() {
                match lease.lock() {
                    Ok(mut slot) if slot.is_none() => *slot = Some(lock),
                    _ => return Err("Cannot return retained runner lease".into()),
                }
            } else {
                // Explicitly release the open-file-description lock: a concurrent
                // fork can briefly inherit it before CLOEXEC takes effect.
                // SAFETY: this descriptor is owned and the child has been cleaned up.
                unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_UN) };
            }
        }
        result
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn startup_capture_keeps_raw_prefix_limit_and_never_rolls_evidence() {
        let record = b"load_tensors: offloaded 33/33 layers to GPU\n";
        let mut evidence = StartupEvidence::default();
        evidence.extend(record);
        evidence.extend(&vec![
            b'x';
            acceleration::MAX_EVIDENCE_BYTES - record.len() - 1
        ]);
        evidence.extend(b"\n");
        assert_eq!(evidence.actual_mode().unwrap(), ActualMode::Gpu);
        evidence.extend(b"x");
        assert_eq!(evidence.bytes.len(), EVIDENCE_LIMIT);
        let overflow = evidence.actual_mode().unwrap_err();
        assert_eq!(
            overflow.message,
            "Vulkan startup evidence exceeded its byte limit"
        );
        evidence.extend(&vec![b'z'; 128 * 1024]);
        assert!(evidence.bytes.starts_with(record));
        assert_eq!(evidence.bytes.len(), EVIDENCE_LIMIT);
        assert_eq!(evidence.actual_mode().unwrap_err(), overflow);
    }

    #[test]
    fn startup_capture_failure_cannot_accept_an_earlier_valid_record() {
        struct FailedRead;
        impl Read for FailedRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other(
                    "untrusted diagnostic must not escape",
                ))
            }
        }
        let record = b"load_tensors: offloaded 33/33 layers to GPU\n";
        let reader = std::io::Cursor::new(record).chain(FailedRead);
        let tail = Mutex::new(Vec::new());
        let evidence = Mutex::new(StartupEvidence::default());
        capture_stderr(reader, &tail, &evidence);
        assert_eq!(*tail.lock().unwrap(), record);
        assert_eq!(
            evidence.lock().unwrap().actual_mode().unwrap_err(),
            error("GPU_PREPARATION", "Cannot read Vulkan startup evidence")
        );
    }

    #[test]
    fn startup_capture_retries_interruption_but_records_eof() {
        struct InterruptedRead {
            interrupted: bool,
            input: std::io::Cursor<&'static [u8]>,
        }
        impl Read for InterruptedRead {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(std::io::Error::from(ErrorKind::Interrupted));
                }
                self.input.read(buf)
            }
        }
        let record = b"load_tensors: offloaded 33/33 layers to GPU\n";
        let reader = InterruptedRead {
            interrupted: false,
            input: std::io::Cursor::new(record),
        };
        let tail = Mutex::new(Vec::new());
        let evidence = Mutex::new(StartupEvidence::default());
        capture_stderr(reader, &tail, &evidence);
        assert_eq!(*tail.lock().unwrap(), record);
        assert_eq!(
            evidence.lock().unwrap().actual_mode().unwrap_err(),
            error("GPU_PREPARATION", "Vulkan startup evidence stream closed")
        );
        let invalid = StartupEvidence {
            bytes: vec![0xff],
            ..Default::default()
        };
        assert_eq!(
            invalid.actual_mode().unwrap_err().message,
            "Vulkan startup evidence is not valid UTF-8"
        );
    }

    fn respond(raw: &'static [u8]) -> std::result::Result<(u16, Vec<u8>), Http> {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            let _ = b.write_all(raw);
        });
        let result = read_response(&mut a, Instant::now() + Duration::from_secs(2));
        writer.join().unwrap();
        result
    }

    #[test]
    fn http_reader_accepts_length_chunked_and_eof_bodies() {
        assert_eq!(
            respond(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").unwrap(),
            (200, b"{}".to_vec())
        );
        assert_eq!(
            respond(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3;x=y\r\nabc\r\n2\r\nde\r\n0\r\nTrailer: v\r\n\r\n").unwrap(),
            (200, b"abcde".to_vec())
        );
        assert_eq!(
            respond(b"HTTP/1.0 503 Loading\r\n\r\nbusy").unwrap(),
            (503, b"busy".to_vec())
        );
    }

    #[test]
    fn http_reader_rejects_ambiguous_truncated_and_oversized_responses() {
        for (raw, expected) in [
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}"[..],
                "Protocol",
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\n",
                "Protocol",
            ),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n",
                "Protocol",
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nshort",
                "Protocol",
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n",
                "Limit",
            ),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n100001\r\n",
                "Limit",
            ),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nabXX\r\n0\r\n\r\n",
                "Protocol",
            ),
            (b"SMTP 220 hello\r\n\r\n", "Protocol"),
        ] {
            let got = respond(raw).unwrap_err();
            assert_eq!(
                format!("{got:?}"),
                expected,
                "{}",
                String::from_utf8_lossy(raw)
            );
        }
        let huge = format!(
            "HTTP/1.1 200 OK\r\nX: {}\r\n\r\n",
            "a".repeat(HEADER_LIMIT + 1)
        );
        let raw: &'static [u8] = Box::leak(huge.into_bytes().into_boxed_slice());
        assert_eq!(format!("{:?}", respond(raw).unwrap_err()), "Limit");
    }

    #[test]
    fn http_reader_deadline_is_absolute() {
        let dir = std::env::temp_dir().join(format!("smith-runner-unit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let path = dir.join("s.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            // Trickle bytes forever-ish: each read succeeds but the total must not.
            for _ in 0..40 {
                if s.write_all(b"H").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let mut client = UnixStream::connect(&path).unwrap();
        let started = Instant::now();
        let err =
            read_response(&mut client, Instant::now() + Duration::from_millis(150)).unwrap_err();
        assert!(matches!(err, Http::Timeout));
        assert!(started.elapsed() < Duration::from_millis(600));
        drop(client);
        server.join().unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn completion_is_content_only_with_identity_and_no_tools() {
        let alias = "a".repeat(64);
        let valid = json!({"model":alias,"choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"Hello","reasoning_content":"hidden thoughts","tool_calls":null}}]});
        assert_eq!(chat_result(&alias, &valid).unwrap(), "Hello");
        for (pointer, value) in [
            ("/model", json!("other")),
            (
                "/choices/0/message/tool_calls",
                json!([{"function":{"name":"shell"}}]),
            ),
            ("/choices/0/finish_reason", json!("tool_calls")),
            ("/choices/0/message/role", json!("tool")),
            ("/choices/0/message/content", Value::Null),
            ("/choices/0/message/content", json!("  ")),
            (
                "/choices/0/message/content",
                json!("a".repeat(MAX_CONTEXT + 1)),
            ),
            ("/choices", json!([])),
        ] {
            let mut invalid = valid.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(chat_result(&alias, &invalid).is_err(), "{pointer}");
        }
        let mut truncated = valid.clone();
        truncated["choices"][0]["finish_reason"] = json!("length");
        assert_eq!(
            chat_result(&alias, &truncated).unwrap_err().code,
            "MODEL_TRUNCATED"
        );
        let mut reasoning_only = valid.clone();
        reasoning_only["choices"][0]["message"]["content"] = json!("");
        reasoning_only["choices"][0]["message"]["reasoning_content"] = json!("internal reasoning");
        assert_eq!(
            chat_result(&alias, &reasoning_only).unwrap_err().code,
            "MODEL_RESULT_LIMIT"
        );
        let mut empty_tools = valid.clone();
        empty_tools["choices"][0]["message"]["tool_calls"] = json!([]);
        assert!(chat_result(&alias, &empty_tools).is_ok());
    }

    #[test]
    fn messages_must_alternate_and_end_with_user() {
        let m = |role: &str| Message {
            role: role.into(),
            content: "x".into(),
        };
        assert!(check_messages(&[m("user")]).is_ok());
        assert!(check_messages(&[m("user"), m("assistant"), m("user")]).is_ok());
        for bad in [
            vec![],
            vec![m("assistant")],
            vec![m("user"), m("assistant")],
            vec![m("system")],
            vec![m("user"), m("user"), m("user")],
        ] {
            assert_eq!(check_messages(&bad).unwrap_err().code, "CONTEXT_LIMIT");
        }
        assert_eq!(estimate_tokens(&[m("user")]), 65);
    }
}
