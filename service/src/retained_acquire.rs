//! Repository acquisition for retained jobs: the trusted boundary named by
//! `retained_sources`. An isolated exporter (network-less worker container behind the
//! credential-free `dev_fetch` relay) streams the pinned commit object and every tree
//! entry. Nothing it says is trusted: the host hashes the commit object against the
//! relay-verified pin and rebuilds the root tree from received entries, so the
//! snapshot is exactly that commit minus policy exclusions. Exclusions (blocked names,
//! symlinks, submodules, private-key material) are omitted, never rewritten.
use super::retained_runtime::{SourceBinding, SourceResolver};
use super::retained_sources::{
    allowed_path, PreparedRepository, PreparedSource, PreparedSources, SourceError, SourceFile,
    MAX_EXPANDED_BYTES, MAX_FILES,
};
use super::{FrozenProject, JobType, Repository};
use crate::dev_worker_local;
use crate::git_tree::{self, ObjectId, Tree, EXECUTABLE, GITLINK, REGULAR};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Instant;

type Result<T> = std::result::Result<T, SourceError>;
const INVALID: SourceError = SourceError::Invalid;

/// Totals shared by all repositories of one acquisition.
#[derive(Default)]
pub struct Budget {
    entries: usize,
    received: u64,
    files: usize,
    bytes: u64,
}
struct Current {
    path: String,
    mode: u32,
    bytes: Vec<u8>,
}
/// Per-repository state machine: one commit object, then entries, then end.
pub struct Receiver {
    root: PathBuf,
    commit: String,
    tree: Option<ObjectId>,
    built: Tree,
    files: BTreeMap<String, SourceFile>,
    current: Option<Current>,
    entries: usize,
}
fn included(path: &str, mode: u32, bytes: &[u8]) -> bool {
    matches!(mode, REGULAR | EXECUTABLE)
        && allowed_path(path)
        && path.split('/').count() <= 32
        // Mirrors the registry's refusal; this is not a general secret scanner.
        && !bytes.windows(11).any(|v| v == b"PRIVATE KEY")
}
fn write(root: &Path, path: &str, mode: u32, bytes: &[u8]) -> Result<()> {
    let target = root.join(path);
    let mut directory = root.to_path_buf();
    for part in Path::new(path)
        .parent()
        .into_iter()
        .flat_map(Path::components)
    {
        directory.push(part);
        match DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Only directories this receiver created can exist below a fresh root.
                if !fs::symlink_metadata(&directory).is_ok_and(|m| m.is_dir()) {
                    return Err(INVALID);
                }
            }
            Err(_) => return Err(SourceError::Unavailable),
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(if mode == EXECUTABLE { 0o700 } else { 0o600 })
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&target)
        .map_err(|_| SourceError::Unavailable)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| SourceError::Unavailable)
}
impl Receiver {
    /// `root` must be a fresh, empty, private directory owned by the caller.
    pub fn new(root: PathBuf, commit: String) -> Self {
        Self {
            root,
            commit,
            tree: None,
            built: Tree::default(),
            files: BTreeMap::new(),
            current: None,
            entries: 0,
        }
    }
    pub fn commit(&mut self, raw: &[u8]) -> Result<()> {
        if self.tree.is_some() {
            return Err(INVALID);
        }
        self.tree = Some(git_tree::commit_tree(&self.commit, raw).ok_or(INVALID)?);
        Ok(())
    }
    pub fn entry(
        &mut self,
        path: String,
        mode: u32,
        bytes: Vec<u8>,
        last: bool,
        budget: &mut Budget,
    ) -> Result<()> {
        if self.tree.is_none() || path.len() > git_tree::MAX_PATH {
            return Err(INVALID);
        }
        // Paths are charged once per entry, so path bytes cannot bypass the budget.
        let charged = if self.current.is_none() {
            path.len()
        } else {
            0
        };
        budget.received = budget
            .received
            .saturating_add((bytes.len() + charged) as u64);
        if budget.received > dev_worker_local::EXPORT_BYTES {
            return Err(INVALID);
        }
        let current = match self.current.take() {
            Some(current) if current.path == path && current.mode == mode => current,
            Some(_) => return Err(INVALID),
            None => Current {
                path,
                mode,
                bytes: Vec::new(),
            },
        };
        let mut current = current;
        current.bytes.extend_from_slice(&bytes);
        if current.bytes.len() as u64 > MAX_EXPANDED_BYTES {
            return Err(INVALID);
        }
        if !last {
            self.current = Some(current);
            return Ok(());
        }
        let Current { path, mode, bytes } = current;
        let id = if mode == GITLINK {
            git_tree::parse(&bytes).ok_or(INVALID)?
        } else {
            git_tree::object_id("blob", &bytes)
        };
        budget.entries += 1;
        self.entries += 1;
        if budget.entries > dev_worker_local::EXPORT_ENTRIES || !self.built.insert(&path, mode, id)
        {
            return Err(INVALID);
        }
        if included(&path, mode, &bytes) {
            budget.files += 1;
            budget.bytes += bytes.len() as u64;
            if budget.files > MAX_FILES || budget.bytes > MAX_EXPANDED_BYTES {
                return Err(INVALID);
            }
            write(&self.root, &path, mode, &bytes)?;
            self.files.insert(
                path,
                SourceFile {
                    sha256: format!("{:x}", Sha256::digest(&bytes)),
                    size: bytes.len() as u64,
                    mode,
                },
            );
        }
        Ok(())
    }
    /// Succeeds only if the received entries reproduce the pinned commit's tree.
    pub fn end(self, count: usize) -> Result<PreparedRepository> {
        if self.current.is_some() || count != self.entries || self.tree != Some(self.built.id()) {
            return Err(INVALID);
        }
        Ok(PreparedRepository {
            root: self.root,
            commit: self.commit,
            files: self.files,
        })
    }
}

