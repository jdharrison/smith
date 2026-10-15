//! Versioned durable conversations and the single-worker durable scheduler.
#[cfg(test)]
#[path = "app_v2_tests.rs"]
mod tests;
use super::*;
const MAX_CONVERSATIONS: usize = 32;
const MAX_PENDING: usize = 4;
const QUEUE_MS: u64 = 600_000;
pub(super) const MAX_REVISION: u64 = 9_007_199_254_740_991;

fn wire_config_revision(revision: u64) -> Value {
    if revision <= MAX_REVISION {
        json!(revision)
    } else {
        json!(revision.to_string())
    }
}
fn parse_config_revision(value: Value) -> AppResult<u64> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .filter(|n| *n <= MAX_REVISION)
            .ok_or_else(invalid),
        Value::String(s) => s
            .parse::<u64>()
            .ok()
            .filter(|n| *n > MAX_REVISION && n.to_string() == s)
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}
fn config_revision<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<u64, D::Error> {
    parse_config_revision(Value::deserialize(d)?)
        .map_err(|_| serde::de::Error::custom("Noncanonical configuration revision"))
}
fn wire_config(config: &Config) -> Value {
    json!({"model":config.model,"voice":config.voice,"revision":wire_config_revision(config.revision)})
}
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(MAX_REVISION as u128) as u64
}
pub(super) fn pending(o: &Operation) -> bool {
    matches!(o.status.as_str(), "queued" | "running")
}
pub(super) fn capacity() -> AppError {
    AppError::new("CAPACITY", "Durable capacity exhausted; no change was made")
}
fn stale() -> AppError {
    AppError::new(
        "STALE_REVISION",
        "State changed; observe again without retry",
    )
}
fn missing() -> AppError {
    AppError::new(
        "CONVERSATION_NOT_FOUND",
        "Conversation is absent; no replacement was created",
    )
}
fn blocked() -> AppError {
    AppError::new("BUSY", "Target or global worker is occupied")
}
const MAX_NEW_TITLE_SCALARS: usize = 40;

