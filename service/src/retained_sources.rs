//! Private, one-use source registry for the retained supervisor.
//!
//! This module never invokes Git or derives host paths from Projects. A trusted
//! acquisition boundary supplies explicit sanitized roots, exact commit pins and
//! a complete file inventory. That boundary must bind the pin to the fetched Git
//! tree (using isolated Git and the dev_fetch relay, not host Git/config). File
//! hashes alone do not establish Git provenance. No transport attestation from a
//! model, repository file or JSONL request may construct PreparedSource.
//!
//! Registry v1: `<root>/<source_id>/manifest.json` and `snapshot/rN/<path>`.
//! The manifest hash is SHA256 of the exact manifest.json bytes (no newline).
//! Consumers must revalidate bytes, inventory and modes, not trust a pathname.
use crate::retained_jobs::{
    retained_runtime::{SourceBinding, SourceResolver},
    FrozenProject, JobType,
};
use crate::{dev_fetch, dev_local, filesystem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Instant;

pub const MAX_REPOSITORIES: usize = 10;
pub const MAX_FILES: usize = 10_000;
pub const MAX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceError {
    Invalid,
    Unavailable,
    Consumed,
    Interrupted,
    /// Exact-owned container cleanup was not verified; reconciliation is required.
    CleanupUnknown,
}
type Result<T> = std::result::Result<T, SourceError>;

/// Inventory produced by the trusted Git-tree extractor, not discovered from
/// untrusted checkout metadata. Modes are Git's regular-file modes only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub sha256: String,
    pub size: u64,
    pub mode: u32,
}
#[derive(Clone)]
pub struct PreparedRepository {
    pub root: PathBuf,
    /// Lowercase, full SHA-1 commit object ID; abbreviated/tree/tag pins refused.
    pub commit: String,
    pub files: BTreeMap<String, SourceFile>,
}
/// Owner-only input. The complete frozen project, mode, order and permissions
/// are bound, including context; preparation is never inferred from ambient cwd.
#[derive(Clone)]
pub struct PreparedSource {
    pub source_id: String,
    pub project: FrozenProject,
    pub mode: JobType,
    pub repositories: Vec<PreparedRepository>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryManifest {
    pub path: String,
    pub url: String,
    pub reference: Option<String>,
    pub commit: String,
    pub read_only: bool,
}
/// Exact JSON schema for the Python registry loader. `files` is sorted and uses
/// snapshot-relative POSIX paths (r0/...), modes are decimal 33188 or 33261.
/// `project_sha256` binds serde_json's exact FrozenProject serialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub v: u32,
    pub source_id: String,
    pub project_sha256: String,
    pub mode: JobType,
    pub repositories: Vec<RepositoryManifest>,
    pub files: BTreeMap<String, SourceFile>,
    pub expanded_bytes: u64,
}

