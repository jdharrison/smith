//! Host driver for the network-less source-export worker. Reuses the inspection
//! worker image, isolation shape, credential-free fetch relay and exact-owned cleanup.
//! The worker never starts OpenCode and never receives task, context or model input.
use super::local::{expect_eof, next_sequence, prerequisite, respond, ISOLATION};
use super::*;
use crate::dev_fetch::FetchSession;
use crate::dev_local::{self as protocol, Checkout, Frame, LocalError, LocalResult, Repository};
use crate::retained_jobs::retained_acquire::{Budget, Exporter, Receiver};
use crate::retained_jobs::retained_sources::{PreparedRepository, SourceError};
use std::io::{BufReader, Write};

/// Host side of the repository relay; `FetchSession` in production.
pub(crate) trait Relay {
    fn request(
        &mut self,
        index: usize,
        advertisement: bool,
        body: &[u8],
        until: Instant,
        cancelled: &AtomicBool,
    ) -> LocalResult<(String, Vec<u8>)>;
    /// Verifies worker-claimed checkouts against fetched refs; returns host pins.
    fn pins(&mut self, checkouts: &[Checkout]) -> LocalResult<Vec<Checkout>>;
}
impl Relay for FetchSession {
    fn request(
        &mut self,
        index: usize,
        advertisement: bool,
        body: &[u8],
        until: Instant,
        cancelled: &AtomicBool,
    ) -> LocalResult<(String, Vec<u8>)> {
        FetchSession::request(self, index, advertisement, body, until, cancelled)
    }
    fn pins(&mut self, checkouts: &[Checkout]) -> LocalResult<Vec<Checkout>> {
        self.verify_checkouts(checkouts)?;
        self.pinned_checkouts()
    }
}

/// One export conversation. Repositories arrive strictly in order; each must
/// reproduce its host-pinned commit tree before the next may start.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive<W: Write + AsRawFd, R: Read + AsRawFd>(
    input: &mut W,
    output: &mut BufReader<R>,
    relay: &mut dyn Relay,
    request_id: &str,
    repositories: &[Repository],
    roots: &[PathBuf],
    prep: Instant,
    work: Instant,
    cancelled: &AtomicBool,
) -> LocalResult<Vec<PreparedRepository>> {
    let cancel = || cancelled.load(Ordering::Acquire);
    // Only content/protocol verification is a rejection; host I/O is not.
    let rejected = |error: SourceError| match error {
        SourceError::Invalid => LocalError::RepositoryRejected,
        SourceError::Interrupted => LocalError::Interrupted,
        _ => LocalError::RuntimeUnavailable,
    };
    if repositories.is_empty() || repositories.len() != roots.len() {
        return Err(LocalError::Protocol);
    }
    protocol::send(
        input,
        &Frame::Initialize {
            request_id: request_id.into(),
            version: 1,
            repositories: repositories.to_vec(),
        },
        prep,
        &cancel,
    )?;
    let mut sequence = 1;
    let mut pins: Option<Vec<Checkout>> = None;
    let mut current: Option<Receiver> = None;
    let mut done = Vec::new();
    let mut budget = Budget::default();
    loop {
        let until = if pins.is_none() { prep } else { work };
        let frame = protocol::receive(output, until, &cancel)?;
        if frame.request_id() != request_id {
            return Err(LocalError::Protocol);
        }
        match frame {
            Frame::RepositoryRequest {
                sequence: value,
                repository,
                advertisement,
                body,
                ..
            } if pins.is_none() => {
                next_sequence(value, &mut sequence)?;
                let (content_type, bytes) =
                    relay.request(repository, advertisement, &body, prep, cancelled)?;
                respond(
                    input,
                    request_id,
                    value,
                    &content_type,
                    &bytes,
                    prep,
                    &cancel,
                )?;
            }
            Frame::Repositories { repositories, .. } if pins.is_none() => {
                pins = Some(relay.pins(&repositories)?);
                respond(
                    input,
                    request_id,
                    0,
                    "application/json",
                    b"true",
                    prep,
                    &cancel,
                )?;
            }
            Frame::SourceCommit {
                repository, object, ..
            } if current.is_none() && repository == done.len() => {
                let pin = pins
                    .as_ref()
                    .and_then(|p| p.get(repository))
                    .ok_or(LocalError::Protocol)?;
                let mut receiver = Receiver::new(roots[repository].clone(), pin.commit.clone());
                receiver.commit(&object).map_err(rejected)?;
                current = Some(receiver);
            }
            Frame::SourceFile {
                repository,
                path,
                mode,
                bytes,
                last,
                ..
            } if repository == done.len() && bytes.len() <= protocol::CHUNK => {
                current
                    .as_mut()
                    .ok_or(LocalError::Protocol)?
                    .entry(path, mode, bytes, last, &mut budget)
                    .map_err(rejected)?;
            }
            Frame::SourceEnd {
                repository, files, ..
            } if repository == done.len() => {
                let receiver = current.take().ok_or(LocalError::Protocol)?;
                done.push(receiver.end(files).map_err(rejected)?);
                if done.len() == roots.len() {
                    return Ok(done);
                }
            }
            Frame::Failed { kind, .. } if kind == "source_export_failed" => {
                return Err(LocalError::RepositoryUnavailable)
            }
            _ => return Err(LocalError::Protocol),
        }
    }
}

