//! Bounded application domain. Providers run only on the single worker.
use crate::filesystem;
use crate::providers::{Inventory, Provider, Providers};
use crate::{Result, Roots};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
#[path = "app_v2.rs"]
mod v2;
use v2::{Durable, Job};
#[path = "app_agents.rs"]
mod agents;
#[path = "app_dev_jobs.rs"]
mod dev_jobs;
#[path = "app_instructions.rs"]
mod instructions;
#[path = "app_dev_local.rs"]
mod local_dev;
#[path = "app_projects.rs"]
mod projects;
#[path = "app_retained_jobs.rs"]
mod retained_api;
#[path = "retained_jobs.rs"]
pub mod retained_jobs;
#[path = "app_timestamps.rs"]
mod timestamps;
#[path = "app_v3.rs"]
mod v3;

pub const FRAME_LIMIT: usize = 4 * 1024 * 1024;
pub const MAX_JOBS: usize = 32;
pub const MAX_TEXT: usize = 16 * 1024;
pub const MAX_CONTEXT: usize = 64 * 1024;
const STATE_LIMIT: u64 = 16 * 1024 * 1024;
const MAX_EXPIRED_IDS: usize = 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppError {
    pub code: String,
    pub message: String,
}
impl AppError {
    pub(crate) fn valid(&self) -> bool {
        self.code
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_uppercase)
            && self.code.len() <= 256
            && self
                .code
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            && !self.message.trim().is_empty()
            && self.message.len() <= 1024
    }
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
pub type AppResult<T> = std::result::Result<T, AppError>;
fn invalid() -> AppError {
    AppError::new("INVALID_REQUEST", "Invalid application request")
}
fn retired() -> AppError {
    AppError::new("RETIRED", "Speech capability is retired; use text chat")
}
fn storage() -> AppError {
    AppError::new(
        "STATE_UNAVAILABLE",
        "Application state could not be safely published; inspect before retry",
    )
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
pub fn new_id() -> Result<String> {
    use std::io::Read;
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| "Cannot create instance identity")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppRequest {
    pub schema_version: u32,
    pub service: String,
    pub protocol: String,
    pub call_id: String,
    pub op: String,
    pub args: Value,
}
impl std::fmt::Debug for AppRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppRequest")
            .field("schema_version", &self.schema_version)
            .field("op", &self.op)
            .field("args", &"[REDACTED]")
            .finish()
    }
}
impl AppRequest {
    pub fn validate(&self) -> AppResult<()> {
        if ![1, 2, 3].contains(&self.schema_version)
            || self.service != "smith-next"
            || self.protocol != "app"
            || !valid_id(&self.call_id)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppReply {
    pub schema_version: u32,
    pub service: String,
    pub protocol: String,
    pub pid: u32,
    pub instance_id: String,
    pub call_id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<AppError>,
}
impl AppReply {
    pub fn from_result(instance: &str, call: &str, result: AppResult<Value>) -> Self {
        Self::from_versioned_result(1, instance, call, result)
    }
    pub fn from_versioned_result(
        version: u32,
        instance: &str,
        call: &str,
        result: AppResult<Value>,
    ) -> Self {
        let (result, error) = match result {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            schema_version: version,
            service: "smith-next".into(),
            protocol: "app".into(),
            pid: std::process::id(),
            instance_id: instance.into(),
            call_id: call.into(),
            ok: error.is_none(),
            result,
            error,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub model: Option<String>,
    pub voice: Option<String>,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: String,
    pub content: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub request_id: String,
    pub kind: String,
    pub status: String,
    pub phase: String,
    pub config_revision: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub result: Option<Value>,
    #[serde(deserialize_with = "required_nullable")]
    pub error: Option<AppError>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyJob {
    operation: Operation,
    fingerprint: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDurable {
    schema_version: u32,
    config: Config,
    history: Vec<Message>,
    jobs: Vec<LegacyJob>,
    expired_ids: Vec<String>,
}
impl Default for LegacyDurable {
    fn default() -> Self {
        Self {
            schema_version: 1,
            config: Config::default(),
            history: vec![],
            jobs: vec![],
            expired_ids: vec![],
        }
    }
}
impl LegacyDurable {
    fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        if self.schema_version != 1
            || self.jobs.len() > MAX_JOBS
            || self
                .jobs
                .iter()
                .filter(|j| j.operation.status == "running")
                .count()
                > 1
            || self.history.len() > 32
            || self.history.len() % 2 != 0
            || self.history.iter().map(|m| m.content.len()).sum::<usize>() > MAX_CONTEXT
        {
            return Err("Unsupported or corrupt application state; preserved".into());
        }
        if self.expired_ids.len() > MAX_EXPIRED_IDS
            || self
                .expired_ids
                .iter()
                .any(|id| !valid_id(id) || !ids.insert(id))
        {
            return Err("Invalid expiry metadata; preserved".into());
        }
        for (i, m) in self.history.iter().enumerate() {
            if m.role != if i % 2 == 0 { "user" } else { "assistant" }
                || m.content.len() > if i % 2 == 0 { MAX_TEXT } else { MAX_CONTEXT }
            {
                return Err("Invalid application history; preserved".into());
            }
        }
        if self
            .config
            .model
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 256)
            || self
                .config
                .voice
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 256)
        {
            return Err("Invalid application configuration; preserved".into());
        }
        for j in &self.jobs {
            let o = &j.operation;
            if !valid_id(&o.request_id)
                || !ids.insert(&o.request_id)
                || j.fingerprint.len() != 64
                || !j
                    .fingerprint
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                || ![
                    "models",
                    "submit",
                    "transcribe",
                    "speak",
                    "verify_model",
                    "remove_model",
                ]
                .contains(&o.kind.as_str())
                || !["running", "succeeded", "failed", "interrupted"].contains(&o.status.as_str())
                || ![
                    "inventory",
                    "verification",
                    "removal",
                    "transcription",
                    "generation",
                    "synthesis",
                    "complete",
                ]
                .contains(&o.phase.as_str())
                || o.config_revision > self.config.revision
                || !valid_operation(o)
            {
                return Err("Invalid operation metadata; preserved".into());
            }
        }
        if !reply_fits(&state_result(self, &Inventory::default())) {
            return Err("Application state reply exceeds frame limit; preserved".into());
        }
        Ok(())
    }
}
fn state_result(durable: &LegacyDurable, inventory: &Inventory) -> Value {
    json!({"config":durable.config,"history":durable.history,"active":durable.jobs.iter().find(|j|j.operation.status=="running").map(|j|&j.operation),"providers":inventory.providers,"retention":{"max_turn_pairs":16,"max_context_bytes":MAX_CONTEXT,"max_terminal_jobs":MAX_JOBS,"max_expired_ids":MAX_EXPIRED_IDS,"capacity_policy":"reject_when_expiry_metadata_full","audio":"transient","missing_request_code":"REQUEST_EXPIRED"}})
}
fn valid_operation(o: &Operation) -> bool {
    if !reply_fits(o) {
        return false;
    }
    if o.status == "running" {
        return o.result.is_none()
            && o.error.is_none()
            && match o.kind.as_str() {
                "models" => o.phase == "inventory",
                "verify_model" => o.phase == "verification",
                "remove_model" => o.phase == "removal",
                "transcribe" => o.phase == "transcription",
                "submit" => ["generation", "synthesis"].contains(&o.phase.as_str()),
                "speak" => o.phase == "synthesis",
                _ => false,
            };
    }
    if o.phase != "complete" {
        return false;
    }
    if let Some(e) = &o.error {
        if !e.valid() {
            return false;
        }
    }
    if o.status != "succeeded" {
        return o.result.is_none() && o.error.is_some();
    }
    if o.kind == "speak" {
        return o.result.is_none() && o.error.as_ref().is_some_and(|e| e.code == "AUDIO_EXPIRED");
    }
    if o.error.is_some() {
        return false;
    }
    let Some(v) = o.result.clone() else {
        return false;
    };
    match o.kind.as_str() {
        "models" => serde_json::from_value::<Inventory>(v).is_ok_and(|i| i.valid()),
        "verify_model" => serde_json::from_value::<VerifyResult>(v).is_ok_and(|r| {
            model_identity(&r.model, &r.sha256)
                && r.verified
                && !r.assistant_text.trim().is_empty()
                && r.assistant_text.len() <= MAX_CONTEXT
        }),
        "remove_model" => serde_json::from_value::<RemoveResult>(v)
            .is_ok_and(|r| model_identity(&r.model, &r.sha256) && r.removed),
        "submit" => serde_json::from_value::<TurnResult>(v).is_ok_and(|r| {
            !r.assistant_text.is_empty()
                && r.assistant_text.len() <= MAX_CONTEXT
                && r.audio.is_none()
                && r.speech_error.as_ref().is_none_or(AppError::valid)
        }),
        "transcribe" => serde_json::from_value::<TranscriptResult>(v).is_ok_and(|r| {
            r.text.len() <= MAX_TEXT && r.language.len() <= 64 && r.duration_ms <= 60_000
        }),
        _ => false,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyResult {
    model: String,
    sha256: String,
    verified: bool,
    assistant_text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveResult {
    model: String,
    sha256: String,
    removed: bool,
}
fn model_identity(model: &str, sha: &str) -> bool {
    !model.is_empty()
        && model.len() <= 256
        && model
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/:_.-".contains(&c))
        && sha.len() == 64
        && sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TurnResult {
    assistant_text: String,
    #[serde(deserialize_with = "required_nullable")]
    audio: Option<()>,
    #[serde(deserialize_with = "required_nullable")]
    speech_error: Option<AppError>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscriptResult {
    text: String,
    language: String,
    duration_ms: u64,
}
pub(crate) fn required_nullable<'de, D, T>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
pub(crate) fn reply_fits(value: &impl Serialize) -> bool {
    serde_json::to_vec(&json!({"schema_version":1,"service":"smith-next","protocol":"app","pid":u32::MAX,"instance_id":"0".repeat(32),"call_id":"0".repeat(32),"ok":true,"result":value,"error":null})).is_ok_and(|bytes| bytes.len() <= FRAME_LIMIT)
}
fn provider_protocol() -> AppError {
    AppError::new(
        "PROVIDER_PROTOCOL",
        "Provider returned invalid or oversized application metadata",
    )
}
fn bounded_error(error: AppError) -> AppError {
    if error.valid() {
        error
    } else {
        provider_protocol()
    }
}
struct Inner {
    durable: Durable,
    inventory: Inventory,
    fault: bool,
    owned: bool,
    inventory_observed: bool,
    cloud: v3::Volatile,
}
struct WorkerOwner {
    dev_jobs: Arc<dev_jobs::Owner>,
    provider: Arc<dyn Provider>,
    cloud: Arc<crate::cloud::Dependencies>,
    inner: Arc<Mutex<Inner>>,
    sender: Option<mpsc::SyncSender<()>>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for WorkerOwner {
    fn drop(&mut self) {
        // The publication fence shares the commit mutex: after revocation, even a
        // provider that completes after reopening these roots cannot write old state.
        if let Ok(mut inner) = self.inner.lock() {
            inner.owned = false;
        }
        self.dev_jobs.close();
        self.cloud.cancel();
        let join_cleanup = self.provider.cancel_owned();
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(());
        }
        if let Some(thread) = self.thread.take() {
            if join_cleanup {
                let _ = thread.join();
                return;
            }
            let deadline = Instant::now() + Duration::from_millis(250);
            while !thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
            // A blocking provider cannot be forcibly cancelled. An unfinished
            // thread is detached only after fencing it; production calls have
            // finite budgets and cannot enter another publication or phase.
        }
    }
}
#[derive(Clone)]
pub struct Application {
    inner: Arc<Mutex<Inner>>,
    owner: Option<Arc<WorkerOwner>>,
    dev_jobs: Arc<dev_jobs::Owner>,
    path: PathBuf,
    pub instance_id: String,
    cloud: Arc<crate::cloud::Dependencies>,
}
struct Work {
    id: String,
    task: Task,
    config: Config,
    history: Vec<Message>,
    conversation_id: Option<String>,
    history_revision: Option<u64>,
    instructions: Option<instructions::Snapshot>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Task {
    Models,
    Submit { text: String },
    Verify(ModelArgs),
    Remove(ModelArgs),
    Routed(v3::RoutedTask),
    Provider(v3::ProviderTask),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdArgs {
    request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PollArgs {
    request_id: String,
    instance_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigArgs {
    model: String,
    #[serde(deserialize_with = "required_nullable")]
    voice: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedConfigArgs {
    model: String,
    #[serde(deserialize_with = "required_nullable")]
    voice: Option<String>,
    expected_revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelArgs {
    request_id: String,
    model: String,
    sha256: String,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitArgs {
    request_id: String,
    text: String,
    output: String,
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> AppResult<T> {
    serde_json::from_value(v).map_err(|_| invalid())
}
fn text(value: &str, limit: usize) -> AppResult<()> {
    if value.trim().is_empty() || value.len() > limit {
        Err(AppError::new(
            "TEXT_LIMIT",
            "Text is empty or exceeds the byte limit",
        ))
    } else {
        Ok(())
    }
}

impl Application {
    pub fn open(roots: &Roots, provider: Arc<dyn Provider>) -> Result<Self> {
        Self::open_with_cloud(roots, provider, crate::cloud::Dependencies::disabled())
    }
    /// Isolated injection; production is the only constructor that enables real keyring/HTTP.
    pub fn open_with_cloud(
        roots: &Roots,
        provider: Arc<dyn Provider>,
        cloud: crate::cloud::Dependencies,
    ) -> Result<Self> {
        Self::open_with_runtime(roots, provider, cloud, None)
    }
    /// Trusted in-process injection only; no environment or production capability switch.
    /// The runtime must bound execute and cooperate with cancel during shutdown.
    pub fn open_with_retained_runtime(
        roots: &Roots,
        provider: Arc<dyn Provider>,
        cloud: crate::cloud::Dependencies,
        runtime: Arc<dyn retained_jobs::Runtime>,
    ) -> Result<Self> {
        Self::open_with_runtime(roots, provider, cloud, Some(runtime))
    }
    fn open_with_runtime(
        roots: &Roots,
        provider: Arc<dyn Provider>,
        cloud: crate::cloud::Dependencies,
        runtime: Option<Arc<dyn retained_jobs::Runtime>>,
    ) -> Result<Self> {
        let cloud = Arc::new(cloud);
        let path = roots.data.join("smith-next/app-state.json");
        filesystem::check_directory(&roots.data.join("smith-next"), true)?;
        let (mut durable, migrated) = match filesystem::read_file(&path, 0o600, STATE_LIMIT)? {
            Some(bytes) => Durable::load(&bytes)?,
            None => (projects::bootstrap(&path)?, false),
        };
        durable.validate()?;
        let mut changed = migrated;
        for j in &mut durable.jobs {
            if v2::pending(&j.operation) {
                j.operation.status = "interrupted".into();
                j.operation.phase = "complete".into();
                j.operation.result = None;
                j.operation.error = Some(v3::restart_error(j));
                v3::reconcile_validation(&mut durable.cloud, j, cloud.clock.wall_ms());
                changed = true;
            }
        }
        changed |= durable.instructions.activate(&mut durable.config)?;
        if changed {
            durable.revision = durable
                .revision
                .checked_add(1)
                .ok_or("State revision exhausted; preserved")?;
            persist(&path, &durable)?;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let inner = Arc::new(Mutex::new(Inner {
            durable,
            inventory: Inventory::default(),
            fault: false,
            owned: true,
            inventory_observed: false,
            cloud: v3::Volatile::default(),
        }));
        let instance_id = new_id()?;
        let mut dev_owner = dev_jobs::Owner::open(
            roots.data.join("smith-next/dev-jobs.json"),
            Arc::new(crate::dev_runtime::DockerRuntime::new(
                roots.runtime.clone(),
            )),
        );
        dev_owner.local = Some(Arc::new(local_dev::Owner::open(
            roots.data.join("smith-next/dev-local-jobs.json"),
            Arc::new(crate::dev_runtime::DockerLocalRuntime::new(
                roots.runtime.clone(),
            )),
        )));
        dev_owner.retained_runtime = runtime;
        let dev_jobs = Arc::new(dev_owner);
        let worker = Self {
            inner: inner.clone(),
            owner: None,
            dev_jobs: dev_jobs.clone(),
            path: path.clone(),
            instance_id: instance_id.clone(),
            cloud: cloud.clone(),
        };
        let owner_provider = provider.clone();
        let scheduler_sender = sender.clone();
        let thread = std::thread::Builder::new()
            .name("smith-app-worker".into())
            .spawn(move || {
                // This thread is the only runner owner: idle stop uses its own
                // receive timeout and shutdown happens here on exit.
                let mut prefer_dev = true;
                loop {
                    if !worker.inner.lock().is_ok_and(|inner| inner.owned) {
                        break;
                    }
                    let _ = worker.dispatch_dev(scheduler_sender.clone());
                    if matches!(worker.run_retained_command(), Ok(true)) {
                        continue;
                    }
                    if prefer_dev
                        && matches!(worker.run_dev_completion(provider.as_ref()), Ok(true))
                    {
                        prefer_dev = false;
                        continue;
                    }
                    match worker.next_work() {
                        Ok(Some(work)) => {
                            worker.run(work, provider.as_ref());
                            prefer_dev = true;
                            continue;
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                    if matches!(worker.run_dev_completion(provider.as_ref()), Ok(true)) {
                        prefer_dev = false;
                        continue;
                    }
                    let wait = match (provider.idle_timeout(), worker.dev_queue_timeout()) {
                        (Some(provider), Some(queue)) => Some(provider.min(queue)),
                        (provider, queue) => provider.or(queue),
                    };
                    match wait {
                        Some(wait) => match receiver.recv_timeout(wait) {
                            Ok(work) => work,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                provider.idle();
                                continue;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        },
                        None => match receiver.recv() {
                            Ok(work) => work,
                            Err(_) => break,
                        },
                    };
                    if !worker.inner.lock().is_ok_and(|inner| inner.owned) {
                        break;
                    }
                }
                worker.join_dev_workers();
                provider.shutdown();
            })
            .map_err(|_| "Cannot start provider worker")?;
        Ok(Self {
            owner: Some(Arc::new(WorkerOwner {
                dev_jobs: dev_jobs.clone(),
                provider: owner_provider,
                cloud: cloud.clone(),
                inner: inner.clone(),
                sender: Some(sender),
                thread: Some(thread),
            })),
            inner,
            dev_jobs,

            path,
            instance_id,
            cloud,
        })
    }
    pub fn production(roots: &Roots) -> Result<Self> {
        Self::open_with_cloud(
            roots,
            Arc::new(Providers::new(roots)?),
            crate::cloud::Dependencies::production(),
        )
    }
    pub fn call(&self, req: AppRequest) -> AppResult<Value> {
        req.validate()?;
        let encoded = zeroize::Zeroizing::new(serde_json::to_vec(&req).map_err(|_| invalid())?);
        if encoded.len() > FRAME_LIMIT {
            return Err(invalid());
        }
        drop(encoded);
        if req.schema_version == 3 {
            return self.call_v3(req);
        }
        self.legacy_guard(&req)?;
        if req.schema_version == 2 {
            return self.call_v2(req);
        }
        self.call_common(req)
    }
    fn call_common(&self, req: AppRequest) -> AppResult<Value> {
        // Legacy identities are resolved before today's admission validation. No task
        // is constructed or dispatched for a retained request, even if retired.
        if [
            "models",
            "submit",
            "transcribe",
            "speak",
            "verify_model",
            "remove_model",
        ]
        .contains(&req.op.as_str())
        {
            let id = req
                .args
                .get("request_id")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            if !valid_id(id) {
                return Err(invalid());
            }
            let fingerprint = v2::request_fingerprint(req.schema_version, &req.op, &req.args)?;
            let inner = self.inner.lock().map_err(|_| storage())?;
            if let Some(result) = retained(&inner, id, &fingerprint)? {
                return Ok(result);
            }
        }
        match req.op.as_str() {
            "state" => {
                let _: Empty = decode(req.args)?;
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
                }
                Ok(state_result(&inner.durable.legacy(), &inner.inventory))
            }
            "poll" => {
                let a: PollArgs = decode(req.args)?;
                if !valid_id(&a.request_id) || !valid_id(&a.instance_id) {
                    return Err(invalid());
                }
                if a.instance_id != self.instance_id {
                    return Err(AppError::new(
                        "INSTANCE_MISMATCH",
                        "Service instance changed; synchronize without resending",
                    ));
                }
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
                }
                inner
                    .durable
                    .jobs
                    .iter()
                    .find(|j| j.operation.request_id == a.request_id && j.version == 1)
                    .map(|j| json!(j.operation))
                    .ok_or_else(|| {
                        AppError::new(
                            "REQUEST_EXPIRED",
                            "Request is absent or expired; it must not be silently resent",
                        )
                    })
            }
            "configure" | "configure_checked" => {
                let (a, expected) = if req.op == "configure_checked" {
                    let checked: CheckedConfigArgs = decode(req.args)?;
                    (
                        ConfigArgs {
                            model: checked.model,
                            voice: checked.voice,
                        },
                        Some(checked.expected_revision),
                    )
                } else {
                    (decode::<ConfigArgs>(req.args)?, None)
                };
                if a.voice.is_some() {
                    return Err(retired());
                }
                let mut inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
                }
                if expected.is_some_and(|revision| revision != inner.durable.config.revision) {
                    return Err(AppError::new(
                        "STALE_REVISION",
                        "Configuration changed; observe again without retry",
                    ));
                }
                if busy(&inner) {
                    return Err(AppError::new(
                        "BUSY",
                        "Configuration is blocked during work",
                    ));
                }
                if !inner.inventory.models.iter().any(|m| m.name == a.model)
                    || !inner.inventory.providers.llm.ready
                {
                    return Err(AppError::new(
                        "MODEL_UNAVAILABLE",
                        "Select an exact observed eligible installed local tag",
                    ));
                }
                let mut next = inner.durable.clone();
                next.config = Config {
                    model: Some(a.model),
                    voice: a.voice,
                    revision: next.config.revision.checked_add(1).ok_or_else(|| {
                        AppError::new(
                            "CONFIG_REVISION_EXHAUSTED",
                            "Configuration revision exhausted; no change was made",
                        )
                    })?,
                };
                self.commit(&mut inner, next)?;
                Ok(json!(inner.durable.config))
            }
            "verify_model" | "remove_model" => {
                let a: ModelArgs = decode(req.args.clone())?;
                if !model_identity(&a.model, &a.sha256) {
                    return Err(invalid());
                }
                let id = a.request_id.clone();
                let task = if req.op == "verify_model" {
                    Task::Verify(a)
                } else {
                    Task::Remove(a)
                };
                self.admit(id, &req.op, req.args, Some(task), req.schema_version)
            }
            "models" => {
                let a: IdArgs = decode(req.args.clone())?;
                self.admit(
                    a.request_id,
                    "models",
                    req.args,
                    Some(Task::Models),
                    req.schema_version,
                )
            }
            "submit" => {
                let a: SubmitArgs = decode(req.args.clone())?;
                text(&a.text, MAX_TEXT)?;
                if !["chat", "speech"].contains(&a.output.as_str()) {
                    return Err(invalid());
                }
                let task = (a.output == "chat").then_some(Task::Submit { text: a.text });
                self.admit(a.request_id, "submit", req.args, task, req.schema_version)
            }
            "transcribe" | "speak" => {
                let id = req
                    .args
                    .get("request_id")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?;
                self.admit(id.into(), &req.op, req.args, None, req.schema_version)
            }
            _ => Err(invalid()),
        }
    }
    fn commit(&self, inner: &mut Inner, mut next: Durable) -> AppResult<()> {
        // Admission and Dev model dependencies share the app -> ledger lock order.
        if (serde_json::to_value(&next.config).map_err(|_| storage())?
            != serde_json::to_value(&inner.durable.config).map_err(|_| storage())?
            && self.dev_has_model_dependencies())
            || next.jobs.iter().any(|job| {
                v2::pending(&job.operation)
                    && !inner.durable.jobs.iter().any(|old| old.operation.request_id == job.operation.request_id)
                    && matches!(job.task.as_ref(), Some(Task::Remove(model)) if self.dev_model_referenced(&model.model))
            })
        {
            return Err(AppError::new("BUSY", "Queued or executing Dev work references this model configuration"));
        }
        if self.dev_integrity_blocked()
            && next.jobs.iter().any(|job| {
                v2::pending(&job.operation)
                    && !inner
                        .durable
                        .jobs
                        .iter()
                        .any(|old| old.operation.request_id == job.operation.request_id)
            })
        {
            return Err(AppError::new(
                "BUSY",
                "Local inspection owns the shared provider capacity",
            ));
        }
        next.instructions.prune(&next.jobs, &next.conversations);
        next.revision = next.revision.checked_add(1).ok_or_else(v2::capacity)?;
        next.preflight()?;
        if !inner.owned {
            return Err(storage());
        }
        if persist(&self.path, &next).is_err() {
            inner.fault = true;
            return Err(storage());
        }
        inner.durable = next;
        Ok(())
    }
    fn commit_worker(&self, inner: &mut Inner, next: Durable) -> AppResult<()> {
        self.commit(inner, next).map_err(|_| {
            // Accepted work already owns capacity. Any transition failure is an
            // invariant/storage fault, never an idle queue or a replay invitation.
            inner.fault = true;
            storage()
        })
    }
    fn admit(
        &self,
        id: String,
        kind: &str,
        args: Value,
        task: Option<Task>,
        version: u32,
    ) -> AppResult<Value> {
        if !valid_id(&id) {
            return Err(invalid());
        }
        let fingerprint = v2::request_fingerprint(version, kind, &args)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if let Some(result) = retained(&inner, &id, &fingerprint)? {
            return Ok(result);
        }
        let task = task.ok_or_else(retired)?;
        if inner.durable.jobs.len() == MAX_JOBS
            && inner.durable.expired_ids.len() == MAX_EXPIRED_IDS
        {
            return Err(AppError::new(
                "CAPACITY",
                "Durable identifier capacity reached; explicit state maintenance is required",
            ));
        }
        if busy(&inner) {
            return Err(AppError::new(
                "BUSY",
                "One operation is already active; no queue",
            ));
        }
        if matches!(task, Task::Submit { .. }) && inner.durable.config.model.is_none() {
            return Err(AppError::new(
                "MODEL_UNAVAILABLE",
                "Configure an observed local model before sending",
            ));
        }
        if let Task::Verify(a) | Task::Remove(a) = &task {
            if a.expected_revision != inner.durable.config.revision {
                return Err(AppError::new(
                    "STALE_REVISION",
                    "Configuration changed; observe again without retry",
                ));
            }
            if matches!(task, Task::Remove(_))
                && inner.durable.config.model.as_deref() == Some(&a.model)
            {
                return Err(AppError::new(
                    "MODEL_SELECTED",
                    "Selected model cannot be removed; explicitly choose another model first",
                ));
            }
            if matches!(task, Task::Verify(_))
                && inner.durable.config.model.as_deref() != Some(&a.model)
            {
                return Err(AppError::new(
                    "MODEL_MISMATCH",
                    "Verification must name the selected model",
                ));
            }
        }
        let operation = Operation {
            request_id: id.clone(),
            kind: kind.into(),
            status: "running".into(),
            phase: match task {
                Task::Models => "inventory",
                Task::Submit { .. } => "generation",
                Task::Verify(_) => "verification",
                Task::Remove(_) => "removal",
                Task::Routed(_) | Task::Provider(_) => return Err(invalid()),
            }
            .into(),
            config_revision: inner.durable.config.revision,
            result: None,
            error: None,
        };
        let mut next = inner.durable.clone();
        let conversation_id = if matches!(task, Task::Submit { .. }) {
            Some(next.legacy_target()?)
        } else {
            None
        };
        let history_revision = conversation_id
            .as_ref()
            .and_then(|id| next.conversations.iter().find(|c| &c.conversation_id == id))
            .map(|c| c.history_revision);
        if history_revision.is_some_and(|r| r >= v2::MAX_REVISION) {
            return Err(v2::capacity());
        }
        next.make_room()?;
        if matches!(task, Task::Submit { .. }) {
            next.instructions.capture(&id);
        }
        next.jobs.push(Job {
            operation: operation.clone(),
            fingerprint,
            version,
            conversation_id,
            history_revision,
            task: Some(task),
            accepted_ms: v2::now_ms(),
        });
        self.commit(&mut inner, next)?;
        self.wake()?;
        Ok(admission(&operation))
    }
    fn app_effect_permission(&self) -> AppResult<()> {
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault || !inner.owned || self.dev_integrity_blocked() {
            return Err(storage());
        }
        Ok(())
    }
    fn run(&self, work: Work, provider: &dyn Provider) {
        if matches!(work.task, Task::Routed(_) | Task::Provider(_)) {
            self.run_v3(work, provider);
            return;
        }
        let mut turn = None;
        let mut inventory = None;
        let result = (|| -> AppResult<Value> {
            self.app_effect_permission()?;
            match &work.task {
                Task::Models => {
                    let mut found = provider.inventory()?;
                    if !found.valid() {
                        return Err(provider_protocol());
                    }
                    found.retire_speech();
                    let value = json!(found);
                    inventory = Some(found);
                    Ok(value)
                }
                Task::Verify(a) => {
                    let found = provider.inventory()?;
                    if !found.valid() { return Err(provider_protocol()); }
                    if !found.models.iter().any(|m| m.name == a.model && m.digest == a.sha256) {
                        return Err(AppError::new("MODEL_MISMATCH", "Installed model identity differs from the approved model"));
                    }
                    self.app_effect_permission()?;
                    let assistant = provider.chat(&a.model, &[Message {
                        role: "user".into(), content: "Say hello in one short sentence.".into(),
                    }])?;
                    if assistant.trim().is_empty() || assistant.len() > MAX_CONTEXT {
                        return Err(AppError::new("MODEL_RESULT_LIMIT", "Verification returned empty or oversized final text"));
                    }
                    Ok(json!({"model":a.model,"sha256":a.sha256,"verified":true,"assistant_text":assistant}))
                }
                Task::Remove(a) => {
                    provider.remove_model(&a.model, &a.sha256)?;
                    Ok(json!({"model":a.model,"sha256":a.sha256,"removed":true}))
                }
                Task::Submit { text } => {
                    let model = work.config.model.as_deref().ok_or_else(invalid)?;
                    let mut history = work.history.clone();
                    while history.iter().map(|m| m.content.len()).sum::<usize>() + text.len()
                        > MAX_CONTEXT
                    {
                        history.drain(..2);
                    }
                    history.push(Message {
                        role: "user".into(),
                        content: text.clone(),
                    });
                    let snapshot = work.instructions.as_ref().ok_or_else(storage)?;
                    while history.iter().map(|m| m.content.len()).sum::<usize>() + snapshot.text.len() > MAX_CONTEXT {
                        if history.len() <= 1 { return Err(crate::cloud::error("CONTEXT_LIMIT")); }
                        history.drain(..2);
                    }
                    self.app_effect_permission()?;
                    let assistant = provider.chat(model, &snapshot.messages(&history))?;
                    if assistant.trim().is_empty() || assistant.len() > MAX_CONTEXT {
                        return Err(AppError::new(
                            "MODEL_RESULT_LIMIT",
                            "Model returned empty or oversized text",
                        ));
                    }
                    turn = Some((text.clone(), assistant.clone()));
                    Ok(json!({"assistant_text":assistant,"audio":null,"speech_error":null}))
                }
                Task::Routed(_) | Task::Provider(_) => Err(invalid()),
            }
        })()
        .map_err(bounded_error);
        if let Ok(mut inner) = self.inner.lock() {
            if inner.fault
                || !inner.owned
                || work.conversation_id.as_ref().is_some_and(|id| {
                    !inner.durable.conversations.iter().any(|c| {
                        &c.conversation_id == id
                            && Some(c.history_revision) == work.history_revision
                    })
                })
                || !inner.durable.jobs.iter().any(|j| {
                    j.operation.request_id == work.id
                        && j.operation.status == "running"
                        && j.conversation_id == work.conversation_id
                        && j.history_revision == work.history_revision
                })
            {
                return;
            }
            let mut next = inner.durable.clone();
            if let Some((user, assistant)) = turn {
                let Some(job) = next.jobs.iter().find(|j| j.operation.request_id == work.id) else {
                    return;
                };
                let accepted_ms = job.accepted_ms;
                let Some(conversation) = next.conversations.iter_mut().find(|c| {
                    Some(&c.conversation_id) == work.conversation_id.as_ref()
                        && Some(c.history_revision) == work.history_revision
                }) else {
                    return;
                };
                conversation.history_revision += 1;
                let history = &mut conversation.history;
                let previous_pairs = history.len() / 2;
                history.push(Message {
                    role: "user".into(),
                    content: user,
                });
                history.push(Message {
                    role: "assistant".into(),
                    content: assistant,
                });
                v3::append_provenance(
                    &mut next.cloud,
                    &conversation.conversation_id,
                    history.len() / 2 - 1,
                    None,
                    None,
                );
                while history.len() > 32
                    || history.iter().map(|m| m.content.len()).sum::<usize>() > MAX_CONTEXT
                {
                    history.drain(..2);
                }
                v3::trim_provenance(
                    &mut next.cloud,
                    &conversation.conversation_id,
                    history.len() / 2,
                );
                timestamps::append(
                    &mut next.timestamps,
                    &conversation.conversation_id,
                    previous_pairs,
                    history.len() / 2,
                    accepted_ms,
                    self.cloud.clock.wall_ms(),
                );
            }
            if let Some(job) = next
                .jobs
                .iter_mut()
                .find(|j| j.operation.request_id == work.id)
            {
                job.operation.phase = "complete".into();
                match result {
                    Ok(v) => {
                        job.operation.status = "succeeded".into();
                        job.operation.result = Some(v);
                    }
                    Err(e) => {
                        job.operation.status = "failed".into();
                        job.operation.error = Some(e);
                    }
                }
            }
            if let Some(job) = next
                .jobs
                .iter_mut()
                .find(|j| j.operation.request_id == work.id)
            {
                if !reply_fits(&job.operation) {
                    job.operation.status = "failed".into();
                    job.operation.result = None;
                    job.operation.error = Some(provider_protocol());
                    inventory = None;
                }
            }
            if self.commit_worker(&mut inner, next).is_ok() {
                if let Task::Remove(a) = &work.task {
                    // Even partial removal may have unpublished the marker.
                    inner.inventory.models.retain(|m| m.name != a.model);
                }
                if let Some(found) = inventory {
                    inner.inventory = found;
                    inner.inventory_observed = true;
                }
            }
        }
    }
}
fn fingerprint(kind: &str, args: &Value) -> AppResult<String> {
    Ok(filesystem::digest(
        &serde_json::to_vec(&json!({"kind":kind,"args":args})).map_err(|_| invalid())?,
    ))
}
fn retained(inner: &Inner, id: &str, fingerprint: &str) -> AppResult<Option<Value>> {
    if inner.fault {
        return Err(storage());
    }
    if let Some(job) = inner
        .durable
        .jobs
        .iter()
        .find(|j| j.operation.request_id == id)
    {
        if job.fingerprint != fingerprint {
            return Err(AppError::new(
                "ID_CONFLICT",
                "Request identifier is bound to different input",
            ));
        }
        return Ok(Some(admission(&job.operation)));
    }
    if inner
        .durable
        .expired_ids
        .iter()
        .any(|expired| expired == id)
    {
        return Err(AppError::new(
            "REQUEST_EXPIRED",
            "Expired identifier cannot be reused",
        ));
    }
    Ok(None)
}
fn busy(inner: &Inner) -> bool {
    inner.durable.jobs.iter().any(|j| v2::pending(&j.operation))
}
fn admission(o: &Operation) -> Value {
    json!({"request_id":o.request_id,"status":o.status,"phase":o.phase,"config_revision":o.config_revision})
}
fn persist(path: &std::path::Path, durable: &Durable) -> Result<()> {
    let disk = durable;
    disk.validate()?;
    let bytes = serde_json::to_vec(&disk).map_err(|_| "Cannot encode application state")?;
    if bytes.len() as u64 > STATE_LIMIT {
        return Err("Application state exceeds durable limit".into());
    }
    filesystem::read_file(path, 0o600, STATE_LIMIT)?;
    filesystem::publish(path, &bytes, 0o600, true)
}
