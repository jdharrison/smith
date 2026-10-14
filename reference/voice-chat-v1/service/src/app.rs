//! Bounded application domain. Providers run only on the single worker.
use crate::filesystem;
use crate::providers::{Audio, Inventory, Provider, Providers};
use crate::{Result, Roots};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppRequest {
    pub schema_version: u32,
    pub service: String,
    pub protocol: String,
    pub call_id: String,
    pub op: String,
    pub args: Value,
}
impl AppRequest {
    pub fn validate(&self) -> AppResult<()> {
        if self.schema_version != 1
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
        let (result, error) = match result {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            schema_version: 1,
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
struct Job {
    operation: Operation,
    fingerprint: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Durable {
    schema_version: u32,
    config: Config,
    history: Vec<Message>,
    jobs: Vec<Job>,
    expired_ids: Vec<String>,
}
impl Default for Durable {
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
impl Durable {
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
                || !["models", "submit", "transcribe", "speak"].contains(&o.kind.as_str())
                || !["running", "succeeded", "failed", "interrupted"].contains(&o.status.as_str())
                || ![
                    "inventory",
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
fn state_result(durable: &Durable, inventory: &Inventory) -> Value {
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
struct TurnResult {
    assistant_text: String,
    #[serde(deserialize_with = "required_nullable")]
    audio: Option<Audio>,
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
}
struct WorkerOwner {
    inner: Arc<Mutex<Inner>>,
    sender: Option<mpsc::SyncSender<Work>>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for WorkerOwner {
    fn drop(&mut self) {
        // The publication fence shares the commit mutex: after revocation, even a
        // provider that completes after reopening these roots cannot write old state.
        if let Ok(mut inner) = self.inner.lock() {
            inner.owned = false;
        }
        self.sender.take();
        if let Some(thread) = self.thread.take() {
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
    path: PathBuf,
    pub instance_id: String,
}
struct Work {
    id: String,
    task: Task,
    config: Config,
    history: Vec<Message>,
}
enum Task {
    Models,
    Submit { text: String, speech: bool },
    Transcribe(Audio),
    Speak(String),
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
    voice: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitArgs {
    request_id: String,
    text: String,
    output: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeakArgs {
    request_id: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscribeArgs {
    request_id: String,
    audio: InputAudio,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputAudio {
    mime: String,
    data_base64: String,
}
impl From<InputAudio> for Audio {
    fn from(a: InputAudio) -> Self {
        Self {
            mime: a.mime,
            data_base64: a.data_base64,
            sample_rate: None,
        }
    }
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
        let path = roots.data.join("smith-next/app-state.json");
        filesystem::check_directory(&roots.data.join("smith-next"), true)?;
        let mut durable: Durable = match filesystem::read_file(&path, 0o600, STATE_LIMIT)? {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Corrupt application state; preserved without reset")?,
            None => Durable::default(),
        };
        durable.validate()?;
        let mut changed = false;
        for j in &mut durable.jobs {
            if j.operation.status == "running" {
                j.operation.status = "interrupted".into();
                j.operation.phase = "complete".into();
                j.operation.result = None;
                j.operation.error = Some(AppError::new(
                    "INTERRUPTED",
                    "Service restarted; operation was not replayed",
                ));
                changed = true;
            }
        }
        if changed {
            persist(&path, &durable)?;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let inner = Arc::new(Mutex::new(Inner {
            durable,
            inventory: Inventory::default(),
            fault: false,
            owned: true,
        }));
        let instance_id = new_id()?;
        let worker = Self {
            inner: inner.clone(),
            owner: None,
            path: path.clone(),
            instance_id: instance_id.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("smith-app-worker".into())
            .spawn(move || {
                while let Ok(work) = receiver.recv() {
                    if !worker.inner.lock().is_ok_and(|inner| inner.owned) {
                        break;
                    }
                    worker.run(work, provider.as_ref());
                }
            })
            .map_err(|_| "Cannot start provider worker")?;
        Ok(Self {
            owner: Some(Arc::new(WorkerOwner {
                inner: inner.clone(),
                sender: Some(sender),
                thread: Some(thread),
            })),
            inner,
            path,
            instance_id,
        })
    }
    pub fn production(roots: &Roots) -> Result<Self> {
        Self::open(roots, Arc::new(Providers::new()?))
    }
    pub fn call(&self, req: AppRequest) -> AppResult<Value> {
        req.validate()?;
        match req.op.as_str() {
            "state" => {
                let _: Empty = decode(req.args)?;
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
                }
                Ok(state_result(&inner.durable, &inner.inventory))
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
                    .find(|j| j.operation.request_id == a.request_id)
                    .map(|j| json!(j.operation))
                    .ok_or_else(|| {
                        AppError::new(
                            "REQUEST_EXPIRED",
                            "Request is absent or expired; it must not be silently resent",
                        )
                    })
            }
            "configure" => {
                let a: ConfigArgs = decode(req.args)?;
                let mut inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
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
                if let Some(v) = &a.voice {
                    if !inner.inventory.providers.tts.ready || !inner.inventory.voices.contains(v) {
                        return Err(AppError::new(
                            "VOICE_UNAVAILABLE",
                            "Select an observed voice or null",
                        ));
                    }
                }
                let mut next = inner.durable.clone();
                next.config = Config {
                    model: Some(a.model),
                    voice: a.voice,
                    revision: next.config.revision.checked_add(1).ok_or_else(storage)?,
                };
                self.commit(&mut inner, next)?;
                Ok(json!(inner.durable.config))
            }
            "models" => {
                let a: IdArgs = decode(req.args.clone())?;
                self.admit(a.request_id, "models", req.args, Task::Models)
            }
            "submit" => {
                let a: SubmitArgs = decode(req.args.clone())?;
                text(&a.text, MAX_TEXT)?;
                if !["chat", "speech"].contains(&a.output.as_str()) {
                    return Err(invalid());
                }
                self.admit(
                    a.request_id,
                    "submit",
                    req.args,
                    Task::Submit {
                        text: a.text,
                        speech: a.output == "speech",
                    },
                )
            }
            "transcribe" => {
                let a: TranscribeArgs = decode(req.args.clone())?;
                let audio: Audio = a.audio.into();
                audio.input_bytes()?;
                self.admit(
                    a.request_id,
                    "transcribe",
                    req.args,
                    Task::Transcribe(audio),
                )
            }
            "speak" => {
                let a: SpeakArgs = decode(req.args.clone())?;
                text(&a.text, 4096)?;
                self.admit(a.request_id, "speak", req.args, Task::Speak(a.text))
            }
            _ => Err(invalid()),
        }
    }
    fn commit(&self, inner: &mut Inner, next: Durable) -> AppResult<()> {
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
    fn admit(&self, id: String, kind: &str, args: Value, task: Task) -> AppResult<Value> {
        if !valid_id(&id) {
            return Err(invalid());
        }
        let fingerprint = filesystem::digest(
            &serde_json::to_vec(&json!({"kind":kind,"args":args})).map_err(|_| invalid())?,
        );
        let mut inner = self.inner.lock().map_err(|_| storage())?;
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
            return Ok(admission(&job.operation));
        }
        if inner.durable.expired_ids.contains(&id) {
            return Err(AppError::new(
                "REQUEST_EXPIRED",
                "Expired identifier cannot be reused",
            ));
        }
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
        let operation = Operation {
            request_id: id.clone(),
            kind: kind.into(),
            status: "running".into(),
            phase: match task {
                Task::Models => "inventory",
                Task::Transcribe(_) => "transcription",
                Task::Submit { .. } => "generation",
                Task::Speak(_) => "synthesis",
            }
            .into(),
            config_revision: inner.durable.config.revision,
            result: None,
            error: None,
        };
        let work = Work {
            id,
            task,
            config: inner.durable.config.clone(),
            history: inner.durable.history.clone(),
        };
        let mut next = inner.durable.clone();
        if next.jobs.len() == MAX_JOBS {
            let expired = next.jobs.remove(0);
            next.expired_ids.push(expired.operation.request_id);
        }
        next.jobs.push(Job {
            operation: operation.clone(),
            fingerprint,
        });
        self.commit(&mut inner, next)?;
        if self
            .owner
            .as_ref()
            .and_then(|owner| owner.sender.as_ref())
            .ok_or_else(storage)?
            .try_send(work)
            .is_err()
        {
            inner.fault = true;
            return Err(storage());
        }
        Ok(admission(&operation))
    }
    fn phase(&self, id: &str, phase: &str) -> AppResult<()> {
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        let mut next = inner.durable.clone();
        let job = next
            .jobs
            .iter_mut()
            .find(|j| j.operation.request_id == id)
            .ok_or_else(storage)?;
        job.operation.phase = phase.into();
        self.commit(&mut inner, next)
    }
    fn run(&self, work: Work, provider: &dyn Provider) {
        let mut turn = None;
        let mut inventory = None;
        let result = (|| -> AppResult<Value> {
            match &work.task {
                Task::Models => {
                    let found = provider.inventory()?;
                    if !found.valid() { return Err(provider_protocol()); }
                    let value = json!(found);
                    inventory = Some(found);
                    Ok(value)
                }
                Task::Transcribe(audio) => {
                    let bytes = audio.input_bytes()?;
                    let result = provider.transcribe(&bytes)?;
                    if !serde_json::from_value::<TranscriptResult>(result.clone()).is_ok_and(|r| r.text.len()<=MAX_TEXT && r.language.len()<=64 && r.duration_ms<=60_000) { return Err(provider_protocol()); }
                    Ok(result)
                }
                Task::Speak(t) => {
                    let audio = provider.speak(t, work.config.voice.as_deref())?;
                    audio.output_bytes()?;
                    Ok(json!({"audio":audio}))
                }
                Task::Submit { text, speech } => {
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
                    let assistant = provider.chat(model, &history)?;
                    if assistant.trim().is_empty() || assistant.len() > MAX_CONTEXT {
                        return Err(AppError::new(
                            "MODEL_RESULT_LIMIT",
                            "Model returned empty or oversized text",
                        ));
                    }
                    turn = Some((text.clone(), assistant.clone()));
                    let mut audio = None;
                    let mut speech_error = None;
                    if *speech {
                        self.phase(&work.id, "synthesis")?;
                        match if assistant.len() > 4096 {
                            Err(AppError::new(
                                "TTS_INPUT_LIMIT",
                                "Answer exceeds the TTS input limit; text retained",
                            ))
                        } else {
                            provider
                                .speak(&assistant, work.config.voice.as_deref())
                                .and_then(|a| {
                                    a.output_bytes()?;
                                    Ok(a)
                                })
                        } {
                            Ok(a) => audio = Some(a),
                            Err(e) => speech_error = Some(bounded_error(e)),
                        }
                    }
                    Ok(
                        json!({"assistant_text":assistant,"audio":audio,"speech_error":speech_error}),
                    )
                }
            }
        })().map_err(bounded_error);
        if let Ok(mut inner) = self.inner.lock() {
            if inner.fault || !inner.owned {
                return;
            }
            let mut next = inner.durable.clone();
            if let Some((user, assistant)) = turn {
                next.history.push(Message {
                    role: "user".into(),
                    content: user,
                });
                next.history.push(Message {
                    role: "assistant".into(),
                    content: assistant,
                });
                while next.history.len() > 32
                    || next.history.iter().map(|m| m.content.len()).sum::<usize>() > MAX_CONTEXT
                {
                    next.history.drain(..2);
                }
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
            if self.commit(&mut inner, next).is_ok() {
                if let Some(found) = inventory {
                    inner.inventory = found;
                }
            }
        }
    }
}
fn busy(inner: &Inner) -> bool {
    inner
        .durable
        .jobs
        .iter()
        .any(|j| j.operation.status == "running")
}
fn admission(o: &Operation) -> Value {
    json!({"request_id":o.request_id,"status":o.status,"phase":o.phase,"config_revision":o.config_revision})
}
fn persist(path: &std::path::Path, durable: &Durable) -> Result<()> {
    let mut disk = durable.clone();
    for j in &mut disk.jobs {
        if let Some(result) = j.operation.result.as_mut() {
            if result.get("audio").is_some_and(|a| !a.is_null()) {
                result["audio"] = Value::Null;
                if j.operation.kind == "submit" {
                    result["speech_error"] = json!(AppError::new(
                        "AUDIO_EXPIRED",
                        "Audio was transient; deliberate replay is required"
                    ));
                } else {
                    j.operation.result = None;
                    j.operation.error = Some(AppError::new(
                        "AUDIO_EXPIRED",
                        "Audio was transient; deliberate replay is required",
                    ));
                }
            }
        }
    }
    disk.validate()?;
    let bytes = serde_json::to_vec(&disk).map_err(|_| "Cannot encode application state")?;
    if bytes.len() as u64 > STATE_LIMIT {
        return Err("Application state exceeds durable limit".into());
    }
    filesystem::read_file(path, 0o600, STATE_LIMIT)?;
    filesystem::publish(path, &bytes, 0o600, true)
}