fn forbidden_title_char(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

// Historical titles and lifecycle receipts retain their original validity rules.
fn valid_title(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && s.len() <= 256 && !s.chars().any(forbidden_title_char)
}

fn valid_new_title(s: &str) -> bool {
    valid_title(s) && s.chars().count() <= MAX_NEW_TITLE_SCALARS
}

fn initial_title(input: &str) -> String {
    let sanitized: String = input
        .chars()
        // Dart trims FEFF but Rust does not; remove it before truncation can expose it.
        .filter(|c| *c != '\u{feff}' && (c.is_whitespace() || !forbidden_title_char(*c)))
        .collect();
    let collapsed = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(MAX_NEW_TITLE_SCALARS).collect();
    let title = truncated.trim_end();
    if title.is_empty() {
        "New chat".into()
    } else {
        title.into()
    }
}
pub(super) fn request_fingerprint(version: u32, kind: &str, args: &Value) -> AppResult<String> {
    if version == 1 {
        fingerprint(kind, args)
    } else {
        fingerprint(&format!("v2:{kind}"), args)
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Conversation {
    pub conversation_id: String,
    pub title: String,
    pub archived: bool,
    pub metadata_revision: u64,
    pub history_revision: u64,
    pub sequence: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub origin_request_id: Option<String>,
    pub history: Vec<Message>,
}
impl Conversation {
    fn summary(&self) -> Value {
        json!({"conversation_id":self.conversation_id,"title":self.title,"archived":self.archived,"metadata_revision":self.metadata_revision,"history_revision":self.history_revision,"sequence":self.sequence,"origin_request_id":self.origin_request_id})
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Job {
    pub operation: Operation,
    pub fingerprint: String,
    pub version: u32,
    #[serde(deserialize_with = "required_nullable")]
    pub conversation_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub history_revision: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub task: Option<Task>,
    pub accepted_ms: u64,
}
impl Job {
    fn summary(&self) -> Value {
        let o = &self.operation;
        json!({"request_id":o.request_id,"conversation_id":self.conversation_id,"kind":o.kind,"status":o.status,"phase":o.phase,"config_revision":wire_config_revision(o.config_revision)})
    }
    fn poll(&self) -> Value {
        let mut v = self.summary();
        v["result"] = json!(self.operation.result);
        v["error"] = json!(self.operation.error);
        v
    }
    fn receipt(&self) -> Value {
        if self.operation.kind.starts_with("conversation_") {
            self.operation.result.clone().unwrap_or(Value::Null)
        } else {
            self.summary()
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Durable {
    schema_version: u32,
    pub config: Config,
    pub conversations: Vec<Conversation>,
    pub jobs: Vec<Job>,
    pub expired_ids: Vec<String>,
    #[serde(deserialize_with = "required_nullable")]
    legacy_conversation_id: Option<String>,
    pub revision: u64,
    next_sequence: u64,
}
impl Default for Durable {
    fn default() -> Self {
        Self {
            schema_version: 2,
            config: Config::default(),
            conversations: vec![],
            jobs: vec![],
            expired_ids: vec![],
            legacy_conversation_id: None,
            revision: 0,
            next_sequence: 0,
        }
    }
}
impl Durable {
    pub fn load(bytes: &[u8]) -> Result<(Self, bool)> {
        let v: Value = serde_json::from_slice(bytes).map_err(|_| "Corrupt state; preserved")?;
        match v.get("schema_version").and_then(Value::as_u64) {
            Some(1) => {
                let old: LegacyDurable =
                    serde_json::from_slice(bytes).map_err(|_| "Corrupt legacy state; preserved")?;
                old.validate()?;
                let id = new_id()?;
                let mut next = Self {
                    config: old.config,
                    expired_ids: old.expired_ids,
                    legacy_conversation_id: Some(id.clone()),
                    next_sequence: 1,
                    ..Self::default()
                };
                next.conversations.push(Conversation {
                    conversation_id: id.clone(),
                    title: "Current chat".into(),
                    archived: false,
                    metadata_revision: 0,
                    history_revision: 0,
                    sequence: 0,
                    origin_request_id: None,
                    history: old.history,
                });
                next.jobs = old
                    .jobs
                    .into_iter()
                    .map(|j| {
                        let chat =
                            matches!(j.operation.kind.as_str(), "submit" | "speak" | "transcribe");
                        Job {
                            operation: j.operation,
                            fingerprint: j.fingerprint,
                            version: 1,
                            conversation_id: chat.then(|| id.clone()),
                            history_revision: chat.then_some(0),
                            task: None,
                            accepted_ms: 0,
                        }
                    })
                    .collect();
                Ok((next, true))
            }
            Some(2) => serde_json::from_slice(bytes)
                .map(|d| (d, false))
                .map_err(|_| "Corrupt v2 state; preserved".into()),
            _ => Err("Unsupported state; preserved".into()),
        }
    }
    pub fn legacy(&self) -> LegacyDurable {
        LegacyDurable {
            schema_version: 1,
            config: self.config.clone(),
            history: self
                .conversations
                .iter()
                .find(|c| Some(&c.conversation_id) == self.legacy_conversation_id.as_ref())
                .map(|c| c.history.clone())
                .unwrap_or_default(),
            jobs: self
                .jobs
                .iter()
                .filter(|j| j.version == 1 || j.operation.status == "running")
                .map(|j| {
                    let mut operation = j.operation.clone();
                    if operation.kind == "submit_new" {
                        operation.kind = "submit".into();
                    }
                    LegacyJob {
                        operation,
                        fingerprint: j.fingerprint.clone(),
                    }
                })
                .collect(),
            expired_ids: self.expired_ids.clone(),
        }
    }
    pub fn legacy_target(&mut self) -> AppResult<String> {
        if let Some(id) = &self.legacy_conversation_id {
            let c = self
                .conversations
                .iter()
                .find(|c| &c.conversation_id == id)
                .ok_or_else(missing)?;
            if c.archived {
                return Err(AppError::new(
                    "CONVERSATION_ARCHIVED",
                    "Restore before sending",
                ));
            }
            return Ok(id.clone());
        }
        let id = self.allocate(None, "Current chat".into())?;
        self.legacy_conversation_id = Some(id.clone());
        Ok(id)
    }
    fn allocate(&mut self, origin: Option<String>, title: String) -> AppResult<String> {
        if self.conversations.len() >= MAX_CONVERSATIONS || self.next_sequence >= MAX_REVISION {
            return Err(capacity());
        }
        // Sequence never rolls back or reuses deleted identities, even with repeated entropy.
        let seed = new_id().map_err(|_| storage())?;
        let id = format!("{}{:016x}", &seed[..16], self.next_sequence);
        if self.legacy_conversation_id.as_ref() == Some(&id)
            || self.conversations.iter().any(|c| c.conversation_id == id)
        {
            return Err(capacity());
        }
        self.conversations.push(Conversation {
            conversation_id: id.clone(),
            title,
            archived: false,
            metadata_revision: 0,
            history_revision: 0,
            sequence: self.next_sequence,
            origin_request_id: origin,
            history: vec![],
        });
        self.next_sequence += 1;
        Ok(id)
    }
    pub fn make_room(&mut self) -> AppResult<()> {
        if self.jobs.len() < MAX_JOBS {
            return Ok(());
        }
        if self.expired_ids.len() >= MAX_EXPIRED_IDS {
            return Err(capacity());
        }
        let index = self
            .jobs
            .iter()
            .position(|j| !pending(&j.operation))
            .ok_or_else(capacity)?;
        self.expired_ids
            .push(self.jobs.remove(index).operation.request_id);
        Ok(())
    }
    fn reserved_bytes(&self) -> AppResult<usize> {
        // Normalize growing envelopes to their maximum reachable encoding. As a
        // phase advances, its actual growth consumes this headroom, not new space.
        let mut maximal = self.clone();
        maximal.revision = MAX_REVISION;
        for job in &mut maximal.jobs {
            if pending(&job.operation) {
                job.operation.status = "interrupted".into();
                job.operation.phase = "generation".into();
                if let Some(c) = maximal
                    .conversations
                    .iter_mut()
                    .find(|c| Some(&c.conversation_id) == job.conversation_id.as_ref())
                {
                    c.history_revision = MAX_REVISION;
                }
            }
        }
        let bytes = serde_json::to_vec(&maximal).map_err(|_| storage())?.len();
        // Each pending job reserves JSON worst-case escaped final text, history pair,
        // provider inventory and bounded error metadata. No pending job is evicted.
        let reserve = self
            .jobs
            .iter()
            .filter(|j| pending(&j.operation))
            .map(|j| {
                if j.operation.kind == "models" {
                    FRAME_LIMIT + 8192
                } else {
                    2 * 6 * MAX_CONTEXT + 8192
                }
            })
            .sum::<usize>();
        Ok(bytes.saturating_add(reserve))
    }
    pub fn preflight(&self) -> AppResult<()> {
        let transitions: u64 = self
            .jobs
            .iter()
            .map(|j| match j.operation.status.as_str() {
                "queued" => 2,
                "running" => 1,
                _ => 0,
            })
            .sum();
        if self.reserved_bytes()? > STATE_LIMIT as usize
            || self.revision > MAX_REVISION.saturating_sub(transitions)
        {
            return Err(capacity());
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        let fail = || "Invalid v2 application state; preserved".to_owned();
        if self.schema_version != 2
            || self.conversations.len() > MAX_CONVERSATIONS
            || self.jobs.len() > MAX_JOBS
            || self.expired_ids.len() > MAX_EXPIRED_IDS
            || self.revision > MAX_REVISION
            || self.next_sequence > MAX_REVISION
            || self
                .legacy_conversation_id
                .as_ref()
                .is_some_and(|id| !valid_id(id))
        {
            return Err(fail());
        }
        let mut ids = HashSet::new();
        for id in &self.expired_ids {
            if !valid_id(id) || !ids.insert(id.clone()) {
                return Err(fail());
            }
        }
        let mut conversations = HashSet::new();
        let mut sequences = HashSet::new();
        let mut origins = HashSet::new();
        for c in &self.conversations {
            if !valid_id(&c.conversation_id)
                || !conversations.insert(c.conversation_id.clone())
                || !sequences.insert(c.sequence)
                || c.sequence >= self.next_sequence
                || !valid_title(&c.title)
                || c.metadata_revision > MAX_REVISION
                || c.history_revision > MAX_REVISION
                || c.origin_request_id
                    .as_ref()
                    .is_some_and(|id| !valid_id(id) || !origins.insert(id.clone()))
            {
                return Err(fail());
            }
            if let Some(job) = c
                .origin_request_id
                .as_ref()
                .and_then(|origin| self.jobs.iter().find(|j| &j.operation.request_id == origin))
            {
                if job.version != 2
                    || job.operation.kind != "submit_new"
                    || job.conversation_id.as_ref() != Some(&c.conversation_id)
                {
                    return Err(fail());
                }
            }
            LegacyDurable {
                config: self.config.clone(),
                history: c.history.clone(),
                ..LegacyDurable::default()
            }
            .validate()?;
        }
        // Validate global configuration even in an empty store.
        LegacyDurable {
            config: self.config.clone(),
            ..LegacyDurable::default()
        }
        .validate()?;
        let mut outstanding = HashSet::new();
        let mut running = 0;
        let mut global_pending = 0;
        for j in &self.jobs {
            let o = &j.operation;
            if ![
                "models",
                "submit",
                "submit_new",
                "verify_model",
                "remove_model",
                "speak",
                "transcribe",
                "conversation_rename",
                "conversation_archive",
                "conversation_restore",
                "conversation_delete",
            ]
            .contains(&o.kind.as_str())
                || ![
                    "queued",
                    "generation",
                    "synthesis",
                    "inventory",
                    "verification",
                    "removal",
                    "transcription",
                    "complete",
                ]
                .contains(&o.phase.as_str())
                || (o.status == "queued" && o.phase != "queued")
                || (j.version == 1 && (o.status == "queued" || o.kind == "submit_new"))
            {
                return Err(fail());
            }
            if !valid_id(&o.request_id)
                || !ids.insert(o.request_id.clone())
                || ![1, 2].contains(&j.version)
                || j.fingerprint.len() != 64
                || !j
                    .fingerprint
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || o.config_revision > self.config.revision
                || j.accepted_ms > MAX_REVISION
                || !["queued", "running", "succeeded", "failed", "interrupted"]
                    .contains(&o.status.as_str())
            {
                return Err(fail());
            }
            if let Some(id) = &j.conversation_id {
                if !valid_id(id)
                    || if o.kind == "conversation_delete" {
                        conversations.contains(id)
                    } else {
                        !conversations.contains(id)
                    }
                    || j.history_revision.is_none_or(|r| r > MAX_REVISION)
                {
                    return Err(fail());
                }
            } else if j.history_revision.is_some() {
                return Err(fail());
            }
            if o.kind == "submit_new"
                && !self.conversations.iter().any(|c| {
                    Some(&c.conversation_id) == j.conversation_id.as_ref()
                        && c.origin_request_id.as_ref() == Some(&o.request_id)
                })
            {
                return Err(fail());
            }
            if pending(o) {
                if o.status == "running" {
                    running += 1;
                }
                if let Some(id) = &j.conversation_id {
                    if !outstanding.insert(id) {
                        return Err(fail());
                    }
                    let c = self
                        .conversations
                        .iter()
                        .find(|c| &c.conversation_id == id)
                        .ok_or_else(fail)?;
                    if c.archived || Some(c.history_revision) != j.history_revision {
                        return Err(fail());
                    }
                } else {
                    global_pending += 1;
                }
                // A missing task is permitted only for unreplayable migrated v1 work.
                if j.task.is_none() && j.version != 1 {
                    return Err(fail());
                }
            }
            if o.kind.starts_with("conversation_") {
                if j.version != 2 || !valid_lifecycle(j) {
                    return Err(fail());
                }
            } else {
                let mut normalized = o.clone();
                if normalized.kind == "submit_new" {
                    normalized.kind = "submit".into();
                }
                if normalized.status == "queued" {
                    normalized.status = "running".into();
                    normalized.phase = "generation".into();
                    if !matches!(o.kind.as_str(), "submit" | "submit_new") {
                        return Err(fail());
                    }
                }
                if !valid_operation(&normalized) {
                    return Err(fail());
                }
            }
            let chat_kind = matches!(
                o.kind.as_str(),
                "submit" | "submit_new" | "speak" | "transcribe"
            );
            if (chat_kind && j.conversation_id.is_none())
                || (!chat_kind
                    && !o.kind.starts_with("conversation_")
                    && j.conversation_id.is_some())
                || (j.version == 2
                    && matches!(o.kind.as_str(), "submit" | "submit_new")
                    && j.task.is_none())
                || (pending(o) && o.config_revision != self.config.revision)
            {
                return Err(fail());
            }
            if let Some(task) = &j.task {
                let valid = match task {
                    Task::Submit { text: input } => {
                        matches!(o.kind.as_str(), "submit" | "submit_new")
                            && text(input, MAX_TEXT).is_ok()
                            && j.conversation_id.is_some()
                    }
                    Task::Models => o.kind == "models" && j.conversation_id.is_none(),
                    Task::Verify(a) | Task::Remove(a) => {
                        model_identity(&a.model, &a.sha256)
                            && a.request_id == o.request_id
                            && a.expected_revision == o.config_revision
                            && j.conversation_id.is_none()
                            && matches!(
                                (task, o.kind.as_str()),
                                (Task::Verify(_), "verify_model")
                                    | (Task::Remove(_), "remove_model")
                            )
                    }
                };
                if !valid {
                    return Err(fail());
                }
            }
        }
        if running > 1
            || outstanding.len() > MAX_PENDING
            || global_pending > 1
            || (global_pending > 0 && !outstanding.is_empty())
        {
            return Err(fail());
        }
        self.preflight().map_err(|e| e.message)?;
        Ok(())
    }
}
fn valid_lifecycle(j: &Job) -> bool {
    let o = &j.operation;
    if o.status != "succeeded"
        || o.phase != "complete"
        || o.error.is_some()
        || j.task.is_some()
        || j.conversation_id.is_none()
    {
        return false;
    }
    let Some(v) = &o.result else {
        return false;
    };
    if o.kind == "conversation_delete" {
        return v
            == &json!({"request_id":o.request_id,"conversation_id":j.conversation_id,"deleted":true});
    }
    if ![
        "conversation_rename",
        "conversation_archive",
        "conversation_restore",
    ]
    .contains(&o.kind.as_str())
    {
        return false;
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Receipt {
        request_id: String,
        conversation: Summary,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Summary {
        conversation_id: String,
        title: String,
        archived: bool,
        metadata_revision: u64,
        history_revision: u64,
        sequence: u64,
        #[serde(deserialize_with = "required_nullable")]
        origin_request_id: Option<String>,
    }
    serde_json::from_value::<Receipt>(v.clone()).is_ok_and(|r| {
        r.request_id == o.request_id
            && Some(r.conversation.conversation_id) == j.conversation_id
            && valid_title(&r.conversation.title)
            && r.conversation.metadata_revision <= MAX_REVISION
            && r.conversation.history_revision <= MAX_REVISION
            && r.conversation.sequence < MAX_REVISION
            && r.conversation
                .origin_request_id
                .is_none_or(|id| valid_id(&id))
            && (o.kind != "conversation_archive" || r.conversation.archived)
            && (o.kind != "conversation_restore" || !r.conversation.archived)
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    conversation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewSubmit {
    request_id: String,
    text: String,
    output: String,
    #[serde(deserialize_with = "config_revision")]
    expected_config_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submit {
    request_id: String,
    conversation_id: String,
    text: String,
    output: String,
    #[serde(deserialize_with = "config_revision")]
    expected_config_revision: u64,
    expected_history_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    request_id: String,
    conversation_id: String,
    expected_metadata_revision: u64,
    title: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    request_id: String,
    conversation_id: String,
    expected_metadata_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delete {
    request_id: String,
    conversation_id: String,
    expected_metadata_revision: u64,
    expected_history_revision: u64,
}

impl Application {
    pub(super) fn call_v2(&self, mut req: AppRequest) -> AppResult<Value> {
        if matches!(
            req.op.as_str(),
            "configure_checked" | "verify_model" | "remove_model"
        ) {
            let revision = parse_config_revision(
                req.args
                    .get("expected_revision")
                    .cloned()
                    .ok_or_else(invalid)?,
            )?;
            req.args["expected_revision"] = json!(revision);
        }
        match req.op.as_str() {
            "configure" | "configure_checked" => {
                let config: Config = decode(self.call_common(req)?)?;
                Ok(wire_config(&config))
            }
            "models" | "verify_model" | "remove_model" => {
                let id = req
                    .args
                    .get("request_id")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?
                    .to_owned();
                self.call_common(req)?;
                let inner = self.inner.lock().map_err(|_| storage())?;
                Ok(inner
                    .durable
                    .jobs
                    .iter()
                    .find(|j| j.operation.request_id == id)
                    .ok_or_else(storage)?
                    .summary())
            }
            "state" => {
                let _: Empty = decode(req.args)?;
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
                }
                let mut conversations = inner.durable.conversations.iter().collect::<Vec<_>>();
                conversations.sort_by_key(|c| std::cmp::Reverse(c.sequence));
                Ok(
                    json!({"config":wire_config(&inner.durable.config),"inventory_observed":inner.inventory_observed,"inventory":inner.inventory,"conversations":conversations.into_iter().map(Conversation::summary).collect::<Vec<_>>(),"operations":inner.durable.jobs.iter().map(Job::summary).collect::<Vec<_>>(),"retention":{"max_conversations":32,"max_turn_pairs":16,"max_context_bytes":MAX_CONTEXT,"max_terminal_jobs":MAX_JOBS,"max_expired_ids":MAX_EXPIRED_IDS,"max_pending_jobs":4,"queue_timeout_ms":QUEUE_MS,"capacity_policy":"reject_when_expiry_metadata_full","missing_request_code":"REQUEST_EXPIRED"},"revision":inner.durable.revision}),
                )
            }
            "conversation_get" => {
                let a: Get = decode(req.args)?;
                if !valid_id(&a.conversation_id) {
                    return Err(invalid());
                }
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault {
                    return Err(storage());
                }
                let c = inner
                    .durable
                    .conversations
                    .iter()
                    .find(|c| c.conversation_id == a.conversation_id)
                    .ok_or_else(missing)?;
                Ok(json!({"conversation":c.summary(),"history":c.history}))
            }
            "poll" => {
                let a: PollArgs = decode(req.args)?;
                if !valid_id(&a.request_id) || !valid_id(&a.instance_id) {
                    return Err(invalid());
                }
                if a.instance_id != self.instance_id {
                    return Err(AppError::new(
                        "INSTANCE_MISMATCH",
                        "Synchronize without resending",
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
                    .map(Job::poll)
                    .ok_or_else(|| {
                        AppError::new(
                            "REQUEST_EXPIRED",
                            "Request is absent or expired; do not resend",
                        )
                    })
            }
            "submit_new"
            | "submit"
            | "conversation_rename"
            | "conversation_archive"
            | "conversation_restore"
            | "conversation_delete" => self.mutate_v2(req),
            _ => Err(invalid()),
        }
    }
    fn mutate_v2(&self, req: AppRequest) -> AppResult<Value> {
        let id = req
            .args
            .get("request_id")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?
            .to_owned();
        if !valid_id(&id) {
            return Err(invalid());
        }
        let fp = request_fingerprint(2, &req.op, &req.args)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if retained(&inner, &id, &fp)?.is_some() {
            return Ok(inner
                .durable
                .jobs
                .iter()
                .find(|j| j.operation.request_id == id)
                .ok_or_else(storage)?
                .receipt());
        }
        let mut next = inner.durable.clone();
        if matches!(req.op.as_str(), "submit" | "submit_new") {
            let (target, input, output, config_revision, history_revision) =
                if req.op == "submit_new" {
                    let a: NewSubmit = decode(req.args)?;
                    debug_assert_eq!(a.request_id, id);
                    (None, a.text, a.output, a.expected_config_revision, None)
                } else {
                    let a: Submit = decode(req.args)?;
                    debug_assert_eq!(a.request_id, id);
                    if !valid_id(&a.conversation_id) {
                        return Err(invalid());
                    }
                    (
                        Some(a.conversation_id),
                        a.text,
                        a.output,
                        a.expected_config_revision,
                        Some(a.expected_history_revision),
                    )
                };
            text(&input, MAX_TEXT)?;
            if output != "chat" {
                return Err(invalid());
            }
            if config_revision != next.config.revision {
                return Err(stale());
            }
            if next
                .jobs
                .iter()
                .any(|j| pending(&j.operation) && j.conversation_id.is_none())
            {
                return Err(blocked());
            }
            if next.jobs.iter().filter(|j| pending(&j.operation)).count() >= MAX_PENDING {
                return Err(blocked());
            }
            if next.config.model.is_none() {
                return Err(AppError::new(
                    "MODEL_UNAVAILABLE",
                    "Configure a local model before sending",
                ));
            }
            let target = match target {
                Some(target) => {
                    let c = next
                        .conversations
                        .iter()
                        .find(|c| c.conversation_id == target)
                        .ok_or_else(missing)?;
                    if c.archived {
                        return Err(AppError::new(
                            "CONVERSATION_ARCHIVED",
                            "Restore before sending",
                        ));
                    }
                    if c.history_revision >= MAX_REVISION {
                        return Err(capacity());
                    }
                    if Some(c.history_revision) != history_revision {
                        return Err(stale());
                    }
                    if next.jobs.iter().any(|j| {
                        pending(&j.operation) && j.conversation_id.as_ref() == Some(&target)
                    }) {
                        return Err(blocked());
                    }
                    target
                }
                None => next.allocate(Some(id.clone()), initial_title(&input))?,
            };
            next.make_room()?;
            let job = Job {
                operation: Operation {
                    request_id: id,
                    kind: req.op,
                    status: "queued".into(),
                    phase: "queued".into(),
                    config_revision,
                    result: None,
                    error: None,
                },
                fingerprint: fp,
                version: 2,
                conversation_id: Some(target),
                history_revision: Some(history_revision.unwrap_or(0)),
                task: Some(Task::Submit { text: input }),
                accepted_ms: now_ms(),
            };
            let receipt = job.summary();
            next.jobs.push(job);
            self.commit(&mut inner, next)?;
            self.wake()?;
            return Ok(receipt);
        }
        let (target, expected, expected_history, title) = match req.op.as_str() {
            "conversation_rename" => {
                let a: Rename = decode(req.args)?;
                debug_assert_eq!(a.request_id, id);
                (
                    a.conversation_id,
                    a.expected_metadata_revision,
                    None,
                    Some(a.title.trim().to_owned()),
                )
            }
            "conversation_delete" => {
                let a: Delete = decode(req.args)?;
                debug_assert_eq!(a.request_id, id);
                (
                    a.conversation_id,
                    a.expected_metadata_revision,
                    Some(a.expected_history_revision),
                    None,
                )
            }
            _ => {
                let a: Metadata = decode(req.args)?;
                debug_assert_eq!(a.request_id, id);
                (a.conversation_id, a.expected_metadata_revision, None, None)
            }
        };
        if !valid_id(&target) || title.as_ref().is_some_and(|t| !valid_new_title(t)) {
            return Err(invalid());
        }
        let index = next
            .conversations
            .iter()
            .position(|c| c.conversation_id == target)
            .ok_or_else(missing)?;
        let c = &next.conversations[index];
        if c.metadata_revision != expected
            || expected_history.is_some_and(|r| r != c.history_revision)
        {
            return Err(stale());
        }
        if req.op != "conversation_rename"
            && next
                .jobs
                .iter()
                .any(|j| j.conversation_id.as_ref() == Some(&target) && pending(&j.operation))
        {
            return Err(blocked());
        }
        let history_revision = c.history_revision;
        let result = if req.op == "conversation_delete" {
            let removed = next
                .jobs
                .iter()
                .filter(|j| j.conversation_id.as_ref() == Some(&target))
                .count();
            if next.expired_ids.len() + removed > MAX_EXPIRED_IDS {
                return Err(capacity());
            }
            next.jobs.retain(|j| {
                if j.conversation_id.as_ref() == Some(&target) {
                    next.expired_ids.push(j.operation.request_id.clone());
                    false
                } else {
                    true
                }
            });
            next.conversations.remove(index);
            json!({"request_id":id,"conversation_id":target,"deleted":true})
        } else {
            let c = &mut next.conversations[index];
            c.metadata_revision = c
                .metadata_revision
                .checked_add(1)
                .filter(|r| *r <= MAX_REVISION)
                .ok_or_else(capacity)?;
            if let Some(title) = title {
                c.title = title;
            } else {
                c.archived = req.op == "conversation_archive";
            }
            json!({"request_id":id,"conversation":c.summary()})
        };
        next.make_room()?;
        next.jobs.push(Job {
            operation: Operation {
                request_id: id,
                kind: req.op,
                status: "succeeded".into(),
                phase: "complete".into(),
                config_revision: next.config.revision,
                result: Some(result.clone()),
                error: None,
            },
            fingerprint: fp,
            version: 2,
            conversation_id: Some(target),
            history_revision: Some(history_revision),
            task: None,
            accepted_ms: now_ms(),
        });
        self.commit(&mut inner, next)?;
        Ok(result)
    }
    pub(super) fn wake(&self) -> AppResult<()> {
        match self
            .owner
            .as_ref()
            .and_then(|o| o.sender.as_ref())
            .ok_or_else(storage)?
            .try_send(())
        {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => Ok(()),
            Err(_) => Err(storage()),
        }
    }
    pub(super) fn next_work(&self) -> AppResult<Option<Work>> {
        self.next_work_at(now_ms())
    }
    fn next_work_at(&self, now: u64) -> AppResult<Option<Work>> {
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        loop {
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            let Some(index) = inner
                .durable
                .jobs
                .iter()
                .position(|j| pending(&j.operation))
            else {
                return Ok(None);
            };
            let mut next = inner.durable.clone();
            let job = &mut next.jobs[index];
            if job.operation.status == "queued" && now.saturating_sub(job.accepted_ms) > QUEUE_MS {
                job.operation.status = "failed".into();
                job.operation.phase = "complete".into();
                job.operation.error = Some(AppError::new(
                    "QUEUE_EXPIRED",
                    "Queue wait exceeded ten minutes; not dispatched",
                ));
                self.commit_worker(&mut inner, next)?;
                continue;
            }
            let Some(task) = job.task.clone() else {
                inner.fault = true;
                return Err(storage());
            };
            let transition = job.operation.status == "queued";
            job.operation.status = "running".into();
            if matches!(task, Task::Submit { .. }) {
                job.operation.phase = "generation".into();
            }
            let work = Work {
                id: job.operation.request_id.clone(),
                task,
                config: next.config.clone(),
                history: next
                    .conversations
                    .iter()
                    .find(|c| Some(&c.conversation_id) == job.conversation_id.as_ref())
                    .map(|c| c.history.clone())
                    .unwrap_or_default(),
                conversation_id: job.conversation_id.clone(),
                history_revision: job.history_revision,
            };
            if transition {
                self.commit_worker(&mut inner, next)?;
            }
            return Ok(Some(work));
        }
    }
}