/// Not yet constructed by the service: the retained-runtime wiring slice owns enablement.
pub struct DockerSourceExporter {
    docker: DockerRuntime,
}
impl DockerSourceExporter {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self {
            docker: DockerRuntime::new_rootful(runtime_root),
        }
    }
}
impl Exporter for DockerSourceExporter {
    fn export(
        &self,
        request_id: &str,
        repositories: &[crate::retained_jobs::Repository],
        roots: &[PathBuf],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::result::Result<Vec<PreparedRepository>, SourceError> {
        let started = Instant::now();
        let total = (started + Duration::from_secs(protocol::JOB_SECONDS)).min(deadline);
        let reserve = Duration::from_secs(protocol::CLEANUP_SECONDS);
        if total <= started + reserve {
            return Err(SourceError::Interrupted);
        }
        let work = total - reserve;
        let prep = (started + Duration::from_secs(protocol::PREP_SECONDS)).min(work);
        let cancel = || cancelled.load(Ordering::Acquire);
        let mut guard = ContainerGuard {
            runtime: &self.docker,
            request: request_id,
            id: None,
            owner: String::new(),
            ambiguous: false,
            armed: false,
        };
        // Only URL/reference reach the relay; capability flags stay host-side.
        let repositories: Vec<Repository> = repositories
            .iter()
            .map(|r| Repository {
                url: r.url.clone(),
                reference: r.reference.clone(),
            })
            .collect();
        let repositories = repositories.as_slice();
        let result = (|| -> LocalResult<Vec<PreparedRepository>> {
            if !dev_worker::valid_id(request_id)
                || repositories.is_empty()
                || repositories.len() != roots.len()
            {
                return Err(LocalError::Protocol);
            }
            let mut fetch = FetchSession::new(repositories.to_vec())?;
            let container_name = name(request_id).map_err(|_| LocalError::Protocol)?;
            self.docker
                .verify_daemon(prep.min(Instant::now() + Duration::from_secs(5)), &cancel)
                .map_err(prerequisite)?;
            if self
                .docker
                .owned(request_id, prep, &cancel)
                .map_err(prerequisite)?
                .is_some()
            {
                return Err(LocalError::RuntimeUnavailable);
            }
            let image = self
                .docker
                .json(&["image", "inspect", protocol::IMAGE], prep, &cancel)
                .map_err(|e| prerequisite(e.during(RuntimeDiagnostic::ImageInspectionFailed)))?;
            let image = worker_image(&image).map_err(prerequisite)?;
            let mut nonce = [0u8; 16];
            rand::rngs::OsRng
                .try_fill_bytes(&mut nonce)
                .map_err(|_| LocalError::RuntimeUnavailable)?;
            guard.owner = nonce.iter().map(|b| format!("{b:02x}")).collect();
            guard.armed = true;
            guard.ambiguous = true;
            let owner_label = format!("{OWNER_LABEL}={}", guard.owner);
            let request_label = format!("{LABEL}={request_id}");
            let kind_label = format!("{KIND_LABEL}={}", dev_worker::MARKER);
            let marker = format!("SMITH_DEV_SOURCE_EXPORT={}", protocol::EXPORT_POLICY);
            let mut options = vec![
                "--pull=never",
                "--name",
                &container_name,
                "--label",
                &request_label,
                "--label",
                &kind_label,
                "--label",
                &owner_label,
            ];
            options.extend_from_slice(ISOLATION);
            options.extend([
                "--env",
                &marker,
                "--entrypoint=/usr/local/bin/smith-service",
                &image,
                "dev-source-export",
                request_id,
            ]);
            let created = self
                .docker
                .capture(&create_arguments(&options), prep, &cancel)
                .map_err(prerequisite)?;
            let id = std::str::from_utf8(&created)
                .map_err(|_| LocalError::Protocol)?
                .trim();
            if !hex(id, 64) {
                return Err(LocalError::Protocol);
            }
            guard.id = Some(id.into());
            let identity = self
                .docker
                .owned(request_id, prep, &cancel)
                .map_err(prerequisite)?
                .ok_or(LocalError::Protocol)?;
            if identity.id != id
                || identity.owner != guard.owner
                || identity.image != image
                || identity.running
            {
                return Err(LocalError::Protocol);
            }
            guard.ambiguous = false;
            let mut command = self.docker.command().map_err(prerequisite)?;
            command
                .args(["container", "start", "--attach", "--interactive", id])
                .stdin(Stdio::piped());
            guard.ambiguous = true;
            let mut child = OwnedChild(
                command
                    .spawn()
                    .map_err(|_| LocalError::RuntimeUnavailable)?,
            );
            let mut input = child.0.stdin.take().ok_or(LocalError::Protocol)?;
            let output = child.0.stdout.take().ok_or(LocalError::Protocol)?;
            dev_worker::nonblocking(input.as_raw_fd()).map_err(|_| LocalError::Protocol)?;
            dev_worker::nonblocking(output.as_raw_fd()).map_err(|_| LocalError::Protocol)?;
            let mut output = BufReader::new(output);
            let prepared = drive(
                &mut input,
                &mut output,
                &mut fetch,
                request_id,
                repositories,
                roots,
                prep,
                work,
                cancelled,
            )?;
            // Correlated, tree-verified output from this exact attached container proves start.
            guard.ambiguous = false;
            drop(input);
            expect_eof(&mut output, work, &cancel)?;
            loop {
                if cancel() || Instant::now() >= work {
                    return Err(LocalError::Shutdown);
                }
                if let Some(status) = child.0.try_wait().map_err(|_| LocalError::Shutdown)? {
                    if !status.success() {
                        return Err(LocalError::Shutdown);
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(prepared)
        })();
        let cleanup = if guard.armed {
            guard.stop(total)
        } else {
            Ok(())
        };
        // An unverified cleanup outranks every other outcome, including success.
        match (result, cleanup) {
            (_, Err(_)) => Err(SourceError::CleanupUnknown),
            (Ok(prepared), Ok(())) => Ok(prepared),
            (Err(LocalError::Interrupted | LocalError::Deadline), _) => {
                Err(SourceError::Interrupted)
            }
            (Err(_), _) if cancel() => Err(SourceError::Interrupted),
            (Err(LocalError::RepositoryRejected), _) => Err(SourceError::Invalid),
            _ => Err(SourceError::Unavailable),
        }
    }
}

#[cfg(test)]
#[path = "dev_runtime_export_tests.rs"]
mod tests;
