//! Local inspection domain. No runtime/provider observation occurs during admission or reads.
use super::*;
use crate::dev_local::{
    Checkout, FrozenModel, LocalError, LocalEvent, LocalJob, LocalReceipt, LocalRuntime, Repository,
};
use std::sync::atomic::{AtomicBool, Ordering};

const LIMIT: u64 = 4 * 1024 * 1024;
// Includes worst-case JSON escaping of the bounded reply and checkout evidence.
const RESULT_RESERVE: usize = 768 * 1024;
const QUEUED: &str = "Queued for local inspection";
const QUEUE_INTERRUPTED: &str = "Service interrupted while queued; not dispatched; no replay";
const QUEUE_EXPIRED: &str = "Queue wait exceeded ten minutes; not dispatched; no replay";
const SPAWN_FAILED: &str = "Local inspection worker could not start; not dispatched; no replay";
const QUEUE_MODEL_CHANGED: &str =
    "Frozen local model configuration unavailable; not dispatched; no replay";
const PREPARING: &str = "Preparing local inspection; no automatic setup or replay";
const CLONING: &str = "Acquiring frozen public repositories for read-only inspection";
const READY: &str = "Read-only checkout and local session ready";
const RUNNING: &str = "Local inspection dispatched; observe this request only";
const CAPTURED: &str = "Reply saved; owned shutdown not yet verified";
const COMPLETED: &str = "Reply saved and owned runtime shutdown verified";
const RESTARTED: &str = "Service interrupted; cleanup unknown; no replay";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    request_id: String,
    project_id: String,
    project_key: String,
    status: String,
    message: String,
    #[serde(deserialize_with = "required_nullable")]
    session_id: Option<String>,
    created_ms: u64,
    reply_available: bool,
    cleanup_verified: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    #[serde(deserialize_with = "projects::object")]
    summary: Summary,
    fingerprint: String,
    config_revision: String,
    projects_revision: u64,
    #[serde(deserialize_with = "projects::object")]
    job: LocalJob,
    repositories: Vec<Checkout>,
    #[serde(deserialize_with = "required_nullable")]
    image_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    message_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    reply: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    failed_stage: Option<String>,
    may_have_sent: bool,
    cleanup_required: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    deleted: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema_version: u32,
    records: Vec<Record>,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            schema_version: 1,
            records: Vec::new(),
        }
    }
}
fn is_false(value: &bool) -> bool {
    !value
}
fn deletable(r: &Record) -> bool {
    matches!(
        r.summary.status.as_str(),
        "completed" | "failed" | "reconciled"
    ) && r.summary.cleanup_verified
        && !r.cleanup_required
}
fn deleted() -> AppError {
    AppError::new("JOB_DELETED", "Dev job removed from history; no replay")
}
fn bounded(value: Value) -> AppResult<Value> {
    if reply_fits(&value) {
        Ok(value)
    } else {
        Err(v2::capacity())
    }
}
fn active(status: &str) -> bool {
    matches!(status, "preparing" | "cloning" | "ready" | "running")
}
fn uncertain(r: &Record) -> bool {
    r.cleanup_required
}
fn result_capacity(ledger: &Ledger, bytes: usize) -> bool {
    let reserved = (|| -> Option<usize> {
        let mut total = bytes;
        for record in &ledger.records {
            if record.summary.status != "queued" && !active(&record.summary.status) {
                continue;
            }
            let materialized = serde_json::to_vec(record).ok()?.len();
            // Reconstruct a stable frozen base: mutable result evidence consumes
            // the reservation instead of adding a second reservation to it.
            let mut base = record.clone();
            base.summary.status = "cleanup_unknown".into();
            base.summary.message = messages()
                .max_by_key(|s| serde_json::to_vec(s).map_or(usize::MAX, |b| b.len()))?
                .into();
            base.summary.session_id = None;
            base.summary.reply_available = false;
            base.summary.cleanup_verified = false;
            base.repositories.clear();
            base.image_id = None;
            base.message_id = None;
            base.reply = None;
            base.failed_stage = None;
            base.may_have_sent = false;
            base.cleanup_required = false;
            let budget = serde_json::to_vec(&base)
                .ok()?
                .len()
                .checked_add(RESULT_RESERVE)?;
            total = total
                .checked_sub(materialized)?
                .checked_add(materialized.max(budget))?;
        }
        Some(total)
    })();
    reserved.is_some_and(|total| u64::try_from(total).is_ok_and(|total| total <= LIMIT))
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn revision(s: &str) -> Option<u64> {
    s.parse::<u64>().ok().filter(|v| v.to_string() == s)
}
fn unavailable() -> AppError {
    AppError::new(
        "DEV_UNAVAILABLE",
        "Local Dev ledger unavailable; inspect original records; no replay",
    )
}
fn protocol() -> AppError {
    AppError::new("LOCAL_PROTOCOL", LocalError::Protocol.message())
}
fn completion_error(error: AppError) -> AppError {
    let (code, error) = match LocalError::from(error) {
        LocalError::ModelUnavailable => ("LOCAL_MODEL_UNAVAILABLE", LocalError::ModelUnavailable),
        LocalError::ModelLimit => ("CONTEXT_LIMIT", LocalError::ModelLimit),
        LocalError::Interrupted => ("INTERRUPTED", LocalError::Interrupted),
        LocalError::Deadline => ("PROVIDER_TIMEOUT", LocalError::Deadline),
        _ => ("PROVIDER_PROTOCOL", LocalError::ModelProtocol),
    };
    AppError::new(code, error.message())
}
fn unknown() -> AppError {
    AppError::new("UNKNOWN_REQUEST", "Unknown Dev request; do not resend")
}
fn valid_model(model: &FrozenModel) -> bool {
    crate::catalog::CATALOG
        .iter()
        .any(|m| m.name == model.name && m.sha256 == model.sha256)
}
fn checkouts(job: &LocalJob, repositories: &[Checkout]) -> bool {
    repositories.len() == job.repositories.len()
        && repositories.iter().zip(&job.repositories).all(|(c, r)| {
            c.url == r.url
                && crate::dev_local::valid_hash(&c.commit, 40)
                && !c.reference.is_empty()
                && c.reference.len() <= 266
                && !c.reference.chars().any(char::is_control)
                && c.reference == crate::dev_fetch::checkout_reference(r.reference.as_deref())
        })
}
fn messages() -> impl Iterator<Item = &'static str> {
    use crate::dev_runtime::RuntimeDiagnostic::*;
    [
        QUEUED,
        QUEUE_INTERRUPTED,
        QUEUE_EXPIRED,
        SPAWN_FAILED,
        QUEUE_MODEL_CHANGED,
        PREPARING,
        CLONING,
        READY,
        RUNNING,
        CAPTURED,
        COMPLETED,
        RESTARTED,
    ]
    .into_iter()
    .chain(
        [
            LocalError::RuntimeUnavailable,
            LocalError::RepositoryRejected,
            LocalError::RepositoryUnavailable,
            LocalError::ReadDenied,
            LocalError::ModelUnavailable,
            LocalError::ModelProtocol,
            LocalError::ModelLimit,
            LocalError::Interrupted,
            LocalError::Deadline,
            LocalError::Publication,
            LocalError::Protocol,
            LocalError::Shutdown,
        ]
        .into_iter()
        .map(LocalError::message),
    )
    .chain(
        [
            OperatorUnsupported,
            CliMissing,
            CliUnavailable,
            CliUnsafe,
            RuntimeRootUnavailable,
            RuntimeRootUnsafe,
            SocketMissing,
            SocketUnavailable,
            SocketUnsafe,
            ConfigIsolationUnavailable,
            DaemonCheckFailed,
            RootlessUnverified,
            RootfulSocketMissing,
            RootfulSocketUnavailable,
            RootfulSocketUnsafe,
            RootfulDaemonCheckFailed,
            RootfulUnverified,
            ImageInspectionFailed,
            WorkerImageRejected,
            UnverifiedReceipt,
        ]
        .into_iter()
        .map(crate::dev_runtime::RuntimeDiagnostic::message),
    )
}
fn known_message(s: &str) -> bool {
    messages().any(|message| message == s)
}
impl Record {
    fn valid(&self, schema_version: u32) -> bool {
        let s = &self.summary;
        valid_id(&s.request_id)
            && valid_id(&s.project_id)
            && self.job.request_id == s.request_id
            && valid_project_key(&s.project_key)
            && instructions::valid_hash(&self.fingerprint)
            && revision(&self.config_revision).is_some()
            && self.projects_revision <= v2::MAX_REVISION
            && valid_model(&self.job.model)
            && !self.job.prompt.trim().is_empty()
            && self.job.prompt.len() <= crate::dev_local::MAX_PROMPT
            && !self.job.repositories.is_empty()
            && self.job.repositories.len() <= 16
            && self
                .job
                .repositories
                .iter()
                .all(|r| crate::dev_fetch::validate_repository(r).is_ok())
            && (self.repositories.is_empty() || checkouts(&self.job, &self.repositories))
            && self.image_id.as_ref().is_none_or(|v| valid_image(v))
            && s.session_id.as_ref().is_none_or(|v| token(v))
            && self.message_id.as_ref().is_none_or(|v| token(v))
            && self
                .reply
                .as_ref()
                .is_none_or(|v| !v.trim().is_empty() && v.len() <= crate::dev_local::MAX_REPLY)
            && s.reply_available == self.reply.is_some()
            && (self.reply.is_none()
                || (self.message_id.is_some()
                    && s.session_id.is_some()
                    && self.may_have_sent
                    && checkouts(&self.job, &self.repositories)))
            && self.message_id.is_some() == self.reply.is_some()
            && self
                .failed_stage
                .as_ref()
                .is_none_or(|v| active(v) || (schema_version == 4 && v == "queued"))
            && known_message(&s.message)
            && (matches!(
                s.status.as_str(),
                "preparing"
                    | "cloning"
                    | "ready"
                    | "running"
                    | "completed"
                    | "failed"
                    | "unknown"
                    | "cleanup_unknown"
            ) || (matches!(schema_version, 2..=4) && s.status == "reconciled")
                || (schema_version == 4 && s.status == "queued"))
            && (!self.deleted || (schema_version >= 3 && deletable(self)))
            && (s.status != "queued"
                || (s.cleanup_verified
                    && !self.cleanup_required
                    && !self.may_have_sent
                    && self.failed_stage.is_none()
                    && self.image_id.is_none()
                    && s.session_id.is_none()
                    && self.repositories.is_empty()
                    && self.reply.is_none()
                    && s.message == QUEUED))
            && (self.failed_stage.as_deref() != Some("queued")
                || (s.status == "failed"
                    && s.cleanup_verified
                    && !self.cleanup_required
                    && !self.may_have_sent
                    && self.image_id.is_none()
                    && s.session_id.is_none()
                    && self.repositories.is_empty()
                    && self.reply.is_none()
                    && matches!(
                        s.message.as_str(),
                        QUEUE_INTERRUPTED | QUEUE_EXPIRED | SPAWN_FAILED | QUEUE_MODEL_CHANGED
                    )))
            && s.cleanup_verified != self.cleanup_required
            && (!active(&s.status) || self.cleanup_required)
            && (!matches!(s.status.as_str(), "ready" | "running" | "completed")
                || (s.session_id.is_some() && checkouts(&self.job, &self.repositories)))
            && (!matches!(s.status.as_str(), "running" | "completed") || self.may_have_sent)
            && (s.status != "completed"
                || (self.reply.is_some() && s.cleanup_verified && self.failed_stage.is_none()))
            && (s.status != "failed" || (!self.may_have_sent && s.cleanup_verified))
            && (s.status != "cleanup_unknown" || self.cleanup_required)
            && (s.status != "reconciled"
                || (s.cleanup_verified
                    && !self.cleanup_required
                    && self.may_have_sent
                    && self.failed_stage.as_deref() == Some("running")
                    && self.image_id.is_some()
                    && s.session_id.is_some()
                    && checkouts(&self.job, &self.repositories)))
    }
}
fn valid_project_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn valid_image(s: &str) -> bool {
    s.strip_prefix("sha256:")
        .is_some_and(|s| crate::dev_local::valid_hash(s, 64))
}
impl Ledger {
    fn load(bytes: &[u8]) -> AppResult<Self> {
        let value = crate::cloud::unique_json(bytes).map_err(|_| unavailable())?;
        // Reject serde's positional struct decoding at every object boundary.
        if !value.is_object()
            || !value["records"].as_array().is_some_and(|rs| {
                rs.iter().all(|r| {
                    r.is_object()
                        && r["summary"].is_object()
                        && r["job"].is_object()
                        && r["job"]["model"].is_object()
                        && [&r["repositories"], &r["job"]["repositories"]]
                            .iter()
                            .all(|v| {
                                v.as_array()
                                    .is_some_and(|rs| rs.iter().all(Value::is_object))
                            })
                })
            })
        {
            return Err(unavailable());
        }
        let ledger: Self = serde_json::from_value(value.clone()).map_err(|_| unavailable())?;
        if serde_json::to_value(&ledger).map_err(|_| unavailable())? != value {
            return Err(unavailable());
        }
        let mut ids = HashSet::new();
        if bytes.len() as u64 > LIMIT
            || !matches!(ledger.schema_version, 1..=4)
            || (ledger.schema_version < 3 && ledger.records.len() > 32)
            || ledger
                .records
                .iter()
                .any(|r| !r.valid(ledger.schema_version) || !ids.insert(&r.summary.request_id))
            || ledger
                .records
                .iter()
                .filter(|r| active(&r.summary.status))
                .count()
                > if ledger.schema_version == 4 { 3 } else { 1 }
        {
            return Err(unavailable());
        }
        Ok(ledger)
    }
    fn interrupt(&mut self) {
        for r in &mut self.records {
            if r.summary.status == "queued" {
                r.failed_stage = Some("queued".into());
                r.summary.status = "failed".into();
                r.summary.message = QUEUE_INTERRUPTED.into();
                r.summary.cleanup_verified = true;
                r.cleanup_required = false;
            } else if active(&r.summary.status) {
                r.failed_stage = Some(r.summary.status.clone());
                r.summary.status = "cleanup_unknown".into();
                r.summary.message = RESTARTED.into();
                r.summary.cleanup_verified = false;
                r.cleanup_required = true;
            }
        }
    }
}
struct State {
    ledger: Ledger,
    fault: bool,
    // Startup corruption isolates Dev; an attempted publication must also fence providers.
    publication_ambiguous: bool,
    owned: bool,
    pending: Option<String>,
    disk_digest: Option<String>,
}
pub(super) struct Owner {
    state: Mutex<State>,
    cancelled: AtomicBool,
    path: PathBuf,
    runtime: Arc<dyn LocalRuntime>,
    scheduler: scheduler::Scheduler,
}
impl Owner {
    pub(super) fn open(path: PathBuf, runtime: Arc<dyn LocalRuntime>) -> Self {
        let loaded = filesystem::read_file(&path, 0o600, LIMIT)
            .map_err(|_| unavailable())
            .and_then(|b| match b {
                Some(bytes) => Ok((Ledger::load(&bytes)?, Some(filesystem::digest(&bytes)))),
                None => Ok((Ledger::default(), None)),
            });
        let fault = loaded.is_err();
        let (mut ledger, disk_digest) = loaded.unwrap_or_default();
        ledger.interrupt(); // Observe only. Never probe, rewrite, replay or reconcile.
        Self {
            state: Mutex::new(State {
                ledger,
                fault,
                publication_ambiguous: false,
                owned: true,
                pending: None,
                disk_digest,
            }),
            cancelled: AtomicBool::new(false),
            path,
            runtime,
            scheduler: scheduler::Scheduler::default(),
        }
    }
    pub(super) fn close(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Ok(mut s) = self.state.lock() {
            s.owned = false;
            s.pending = None;
        }
        self.scheduler.reject();
    }
    pub(super) fn reserved(&self) -> bool {
        self.state
            .lock()
            .map(|s| s.publication_ambiguous)
            .unwrap_or(true)
    }
    fn available(&self, s: &State) -> AppResult<()> {
        if s.fault || !s.owned || self.cancelled.load(Ordering::Acquire) {
            Err(unavailable())
        } else {
            Ok(())
        }
    }
    fn commit(&self, state: &mut State, next: Ledger) -> AppResult<()> {
        self.available(state)?;
        let result = serde_json::to_vec(&next)
            .map_err(|_| unavailable())
            .and_then(|bytes| {
                if !result_capacity(&next, bytes.len()) {
                    return Err(v2::capacity());
                }
                Ledger::load(&bytes)?;
                let current =
                    filesystem::read_file(&self.path, 0o600, LIMIT).map_err(|_| unavailable())?;
                if current.as_ref().map(|b| filesystem::digest(b)) != state.disk_digest {
                    return Err(unavailable());
                }
                filesystem::publish(&self.path, &bytes, 0o600, current.is_some())
                    .map_err(|_| unavailable())?;
                state.disk_digest = Some(filesystem::digest(&bytes));
                Ok(())
            });
        // Even an ambiguous rename/fsync failure retains the attempted evidence in memory.
        state.ledger = next;
        if result.is_err() {
            state.fault = true;
            state.publication_ambiguous = true;
            state.pending = None;
            state.ledger.interrupt();
            self.cancelled.store(true, Ordering::Release);
        }
        result
    }
    fn update(&self, id: &str, change: impl FnOnce(&mut Record) -> AppResult<()>) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        self.available(&state)?;
        let mut next = state.ledger.clone();
        let r = next
            .records
            .iter_mut()
            .find(|r| r.summary.request_id == id)
            .ok_or_else(unavailable)?;
        change(r)?;
        self.commit(&mut state, next)
    }
    fn event(&self, id: &str, event: LocalEvent) -> AppResult<()> {
        self.update(id, |r| {
            match event {
                LocalEvent::Preparing { image_id } => {
                    if r.summary.status != "preparing"
                        || r.image_id.is_some()
                        || !valid_image(&image_id)
                    {
                        return Err(protocol());
                    }
                    r.image_id = Some(image_id);
                }
                LocalEvent::Cloning => {
                    if r.summary.status != "preparing" || r.image_id.is_none() {
                        return Err(protocol());
                    }
                    r.summary.status = "cloning".into();
                    r.summary.message = CLONING.into();
                }
                LocalEvent::Ready {
                    session_id,
                    repositories,
                } => {
                    if r.summary.status != "cloning"
                        || !token(&session_id)
                        || !checkouts(&r.job, &repositories)
                    {
                        return Err(protocol());
                    }
                    r.repositories = repositories;
                    r.summary.session_id = Some(session_id);
                    r.summary.status = "ready".into();
                    r.summary.message = READY.into();
                }
                LocalEvent::Dispatching => {
                    if r.summary.status != "ready" || r.may_have_sent {
                        return Err(protocol());
                    }
                    r.may_have_sent = true;
                    r.summary.status = "running".into();
                    r.summary.message = RUNNING.into();
                }
                LocalEvent::Reply {
                    session_id,
                    message_id,
                    text,
                } => {
                    if r.summary.status != "running"
                        || r.reply.is_some()
                        || r.summary.session_id.as_deref() != Some(&session_id)
                        || !token(&message_id)
                        || text.trim().is_empty()
                        || text.len() > crate::dev_local::MAX_REPLY
                    {
                        return Err(protocol());
                    }
                    r.reply = Some(text);
                    r.message_id = Some(message_id);
                    r.summary.reply_available = true;
                    r.summary.message = CAPTURED.into();
                }
            }
            Ok(())
        })
    }
    fn completion_fence(&self, id: &str) -> AppResult<()> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        self.available(&state)?;
        if !state.ledger.records.iter().any(|r| {
            r.summary.request_id == id
                && r.summary.status == "running"
                && r.may_have_sent
                && r.reply.is_none()
        }) {
            return Err(protocol());
        }
        Ok(())
    }
    fn finish(
        &self,
        id: &str,
        result: std::result::Result<LocalReceipt, crate::dev_local::LocalFailure>,
    ) -> AppResult<()> {
        self.update(id, |r| {
            match result {
                Ok(receipt) => {
                    if r.summary.status != "running"
                        || r.summary.session_id.as_deref() != Some(&receipt.session_id)
                        || r.message_id.as_deref() != Some(&receipt.message_id)
                        || r.reply.as_deref() != Some(&receipt.reply)
                        || r.repositories != receipt.repositories
                        || !checkouts(&r.job, &receipt.repositories)
                    {
                        r.failed_stage = Some(r.summary.status.clone());
                        r.summary.status = "unknown".into();
                        r.summary.message = LocalError::Protocol.message().into();
                    } else {
                        r.summary.status = "completed".into();
                        r.summary.message = COMPLETED.into();
                    }
                    // Ok is the runtime contract's verified-owned-stop evidence, not merely a reply.
                    r.cleanup_required = false;
                    r.summary.cleanup_verified = true;
                }
                Err(failure) => {
                    r.failed_stage = Some(r.summary.status.clone());
                    r.may_have_sent |= failure.may_have_sent;
                    r.cleanup_required = failure.cleanup_unknown;
                    r.summary.cleanup_verified = !failure.cleanup_unknown;
                    r.summary.status = if failure.cleanup_unknown {
                        "cleanup_unknown"
                    } else if r.may_have_sent || r.reply.is_some() {
                        "unknown"
                    } else {
                        "failed"
                    }
                    .into();
                    r.summary.message = failure.error.message().into();
                }
            }
            Ok(())
        })
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    request_id: String,
    instance_id: String,
    project_id: String,
    expected_projects_revision: u64,
    expected_config_revision: String,
    prompt: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delete {
    request_id: String,
    instance_id: String,
}
fn model_state(inner: &Inner) -> (Value, Option<FrozenModel>) {
    let name = inner.durable.config.model.as_deref().unwrap_or("");
    let selected = crate::catalog::CATALOG.iter().find(|m| m.name == name);
    let model = selected
        .filter(|m| {
            inner.inventory_observed
                && inner.inventory.providers.llm.ready
                && inner.inventory.providers.llm.error.is_none()
                && inner
                    .inventory
                    .models
                    .iter()
                    .filter(|i| i.name == name)
                    .count()
                    == 1
                && inner
                    .inventory
                    .models
                    .iter()
                    .any(|i| i.name == name && i.digest == m.sha256)
        })
        .map(|m| FrozenModel {
            name: m.name.into(),
            sha256: m.sha256.into(),
        });
    (
        json!({"name":name,"sha256":selected.map(|m|m.sha256),"config_revision":inner.durable.config.revision.to_string(),"ready":model.is_some()}),
        model,
    )
}
impl Application {
    #[cfg(test)]
    pub(super) fn dev_reserved(&self) -> bool {
        self.dev_jobs.local.as_ref().is_some_and(|o| o.reserved())
    }
    /// Integrity, not occupancy: a healthy full scheduler still permits commands
    /// against already-retained environments.
    pub(super) fn retained_inspection_integrity(&self) -> AppResult<()> {
        let (_, unsafe_legacy, _) = self.dev_jobs.legacy_state()?;
        if unsafe_legacy {
            return Err(unavailable());
        }
        let Some(owner) = self.dev_jobs.local.as_ref() else {
            return Ok(());
        };
        let mut state = owner.state.lock().map_err(|_| unavailable())?;
        owner.available(&state)?;
        let unchanged = filesystem::read_file(&owner.path, 0o600, LIMIT)
            .map(|bytes| bytes.as_ref().map(|b| filesystem::digest(b)) == state.disk_digest)
            .unwrap_or(false);
        if state.publication_ambiguous || !unchanged {
            state.fault = true;
            state.publication_ambiguous = true;
            state.pending = None;
            state.ledger.interrupt();
            owner.cancelled.store(true, Ordering::Release);
            return Err(unavailable());
        }
        Ok(())
    }
    pub(super) fn retained_inspection_occupied(&self) -> AppResult<usize> {
        let (_, legacy_unsafe, _) = self.dev_jobs.legacy_state()?;
        if legacy_unsafe {
            return Ok(crate::app::retained_jobs::SLOT_LIMIT);
        }
        let Some(owner) = self.dev_jobs.local.as_ref() else {
            return Ok(0);
        };
        let state = owner.state.lock().map_err(|_| unavailable())?;
        owner.available(&state)?;
        Ok(state
            .ledger
            .records
            .iter()
            .filter(|r| active(&r.summary.status) || uncertain(r))
            .count())
    }
    pub(super) fn call_dev_local(&self, req: AppRequest) -> AppResult<Value> {
        if matches!(
            req.op.as_str(),
            "dev_job_start_v2" | "dev_jobs_state_v2" | "dev_job_get_v2"
        ) {
            return Err(AppError::new(
                "UPGRADE_REQUIRED",
                "Use scheduler-aware Dev operations",
            ));
        }
        if !req.args.is_object() {
            return Err(invalid());
        }
        match req.op.as_str() {
            "dev_job_start_v4" => self.start_dev_local(req.args),

            "dev_job_delete_v3" => self.delete_dev_local(req.args),
            "dev_job_deletion_v3" => {
                let a: Get = decode(req.args)?;
                if !valid_id(&a.request_id) {
                    return Err(invalid());
                }
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault || !inner.owned {
                    return Err(storage());
                }
                let owner = self.dev_jobs.local.as_ref().ok_or_else(unavailable)?;
                let state = owner.state.lock().map_err(|_| unavailable())?;
                owner.available(&state)?;
                let r = state
                    .ledger
                    .records
                    .iter()
                    .find(|r| r.summary.request_id == a.request_id)
                    .ok_or_else(unknown)?;
                bounded(json!({"job":r.summary,"deleted":r.deleted}))
            }
            "dev_jobs_state_v4" => {
                let _: Empty = decode(req.args)?;
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault || !inner.owned {
                    return Err(storage());
                }
                let (legacy_count, legacy_unsafe, _) = self.dev_jobs.legacy_state()?;
                let owner = self.dev_jobs.local.as_ref().ok_or_else(unavailable)?;
                let state = owner.state.lock().map_err(|_| unavailable())?;
                if state.fault {
                    return Err(unavailable());
                }
                let (model, frozen) = model_state(&inner);
                let occupied = state
                    .ledger
                    .records
                    .iter()
                    .filter(|r| active(&r.summary.status) || uncertain(r))
                    .count()
                    .saturating_add(self.retained_reserved());
                let queued = state
                    .ledger
                    .records
                    .iter()
                    .filter(|r| r.summary.status == "queued")
                    .count();
                let bytes = serde_json::to_vec(&state.ledger)
                    .map_err(|_| unavailable())?
                    .len();
                let storage_ready = result_capacity(
                    &state.ledger,
                    bytes.saturating_add(RESULT_RESERVE + crate::dev_local::MAX_PROMPT),
                );
                let accepting = owner.available(&state).is_ok()
                    && !legacy_unsafe
                    && frozen.is_some()
                    && storage_ready
                    && !inner.durable.jobs.iter().any(|j| {
                        v2::pending(&j.operation)
                            && matches!(j.task.as_ref(), Some(Task::Remove(_)))
                    });
                bounded(
                    json!({"jobs":state.ledger.records.iter().filter(|r| !r.deleted).map(|r| &r.summary).collect::<Vec<_>>(),"used":legacy_count + state.ledger.records.len(),"model":model,"scheduler":{"limit":3,"occupied":occupied,"queued":queued,"accepting":accepting}}),
                )
            }
            "dev_job_get_v4" => {
                let a: Get = decode(req.args)?;
                if !valid_id(&a.request_id) {
                    return Err(invalid());
                }
                let owner = self.dev_jobs.local.as_ref().ok_or_else(unavailable)?;
                let state = owner.state.lock().map_err(|_| unavailable())?;
                if state.fault {
                    return Err(unavailable());
                }
                let r = state
                    .ledger
                    .records
                    .iter()
                    .find(|r| r.summary.request_id == a.request_id)
                    .ok_or_else(unknown)?;
                if r.deleted {
                    return Err(deleted());
                }
                bounded(
                    json!({"job":r.summary,"model":r.job.model,"repositories":r.repositories,"reply":r.reply,"failed_stage":r.failed_stage}),
                )
            }
            _ => Err(invalid()),
        }
    }
    fn delete_dev_local(&self, args: Value) -> AppResult<Value> {
        let a: Delete = decode(args)?;
        if !valid_id(&a.request_id) || !valid_id(&a.instance_id) {
            return Err(invalid());
        }
        // Match Start's app -> local lock order; deletion never touches runtime ownership.
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault || !inner.owned {
            return Err(storage());
        }
        if a.instance_id != self.instance_id {
            return Err(AppError::new("STALE_INSTANCE", "Service instance changed"));
        }
        let owner = self.dev_jobs.local.as_ref().ok_or_else(unavailable)?;
        let mut state = owner.state.lock().map_err(|_| unavailable())?;
        owner.available(&state)?;
        let index = state
            .ledger
            .records
            .iter()
            .position(|r| r.summary.request_id == a.request_id)
            .ok_or_else(unknown)?;
        let r = &state.ledger.records[index];
        if !deletable(r) {
            return Err(AppError::new(
                "JOB_NOT_DELETABLE",
                "Dev job cleanup is not verified terminal",
            ));
        }
        let result = bounded(json!({"job":r.summary,"deleted":true}))?;
        if r.deleted {
            return Ok(result);
        }
        let mut next = state.ledger.clone();
        next.schema_version = next.schema_version.max(3);
        next.records[index].deleted = true;
        if !result_capacity(
            &next,
            serde_json::to_vec(&next).map_err(|_| unavailable())?.len(),
        ) {
            return Err(v2::capacity());
        }
        owner.commit(&mut state, next)?;
        Ok(result)
    }
    fn start_dev_local(&self, args: Value) -> AppResult<Value> {
        let a: Start = decode(args)?;
        if !valid_id(&a.request_id) {
            return Err(invalid());
        }
        let fp = fingerprint(
            "dev_job_start:2",
            &serde_json::to_value(&a).map_err(|_| invalid())?,
        )?;
        // Global app admission and the local reservation share this lock order.
        let inner = self.inner.lock().map_err(|_| storage())?;
        let owner = self.dev_jobs.local.as_ref().ok_or_else(unavailable)?;
        let mut state = owner.state.lock().map_err(|_| unavailable())?;
        owner.available(&state)?;
        if let Some(r) = state
            .ledger
            .records
            .iter()
            .find(|r| r.summary.request_id == a.request_id)
        {
            return if r.deleted {
                Err(deleted())
            } else if r.fingerprint == fp {
                bounded(json!(r.summary))
            } else {
                Err(AppError::new(
                    "REQUEST_CONFLICT",
                    "Dev request identity already used",
                ))
            };
        }
        // Retained legacy metadata only: no runtime observation or ledger mutation.
        let (_, legacy_unsafe, legacy_ids) = self.dev_jobs.legacy_state()?;
        if legacy_ids.contains(&a.request_id) {
            return Err(AppError::new(
                "REQUEST_EXPIRED",
                "Request identity belongs to retained legacy Dev work; no resend",
            ));
        }
        if inner.fault || !inner.owned {
            return Err(storage());
        }
        if legacy_unsafe {
            return Err(AppError::new(
                "CLEANUP_UNKNOWN",
                "Prior Dev work requires separately authorized reconciliation; no replay",
            ));
        }
        if inner
            .durable
            .jobs
            .iter()
            .any(|j| v2::pending(&j.operation) && matches!(j.task.as_ref(), Some(Task::Remove(_))))
        {
            return Err(AppError::new(
                "BUSY",
                "Shared provider capacity is already reserved",
            ));
        }
        if revision(&a.expected_config_revision).is_none()
            || !valid_id(&a.instance_id)
            || !valid_id(&a.project_id)
            || a.prompt.trim().is_empty()
            || a.prompt.len() > MAX_TEXT
            || a.prompt
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(invalid());
        }
        if a.instance_id != self.instance_id {
            return Err(AppError::new("STALE_INSTANCE", "Service instance changed"));
        }
        let projects = serde_json::to_value(&inner.durable.projects).map_err(|_| unavailable())?;
        if projects["revision"].as_u64() != Some(a.expected_projects_revision)
            || a.expected_config_revision != inner.durable.config.revision.to_string()
        {
            return Err(v2::stale());
        }
        let p = projects["projects"]
            .as_array()
            .and_then(|ps| {
                ps.iter()
                    .find(|p| p["project_id"].as_str() == Some(&a.project_id))
            })
            .ok_or_else(invalid)?;
        let (_, model) = model_state(&inner);
        let model = model.ok_or_else(|| {
            AppError::new(
                "LOCAL_MODEL_UNAVAILABLE",
                LocalError::ModelUnavailable.message(),
            )
        })?;
        let repositories = saved_repositories(p)?;
        let prompt = format!(
            "Global:\n{}\nProject:\n{}\nTask:\n{}",
            inner.durable.instructions.dev_settings_text(),
            p["prompt"].as_str().ok_or_else(invalid)?,
            a.prompt
        );
        // prompt_for appends immutable checkout identities; reserve their exact maximum shape.
        let job = LocalJob {
            request_id: a.request_id.clone(),
            prompt,
            model,
            repositories,
        };
        let maximum_checkouts = job
            .repositories
            .iter()
            .map(|r| Checkout {
                url: r.url.clone(),
                reference: r.reference.clone().unwrap_or_else(|| "HEAD".into()),
                commit: "0".repeat(40),
            })
            .collect::<Vec<_>>();
        crate::dev_local::prompt_for(&job, &maximum_checkouts)
            .map_err(|_| AppError::new("CONTEXT_LIMIT", LocalError::ModelLimit.message()))?;
        let summary = Summary {
            request_id: a.request_id.clone(),
            project_id: a.project_id,
            project_key: p["key"].as_str().ok_or_else(invalid)?.into(),
            status: "queued".into(),
            message: QUEUED.into(),
            session_id: None,
            created_ms: self.cloud.clock.wall_ms(),
            reply_available: false,
            cleanup_verified: true,
        };
        let mut next = state.ledger.clone();
        next.schema_version = 4;
        next.records.push(Record {
            summary: summary.clone(),
            fingerprint: fp,
            config_revision: a.expected_config_revision,
            projects_revision: a.expected_projects_revision,
            job,
            repositories: Vec::new(),
            image_id: None,
            message_id: None,
            reply: None,
            failed_stage: None,
            may_have_sent: false,
            cleanup_required: false,
            deleted: false,
        });
        let bytes = serde_json::to_vec(&next).map_err(|_| unavailable())?;
        if !result_capacity(&next, bytes.len()) {
            return Err(v2::capacity());
        }
        // Reserve the largest summary projection, not just today's preparing rows.
        let maximum_message = messages()
            .max_by_key(|s| serde_json::to_vec(s).map_or(0, |bytes| bytes.len()))
            .ok_or_else(unavailable)?;
        let summaries = next
            .records
            .iter()
            .filter(|r| !r.deleted)
            .map(|r| {
                let mut s = r.summary.clone();
                s.status = "cleanup_unknown".into();
                s.message = maximum_message.into();
                s.session_id = Some("x".repeat(256));
                s.reply_available = false;
                s.cleanup_verified = false;
                s
            })
            .collect::<Vec<_>>();
        let (mut model, _) = model_state(&inner);
        model["ready"] = json!(false);
        bounded(
            json!({"jobs":summaries,"used":usize::MAX,"model":model,"scheduler":{"limit":3,"occupied":3,"queued":usize::MAX,"accepting":false}}),
        )?;
        owner.commit(&mut state, next)?;

        drop(state);
        drop(inner);
        self.wake()?; // Publication and reservation precede waking the SAME app worker.
        Ok(json!(summary))
    }
    // Compatibility seam for deterministic fixtures, never an inference owner.
    #[cfg(test)]
    pub(super) fn run_pending_dev(&self, provider: &dyn Provider) -> AppResult<bool> {
        {
            let inner = self.inner.lock().map_err(|_| storage())?;
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            if let Some(owner) = &self.dev_jobs.local {
                let state = owner.state.lock().map_err(|_| unavailable())?;
                if state.fault || !state.owned {
                    return Ok(false);
                }
            }
        }
        let (wake, receiver) = mpsc::sync_channel(1);
        let mut dispatched = self.dispatch_dev(wake.clone())?;
        let Some(owner) = self.dev_jobs.local.as_ref() else {
            return Ok(false);
        };
        loop {
            dispatched |= self.dispatch_dev(wake.clone())?;
            self.run_dev_completion(provider)?;
            if !owner.scheduler.has_workers()? {
                break;
            }
            let _ = receiver.recv_timeout(Duration::from_millis(20));
        }
        owner.scheduler.reap_finished();
        let state = owner.state.lock().map_err(|_| unavailable())?;
        owner.available(&state)?;
        Ok(dispatched)
    }
}
fn saved_repositories(project: &Value) -> AppResult<Vec<Repository>> {
    let saved = project["repositories"].as_array().ok_or_else(invalid)?;
    if saved.is_empty() || saved.len() > 16 {
        return Err(AppError::new(
            "REPOSITORY_UNSUPPORTED",
            LocalError::RepositoryRejected.message(),
        ));
    }
    saved
        .iter()
        .map(|r| {
            let url = r["url"].as_str().ok_or_else(invalid)?.to_owned();
            let base = &r["branch"]["base"];
            let reference = if base.is_null() {
                None
            } else {
                Some(base.as_str().ok_or_else(invalid)?.to_owned())
            };
            let repository = Repository { url, reference };
            crate::dev_fetch::validate_repository(&repository).map_err(|_| {
                AppError::new(
                    "REPOSITORY_UNSUPPORTED",
                    LocalError::RepositoryRejected.message(),
                )
            })?;
            Ok(repository)
        })
        .collect()
}

#[cfg(test)]
#[path = "app_dev_local_tests.rs"]
mod tests;

#[path = "app_dev_scheduler.rs"]
mod scheduler;