/// An explicit finite prepared-input queue, not a network adapter or admission
/// readiness claim. A resolve attempt consumes its preparation even on failure.
/// `.consumed-ID` survives reconstruction; failures never trigger reacquisition.
pub struct PreparedSources {
    registry: PathBuf,
    pending: Mutex<HashMap<String, PreparedSource>>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
        Err(SourceError::Interrupted)
    } else {
        Ok(())
    }
}
fn key(project: &FrozenProject, mode: JobType) -> Result<String> {
    let raw = serde_json::to_vec(&(project, mode)).map_err(|_| SourceError::Invalid)?;
    Ok(hash(&raw))
}
pub(super) fn allowed_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && path.split('/').all(|part| {
            !part.is_empty()
                && part.len() <= 255
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                && !blocked(part)
        })
}
fn blocked(part: &str) -> bool {
    let name = part.to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".git"
            | ".gitmodules"
            | ".gitattributes"
            | ".lfsconfig"
            | ".opencode"
            | "opencode.json"
            | "opencode.jsonc"
            | "agents.md"
            | "claude.md"
            | ".claude"
            | ".ssh"
            | ".aws"
            | ".azure"
            | ".gnupg"
            | ".netrc"
            | ".gitconfig"
            | ".git-credentials"
            | ".npmrc"
            | ".pypirc"
            | "credentials"
            | "credentials.json"
            | "id_rsa"
            | "id_ed25519"
    ) || name == ".env"
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
}
fn validate(source: &PreparedSource) -> Result<()> {
    let count = source.repositories.len();
    if !hex(&source.source_id, 32)
        || count == 0
        || count > MAX_REPOSITORIES
        || count != source.project.repositories.len()
        || (source.mode == JobType::Write
            && source.project.repositories.iter().any(|r| r.read_only))
    {
        return Err(SourceError::Invalid);
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    for (repo, prepared) in source.project.repositories.iter().zip(&source.repositories) {
        dev_fetch::validate_repository(&dev_local::Repository {
            url: repo.url.clone(),
            reference: repo.reference.clone(),
        })
        .map_err(|_| SourceError::Invalid)?;
        if !hex(&prepared.commit, 40) || prepared.commit.bytes().all(|b| b == b'0') {
            return Err(SourceError::Invalid);
        }
        filesystem::check_directory(&prepared.root, true).map_err(|_| SourceError::Invalid)?;
        files = files
            .checked_add(prepared.files.len())
            .ok_or(SourceError::Invalid)?;
        for (path, spec) in &prepared.files {
            if !allowed_path(path)
                || !hex(&spec.sha256, 64)
                || !matches!(spec.mode, 0o100644 | 0o100755)
            {
                return Err(SourceError::Invalid);
            }
            bytes = bytes.checked_add(spec.size).ok_or(SourceError::Invalid)?;
        }
    }
    if files > MAX_FILES || bytes > MAX_EXPANDED_BYTES {
        return Err(SourceError::Invalid);
    }
    Ok(())
}
fn mkdir(path: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|_| SourceError::Unavailable)
}
fn create(path: &Path, mode: u32) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| SourceError::Unavailable)
}
fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| SourceError::Unavailable)
}
impl PreparedSources {
    /// Registry must already be an explicit private owner-controlled directory.
    /// At most 32 preparations may be queued; no filesystem traversal at startup.
    pub fn new(registry: PathBuf, sources: Vec<PreparedSource>) -> Result<Self> {
        filesystem::check_directory(&registry, true).map_err(|_| SourceError::Invalid)?;
        if sources.len() > 32 {
            return Err(SourceError::Invalid);
        }
        let mut pending = HashMap::new();
        let mut ids = std::collections::HashSet::new();
        for source in sources {
            validate(&source)?;
            if !ids.insert(source.source_id.clone())
                || pending
                    .insert(key(&source.project, source.mode)?, source)
                    .is_some()
            {
                return Err(SourceError::Invalid);
            }
        }
        Ok(Self {
            registry,
            pending: Mutex::new(pending),
        })
    }
    pub fn prepare(
        &self,
        project: &FrozenProject,
        mode: JobType,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceBinding> {
        let source = self
            .pending
            .lock()
            .map_err(|_| SourceError::Unavailable)?
            .remove(&key(project, mode)?)
            .ok_or(SourceError::Consumed)?;
        // Durable no-replay fence precedes reading/copying any source content.
        filesystem::check_directory(&self.registry, true).map_err(|_| SourceError::Invalid)?;
        let marker = self
            .registry
            .join(format!(".consumed-{}", source.source_id));
        let mut consumed = create(&marker, 0o600).map_err(|_| SourceError::Consumed)?;
        consumed
            .write_all(key(project, mode)?.as_bytes())
            .and_then(|_| consumed.sync_all())
            .map_err(|_| SourceError::Unavailable)?;
        sync_dir(&self.registry)?;
        check(deadline, cancelled)?;
        validate(&source)?;
        let staging = self.registry.join(format!(".pending-{}", source.source_id));
        mkdir(&staging)?;
        // Failed staging is deliberately retained privately, never auto-replayed.
        let snapshot = staging.join("snapshot");
        mkdir(&snapshot)?;
        let mut manifest = Manifest {
            v: 1,
            source_id: source.source_id.clone(),
            project_sha256: hash(&serde_json::to_vec(project).map_err(|_| SourceError::Invalid)?),
            mode,
            repositories: Vec::new(),
            files: BTreeMap::new(),
            expanded_bytes: 0,
        };
        let mut entries = 0;
        for (index, (repo, prepared)) in project
            .repositories
            .iter()
            .zip(&source.repositories)
            .enumerate()
        {
            check(deadline, cancelled)?;
            let path = format!("r{index}");
            let output = snapshot.join(&path);
            mkdir(&output)?;
            let mut found = BTreeMap::new();
            copy_tree(
                &prepared.root,
                &output,
                "",
                prepared,
                &mut found,
                &mut entries,
                deadline,
                cancelled,
            )?;
            if found != prepared.files {
                return Err(SourceError::Invalid);
            }
            for (name, file) in found {
                manifest.expanded_bytes += file.size;
                manifest.files.insert(format!("{path}/{name}"), file);
            }
            manifest.repositories.push(RepositoryManifest {
                path,
                url: repo.url.clone(),
                reference: repo.reference.clone(),
                commit: prepared.commit.clone(),
                read_only: mode == JobType::Read || repo.read_only,
            });
        }
        let raw = serde_json::to_vec(&manifest).map_err(|_| SourceError::Invalid)?;
        if raw.len() > MAX_MANIFEST_BYTES {
            return Err(SourceError::Invalid);
        }
        let mut file = create(&staging.join("manifest.json"), 0o600)?;
        file.write_all(&raw)
            .and_then(|_| file.sync_all())
            .map_err(|_| SourceError::Unavailable)?;
        sync_dir(&snapshot)?;
        sync_dir(&staging)?;
        check(deadline, cancelled)?;
        // Linux no-replace publication: even an empty preexisting entry is never
        // replaced. Readers ignore .pending-* and see only complete directories.
        let target = self.registry.join(&source.source_id);
        rename_no_replace(&staging, &target)?;
        sync_dir(&self.registry)?;
        Ok(SourceBinding {
            source_id: source.source_id,
            manifest_sha256: hash(&raw),
        })
    }
}
impl SourceResolver for PreparedSources {
    fn resolve(
        &self,
        project: &FrozenProject,
        mode: JobType,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<SourceBinding> {
        self.prepare(project, mode, deadline, cancelled).ok()
    }
}
fn rename_no_replace(from: &Path, to: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from =
        std::ffi::CString::new(from.as_os_str().as_bytes()).map_err(|_| SourceError::Invalid)?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(|_| SourceError::Invalid)?;
    // SAFETY: both C strings are live and NUL-terminated; no borrowed descriptors.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(SourceError::Unavailable)
    }
}
#[allow(clippy::too_many_arguments)]
fn copy_tree(
    input: &Path,
    output: &Path,
    prefix: &str,
    prepared: &PreparedRepository,
    found: &mut BTreeMap<String, SourceFile>,
    entries: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    check(deadline, cancelled)?;
    let mut children = Vec::new();
    for child in fs::read_dir(input).map_err(|_| SourceError::Unavailable)? {
        check(deadline, cancelled)?;
        *entries += 1;
        if *entries > MAX_ENTRIES {
            return Err(SourceError::Invalid);
        }
        children.push(child.map_err(|_| SourceError::Unavailable)?);
    }
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        check(deadline, cancelled)?;
        let name = child
            .file_name()
            .into_string()
            .map_err(|_| SourceError::Invalid)?;
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if !allowed_path(&path) || path.split('/').count() > 32 {
            return Err(SourceError::Invalid);
        }
        let metadata = fs::symlink_metadata(child.path()).map_err(|_| SourceError::Unavailable)?;
        if metadata.uid() != filesystem::uid() || metadata.mode() & 0o7022 != 0 {
            return Err(SourceError::Invalid);
        }
        let dest = output.join(child.file_name());
        if metadata.is_dir() {
            // No implicit empty/unlisted directory trees.
            if !prepared
                .files
                .keys()
                .any(|p| p.starts_with(&format!("{path}/")))
            {
                return Err(SourceError::Invalid);
            }
            mkdir(&dest)?;
            copy_tree(
                &child.path(),
                &dest,
                &path,
                prepared,
                found,
                entries,
                deadline,
                cancelled,
            )?;
        } else if metadata.is_file() {
            let expected = prepared.files.get(&path).ok_or(SourceError::Invalid)?;
            let actual_mode = if metadata.mode() & 0o111 == 0 {
                0o100644
            } else {
                0o100755
            };
            if metadata.nlink() != 1
                || metadata.len() != expected.size
                || actual_mode != expected.mode
            {
                return Err(SourceError::Invalid);
            }
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(child.path())
                .map_err(|_| SourceError::Unavailable)?;
            let opened = file.metadata().map_err(|_| SourceError::Unavailable)?;
            if !opened.is_file() || opened.dev() != metadata.dev() || opened.ino() != metadata.ino()
            {
                return Err(SourceError::Invalid);
            }
            let mut out = create(
                &dest,
                if expected.mode == 0o100755 {
                    0o700
                } else {
                    0o600
                },
            )?;
            let mut digest = Sha256::new();
            let mut size = 0u64;
            let mut buffer = [0u8; 32768];
            loop {
                check(deadline, cancelled)?;
                let n = file
                    .read(&mut buffer)
                    .map_err(|_| SourceError::Unavailable)?;
                if n == 0 {
                    break;
                }
                size += n as u64;
                if size > expected.size {
                    return Err(SourceError::Invalid);
                }
                digest.update(&buffer[..n]);
                out.write_all(&buffer[..n])
                    .map_err(|_| SourceError::Unavailable)?;
                // Common secret material is refused even under an innocuous name.
                // This is deliberately not a promise to identify arbitrary secrets.
                if buffer[..n].windows(11).any(|v| v == b"PRIVATE KEY") {
                    return Err(SourceError::Invalid);
                }
            }
            if size != expected.size || format!("{:x}", digest.finalize()) != expected.sha256 {
                return Err(SourceError::Invalid);
            }
            out.sync_all().map_err(|_| SourceError::Unavailable)?;
            found.insert(path, expected.clone());
        } else {
            return Err(SourceError::Invalid);
        }
    }
    sync_dir(output)
}

#[cfg(test)]
#[path = "retained_sources_tests.rs"]
mod tests;
