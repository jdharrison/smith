//! Receipt-only Dev jobs. The private ledger is independent of Chat/disk9.
use super::*;
use crate::dev_runtime::{DevRuntime, RuntimeEvent, RuntimeRequest};
use std::sync::atomic::{AtomicBool, Ordering};

const LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    request_id: String,
    project_id: String,
    project_key: String,
    agent_id: String,
    agent_name: String,
    status: String,
    message: String,
    #[serde(deserialize_with = "required_nullable")]
    session_id: Option<String>,
    created_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    #[serde(deserialize_with = "projects::object")]
    summary: Summary,
    fingerprint: String,
    prompt: String,
    may_have_sent: bool,
    cleanup_required: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    #[serde(deserialize_with = "record_objects")]
    records: Vec<Record>,
}
fn record_objects<'de, D>(d: D) -> std::result::Result<Vec<Record>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<serde_json::Map<String, Value>>::deserialize(d)?
        .into_iter()
        .map(|v| serde_json::from_value(Value::Object(v)).map_err(serde::de::Error::custom))
        .collect()
}
impl Ledger {
    fn load(bytes: &[u8]) -> Result<Self> {
        // Reuse the app's duplicate-member/depth rejection before typed decoding.
        let value = crate::cloud::unique_json(bytes).map_err(|_| "Invalid Dev ledger")?;
        if !value.is_object() {
            return Err("Invalid Dev ledger".into());
        }
        let ledger: Self = serde_json::from_value(value).map_err(|_| "Invalid Dev ledger")?;
        let mut ids = HashSet::new();
        if ledger.records.len() > 32
            || ledger
                .records
                .iter()
                .filter(|r| active(&r.summary.status))
                .count()
                > 1
            || ledger.records.iter().any(|r| {
                !valid_id(&r.summary.request_id)
                    || !ids.insert(&r.summary.request_id)
                    || !valid_id(&r.summary.project_id)
                    || !agents::valid_agent_id(&r.summary.agent_id)
                    || !instructions::valid_hash(&r.fingerprint)
                    || r.prompt.is_empty()
                    || r.prompt.len() > 32 * 1024
                    || r.summary.project_key.is_empty()
                    || r.summary.project_key.len() > 40
                    || r.summary.agent_name.is_empty()
                    || r.summary.agent_name.chars().count() > 40
                    || r.summary.message.len() > 256
                    || r.summary
                        .session_id
                        .as_ref()
                        .is_some_and(|s| !valid_session(s))
                    || !matches!(
                        r.summary.status.as_str(),
                        "initializing"
                            | "ready"
                            | "dispatching"
                            | "recorded"
                            | "failed"
                            | "interrupted"
                            | "unknown"
                            | "cleanup_unknown"
                    )
                    || (r.summary.status == "recorded"
                        && (r.cleanup_required
                            || !r.may_have_sent
                            || r.summary.session_id.is_none()))
            })
        {
            return Err("Invalid Dev ledger".into());
        }
        Ok(ledger)
    }
    fn interrupt(&mut self) {
        for r in &mut self.records {
            if active(&r.summary.status) || r.summary.status == "interrupted" {
                r.may_have_sent |= r.summary.status == "dispatching";
                r.cleanup_required = true;
                r.summary.status = "cleanup_unknown".into();
                r.summary.message =
                    "Service interrupted; operator reconciliation required; no replay".into();
            }
        }
    }
}
fn active(s: &str) -> bool {
    matches!(s, "initializing" | "ready" | "dispatching")
}
fn valid_session(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn unavailable() -> AppError {
    AppError::new(
        "DEV_UNAVAILABLE",
        "Dev ledger unavailable; inspect before starting more work",
    )
}
struct State {
    ledger: Ledger,
    fault: bool,
    owned: bool,
}
struct Shared {
    state: Mutex<State>,
    cancelled: AtomicBool,
    path: PathBuf,
}
pub(super) struct Owner {
    shared: Arc<Shared>,
    runtime: Arc<dyn DevRuntime>,
    thread: Mutex<Option<JoinHandle<()>>>,
    pub(super) local: Option<Arc<super::local_dev::Owner>>,
    pub(super) retained: retained_jobs::Result<retained_jobs::Store>,
    pub(super) retained_runtime: Option<Arc<dyn retained_jobs::Runtime>>,
}
impl Owner {
    pub(super) fn open(path: PathBuf, runtime: Arc<dyn DevRuntime>) -> Self {
        let loaded = filesystem::read_file(&path, 0o600, LIMIT).and_then(|bytes| {
            bytes
                .map(|b| Ledger::load(&b))
                .unwrap_or_else(|| Ok(Ledger::default()))
        });
        let fault = loaded.is_err();
        let mut ledger = loaded.unwrap_or_default();
        ledger.interrupt(); // Observation only: no daemon access or automatic rewrite.
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    ledger,
                    fault,
                    owned: true,
                }),
                cancelled: AtomicBool::new(false),
                path: path.clone(),
            }),
            retained: retained_jobs::Store::open(path.with_file_name("dev-retained-jobs-v1.json")),
            retained_runtime: None,
            runtime,
            thread: Mutex::new(None),
            local: None,
        }
    }
    pub(super) fn legacy_state(&self) -> AppResult<(usize, bool, Vec<String>)> {
        let state = self.shared.state.lock().map_err(|_| unavailable())?;
        if state.fault || !state.owned {
            return Err(unavailable());
        }
        let bytes =
            filesystem::read_file(&self.shared.path, 0o600, LIMIT).map_err(|_| unavailable())?;
        let mut disk = bytes
            .map(|b| Ledger::load(&b))
            .transpose()
            .map_err(|_| unavailable())?
            .unwrap_or_default();
        let mut retained = state.ledger.clone();
        disk.interrupt();
        retained.interrupt();
        if serde_json::to_vec(&disk).map_err(|_| unavailable())?
            != serde_json::to_vec(&retained).map_err(|_| unavailable())?
        {
            return Err(unavailable());
        }
        let unsafe_work = retained.records.iter().any(|r| {
            r.cleanup_required
                || active(&r.summary.status)
                || matches!(
                    r.summary.status.as_str(),
                    "unknown" | "cleanup_unknown" | "interrupted"
                )
        });
        Ok((
            retained.records.len(),
            unsafe_work,
            retained
                .records
                .iter()
                .map(|r| r.summary.request_id.clone())
                .collect(),
        ))
    }
    pub(super) fn close(&self) {
        if let Some(runtime) = &self.retained_runtime {
            runtime.cancel();
        }
        if let Some(local) = &self.local {
            local.close();
        }
        if let Ok(mut state) = self.shared.state.lock() {
            state.owned = false;
            self.shared.cancelled.store(true, Ordering::Release);
        }
        if let Ok(mut slot) = self.thread.lock() {
            if let Some(thread) = slot.take() {
                let deadline = Instant::now() + Duration::from_millis(250);
                while !thread.is_finished() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(5));
                }
                if thread.is_finished() {
                    let _ = thread.join();
                }
                // Finite runtime cleanup may outlive the owner, but can never publish.
            }
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.close();
    }
}
impl Shared {
    fn commit(&self, state: &mut State, next: Ledger) -> AppResult<()> {
        if !state.owned || state.fault || self.cancelled.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        let result = serde_json::to_vec(&next)
            .map_err(|_| "Cannot encode Dev ledger".to_owned())
            .and_then(|bytes| {
                if bytes.len() as u64 > LIMIT {
                    return Err("Dev ledger too large".into());
                }
                filesystem::read_file(&self.path, 0o600, LIMIT)?;
                filesystem::publish(&self.path, &bytes, 0o600, true)
            });
        if result.is_err() {
            state.fault = true;
            self.cancelled.store(true, Ordering::Release);
            return Err(unavailable());
        }
        state.ledger = next;
        Ok(())
    }
    fn update(&self, id: &str, change: impl FnOnce(&mut Record)) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let mut next = state.ledger.clone();
        let r = next
            .records
            .iter_mut()
            .find(|r| r.summary.request_id == id)
            .ok_or_else(unavailable)?;
        change(r);
        self.commit(&mut state, next)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    request_id: String,
    instance_id: String,
    project_id: String,
    agent_id: String,
    expected_projects_revision: u64,
    expected_agents_revision: u64,
    prompt: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    request_id: String,
}
impl Application {
    pub(super) fn call_dev_jobs(&self, req: AppRequest) -> AppResult<Value> {
        if !req.args.is_object() {
            return Err(invalid());
        }
        match req.op.as_str() {
            "dev_jobs_state" => {
                let _: Empty = decode(req.args)?;
                let state = self
                    .dev_jobs
                    .shared
                    .state
                    .lock()
                    .map_err(|_| unavailable())?;
                if state.fault {
                    return Err(unavailable());
                }
                Ok(
                    json!({"jobs":state.ledger.records.iter().map(|r| &r.summary).collect::<Vec<_>>()}),
                )
            }
            "dev_job_get" => {
                let a: Get = decode(req.args)?;
                if !valid_id(&a.request_id) {
                    return Err(invalid());
                }
                let state = self
                    .dev_jobs
                    .shared
                    .state
                    .lock()
                    .map_err(|_| unavailable())?;
                if state.fault {
                    return Err(unavailable());
                }
                let r = state
                    .ledger
                    .records
                    .iter()
                    .find(|r| r.summary.request_id == a.request_id)
                    .ok_or_else(|| {
                        AppError::new("UNKNOWN_REQUEST", "Unknown Dev request; do not resend")
                    })?;
                Ok(json!(r.summary))
            }
            "dev_job_start" => self.start_dev(req.args),
            _ => Err(invalid()),
        }
    }
    fn start_dev(&self, args: Value) -> AppResult<Value> {
        let a: Start = decode(args)?;
        if !valid_id(&a.request_id) {
            return Err(invalid());
        }
        let fp = fingerprint(
            "dev_job_start:1",
            &serde_json::to_value(&a).map_err(|_| invalid())?,
        )?;
        let mut state = self
            .dev_jobs
            .shared
            .state
            .lock()
            .map_err(|_| unavailable())?;
        if state.fault || !state.owned {
            return Err(unavailable());
        }
        if let Some(r) = state
            .ledger
            .records
            .iter()
            .find(|r| r.summary.request_id == a.request_id)
        {
            return if r.fingerprint == fp {
                Ok(json!(r.summary))
            } else {
                Err(AppError::new(
                    "REQUEST_CONFLICT",
                    "Dev request identity already used",
                ))
            };
        }
        // A stopped/absent observation cannot exclude a delayed daemon create/start
        // from an interrupted service. This slice provides no reconciliation workflow.
        if state
            .ledger
            .records
            .iter()
            .any(|r| matches!(r.summary.status.as_str(), "interrupted" | "cleanup_unknown"))
        {
            return Err(AppError::new(
                "CLEANUP_UNKNOWN",
                "Prior Dev runtime requires separately authorized operator reconciliation",
            ));
        }
        if !valid_id(&a.instance_id)
            || !valid_id(&a.project_id)
            || a.prompt.trim().is_empty()
            || a.prompt.len() > MAX_TEXT
            || a.prompt
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(invalid());
        }
        if state
            .ledger
            .records
            .iter()
            .any(|r| active(&r.summary.status))
        {
            return Err(AppError::new("BUSY", "A Dev job is already active"));
        }
        if state.ledger.records.len() >= 32 {
            return Err(AppError::new(
                "CAPACITY",
                "Dev ledger is full; no records were evicted",
            ));
        }
        // Capture all saved constituents together. No runtime work holds this mutex.
        let (project_key, agent_name, prompt) = {
            let inner = self.inner.lock().map_err(|_| storage())?;
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            if a.instance_id != self.instance_id {
                return Err(AppError::new("STALE_INSTANCE", "Service instance changed"));
            }
            let projects =
                serde_json::to_value(&inner.durable.projects).map_err(|_| unavailable())?;
            let agents = serde_json::to_value(&inner.durable.agents).map_err(|_| unavailable())?;
            if projects["revision"].as_u64() != Some(a.expected_projects_revision)
                || agents["revision"].as_u64() != Some(a.expected_agents_revision)
            {
                return Err(AppError::new(
                    "STALE_REVISION",
                    "Saved project or agent changed",
                ));
            }
            let p = projects["projects"]
                .as_array()
                .and_then(|ps| {
                    ps.iter()
                        .find(|p| p["project_id"].as_str() == Some(&a.project_id))
                })
                .ok_or_else(invalid)?;
            let agent = agents["agents"]
                .as_array()
                .and_then(|xs| {
                    xs.iter()
                        .find(|x| x["agent_id"].as_str() == Some(&a.agent_id))
                })
                .ok_or_else(invalid)?;
            let role = agent["prompt"].as_str().ok_or_else(invalid)?;
            if role.trim().is_empty() {
                return Err(AppError::new(
                    "NOT_CONFIGURED",
                    "Saved agent prompt is empty",
                ));
            }
            (
                p["key"].as_str().ok_or_else(invalid)?.to_owned(),
                agent["name"].as_str().ok_or_else(invalid)?.to_owned(),
                format!(
                    "Global:\n{}\nProject:\n{}\nAgent:\n{}\nTask:\n{}",
                    inner.durable.instructions.dev_settings_text(),
                    p["prompt"].as_str().ok_or_else(invalid)?,
                    role,
                    a.prompt
                ),
            )
        };
        let prior = state
            .ledger
            .records
            .iter()
            .filter(|r| {
                r.cleanup_required
                    || matches!(
                        r.summary.status.as_str(),
                        "interrupted" | "unknown" | "cleanup_unknown"
                    )
            })
            .map(|r| r.summary.request_id.clone())
            .collect::<Vec<_>>();
        let summary = Summary {
            request_id: a.request_id.clone(),
            project_id: a.project_id,
            project_key,
            agent_id: a.agent_id,
            agent_name,
            status: "initializing".into(),
            message: "Initializing receipt-only runtime; no inference".into(),
            session_id: None,
            created_ms: self.cloud.clock.wall_ms(),
        };
        let mut next = state.ledger.clone();
        next.records.push(Record {
            summary: summary.clone(),
            fingerprint: fp,
            prompt: prompt.clone(),
            may_have_sent: false,
            cleanup_required: true,
        });
        self.dev_jobs.shared.commit(&mut state, next)?;
        let shared = self.dev_jobs.shared.clone();
        let runtime = self.dev_jobs.runtime.clone();
        let request = RuntimeRequest {
            request_id: a.request_id.clone(),
            prompt,
        };
        let mut slot = self.dev_jobs.thread.lock().map_err(|_| unavailable())?;
        if let Some(previous) = slot.take() {
            if previous.is_finished() {
                let _ = previous.join();
            }
        }
        match std::thread::Builder::new()
            .name("smith-dev-job".into())
            .spawn(move || run(shared, runtime, request, prior))
        {
            Ok(thread) => *slot = Some(thread),
            Err(_) => {
                let mut next = state.ledger.clone();
                if let Some(r) = next.records.last_mut() {
                    r.summary.status = "failed".into();
                    r.summary.message = "Could not start Dev worker".into();
                    r.cleanup_required = false;
                }
                self.dev_jobs.shared.commit(&mut state, next)?;
                return Ok(json!(
                    state.ledger.records.last().ok_or_else(unavailable)?.summary
                ));
            }
        }
        Ok(json!(summary))
    }
}
fn run(
    shared: Arc<Shared>,
    runtime: Arc<dyn DevRuntime>,
    request: RuntimeRequest,
    prior: Vec<String>,
) {
    let id = &request.request_id;
    for old in prior {
        if shared.cancelled.load(Ordering::Acquire) {
            return;
        }
        if !matches!(runtime.is_stopped(&old), Ok(true)) {
            let _ = shared.update(id, |r| {
                r.summary.status = "cleanup_unknown".into();
                r.summary.message = "Prior owned runtime is not proven stopped; no dispatch".into();
            });
            return;
        }
        if shared.update(&old, |r| r.cleanup_required = false).is_err() {
            return;
        }
    }
    if shared.cancelled.load(Ordering::Acquire) {
        return;
    }
    let mut event = |event| -> Result<()> {
        {
            let state = shared.state.lock().map_err(|_| "Dev publication fenced")?;
            let r = state
                .ledger
                .records
                .iter()
                .find(|r| r.summary.request_id == *id)
                .ok_or("Dev publication fenced")?;
            let valid = match &event {
                RuntimeEvent::Ready { session_id } => {
                    r.summary.status == "initializing" && valid_session(session_id)
                }
                RuntimeEvent::Dispatching => r.summary.status == "ready",
            };
            if !valid {
                return Err("Invalid Dev runtime event".into());
            }
        }
        shared
            .update(id, |r| match event {
                RuntimeEvent::Ready { session_id } => {
                    if valid_session(&session_id) && r.summary.status == "initializing" {
                        r.summary.session_id = Some(session_id);
                        r.summary.status = "ready".into();
                        r.summary.message = "Runtime ready; receipt only".into();
                    }
                }
                RuntimeEvent::Dispatching => {
                    if r.summary.status == "ready" {
                        r.summary.status = "dispatching".into();
                        r.summary.message = "Dispatch attempted; delivery not yet verified".into();
                        r.may_have_sent = true;
                    }
                }
            })
            .map_err(|_| "Dev publication fenced".to_owned())?;
        let state = shared.state.lock().map_err(|_| "Dev publication fenced")?;
        let r = state
            .ledger
            .records
            .iter()
            .find(|r| r.summary.request_id == *id)
            .ok_or("Dev publication fenced")?;
        if !matches!(r.summary.status.as_str(), "ready" | "dispatching") {
            return Err("Invalid Dev runtime event".into());
        }
        Ok(())
    };
    let outcome = runtime.run(&request, &shared.cancelled, &mut event);
    if shared.cancelled.load(Ordering::Acquire) {
        return;
    }
    match outcome {
        Ok(receipt) => {
            let stopped = matches!(runtime.is_stopped(id), Ok(true));
            let _ = shared.update(id, |r| {
                if stopped
                    && r.may_have_sent
                    && r.summary.status == "dispatching"
                    && r.summary.session_id.as_deref() == Some(&receipt.session_id)
                {
                    r.summary.status = "recorded".into();
                    r.summary.message =
                        "Receipt verified and owned runtime stopped; no inference".into();
                    r.cleanup_required = false;
                } else {
                    r.summary.status = if stopped {
                        "unknown"
                    } else {
                        "cleanup_unknown"
                    }
                    .into();
                    r.summary.message =
                        "Receipt or cleanup could not be verified; no replay".into();
                    r.cleanup_required = true;
                }
            });
        }
        Err(failure) => {
            // Only compile-time allowlisted diagnostics may cross the runtime boundary.
            let _ = shared.update(id, |r| {
                r.may_have_sent |= failure.may_have_sent;
                r.cleanup_required = failure.cleanup_unknown || r.may_have_sent;
                r.summary.status = if failure.cleanup_unknown {
                    "cleanup_unknown"
                } else if r.may_have_sent {
                    "unknown"
                } else {
                    "failed"
                }
                .into();
                r.summary.message = failure.diagnostic.message().into();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_runtime::{Receipt, RuntimeDiagnostic, RuntimeFailure};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::AtomicUsize;

    struct Fake {
        calls: AtomicUsize,
        stop_checks: AtomicUsize,
        stopped: bool,
        captured: Mutex<Vec<String>>,
        break_publication: bool,
        failure: Mutex<Option<RuntimeFailure>>,
        path: PathBuf,
    }
    impl DevRuntime for Fake {
        fn is_stopped(&self, _: &str) -> Result<bool> {
            self.stop_checks.fetch_add(1, Ordering::SeqCst);
            Ok(self.stopped)
        }
        fn run(
            &self,
            request: &RuntimeRequest,
            _: &AtomicBool,
            event: &mut dyn FnMut(RuntimeEvent) -> Result<()>,
        ) -> std::result::Result<Receipt, RuntimeFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.captured.lock().unwrap().push(request.prompt.clone());
            if let Some(failure) = *self.failure.lock().unwrap() {
                return Err(failure);
            }
            let failure = || RuntimeFailure {
                diagnostic: RuntimeDiagnostic::UnverifiedReceipt,
                may_have_sent: false,
                cleanup_unknown: false,
            };
            event(RuntimeEvent::Ready {
                session_id: "ses_test".into(),
            })
            .map_err(|_| failure())?;
            if self.break_publication {
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
            }
            event(RuntimeEvent::Dispatching).map_err(|_| failure())?;
            Ok(Receipt {
                session_id: "ses_test".into(),
            })
        }
    }
    struct Fixture {
        base: PathBuf,
        owner: Owner,
        runtime: Arc<Fake>,
    }
    impl Fixture {
        fn new(stopped: bool, break_publication: bool) -> Self {
            let base = std::env::temp_dir().join(format!("smith-dev-domain-{}", new_id().unwrap()));
            std::fs::create_dir(&base).unwrap();
            std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
            let path = base.join("dev-jobs.json");
            let runtime = Arc::new(Fake {
                calls: AtomicUsize::new(0),
                stop_checks: AtomicUsize::new(0),
                stopped,
                captured: Mutex::new(vec![]),
                break_publication,
                failure: Mutex::new(None),
                path: path.clone(),
            });
            let owner = Owner::open(path, runtime.clone());
            Self {
                base,
                owner,
                runtime,
            }
        }
        fn seed(&self, status: &str) {
            let r = Record {
                summary: Summary {
                    request_id: format!("{:032x}", 1),
                    project_id: format!("{:032x}", 2),
                    project_key: "smith".into(),
                    agent_id: "developer".into(),
                    agent_name: "Developer".into(),
                    status: status.into(),
                    message: "Receipt only".into(),
                    session_id: None,
                    created_ms: 1,
                },
                fingerprint: "a".repeat(64),
                prompt: "Global:\nexact\nProject:\nproject\nAgent:\nrole\nTask:\ntask".into(),
                may_have_sent: status == "dispatching",
                cleanup_required: true,
            };
            let mut state = self.owner.shared.state.lock().unwrap();
            self.owner
                .shared
                .commit(&mut state, Ledger { records: vec![r] })
                .unwrap();
        }
        fn run(&self, prior: Vec<String>) {
            let r = self.owner.shared.state.lock().unwrap().ledger.records[0].clone();
            run(
                self.owner.shared.clone(),
                self.runtime.clone(),
                RuntimeRequest {
                    request_id: r.summary.request_id,
                    prompt: r.prompt,
                },
                prior,
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.owner.close();
            std::fs::remove_dir_all(&self.base).unwrap();
        }
    }
    #[test]
    fn verified_receipt_requires_stopped_runtime_and_omits_prompt() {
        let f = Fixture::new(true, false);
        f.seed("initializing");
        f.run(vec![]);
        let state = f.owner.shared.state.lock().unwrap();
        let r = &state.ledger.records[0];
        assert_eq!(r.summary.status, "recorded");
        assert!(!r.cleanup_required);
        let wire = serde_json::to_value(&r.summary).unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 9);
        assert!(wire.get("prompt").is_none());
        assert_eq!(f.runtime.captured.lock().unwrap()[0], r.prompt);
    }
    #[test]
    fn restart_preserves_possible_delivery_without_running_and_corruption_is_isolated() {
        let f = Fixture::new(true, false);
        f.seed("dispatching");
        let owner = Owner::open(f.owner.shared.path.clone(), f.runtime.clone());
        let state = owner.shared.state.lock().unwrap();
        let r = &state.ledger.records[0];
        assert_eq!(r.summary.status, "cleanup_unknown");
        assert!(r.may_have_sent && r.cleanup_required);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.runtime.stop_checks.load(Ordering::SeqCst), 0);
        drop(state);
        filesystem::publish(&f.owner.shared.path, b"broken", 0o600, true).unwrap();
        let corrupt = Owner::open(f.owner.shared.path.clone(), f.runtime.clone());
        assert!(corrupt.shared.state.lock().unwrap().fault);
        assert_eq!(std::fs::read(&f.owner.shared.path).unwrap(), b"broken");
    }
    #[test]
    fn custom_agent_ledger_reopens_without_runtime_effects_and_rejects_noncanonical_ids() {
        let f = Fixture::new(true, false);
        f.seed("dispatching");
        let mut ledger = f.owner.shared.state.lock().unwrap().ledger.clone();
        let custom_id = "0123456789abcdef0123456789abcdef";
        ledger.records[0].summary.agent_id = custom_id.into();
        filesystem::publish(
            &f.owner.shared.path,
            &serde_json::to_vec(&ledger).unwrap(),
            0o600,
            true,
        )
        .unwrap();
        let owner = Owner::open(f.owner.shared.path.clone(), f.runtime.clone());
        let state = owner.shared.state.lock().unwrap();
        assert!(!state.fault);
        assert_eq!(state.ledger.records[0].summary.agent_id, custom_id);
        assert_eq!(state.ledger.records[0].summary.status, "cleanup_unknown");
        drop(state);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.runtime.stop_checks.load(Ordering::SeqCst), 0);
        for bad in [
            "planner",
            "Developer",
            "0123456789ABCDEF0123456789ABCDEF",
            "abc",
        ] {
            ledger.records[0].summary.agent_id = bad.into();
            let bytes = serde_json::to_vec(&ledger).unwrap();
            filesystem::publish(&f.owner.shared.path, &bytes, 0o600, true).unwrap();
            let owner = Owner::open(f.owner.shared.path.clone(), f.runtime.clone());
            assert!(owner.shared.state.lock().unwrap().fault);
            assert_eq!(std::fs::read(&f.owner.shared.path).unwrap(), bytes);
        }
    }
    #[test]
    fn uncertain_prior_runtime_blocks_new_dispatch() {
        let f = Fixture::new(false, false);
        f.seed("initializing");
        f.run(vec![format!("{:032x}", 9)]);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            f.owner.shared.state.lock().unwrap().ledger.records[0]
                .summary
                .status,
            "cleanup_unknown"
        );
    }
    #[test]
    fn admission_freezes_saved_context_and_replay_precedes_instance_drift() {
        struct Never;
        impl Provider for Never {
            fn inventory(&self) -> AppResult<Inventory> {
                panic!("Dev must not query Chat providers")
            }
            fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
                panic!("Dev must not infer")
            }
        }
        let f = Fixture::new(true, false);
        let roots = Roots {
            data: f.base.join("data"),
            config: f.base.join("config"),
            runtime: f.base.join("runtime"),
        };
        for path in [
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
        ] {
            std::fs::create_dir(path).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut app = Application::open(&roots, Arc::new(Never)).unwrap();
        app.dev_jobs = Arc::new(Owner::open(f.owner.shared.path.clone(), f.runtime.clone()));
        let (projects, agents, global) = {
            let inner = app.inner.lock().unwrap();
            (
                serde_json::to_value(&inner.durable.projects).unwrap(),
                serde_json::to_value(&inner.durable.agents).unwrap(),
                inner.durable.instructions.dev_settings_text(),
            )
        };
        let task = "  exact task\n\ttext  ";
        let args = json!({"request_id":format!("{:032x}", 80), "instance_id":app.instance_id, "project_id":projects["projects"][0]["project_id"], "agent_id":"developer", "expected_projects_revision":projects["revision"], "expected_agents_revision":agents["revision"], "prompt":task});
        let result = app.start_dev(args.clone()).unwrap();
        assert_eq!(result["status"], "initializing");
        let thread = app.dev_jobs.thread.lock().unwrap().take().unwrap();
        thread.join().unwrap();
        assert_eq!(
            f.runtime.captured.lock().unwrap()[0],
            format!(
                "Global:\n{}\nProject:\n{}\nAgent:\n{}\nTask:\n{}",
                global,
                projects["projects"][0]["prompt"].as_str().unwrap(),
                agents["agents"][0]["prompt"].as_str().unwrap(),
                task
            )
        );
        app.instance_id = new_id().unwrap();
        assert_eq!(app.start_dev(args.clone()).unwrap()["status"], "recorded");
        let mut conflict = args.clone();
        conflict["prompt"] = json!("different");
        assert_eq!(
            app.start_dev(conflict).unwrap_err().code,
            "REQUEST_CONFLICT"
        );
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
        // Even a fake that would report stopped must not unlock a crashed create.
        f.seed("initializing");
        let stop_checks = f.runtime.stop_checks.load(Ordering::SeqCst);
        app.dev_jobs = Arc::new(Owner::open(f.owner.shared.path.clone(), f.runtime.clone()));
        let before = std::fs::read(&f.owner.shared.path).unwrap();
        let mut next = args;
        next["request_id"] = json!(format!("{:032x}", 81));
        assert_eq!(app.start_dev(next).unwrap_err().code, "CLEANUP_UNKNOWN");
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
        assert_eq!(std::fs::read(&f.owner.shared.path).unwrap(), before);
        assert_eq!(f.runtime.stop_checks.load(Ordering::SeqCst), stop_checks);
        drop(app);
    }
    #[test]
    fn prerequisite_diagnostic_preserves_delivery_and_cleanup_uncertainty() {
        for persisted_sent in [false, true] {
            for failure_sent in [false, true] {
                for cleanup_unknown in [false, true] {
                    let f = Fixture::new(true, false);
                    f.seed(if persisted_sent {
                        "dispatching"
                    } else {
                        "initializing"
                    });
                    *f.runtime.failure.lock().unwrap() = Some(RuntimeFailure {
                        diagnostic: RuntimeDiagnostic::SocketMissing,
                        may_have_sent: failure_sent,
                        cleanup_unknown,
                    });
                    f.run(vec![]);
                    let state = f.owner.shared.state.lock().unwrap();
                    let r = &state.ledger.records[0];
                    let sent = persisted_sent || failure_sent;
                    assert_eq!(r.may_have_sent, sent);
                    assert_eq!(r.cleanup_required, cleanup_unknown || sent);
                    assert_eq!(
                        r.summary.status,
                        if cleanup_unknown {
                            "cleanup_unknown"
                        } else if sent {
                            "unknown"
                        } else {
                            "failed"
                        }
                    );
                    assert_eq!(
                        r.summary.message,
                        RuntimeDiagnostic::SocketMissing.message()
                    );
                    assert_eq!(r.summary.session_id, None);
                    assert_eq!(r.summary.request_id, format!("{:032x}", 1));
                    assert_eq!(
                        serde_json::to_value(&r.summary)
                            .unwrap()
                            .as_object()
                            .unwrap()
                            .len(),
                        9
                    );
                    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(f.runtime.stop_checks.load(Ordering::SeqCst), 0);
                    let disk = Ledger::load(&std::fs::read(&f.owner.shared.path).unwrap()).unwrap();
                    assert_eq!(disk.records[0].summary.message, r.summary.message);
                    assert_eq!(disk.records[0].summary.status, r.summary.status);
                }
            }
        }
    }

    #[test]
    fn untrusted_runtime_error_is_not_published() {
        let f = Fixture::new(true, false);
        f.seed("initializing");
        let private_error = "Required private rootless Docker socket was not found /private/canary token=secret prompt=hidden";
        *f.runtime.failure.lock().unwrap() = Some(RuntimeFailure {
            diagnostic: private_error.to_owned().into(),
            may_have_sent: false,
            cleanup_unknown: false,
        });
        f.run(vec![]);
        let state = f.owner.shared.state.lock().unwrap();
        let wire = serde_json::to_string(&state.ledger.records[0].summary).unwrap();
        assert_eq!(
            state.ledger.records[0].summary.message,
            RuntimeDiagnostic::UnverifiedReceipt.message()
        );
        for canary in ["canary", "secret", "hidden", "/private/", "token="] {
            assert!(!wire.contains(canary));
        }
        assert_eq!(f.runtime.stop_checks.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn reopening_historical_failed_record_preserves_generic_diagnostic_without_probing() {
        let f = Fixture::new(true, false);
        f.seed("failed");
        f.owner
            .shared
            .update(&format!("{:032x}", 1), |r| {
                r.summary.message = RuntimeDiagnostic::UnverifiedReceipt.message().into();
                r.cleanup_required = false;
            })
            .unwrap();
        let before = std::fs::read(&f.owner.shared.path).unwrap();
        let owner = Owner::open(f.owner.shared.path.clone(), f.runtime.clone());
        let state = owner.shared.state.lock().unwrap();
        let r = &state.ledger.records[0];
        assert_eq!(r.summary.status, "failed");
        assert_eq!(
            r.summary.message,
            "Runtime did not complete a verified receipt; no replay"
        );
        assert!(!r.may_have_sent);
        assert!(!r.cleanup_required);
        assert_eq!(std::fs::read(&f.owner.shared.path).unwrap(), before);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.runtime.stop_checks.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn dispatch_publication_error_fences_and_keeps_durable_ambiguity() {
        let f = Fixture::new(true, true);
        f.seed("initializing");
        f.run(vec![]);
        assert!(f.owner.shared.state.lock().unwrap().fault);
        assert!(f.owner.shared.cancelled.load(Ordering::Acquire));
        let disk: Ledger =
            serde_json::from_slice(&std::fs::read(&f.owner.shared.path).unwrap()).unwrap();
        assert_eq!(disk.records[0].summary.status, "ready");
        assert!(disk.records[0].cleanup_required);
        assert!(!disk.records[0].may_have_sent);
    }
}