/// Isolated export of exactly `repositories` into the matching fresh `roots`.
/// Implementations own their container/process cleanup and honor deadline/cancel.
pub trait Exporter: Send + Sync {
    fn export(
        &self,
        request_id: &str,
        repositories: &[Repository],
        roots: &[PathBuf],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<PreparedRepository>>;
}

/// Acquires on each resolve, then publishes through the one-use registry. Staging is
/// private and removed after publication or failure; the registry keeps its own
/// consumption fence. Every repository is pinned by the relay, never by the project.
pub struct AcquiringSources {
    work: PathBuf,
    registry: PathBuf,
    exporter: Arc<dyn Exporter>,
}
impl AcquiringSources {
    /// Both directories must already be explicit private owner-controlled roots.
    pub fn new(work: PathBuf, registry: PathBuf, exporter: Arc<dyn Exporter>) -> Result<Self> {
        for path in [&work, &registry] {
            crate::filesystem::check_directory(path, true).map_err(|_| INVALID)?;
        }
        if work.starts_with(&registry) || registry.starts_with(&work) {
            return Err(INVALID);
        }
        Ok(Self {
            work,
            registry,
            exporter,
        })
    }
    pub fn acquire(
        &self,
        project: &FrozenProject,
        mode: JobType,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceBinding> {
        let count = project.repositories.len();
        if count == 0 || count > super::retained_sources::MAX_REPOSITORIES {
            return Err(INVALID);
        }
        let source_id = crate::app::new_id().map_err(|_| SourceError::Unavailable)?;
        let staging = self.work.join(&source_id);
        DirBuilder::new()
            .mode(0o700)
            .create(&staging)
            .map_err(|_| SourceError::Unavailable)?;
        let result = (|| {
            let mut roots = Vec::new();
            for index in 0..count {
                let root = staging.join(format!("r{index}"));
                DirBuilder::new()
                    .mode(0o700)
                    .create(&root)
                    .map_err(|_| SourceError::Unavailable)?;
                roots.push(root);
            }
            let prepared = self.exporter.export(
                &source_id,
                &project.repositories,
                &roots,
                deadline,
                cancelled,
            )?;
            if prepared.len() != count || prepared.iter().zip(&roots).any(|(p, r)| &p.root != r) {
                return Err(INVALID);
            }
            let source = PreparedSource {
                source_id: source_id.clone(),
                project: project.clone(),
                mode,
                repositories: prepared,
            };
            PreparedSources::new(self.registry.clone(), vec![source])?
                .prepare(project, mode, deadline, cancelled)
        })();
        // Only this fresh private staging tree is removed; the registry copy is retained.
        let _ = fs::remove_dir_all(&staging);
        result
    }
}
impl SourceResolver for AcquiringSources {
    fn resolve(
        &self,
        project: &FrozenProject,
        mode: JobType,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<SourceBinding> {
        self.acquire(project, mode, deadline, cancelled).ok()
    }
}

#[cfg(test)]
#[path = "retained_acquire_tests.rs"]
mod tests;
