//! Durable retained environments, deliberately separate from finite inspection jobs.
//! Commands must be persisted before dispatch. An ambiguous result is never replayed.
use crate::{app::valid_id, cloud::unique_json, filesystem};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf, sync::Mutex};

const LIMIT: u64 = 8 * 1024 * 1024;
const MAX_REVISION: u64 = 9_007_199_254_740_991;
#[path = "retained_acquire.rs"]
pub mod retained_acquire;
#[path = "retained_runtime.rs"]
pub mod retained_runtime;
#[path = "retained_sources.rs"]
pub mod retained_sources;

pub const SLOT_LIMIT: usize = 3;
pub const MAX_PROMPT: usize = 16 * 1024;
const MAX_REPLY: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Read,
    Write,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Creating,
    Ready,
    Prompting,
    Failed,
    Unknown,
    Finishing,
    CleanupUnknown,
    Finished,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub url: String,
    pub reference: Option<String>,
    /// Effective capability, frozen at admission. A write job cannot override project read-only.
    pub read_only: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenProject {
    pub project_id: String,
    pub project_key: String,
    pub context: String,
    pub repositories: Vec<Repository>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    pub operation_id: String,
    pub prompt: String,
    pub outcome: TurnOutcome,
    pub reply: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnOutcome {
    Pending,
    Succeeded,
    Failed,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub session_id: String,
    pub project_id: String,
    pub project_key: String,
    pub job_type: JobType,
    pub status: Status,
    pub retained: bool,
    pub deleted: bool,
    pub created_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub job: Summary,
    pub project: FrozenProject,
    pub turns: Vec<Turn>,
    pending: Option<Command>,
    delete_requested: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub operation_id: String,
    pub session_id: String,
    pub revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    fingerprint: String,
    receipt: Receipt,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema_version: u32,
    revision: u64,
    sessions: Vec<Session>,
    operations: Vec<Operation>,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            sessions: Vec::new(),
            operations: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub session_id: String,
    pub operation_id: String,
    pub action: Action,
    pub job_type: JobType,
    pub project: FrozenProject,
    pub prompt: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create,
    Prompt,
    Finish,
}
/// Only an identity-bound acknowledgement from the owned runtime may be applied.
#[derive(Clone, Debug)]
pub struct Acknowledgement {
    pub session_id: String,
    pub operation_id: String,
    pub action: Action,
    pub outcome: Outcome,
}
#[derive(Clone, Debug)]
pub enum Outcome {
    Ready,
    Reply(String),
    /// Known prompt/create failure; the environment remains reserved.
    Failed,
    /// Includes transport loss and malformed/untrusted output.
    Unknown,
    /// Owned environment shutdown/removal verified, not merely supervisor exit.
    Finished,
}
/// The production application has no prepared implementation yet. This seam is not
/// permission to spawn tools or to substitute the inspection runtime.
pub trait Runtime: Send + Sync {
    /// Trusted fake/in-process implementations may admit; merely constructing a
    /// process transport must not imply prepared live execution capability.
    fn admission_ready(&self) -> bool {
        true
    }
    fn execute(&self, command: &Command) -> Acknowledgement;
    fn cancel(&self) {}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Stale,
    Conflict,
    NotFound,
    Blocked,
    Capacity,
    ContextLimit,
    Storage,
}
pub type Result<T> = std::result::Result<T, Error>;
struct State {
    ledger: Ledger,
    digest: Option<String>,
    fault: bool,
    dispatched: HashSet<(String, String, Action)>,
}
pub struct Store {
    path: PathBuf,
    state: Mutex<State>,
}

fn prompt_valid(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= MAX_PROMPT
}
/// Exact Python MAX_TEXT and JSONL frame budget, shared by admission and transport.
/// Errors deliberately contain no prompt/context bytes.
pub fn compose_prompt(project: &FrozenProject, prompt: &str) -> Result<String> {
    if !prompt_valid(prompt) || prompt.contains('\0') || project.context.contains('\0') {
        return Err(Error::Invalid);
    }
    let text = format!("{}\nTask:\n{prompt}", project.context);
    if text.len() > MAX_PROMPT {
        return Err(Error::ContextLimit);
    }
    let frame = serde_json::json!({"v":1,"op":"prompt","operation_id":"0".repeat(32),"job_id":"0".repeat(32),"purpose":"work","text":text});
    if serde_json::to_vec(&frame)
        .map_err(|_| Error::Invalid)?
        .len()
        + 1
        > 32768
    {
        return Err(Error::ContextLimit);
    }
    Ok(text)
}
fn project_valid(p: &FrozenProject) -> bool {
    valid_id(&p.project_id)
        && !p.project_key.is_empty()
        && p.project_key.len() <= 40
        && p.context.len() <= 32 * 1024
        && p.repositories.len() <= 16
        && p.repositories.iter().all(|r| {
            !r.url.is_empty()
                && r.url.len() <= 2048
                && r.reference
                    .as_ref()
                    .is_none_or(|s| !s.is_empty() && s.len() <= 256)
        })
}
fn session_consistent(s: &Session) -> bool {
    let Some(last) = s.turns.last() else {
        return false;
    };
    if s.turns.iter().any(|t| {
        (t.outcome == TurnOutcome::Succeeded) != t.reply.is_some()
            || compose_prompt(&s.project, &t.prompt).is_err()
    }) {
        return false;
    }
    if s.turns[..s.turns.len() - 1]
        .iter()
        .any(|t| !matches!(t.outcome, TurnOutcome::Succeeded | TurnOutcome::Failed))
    {
        return false;
    }
    let expected = match s.job.status {
        Status::Queued | Status::Creating => {
            s.turns.len() == 1 && last.outcome == TurnOutcome::Pending
        }
        Status::Prompting => last.outcome == TurnOutcome::Pending,
        Status::Ready => last.outcome == TurnOutcome::Succeeded,
        Status::Failed | Status::Interrupted => last.outcome == TurnOutcome::Failed,
        Status::Unknown => last.outcome != TurnOutcome::Pending,
        Status::Finishing | Status::CleanupUnknown | Status::Finished => {
            matches!(last.outcome, TurnOutcome::Succeeded | TurnOutcome::Failed)
        }
    };
    if !expected
        || (s.delete_requested
            && !matches!(
                s.job.status,
                Status::Finishing | Status::CleanupUnknown | Status::Finished
            ))
        || s.job.deleted != (s.delete_requested && s.job.status == Status::Finished)
    {
        return false;
    }
    if let Some(c) = &s.pending {
        if c.action != Action::Finish
            && (last.outcome != TurnOutcome::Pending
                || c.operation_id != last.operation_id
                || c.prompt.as_ref() != Some(&last.prompt))
        {
            return false;
        }
    }
    true
}
impl Ledger {
    fn ordinary_count(&self) -> usize {
        self.sessions.iter().map(|s| s.turns.len()).sum()
    }
    fn terminal_count(&self, s: &Session) -> usize {
        self.operations
            .iter()
            .filter(|o| o.receipt.session_id == s.job.session_id)
            .count()
            .saturating_sub(s.turns.len())
    }
    fn terminal_reserve(&self) -> Result<usize> {
        let mut bytes = 0;
        for s in &self.sessions {
            // A dedicated allowance of two terminal receipts per session is never
            // consumed by ordinary work. Already materialized finish commands consume
            // their reservation instead of being charged twice.
            bytes += (2 - self.terminal_count(s)) * 512;
            if !matches!(
                s.job.status,
                Status::Finished | Status::Finishing | Status::CleanupUnknown
            ) {
                bytes += serde_json::to_vec(&command(s, &"0".repeat(32), Action::Finish, None))
                    .map_err(|_| Error::Storage)?
                    .len()
                    + 512;
            }
        }
        Ok(bytes)
    }
    fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        let mut ops = HashSet::new();
        if self.schema_version != 1
            || self.revision > MAX_REVISION
            || self.sessions.len() > 32
            || self.operations.len() > 256 + 2 * 32
            || self.ordinary_count() > 256
            || self.sessions.iter().filter(|s| s.job.retained).count() > SLOT_LIMIT
        {
            return Err(Error::Storage);
        }
        for s in &self.sessions {
            if !valid_id(&s.job.session_id)
                || !ids.insert(&s.job.session_id)
                || !project_valid(&s.project)
                || s.job.project_id != s.project.project_id
                || s.job.project_key != s.project.project_key
                || s.turns.is_empty()
                || s.turns.len() > 32
                || !match s.job.status {
                    Status::Finishing | Status::CleanupUnknown => self.terminal_count(s) == 1,
                    Status::Finished if s.job.deleted => matches!(self.terminal_count(s), 1 | 2),
                    Status::Finished => self.terminal_count(s) == 1,
                    _ => self.terminal_count(s) == 0,
                }
                || !session_consistent(s)
                || (s.job.job_type == JobType::Read
                    && s.project.repositories.iter().any(|r| !r.read_only))
                || (s.job.deleted && (s.job.retained || s.job.status != Status::Finished))
                || (matches!(
                    s.job.status,
                    Status::Queued | Status::Interrupted | Status::Finished
                ) == s.job.retained)
                || s.turns.iter().any(|t| {
                    !valid_id(&t.operation_id)
                        || !prompt_valid(&t.prompt)
                        || t.reply.as_ref().is_some_and(|r| r.len() > MAX_REPLY)
                })
            {
                return Err(Error::Storage);
            }
            let pending_action = match s.job.status {
                Status::Creating => Some(Action::Create),
                Status::Prompting => Some(Action::Prompt),
                Status::Finishing => Some(Action::Finish),
                _ => None,
            };
            if s.pending.as_ref().map(|p| p.action) != pending_action {
                return Err(Error::Storage);
            }
            if let Some(c) = &s.pending {
                if c.session_id != s.job.session_id
                    || !valid_id(&c.operation_id)
                    || c.job_type != s.job.job_type
                    || serde_json::to_vec(&c.project).ok() != serde_json::to_vec(&s.project).ok()
                    || (c.action == Action::Finish) != c.prompt.is_none()
                    || c.prompt.as_ref().is_some_and(|p| !prompt_valid(p))
                {
                    return Err(Error::Storage);
                }
            }
        }
        for o in &self.operations {
            if !valid_id(&o.receipt.operation_id)
                || !ops.insert(&o.receipt.operation_id)
                || !ids.contains(&o.receipt.session_id)
                || o.receipt.revision == 0
                || o.receipt.revision > self.revision
                || o.fingerprint.len() != 64
                || !o
                    .fingerprint
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Error::Storage);
            }
        }
        let mut turns = HashSet::new();
        for s in &self.sessions {
            for t in &s.turns {
                if !turns.insert(&t.operation_id)
                    || !self.operations.iter().any(|o| {
                        o.receipt.operation_id == t.operation_id
                            && o.receipt.session_id == s.job.session_id
                    })
                {
                    return Err(Error::Storage);
                }
            }
            if let Some(c) = &s.pending {
                if !self.operations.iter().any(|o| {
                    o.receipt.operation_id == c.operation_id
                        && o.receipt.session_id == s.job.session_id
                }) || (c.action == Action::Finish
                    && s.turns.iter().any(|t| t.operation_id == c.operation_id))
                {
                    return Err(Error::Storage);
                }
            }
        }
        Ok(())
    }
    fn interrupt(&mut self) {
        for s in &mut self.sessions {
            if s.job.status == Status::Queued {
                s.job.status = Status::Interrupted;
            } else if s.job.retained && s.job.status != Status::CleanupUnknown {
                s.job.status = if s.job.status == Status::Finishing {
                    Status::CleanupUnknown
                } else {
                    Status::Unknown
                };
            }
            s.pending = None;
            for t in &mut s.turns {
                if t.outcome == TurnOutcome::Pending {
                    t.outcome = if s.job.status == Status::Interrupted {
                        TurnOutcome::Failed
                    } else {
                        TurnOutcome::Unknown
                    };
                }
            }
        }
    }
}
impl Store {
    /// Read-only startup: never contacts runtime, replays a queue or clears reservations.
    pub fn open(path: PathBuf) -> Result<Self> {
        let bytes = filesystem::read_file(&path, 0o600, LIMIT).map_err(|_| Error::Storage)?;
        let mut ledger = if let Some(b) = &bytes {
            let value = unique_json(b).map_err(|_| Error::Storage)?;
            let ledger: Ledger =
                serde_json::from_value(value.clone()).map_err(|_| Error::Storage)?;
            if serde_json::to_value(&ledger).map_err(|_| Error::Storage)? != value {
                return Err(Error::Storage);
            }
            ledger
        } else {
            Ledger::default()
        };
        ledger.validate()?;
        ledger.interrupt();
        Ok(Self {
            path,
            state: Mutex::new(State {
                ledger,
                digest: bytes.as_ref().map(|b| filesystem::digest(b)),
                fault: false,
                dispatched: HashSet::new(),
            }),
        })
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        let state = self.state.lock().map_err(|_| Error::Storage)?;
        if state.fault {
            Err(Error::Storage)
        } else {
            Ok(state)
        }
    }
    fn verify_disk(&self, state: &mut State) -> Result<()> {
        let matches = filesystem::read_file(&self.path, 0o600, LIMIT)
            .map(|bytes| bytes.as_ref().map(|b| filesystem::digest(b)) == state.digest)
            .unwrap_or(false);
        if !matches {
            state.fault = true;
            state.ledger.interrupt();
            return Err(Error::Storage);
        }
        Ok(())
    }
    fn commit(&self, state: &mut State, mut next: Ledger) -> Result<()> {
        next.revision = next
            .revision
            .checked_add(1)
            .filter(|r| *r <= MAX_REVISION)
            .ok_or(Error::Capacity)?;
        next.validate()?;
        let bytes = serde_json::to_vec(&next).map_err(|_| Error::Storage)?;
        // Every admitted pending turn owns worst-case JSON-escaped reply space.
        let reserved = next
            .sessions
            .iter()
            .flat_map(|s| &s.turns)
            .filter(|t| t.outcome == TurnOutcome::Pending)
            .count()
            * MAX_REPLY
            * 6;
        if bytes
            .len()
            .saturating_add(reserved)
            .saturating_add(next.terminal_reserve()?)
            > LIMIT as usize
        {
            return Err(Error::Capacity);
        }
        let result = (|| {
            let current =
                filesystem::read_file(&self.path, 0o600, LIMIT).map_err(|_| Error::Storage)?;
            if current.as_ref().map(|b| filesystem::digest(b)) != state.digest {
                return Err(Error::Storage);
            }
            filesystem::publish(&self.path, &bytes, 0o600, current.is_some())
                .map_err(|_| Error::Storage)
        })();
        state.ledger = next;
        if result.is_err() {
            state.fault = true;
            state.ledger.interrupt();
        } else {
            state.digest = Some(filesystem::digest(&bytes));
        }
        result
    }
    pub fn revision(&self) -> Result<u64> {
        Ok(self.lock()?.ledger.revision)
    }
    pub fn sessions(&self) -> Result<Vec<Session>> {
        Ok(self.lock()?.ledger.sessions.clone())
    }
    pub fn reserved(&self) -> usize {
        self.lock()
            .map(|s| s.ledger.sessions.iter().filter(|s| s.job.retained).count())
            .unwrap_or(SLOT_LIMIT)
    }
    pub fn get(&self, id: &str) -> Result<Session> {
        self.lock()?
            .ledger
            .sessions
            .iter()
            .find(|s| s.job.session_id == id)
            .cloned()
            .ok_or(Error::NotFound)
    }
    /// Query-only recovery by original operation identity, including deleted tombstones.
    pub fn receipt(&self, operation_id: &str) -> Result<Receipt> {
        self.lock()?
            .ledger
            .operations
            .iter()
            .find(|o| o.receipt.operation_id == operation_id)
            .map(|o| o.receipt.clone())
            .ok_or(Error::NotFound)
    }
    fn replay(
        state: &State,
        operation_id: &str,
        fingerprint: &str,
        expected: u64,
    ) -> Result<Option<Receipt>> {
        if !valid_id(operation_id) {
            return Err(Error::Invalid);
        }
        if let Some(o) = state
            .ledger
            .operations
            .iter()
            .find(|o| o.receipt.operation_id == operation_id)
        {
            return if o.fingerprint == fingerprint {
                Ok(Some(o.receipt.clone()))
            } else {
                Err(Error::Conflict)
            };
        }
        if state.ledger.revision != expected {
            return Err(Error::Stale);
        }
        Ok(None)
    }
    fn record(
        next: &mut Ledger,
        operation_id: &str,
        session_id: &str,
        fingerprint: String,
    ) -> Receipt {
        let receipt = Receipt {
            operation_id: operation_id.into(),
            session_id: session_id.into(),
            revision: next.revision + 1,
        };
        next.operations.push(Operation {
            fingerprint,
            receipt: receipt.clone(),
        });
        receipt
    }
    pub fn start(&self, request: Start) -> Result<Receipt> {
        let fingerprint = filesystem::digest(
            &serde_json::to_vec(&(
                &request.operation_id,
                request.expected_revision,
                request.job_type,
                &request.project,
                &request.prompt,
            ))
            .map_err(|_| Error::Invalid)?,
        );
        let mut state = self.lock()?;
        if let Some(r) = Self::replay(
            &state,
            &request.operation_id,
            &fingerprint,
            request.expected_revision,
        )? {
            return Ok(r);
        }
        if !valid_id(&request.session_id)
            || !project_valid(&request.project)
            || !prompt_valid(&request.prompt)
        {
            return Err(Error::Invalid);
        }
        compose_prompt(&request.project, &request.prompt)?;
        if state.ledger.sessions.len() >= 32 || state.ledger.ordinary_count() >= 256 {
            return Err(Error::Capacity);
        }
        if state
            .ledger
            .sessions
            .iter()
            .any(|s| s.job.session_id == request.session_id)
        {
            return Err(Error::Conflict);
        }
        let mut project = request.project;
        if request.job_type == JobType::Read {
            for r in &mut project.repositories {
                r.read_only = true;
            }
        }
        let mut next = state.ledger.clone();
        let receipt = Self::record(
            &mut next,
            &request.operation_id,
            &request.session_id,
            fingerprint,
        );
        next.sessions.push(Session {
            job: Summary {
                session_id: request.session_id,
                project_id: project.project_id.clone(),
                project_key: project.project_key.clone(),
                job_type: request.job_type,
                status: Status::Queued,
                retained: false,
                deleted: false,
                created_ms: request.created_ms,
            },
            project,
            turns: vec![Turn {
                operation_id: request.operation_id,
                prompt: request.prompt,
                outcome: TurnOutcome::Pending,
                reply: None,
            }],
            pending: None,
            delete_requested: false,
        });
        self.commit(&mut state, next)?;
        Ok(receipt)
    }
    pub fn change(&self, request: Change) -> Result<Receipt> {
        let fingerprint =
            filesystem::digest(&serde_json::to_vec(&request).map_err(|_| Error::Invalid)?);
        let mut state = self.lock()?;
        if let Some(r) = Self::replay(
            &state,
            &request.operation_id,
            &fingerprint,
            request.expected_revision,
        )? {
            return Ok(r);
        }
        if matches!(request.change, ChangeKind::Prompt { .. })
            && state.ledger.ordinary_count() >= 256
        {
            return Err(Error::Capacity);
        }
        let mut next = state.ledger.clone();
        let s = next
            .sessions
            .iter_mut()
            .find(|s| s.job.session_id == request.session_id)
            .ok_or(Error::NotFound)?;
        match &request.change {
            ChangeKind::Prompt { prompt } => {
                compose_prompt(&s.project, prompt)?;
                if !matches!(s.job.status, Status::Ready | Status::Failed)
                    || !s.job.retained
                    || s.job.deleted
                {
                    return Err(Error::Blocked);
                }
                if s.turns.len() >= 32 {
                    return Err(Error::Capacity);
                }
                s.turns.push(Turn {
                    operation_id: request.operation_id.clone(),
                    prompt: prompt.clone(),
                    outcome: TurnOutcome::Pending,
                    reply: None,
                });
                s.job.status = Status::Prompting;
                s.pending = Some(command(
                    s,
                    &request.operation_id,
                    Action::Prompt,
                    Some(prompt.clone()),
                ));
            }
            ChangeKind::Finish | ChangeKind::Delete => {
                if s.job.deleted
                    || (s.job.status == Status::Finished
                        && matches!(request.change, ChangeKind::Finish))
                {
                    return Err(Error::Blocked);
                }
                if s.pending.is_some() {
                    return Err(Error::Blocked);
                }
                // Unknown effects are observation-only, including unknown cleanup.
                if matches!(s.job.status, Status::Unknown | Status::CleanupUnknown) {
                    return Err(Error::Blocked);
                }
                if s.job.retained {
                    s.job.status = Status::Finishing;
                    s.pending = Some(command(s, &request.operation_id, Action::Finish, None));
                } else {
                    s.job.status = Status::Finished;
                    for t in &mut s.turns {
                        if t.outcome == TurnOutcome::Pending {
                            t.outcome = TurnOutcome::Failed;
                        }
                    }
                }
                if matches!(request.change, ChangeKind::Delete) {
                    s.delete_requested = true;
                    // A live delete stays visible/reserved until owned cleanup is verified.
                    s.job.deleted = !s.job.retained;
                }
            }
        }
        let receipt = Self::record(
            &mut next,
            &request.operation_id,
            &request.session_id,
            fingerprint,
        );
        self.commit(&mut state, next)?;
        Ok(receipt)
    }
    /// Caller must serialize this reservation with the existing inspection scheduler.
    /// external_occupied includes inspection active/uncertain and legacy reservations.
    pub fn prepare_next(&self, external_occupied: usize) -> Result<Option<Command>> {
        let mut state = self.lock()?;
        // Pending commands have already been consumed. Never return one a second time.
        let reserved = state
            .ledger
            .sessions
            .iter()
            .filter(|s| s.job.retained)
            .count();
        if reserved.saturating_add(external_occupied) >= SLOT_LIMIT {
            return Ok(None);
        }
        let mut next = state.ledger.clone();
        let Some(s) = next
            .sessions
            .iter_mut()
            .find(|s| s.job.status == Status::Queued)
        else {
            return Ok(None);
        };
        s.job.status = Status::Creating;
        s.job.retained = true;
        let t = s.turns.first().ok_or(Error::Storage)?;
        let c = command(s, &t.operation_id, Action::Create, Some(t.prompt.clone()));
        s.pending = Some(c.clone());
        self.commit(&mut state, next)?;
        state
            .dispatched
            .insert((c.session_id.clone(), c.operation_id.clone(), c.action));
        Ok(Some(c))
    }
    /// Newly accepted prompt/finish commands are retrieved by their original receipt
    /// only once via take_command, never reconstructed after service restart.
    pub fn take_command(&self, receipt: &Receipt) -> Result<Command> {
        let mut state = self.lock()?;
        self.verify_disk(&mut state)?;
        let s = state
            .ledger
            .sessions
            .iter()
            .find(|s| s.job.session_id == receipt.session_id)
            .ok_or(Error::NotFound)?;
        let c = s
            .pending
            .as_ref()
            .filter(|c| c.operation_id == receipt.operation_id)
            .cloned()
            .ok_or(Error::Blocked)?;
        if !state
            .dispatched
            .insert((c.session_id.clone(), c.operation_id.clone(), c.action))
        {
            return Err(Error::Blocked);
        }
        Ok(c)
    }
    /// Select an admitted prompt/finish without consuming it a second time.
    pub fn take_pending(&self) -> Result<Option<Command>> {
        let mut state = self.lock()?;
        self.verify_disk(&mut state)?;
        let command = state
            .ledger
            .sessions
            .iter()
            .filter_map(|s| s.pending.as_ref())
            .find(|c| {
                !state.dispatched.contains(&(
                    c.session_id.clone(),
                    c.operation_id.clone(),
                    c.action,
                ))
            })
            .cloned();
        if let Some(c) = &command {
            state
                .dispatched
                .insert((c.session_id.clone(), c.operation_id.clone(), c.action));
        }
        Ok(command)
    }
    pub fn acknowledge(&self, ack: Acknowledgement) -> Result<()> {
        let mut state = self.lock()?;
        if !state.dispatched.contains(&(
            ack.session_id.clone(),
            ack.operation_id.clone(),
            ack.action,
        )) {
            return Err(Error::Blocked);
        }
        let mut next = state.ledger.clone();
        let s = next
            .sessions
            .iter_mut()
            .find(|s| s.job.session_id == ack.session_id)
            .ok_or(Error::NotFound)?;
        let c = s.pending.as_ref().ok_or(Error::Blocked)?;
        if c.operation_id != ack.operation_id || c.action != ack.action {
            return Err(Error::Conflict);
        }
        let outcome = match (&ack.action, ack.outcome) {
            (Action::Create, Outcome::Ready) => {
                s.job.status = Status::Prompting;
                None
            }
            (Action::Prompt, Outcome::Reply(reply)) if reply.len() <= MAX_REPLY => {
                s.job.status = Status::Ready;
                Some((TurnOutcome::Succeeded, Some(reply)))
            }
            (Action::Finish, Outcome::Finished) => {
                s.job.status = Status::Finished;
                s.job.retained = false;
                s.job.deleted = s.delete_requested;
                None
            }
            (Action::Create | Action::Prompt, Outcome::Failed) => {
                s.job.status = Status::Failed;
                Some((TurnOutcome::Failed, None))
            }
            _ => {
                s.job.status = if ack.action == Action::Finish {
                    Status::CleanupUnknown
                } else {
                    Status::Unknown
                };
                Some((TurnOutcome::Unknown, None))
            }
        };
        if let Some((outcome, reply)) = outcome {
            if let Some(t) = s
                .turns
                .iter_mut()
                .find(|t| t.operation_id == ack.operation_id)
            {
                t.outcome = outcome;
                t.reply = reply;
            }
        }
        s.pending = if ack.action == Action::Create && s.job.status == Status::Prompting {
            let t = s.turns.first().ok_or(Error::Storage)?;
            Some(command(
                s,
                &t.operation_id,
                Action::Prompt,
                Some(t.prompt.clone()),
            ))
        } else {
            None
        };
        self.commit(&mut state, next)
    }
}
fn command(s: &Session, operation_id: &str, action: Action, prompt: Option<String>) -> Command {
    Command {
        session_id: s.job.session_id.clone(),
        operation_id: operation_id.into(),
        action,
        job_type: s.job.job_type,
        project: s.project.clone(),
        prompt,
    }
}
#[derive(Clone, Debug)]
pub struct Start {
    pub operation_id: String,
    pub session_id: String,
    pub expected_revision: u64,
    pub project: FrozenProject,
    pub job_type: JobType,
    pub prompt: String,
    pub created_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub operation_id: String,
    pub session_id: String,
    pub expected_revision: u64,
    pub change: ChangeKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChangeKind {
    Prompt { prompt: String },
    Finish,
    Delete,
}

#[cfg(test)]
#[path = "retained_jobs_tests.rs"]
mod tests;
