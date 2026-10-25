use super::*;
use crate::dev_local::LocalFailure;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::atomic::AtomicUsize;

fn id(n: usize) -> String {
    format!("{n:032x}")
}
struct NeverLegacy;
impl crate::dev_runtime::DevRuntime for NeverLegacy {
    fn is_stopped(&self, _: &str) -> Result<bool> {
        panic!("no legacy probes")
    }
    fn run(
        &self,
        _: &crate::dev_runtime::RuntimeRequest,
        _: &AtomicBool,
        _: &mut dyn FnMut(crate::dev_runtime::RuntimeEvent) -> Result<()>,
    ) -> std::result::Result<crate::dev_runtime::Receipt, crate::dev_runtime::RuntimeFailure> {
        panic!("no legacy execution")
    }
}
#[derive(Default)]
struct FakeProvider {
    calls: AtomicUsize,
    fail: AtomicBool,
    close_during_completion: Mutex<Option<Arc<Owner>>>,
    threads: Mutex<Vec<std::thread::ThreadId>>,
    in_flight: AtomicUsize,
}
impl Provider for FakeProvider {
    fn inventory(&self) -> AppResult<Inventory> {
        panic!("no inventory probes")
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        panic!("no Chat or cloud fallback")
    }
    fn inspection_completion(
        &self,
        model: &FrozenModel,
        body: &Value,
        _: Instant,
    ) -> AppResult<Value> {
        assert_eq!(self.in_flight.fetch_add(1, Ordering::SeqCst), 0);
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        assert!(valid_model(model));
        assert_eq!(body, &json!({"fixture":"inspection"}));
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(owner) = self.close_during_completion.lock().unwrap().take() {
            owner.close();
        }
        if self.fail.load(Ordering::Acquire) {
            return Err(AppError::new(
                "SECRET_PROVIDER_ERROR",
                "private raw provider error",
            ));
        }
        Ok(json!({"text":"bounded fixture completion"}))
    }
}
struct FakeRuntime {
    calls: AtomicUsize,
    mode: AtomicUsize,
    path: PathBuf,
    captured: Mutex<Vec<LocalJob>>,
    thread: Mutex<Option<std::thread::ThreadId>>,
}
impl FakeRuntime {
    fn disk(&self) -> Ledger {
        Ledger::load(&std::fs::read(&self.path).unwrap()).unwrap()
    }
}
impl LocalRuntime for FakeRuntime {
    fn run(
        &self,
        job: &LocalJob,
        cancel: &AtomicBool,
        event: &mut dyn FnMut(LocalEvent) -> AppResult<()>,
        completion: &mut dyn FnMut(&Value, Instant) -> AppResult<Value>,
    ) -> std::result::Result<LocalReceipt, LocalFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.thread.lock().unwrap() = Some(std::thread::current().id());
        self.captured.lock().unwrap().push(job.clone());
        assert!(!cancel.load(Ordering::Acquire));
        let failure = || LocalFailure {
            error: LocalError::Publication,
            may_have_sent: false,
            cleanup_unknown: true,
        };
        assert_eq!(
            self.disk()
                .records
                .iter()
                .find(|r| r.summary.request_id == job.request_id)
                .unwrap()
                .summary
                .status,
            "preparing"
        );
        event(LocalEvent::Preparing {
            image_id: format!("sha256:{}", "a".repeat(64)),
        })
        .map_err(|_| failure())?;
        if self.mode.load(Ordering::Acquire) == 4 {
            return Err(LocalFailure {
                error: LocalError::RepositoryUnavailable,
                may_have_sent: false,
                cleanup_unknown: false,
            });
        }
        event(LocalEvent::Cloning).map_err(|_| failure())?;
        assert_eq!(
            self.disk()
                .records
                .iter()
                .find(|r| r.summary.request_id == job.request_id)
                .unwrap()
                .summary
                .status,
            "cloning"
        );
        let repositories = job
            .repositories
            .iter()
            .map(|r| Checkout {
                url: r.url.clone(),
                reference: crate::dev_fetch::checkout_reference(r.reference.as_deref()),
                commit: "b".repeat(40),
            })
            .collect::<Vec<_>>();
        event(LocalEvent::Ready {
            session_id: "ses_fixture".into(),
            repositories: repositories.clone(),
        })
        .map_err(|_| failure())?;
        assert_eq!(
            self.disk()
                .records
                .iter()
                .find(|r| r.summary.request_id == job.request_id)
                .unwrap()
                .summary
                .status,
            "ready"
        );
        if self.mode.load(Ordering::Acquire) == 3 {
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        event(LocalEvent::Dispatching).map_err(|_| failure())?;
        let disk = self.disk();
        let r = disk
            .records
            .iter()
            .find(|r| r.summary.request_id == job.request_id)
            .unwrap();
        assert_eq!(r.summary.status, "running");
        assert!(r.may_have_sent);
        completion(
            &json!({"fixture":"inspection"}),
            Instant::now() + Duration::from_secs(5),
        )
        .map_err(|e| LocalFailure {
            error: LocalError::from(e),
            may_have_sent: true,
            cleanup_unknown: false,
        })?;
        event(LocalEvent::Reply {
            session_id: "ses_fixture".into(),
            message_id: "msg_fixture".into(),
            text: "Saved reply\nwith citation r0:1".into(),
        })
        .map_err(|_| failure())?;
        let disk = self.disk();
        let r = disk
            .records
            .iter()
            .find(|r| r.summary.request_id == job.request_id)
            .unwrap();
        assert!(r.summary.reply_available && !r.summary.cleanup_verified);
        assert_eq!(r.reply.as_deref(), Some("Saved reply\nwith citation r0:1"));
        match self.mode.load(Ordering::Acquire) {
            1 => {
                return Err(LocalFailure {
                    error: LocalError::Shutdown,
                    may_have_sent: true,
                    cleanup_unknown: true,
                })
            }
            5 => {
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
                return Err(failure());
            }
            _ => {}
        }
        Ok(LocalReceipt {
            session_id: "ses_fixture".into(),
            message_id: if self.mode.load(Ordering::Acquire) == 2 {
                "msg_foreign"
            } else {
                "msg_fixture"
            }
            .into(),
            reply: "Saved reply\nwith citation r0:1".into(),
            repositories,
        })
    }
}
struct Fixture {
    app: Application,
    runtime: Arc<FakeRuntime>,
    provider: Arc<FakeProvider>,
    base: PathBuf,
    legacy_path: PathBuf,
    _receiver: mpsc::Receiver<()>,
}
impl Fixture {
    fn new() -> Self {
        Self::legacy(None)
    }
    fn legacy(bytes: Option<Vec<u8>>) -> Self {
        Self::ledgers(bytes, None)
    }
    fn ledgers(bytes: Option<Vec<u8>>, local_bytes: Option<Vec<u8>>) -> Self {
        let base = std::env::temp_dir().join(format!("smith-local-domain-{}", new_id().unwrap()));
        filesystem::mkdir(&base).unwrap();
        let legacy_path = base.join("dev-jobs.json");
        if let Some(bytes) = bytes {
            filesystem::publish(&legacy_path, &bytes, 0o600, false).unwrap();
        }
        if let Some(bytes) = local_bytes {
            filesystem::publish(&base.join("dev-local-jobs.json"), &bytes, 0o600, false).unwrap();
        }
        let runtime = Arc::new(FakeRuntime {
            calls: AtomicUsize::new(0),
            mode: AtomicUsize::new(0),
            path: base.join("dev-local-jobs.json"),
            captured: Mutex::new(Vec::new()),
            thread: Mutex::new(None),
        });
        let mut legacy = dev_jobs::Owner::open(legacy_path.clone(), Arc::new(NeverLegacy));
        legacy.local = Some(Arc::new(Owner::open(runtime.path.clone(), runtime.clone())));
        let legacy = Arc::new(legacy);
        let mut durable = Durable::default();
        let entry = &crate::catalog::CATALOG[0];
        durable.config.model = Some(entry.name.into());
        durable.config.revision = u64::MAX;
        durable.projects = serde_json::from_value(json!({"revision":1,"projects":[{
            "project_id":id(900),"name":"Fixture","key":"fixture","description":"", "prompt":"  exact project\n\t ",
            "repositories":[{"name":"fixture","url":"https://github.com/example/fixture","read_only":false,
                "branch":{"base":"main","work":"ignored/*","types":{"feature":null,"fix":null,"release":null,"hotfix":null}}}],
            "agent":"opencode"}],"receipts":[],"expired_ids":[]})).unwrap();
        let mut inventory = Inventory::default();
        inventory.models.push(crate::providers::Model {
            name: entry.name.into(),
            digest: entry.sha256.into(),
        });
        inventory.providers.llm = crate::providers::Observation {
            ready: true,
            error: None,
        };
        let inner = Arc::new(Mutex::new(Inner {
            durable,
            inventory,
            fault: false,
            owned: true,
            inventory_observed: true,
            cloud: v3::Volatile::default(),
        }));
        let cloud = Arc::new(crate::cloud::Dependencies::disabled());
        let provider = Arc::new(FakeProvider::default());
        let (sender, receiver) = mpsc::sync_channel(1);
        let owner = Arc::new(WorkerOwner {
            dev_jobs: legacy.clone(),
            provider: provider.clone(),
            cloud: cloud.clone(),
            inner: inner.clone(),
            sender: Some(sender),
            thread: None,
        });
        // Deliberately no worker thread: the fixture explicitly invokes the worker seam below.
        let app = Application {
            inner,
            owner: Some(owner),
            dev_jobs: legacy,
            path: base.join("app-state.json"),
            instance_id: id(901),
            cloud,
        };
        Self {
            app,
            runtime,
            provider,
            base,
            legacy_path,
            _receiver: receiver,
        }
    }
    fn args(&self, n: usize) -> Value {
        json!({"request_id":id(n),"instance_id":self.app.instance_id,"project_id":id(900),
            "expected_projects_revision":1,"expected_config_revision":u64::MAX.to_string(),"prompt":"  exact task\n\ttext  "})
    }
    fn call(&self, op: &str, args: Value) -> AppResult<Value> {
        let request = AppRequest {
            schema_version: 3,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: id(999),
            op: op.into(),
            args,
        };
        self.app.call(request)
    }
    fn get(&self, n: usize) -> Value {
        self.call("dev_job_get_v4", json!({"request_id":id(n)}))
            .unwrap()
    }
    fn owner(&self) -> &Owner {
        self.app.dev_jobs.local.as_ref().unwrap()
    }
    fn start(&self, n: usize) -> Value {
        self.call("dev_job_start_v4", self.args(n)).unwrap()
    }
    fn run(&self) -> AppResult<bool> {
        self.app.run_pending_dev(self.provider.as_ref())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.app.join_dev_workers();
        std::fs::remove_dir_all(&self.base).unwrap();
    }
}

#[test]
fn exact_projection_frozen_context_same_worker_and_no_replay() {
    let mut f = Fixture::new();
    let args = f.args(1);
    let result = f.start(1);
    assert_eq!(result.as_object().unwrap().len(), 9);
    assert!(f.app.dev_has_model_dependencies());
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert!(f.run().unwrap());
    assert!(!f.run().unwrap());
    assert_ne!(
        *f.runtime.thread.lock().unwrap(),
        Some(std::thread::current().id())
    );
    let global = f
        .app
        .inner
        .lock()
        .unwrap()
        .durable
        .instructions
        .dev_settings_text();
    let captured = f.runtime.captured.lock().unwrap();
    assert_eq!(
        captured[0].prompt,
        format!("Global:\n{global}\nProject:\n  exact project\n\t \nTask:\n  exact task\n\ttext  ")
    );
    assert_eq!(
        captured[0].repositories[0].reference.as_deref(),
        Some("main")
    );
    drop(captured);
    assert_eq!(f.get(1)["job"]["status"], "completed");
    assert_eq!(f.get(1)["job"]["cleanup_verified"], true);
    assert_eq!(f.get(1).as_object().unwrap().len(), 5);
    assert!(!f.owner().reserved());
    let state = f.call("dev_jobs_state_v4", json!({})).unwrap();
    assert_eq!(state.as_object().unwrap().len(), 4);
    assert_eq!(state["used"], 1);
    assert_eq!(state["model"]["config_revision"], u64::MAX.to_string());
    assert_eq!(state["model"]["ready"], true);
    f.app.instance_id = id(902);
    f.app.inner.lock().unwrap().durable.config.revision = 1;
    assert_eq!(
        f.call("dev_job_start_v4", args.clone()).unwrap()["status"],
        "completed"
    );
    let mut conflict = args;
    conflict["prompt"] = json!("changed");
    assert_eq!(
        f.call("dev_job_start_v4", conflict).unwrap_err().code,
        "REQUEST_CONFLICT"
    );
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::metadata(&f.runtime.path).unwrap().mode() & 0o7777,
        0o600
    );
    assert!(!f.legacy_path.exists());
}

#[test]
fn drift_and_unknown_observe_only_with_strict_arguments() {
    let f = Fixture::new();
    for (field, value, code) in [
        ("instance_id", json!(id(903)), "STALE_INSTANCE"),
        ("expected_projects_revision", json!(2), "STALE_REVISION"),
        ("expected_config_revision", json!("1"), "STALE_REVISION"),
        ("expected_config_revision", json!("01"), "INVALID_REQUEST"),
        ("expected_config_revision", json!(1), "INVALID_REQUEST"),
        ("agent_id", json!("developer"), "INVALID_REQUEST"),
        ("prompt", json!("x".repeat(MAX_TEXT + 1)), "INVALID_REQUEST"),
    ] {
        let mut args = f.args(1);
        args[field] = value;
        assert_eq!(f.call("dev_job_start_v4", args).unwrap_err().code, code);
    }
    assert_eq!(
        f.call("dev_job_get_v4", json!({"request_id":id(7)}))
            .unwrap_err()
            .code,
        "UNKNOWN_REQUEST"
    );
    assert!(f.call("dev_jobs_state_v4", json!({"extra":true})).is_err());
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert!(!f.runtime.path.exists());
}

#[test]
fn inventory_must_already_be_observed_matching_and_ready_without_probing() {
    let f = Fixture::new();
    f.app.inner.lock().unwrap().inventory_observed = false;
    assert_eq!(
        f.call("dev_jobs_state_v4", json!({})).unwrap()["model"]["ready"],
        false
    );
    assert_eq!(
        f.call("dev_job_start_v4", f.args(1)).unwrap_err().code,
        "LOCAL_MODEL_UNAVAILABLE"
    );
    {
        let mut inner = f.app.inner.lock().unwrap();
        inner.inventory_observed = true;
        inner.inventory.models[0].digest = "0".repeat(64);
    }
    assert_eq!(
        f.call("dev_job_start_v4", f.args(1)).unwrap_err().code,
        "LOCAL_MODEL_UNAVAILABLE"
    );
    assert!(!f.owner().reserved());
}

#[test]
fn phase_order_and_single_dispatch_are_fail_closed() {
    let f = Fixture::new();
    f.start(1);
    assert!(f.owner().event(&id(1), LocalEvent::Dispatching).is_err());
    assert!(f.owner().completion_fence(&id(1)).is_err());
    assert!(f
        .owner()
        .event(
            &id(1),
            LocalEvent::Ready {
                session_id: "ses_foreign".into(),
                repositories: Vec::new()
            }
        )
        .is_err());
    assert_eq!(f.get(1)["job"]["status"], "queued");
    assert_eq!(f.start(2)["status"], "queued");
}

#[test]
fn reply_survives_cleanup_unknown_and_restart_without_rewrite() {
    let f = Fixture::new();
    f.runtime.mode.store(1, Ordering::Release);
    f.start(1);
    f.run().unwrap();
    let result = f.get(1);
    assert_eq!(result["job"]["status"], "cleanup_unknown");
    assert_eq!(result["job"]["reply_available"], true);
    assert_eq!(result["job"]["cleanup_verified"], false);
    assert!(!f.owner().reserved());
    assert_eq!(f.start(2)["status"], "queued");
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    let reopened = Owner::open(f.runtime.path.clone(), f.runtime.clone());
    assert!(!reopened.reserved());
    assert_eq!(
        reopened.state.lock().unwrap().ledger.records[0]
            .reply
            .as_deref(),
        result["reply"].as_str()
    );
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn unfinished_restart_blocks_and_never_takes_pending() {
    let f = Fixture::new();
    f.start(1);
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    let reopened = Owner::open(f.runtime.path.clone(), f.runtime.clone());
    assert!(!reopened.reserved());
    let state = reopened.state.lock().unwrap();
    assert!(state.pending.is_none());
    assert_eq!(state.ledger.records[0].summary.status, "failed");
    assert_eq!(
        state.ledger.records[0].failed_stage.as_deref(),
        Some("queued")
    );
    assert!(state.ledger.records[0].summary.cleanup_verified);
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn completed_requires_receipt_correlation_even_after_verified_stop() {
    let f = Fixture::new();
    f.runtime.mode.store(2, Ordering::Release);
    f.start(1);
    f.run().unwrap();
    assert_eq!(f.get(1)["job"]["status"], "unknown");
    assert_eq!(f.get(1)["job"]["reply_available"], true);
    assert_eq!(f.get(1)["job"]["cleanup_verified"], true);
    assert!(!f.owner().reserved());
}

#[test]
fn publication_failure_prevents_provider_dispatch_and_fences() {
    let f = Fixture::new();
    f.runtime.mode.store(3, Ordering::Release);
    f.start(1);
    assert!(f.run().is_err());
    assert!(f.owner().reserved());
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    let state = f.owner().state.lock().unwrap();
    assert!(state.fault && state.ledger.records[0].may_have_sent);
    assert_eq!(state.ledger.records[0].summary.status, "cleanup_unknown");
}

#[test]
fn reply_is_on_disk_before_cleanup_publication_can_fail() {
    let f = Fixture::new();
    f.runtime.mode.store(5, Ordering::Release);
    f.start(1);
    assert!(f.run().is_err());
    let disk = Ledger::load(&std::fs::read(&f.runtime.path).unwrap()).unwrap();
    assert!(disk.records[0].reply.is_some());
    assert_eq!(disk.records[0].summary.status, "running");
    assert!(!disk.records[0].summary.cleanup_verified);
    assert!(f.owner().reserved());
}

#[test]
fn checkout_publication_uses_the_same_normalized_ref_as_acquisition() {
    let f = Fixture::new();
    f.start(1);
    let mut job = f.owner().state.lock().unwrap().ledger.records[0]
        .job
        .clone();
    for (input, expected) in [
        (Some("main"), "refs/heads/main"),
        (Some("refs/heads/main"), "refs/heads/main"),
        (Some("release/v1"), "refs/heads/release/v1"),
        (Some("refs/heads/release/v1"), "refs/heads/release/v1"),
        (None, "HEAD"),
    ] {
        job.repositories[0].reference = input.map(str::to_owned);
        let mut evidence = Checkout {
            url: job.repositories[0].url.clone(),
            reference: expected.into(),
            commit: "b".repeat(40),
        };
        assert!(checkouts(&job, &[evidence.clone()]), "{input:?}");
        evidence.reference = format!("refs/heads/{expected}");
        assert!(!checkouts(&job, &[evidence]), "{input:?}");
    }
}

#[test]
fn rootful_and_historical_prerequisite_messages_reopen_without_runtime_effects() {
    use crate::dev_runtime::RuntimeDiagnostic::*;
    for diagnostic in [
        SocketMissing,
        SocketUnavailable,
        SocketUnsafe,
        DaemonCheckFailed,
        RootlessUnverified,
        UnverifiedReceipt,
        RootfulSocketMissing,
        RootfulSocketUnavailable,
        RootfulSocketUnsafe,
        RootfulDaemonCheckFailed,
        RootfulUnverified,
    ] {
        let f = Fixture::new();
        f.start(1);
        claim_fixture(&f, 1);
        f.owner()
            .finish(
                &id(1),
                Err(LocalFailure {
                    error: LocalError::Prerequisite(diagnostic),
                    may_have_sent: false,
                    cleanup_unknown: false,
                }),
            )
            .unwrap();
        let result = f.get(1);
        assert_eq!(result["job"]["message"], diagnostic.message());
        assert_eq!(result["job"]["status"], "failed");
        assert_eq!(result["job"]["cleanup_verified"], true);
        assert!(!f.owner().reserved());
        let bytes = std::fs::read(&f.runtime.path).unwrap();
        let reopened = Owner::open(f.runtime.path.clone(), f.runtime.clone());
        assert!(!reopened.reserved());
        let state = reopened.state.lock().unwrap();
        assert!(!state.fault);
        assert_eq!(
            state.ledger.records[0].summary.message,
            diagnostic.message()
        );
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn rootful_diagnostic_preserves_dispatch_and_cleanup_uncertainty() {
    let f = Fixture::new();
    f.start(1);
    claim_fixture(&f, 1);
    f.owner()
        .finish(
            &id(1),
            Err(LocalFailure {
                error: LocalError::Prerequisite(
                    crate::dev_runtime::RuntimeDiagnostic::RootfulDaemonCheckFailed,
                ),
                may_have_sent: true,
                cleanup_unknown: true,
            }),
        )
        .unwrap();
    let result = f.get(1);
    assert_eq!(result["job"]["status"], "cleanup_unknown");
    assert_eq!(result["job"]["cleanup_verified"], false);
    assert_eq!(result["job"]["reply_available"], false);
    assert!(!f.owner().reserved());
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn safe_predispatch_failure_releases_but_raw_provider_failure_does_not_leak() {
    let f = Fixture::new();
    f.runtime.mode.store(4, Ordering::Release);
    f.start(1);
    f.run().unwrap();
    assert_eq!(f.get(1)["job"]["status"], "failed");
    assert!(!f.owner().reserved());
    f.runtime.mode.store(0, Ordering::Release);
    f.provider.fail.store(true, Ordering::Release);
    f.start(2);
    f.run().unwrap();
    assert_eq!(f.get(2)["job"]["status"], "unknown");
    assert!(!serde_json::to_string(&f.get(2))
        .unwrap()
        .contains("private raw"));
    assert!(!String::from_utf8(std::fs::read(&f.runtime.path).unwrap())
        .unwrap()
        .contains("private raw"));
}

fn legacy_records(count: usize) -> Vec<u8> {
    serde_json::to_vec(&json!({"records":(0..count).map(|n| json!({
        "summary":{"request_id":id(100+n),"project_id":id(900),"project_key":"fixture",
            "agent_id":"developer","agent_name":"Developer","status":"failed","message":"Historical failure",
            "session_id":null,"created_ms":1},"fingerprint":"a".repeat(64),"prompt":"historical exact prompt",
        "may_have_sent":false,"cleanup_required":false})).collect::<Vec<_>>()})).unwrap()
}
#[test]
fn combined_count_is_not_admission_and_cross_version_ids_preserve_legacy_bytes() {
    let bytes = legacy_records(32);
    let f = Fixture::legacy(Some(bytes.clone()));
    assert_eq!(f.call("dev_jobs_state_v4", json!({})).unwrap()["used"], 32);
    f.start(1);
    assert_eq!(f.runtime.disk().schema_version, 4);
    assert_eq!(f.call("dev_jobs_state_v4", json!({})).unwrap()["used"], 33);
    assert_eq!(
        f.call("dev_job_start_v4", f.args(100)).unwrap_err().code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(std::fs::read(&f.legacy_path).unwrap(), bytes);
    assert!(f.runtime.path.exists());
}

#[test]
fn corrupt_local_ledger_blocks_dev_but_not_chat_reservation() {
    let bytes = b"{corrupt local ledger".to_vec();
    let f = Fixture::ledgers(None, Some(bytes.clone()));
    assert!(f.owner().state.lock().unwrap().fault);
    assert!(!f.owner().reserved());
    assert!(!f.app.dev_reserved());
    assert_eq!(
        f.call("dev_job_start_v4", f.args(1)).unwrap_err().code,
        "DEV_UNAVAILABLE"
    );
    assert!(!f.run().unwrap());
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn poisoned_local_state_keeps_provider_reservation_fail_closed() {
    let f = Fixture::new();
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _state = f.owner().state.lock().unwrap();
        panic!("fixture state poison");
    }));
    assert!(poisoned.is_err());
    assert!(f.owner().reserved());
    assert!(f.app.dev_reserved());
    assert!(f.call("dev_job_start_v4", f.args(1)).is_err());
}

#[test]
fn ambiguous_terminal_publication_retains_chat_reservation() {
    let f = Fixture::new();
    f.start(1);
    f.run().unwrap();
    assert!(!f.owner().reserved());
    std::fs::set_permissions(&f.runtime.path, std::fs::Permissions::from_mode(0o644)).unwrap();
    {
        let mut state = f.owner().state.lock().unwrap();
        let next = state.ledger.clone();
        assert!(f.owner().commit(&mut state, next).is_err());
        assert!(state.fault && state.publication_ambiguous);
        assert_eq!(state.ledger.records[0].summary.status, "completed");
    }
    assert!(f.owner().reserved());
    assert!(f.app.dev_reserved());
}

#[test]
fn completion_errors_preserve_typed_classification_with_fixed_codes_and_messages() {
    for (input, code, kind) in [
        (
            "MODEL_UNAVAILABLE",
            "LOCAL_MODEL_UNAVAILABLE",
            LocalError::ModelUnavailable,
        ),
        (
            "UNSUPPORTED",
            "LOCAL_MODEL_UNAVAILABLE",
            LocalError::ModelUnavailable,
        ),
        ("CONTEXT_LIMIT", "CONTEXT_LIMIT", LocalError::ModelLimit),
        ("MODEL_TRUNCATED", "CONTEXT_LIMIT", LocalError::ModelLimit),
        ("RUNNER_CANCELLED", "INTERRUPTED", LocalError::Interrupted),
        ("PROVIDER_TIMEOUT", "PROVIDER_TIMEOUT", LocalError::Deadline),
        (
            "PRIVATE_RAW_CODE",
            "PROVIDER_PROTOCOL",
            LocalError::ModelProtocol,
        ),
    ] {
        let error = completion_error(AppError::new(input, "private raw provider text"));
        assert_eq!(error.code, code);
        assert_eq!(error.message, kind.message());
        assert_eq!(LocalError::from(error), kind);
    }
}

#[test]
fn corrupt_legacy_fails_admission_closed_without_touching_either_ledger() {
    let bytes = b"{corrupt legacy".to_vec();
    let f = Fixture::legacy(Some(bytes.clone()));
    assert!(f.call("dev_job_start_v4", f.args(1)).is_err());
    assert_eq!(std::fs::read(&f.legacy_path).unwrap(), bytes);
    assert!(!f.runtime.path.exists());
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn strict_schema_corruption_and_external_drift_are_preserved() {
    let f = Fixture::new();
    f.start(1);
    f.run().unwrap();
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    for malformed in [
        {
            let mut v = value.clone();
            v["schema_version"] = json!(5);
            v
        },
        {
            let mut v = value.clone();
            v["records"][0]["reply"] = Value::Null;
            v
        },
        {
            let mut v = value.clone();
            v["records"][0]["job"]["repositories"][0]
                .as_object_mut()
                .unwrap()
                .remove("ref");
            v
        },
        {
            let mut v = value.clone();
            v["records"][0]["summary"]["message"] = json!("untrusted injected error");
            v
        },
    ] {
        assert!(Ledger::load(&serde_json::to_vec(&malformed).unwrap()).is_err());
    }
    assert!(Ledger::load(b"{\"schema_version\":1,\"schema_version\":1,\"records\":[]}").is_err());
    filesystem::publish(&f.runtime.path, b"corrupt preserved", 0o600, true).unwrap();
    assert!(f.call("dev_job_start_v4", f.args(2)).is_err());
    assert!(f.owner().reserved());
    assert_eq!(
        std::fs::read(&f.runtime.path).unwrap(),
        b"corrupt preserved"
    );
    let reopened = Owner::open(f.runtime.path.clone(), f.runtime.clone());
    assert!(!reopened.reserved());
    assert!(reopened.state.lock().unwrap().fault);
}

#[test]
fn queued_and_running_app_jobs_allow_queue_admission() {
    let f = Fixture::new();
    for status in ["queued", "running"] {
        f.app.inner.lock().unwrap().durable.jobs = vec![Job {
            operation: Operation {
                request_id: id(80),
                kind: "models".into(),
                status: status.into(),
                phase: status.into(),
                config_revision: 1,
                result: None,
                error: None,
            },
            fingerprint: "a".repeat(64),
            version: 2,
            conversation_id: None,
            history_revision: None,
            task: None,
            accepted_ms: 1,
        }];
        let n = if status == "queued" { 1 } else { 2 };
        assert_eq!(f.start(n)["status"], "queued");
    }
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn revocation_during_completion_prevents_reply_and_final_publication() {
    let f = Fixture::new();
    f.start(1);
    *f.provider.close_during_completion.lock().unwrap() = f.app.dev_jobs.local.clone();
    assert!(f.run().is_err());
    let disk = Ledger::load(&std::fs::read(&f.runtime.path).unwrap()).unwrap();
    assert_eq!(disk.records[0].summary.status, "running");
    assert!(disk.records[0].reply.is_none());
    assert!(!f.owner().reserved());
    assert!(!f.run().unwrap());
}

#[test]
fn reply_headroom_refuses_admission_before_the_byte_limit_without_eviction() {
    let f = Fixture::new();
    f.start(1);
    f.run().unwrap();
    {
        let mut state = f.owner().state.lock().unwrap();
        let mut ledger = state.ledger.clone();
        ledger.records[0].reply = Some("\u{0001}".repeat(crate::dev_local::MAX_REPLY));
        let original = ledger.records[0].clone();
        for n in 2..=9 {
            let mut record = original.clone();
            record.summary.request_id = id(n);
            record.job.request_id = id(n);
            ledger.records.push(record);
        }
        f.owner().commit(&mut state, ledger).unwrap();
    }
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    assert!((bytes.len() as u64) < LIMIT);
    assert_eq!(
        f.call("dev_job_start_v4", f.args(10)).unwrap_err().code,
        "CAPACITY"
    );
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
}

#[test]
fn local_count_above_32_never_evicts_retained_replies() {
    let f = Fixture::new();
    f.start(1);
    f.run().unwrap();
    {
        let mut state = f.owner().state.lock().unwrap();
        let mut ledger = state.ledger.clone();
        let original = ledger.records[0].clone();
        for n in 2..=32 {
            let mut record = original.clone();
            record.summary.request_id = id(n);
            record.job.request_id = id(n);
            ledger.records.push(record);
        }
        f.owner().commit(&mut state, ledger).unwrap();
    }
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    f.start(33);
    assert_eq!(f.runtime.disk().records.len(), 33);
    assert_eq!(f.runtime.disk().schema_version, 4);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["records"][0],
        serde_json::to_value(&f.runtime.disk().records[0]).unwrap()
    );
    assert!(f.get(1)["reply"].is_string());
}

#[test]
fn repository_validation_is_all_or_nothing_and_readonly_metadata_grants_nothing() {
    let project = json!({"repositories":[{"url":"https://github.com/example/fixture","read_only":false,"branch":{"base":"main","work":"ignored/*"}}]});
    let repositories = saved_repositories(&project).unwrap();
    assert_eq!(repositories[0].reference.as_deref(), Some("main"));
    for url in [
        "git@github.com:example/private",
        "http://github.com/example/repo",
        "https://name:secret@github.com/example/repo",
    ] {
        let mut mixed = project.clone();
        mixed["repositories"]
            .as_array_mut()
            .unwrap()
            .push(json!({"url":url,"branch":{"base":null}}));
        assert!(saved_repositories(&mixed).is_err());
    }
    assert!(saved_repositories(&json!({"repositories":[]})).is_err());
}

// All ledger variants below come from the existing isolated runtime/provider seams.
fn reconciled_fixture(reply: bool) -> Value {
    let f = Fixture::new();
    if reply {
        f.runtime.mode.store(1, Ordering::Release);
    } else {
        f.provider.fail.store(true, Ordering::Release);
    }
    f.start(1);
    f.run().unwrap();
    let mut value = serde_json::to_value(f.runtime.disk()).unwrap();
    let original = value["records"][0].clone();
    assert_eq!(original["failed_stage"], "running");
    assert_eq!(original["may_have_sent"], true);
    value["schema_version"] = json!(2);
    let r = &mut value["records"][0];
    r["summary"]["status"] = json!("reconciled");
    r["summary"]["cleanup_verified"] = json!(true);
    r["cleanup_required"] = json!(false);
    let mut restored = r.clone();
    restored["summary"]["status"] = original["summary"]["status"].clone();
    restored["summary"]["cleanup_verified"] = original["summary"]["cleanup_verified"].clone();
    restored["cleanup_required"] = original["cleanup_required"].clone();
    assert_eq!(restored, original);
    value
}

fn ledger_bytes(value: &Value) -> Vec<u8> {
    // Noncanonical whitespace makes accidental read-time rewrites observable.
    serde_json::to_vec_pretty(value).unwrap()
}

#[test]
fn reconciled_schema_versions_are_explicit_and_default_remains_one() {
    assert_eq!(Ledger::default().schema_version, 1);
    for reply in [false, true] {
        let value = reconciled_fixture(reply);
        assert!(Ledger::load(&ledger_bytes(&value)).is_ok());
        for version in [
            json!(1),
            json!(0),
            json!(5),
            json!(-1),
            json!(1.5),
            json!("2"),
            Value::Null,
            json!(4294967296u64),
        ] {
            let mut invalid = value.clone();
            invalid["schema_version"] = version;
            let bytes = ledger_bytes(&invalid);
            assert!(Ledger::load(&bytes).is_err());
            let f = Fixture::ledgers(None, Some(bytes.clone()));
            assert!(f.owner().state.lock().unwrap().fault);
            assert!(f.call("dev_jobs_state_v4", json!({})).is_err());
            assert!(f.call("dev_job_start_v4", f.args(2)).is_err());
            assert!(!f.run().unwrap());
            assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
            assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
            assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
        }
    }
    for version in [1, 2, 3, 4] {
        let value = json!({"schema_version":version,"records":[]});
        assert!(Ledger::load(&ledger_bytes(&value)).is_ok());
    }
}

#[test]
fn reconciled_rejects_missing_malformed_and_inconsistent_evidence() {
    for reply in [false, true] {
        let value = reconciled_fixture(reply);
        for (pointer, bad) in [
            ("/records/0/summary/cleanup_verified", json!(false)),
            ("/records/0/cleanup_required", json!(true)),
            ("/records/0/may_have_sent", json!(false)),
            ("/records/0/failed_stage", Value::Null),
            ("/records/0/failed_stage", json!("ready")),
            ("/records/0/failed_stage", json!("completed")),
            ("/records/0/image_id", Value::Null),
            ("/records/0/image_id", json!("")),
            ("/records/0/image_id", json!("sha256:bad")),
            ("/records/0/summary/session_id", Value::Null),
            ("/records/0/summary/session_id", json!("")),
            ("/records/0/summary/session_id", json!("ses invalid")),
            ("/records/0/repositories", json!([])),
            ("/records/0/repositories/0/commit", json!("bad")),
            ("/records/0/repositories/0/ref", json!("refs/heads/other")),
            (
                "/records/0/repositories/0/url",
                json!("https://github.com/example/other"),
            ),
            ("/records/0/job/repositories", json!([])),
            ("/records/0/job/request_id", json!(id(2))),
            ("/records/0/summary/project_id", json!("invalid")),
            ("/records/0/fingerprint", json!("bad")),
            (
                "/records/0/summary/message",
                json!("invented reconciliation message"),
            ),
            ("/records/0/summary/status", json!("Reconciled")),
            ("/records/0/summary/reply_available", json!(!reply)),
            (
                "/records/0/reply",
                if reply {
                    Value::Null
                } else {
                    json!("orphan reply")
                },
            ),
            ("/records/0/reply", json!("")),
            ("/records/0/reply", json!(" ")),
            (
                "/records/0/reply",
                json!("x".repeat(crate::dev_local::MAX_REPLY + 1)),
            ),
            ("/records/0/message_id", json!("invalid message")),
            (
                "/records/0/message_id",
                if reply {
                    Value::Null
                } else {
                    json!("msg_orphan")
                },
            ),
        ] {
            let mut invalid = value.clone();
            *invalid.pointer_mut(pointer).unwrap() = bad;
            assert!(
                Ledger::load(&ledger_bytes(&invalid)).is_err(),
                "{pointer}, reply={reply}"
            );
        }
        for pointer in [
            "",
            "/records/0",
            "/records/0/summary",
            "/records/0/job",
            "/records/0/job/model",
            "/records/0/job/repositories/0",
            "/records/0/repositories/0",
        ] {
            let object = value.pointer(pointer).unwrap().as_object().unwrap();
            for key in object.keys() {
                let mut invalid = value.clone();
                invalid
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    Ledger::load(&ledger_bytes(&invalid)).is_err(),
                    "missing {pointer}/{key}"
                );
            }
            let mut invalid = value.clone();
            invalid
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("extra".into(), json!(true));
            assert!(
                Ledger::load(&ledger_bytes(&invalid)).is_err(),
                "extra {pointer}"
            );
            *invalid.pointer_mut(pointer).unwrap() = json!([]);
            assert!(
                Ledger::load(&ledger_bytes(&invalid)).is_err(),
                "array {pointer}"
            );
        }
        let mut duplicate = value.clone();
        duplicate["records"]
            .as_array_mut()
            .unwrap()
            .push(value["records"][0].clone());
        assert!(Ledger::load(&ledger_bytes(&duplicate)).is_err());
        let duplicate_key = String::from_utf8(ledger_bytes(&value)).unwrap().replacen(
            "\"schema_version\": 2",
            "\"schema_version\": 2, \"schema_version\": 2",
            1,
        );
        assert!(Ledger::load(duplicate_key.as_bytes()).is_err());
    }
}

#[test]
fn reconciled_reopen_queries_and_original_identity_never_rewrite_or_dispatch() {
    for reply in [false, true] {
        let value = reconciled_fixture(reply);
        let bytes = ledger_bytes(&value);
        let f = Fixture::ledgers(None, Some(bytes.clone()));
        let r = &value["records"][0];
        assert!(!f.owner().reserved());
        assert!(!f.app.dev_reserved());
        let state = f.call("dev_jobs_state_v4", json!({})).unwrap();
        assert_eq!(state["jobs"], json!([r["summary"]]));
        assert_eq!(state["used"], 1);
        assert_eq!(
            f.get(1),
            json!({"job":r["summary"],"model":r["job"]["model"],
            "repositories":r["repositories"],"reply":r["reply"],"failed_stage":"running"})
        );
        assert_eq!(f.start(1), r["summary"]);
        let mut changed = f.args(1);
        changed["prompt"] = json!("different");
        assert_eq!(
            f.call("dev_job_start_v4", changed).unwrap_err().code,
            "REQUEST_CONFLICT"
        );
        assert!(f
            .call("dev_job_reconcile_v2", json!({"request_id":id(1)}))
            .is_err());
        assert!(!f.run().unwrap());
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        let reopened = Owner::open(f.runtime.path.clone(), f.runtime.clone());
        assert!(!reopened.reserved());
        let state = reopened.state.lock().unwrap();
        assert!(!state.fault && state.pending.is_none());
        assert_eq!(serde_json::to_value(&state.ledger).unwrap(), value);
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
        assert!(!f.legacy_path.exists());
    }
}

#[test]
fn successful_start_promotes_loaded_version_and_preserves_old_outcomes() {
    for version in [1, 2] {
        for (mode, provider_fail, expected) in [
            (0, false, "completed"),
            (1, false, "cleanup_unknown"),
            (2, false, "unknown"),
            (4, false, "failed"),
            (0, true, "unknown"),
        ] {
            let f = Fixture::ledgers(
                None,
                Some(ledger_bytes(
                    &json!({"schema_version":version,"records":[]}),
                )),
            );
            f.runtime.mode.store(mode, Ordering::Release);
            f.provider.fail.store(provider_fail, Ordering::Release);
            f.start(1);
            assert_eq!(f.runtime.disk().schema_version, 4);
            f.run().unwrap();
            let ledger = f.runtime.disk();
            assert_eq!(ledger.schema_version, 4);
            assert_eq!(ledger.records[0].summary.status, expected);
            let bytes = std::fs::read(&f.runtime.path).unwrap();
            let reopened = Fixture::ledgers(None, Some(bytes.clone()));
            assert_eq!(reopened.get(1), f.get(1));
            assert_eq!(reopened.owner().reserved(), f.owner().reserved());
            assert!(!reopened.run().unwrap());
            assert_eq!(std::fs::read(&reopened.runtime.path).unwrap(), bytes);
            assert_eq!(reopened.runtime.calls.load(Ordering::SeqCst), 0);
            assert_eq!(reopened.provider.calls.load(Ordering::SeqCst), 0);
            assert!(!f.owner().reserved());
            // Every ordinary result remains valid under both schemas with exact evidence.
            let mut value = serde_json::to_value(&ledger).unwrap();
            for read_version in [1, 2] {
                value["schema_version"] = json!(read_version);
                assert!(Ledger::load(&ledger_bytes(&value)).is_ok());
            }
        }
    }
    let value = reconciled_fixture(true);
    let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
    f.start(2);
    assert_eq!(f.runtime.disk().schema_version, 4);
    f.run().unwrap();
    let ledger = f.runtime.disk();
    assert_eq!(ledger.schema_version, 4);
    assert_eq!(
        serde_json::to_value(&ledger.records[0]).unwrap(),
        value["records"][0]
    );
    assert_eq!(ledger.records[1].summary.status, "completed");
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn old_status_invariants_remain_identical_in_both_schemas() {
    let value = reconciled_fixture(true);
    for version in [1, 2] {
        for status in [
            "preparing",
            "cloning",
            "ready",
            "running",
            "completed",
            "failed",
            "unknown",
            "cleanup_unknown",
        ] {
            let mut old = value.clone();
            old["schema_version"] = json!(version);
            let r = &mut old["records"][0];
            r["summary"]["status"] = json!(status);
            let needs_cleanup = active(status) || status == "cleanup_unknown";
            r["cleanup_required"] = json!(needs_cleanup);
            r["summary"]["cleanup_verified"] = json!(!needs_cleanup);
            if status == "completed" {
                r["failed_stage"] = Value::Null;
            }
            if status == "failed" {
                r["may_have_sent"] = json!(false);
                r["reply"] = Value::Null;
                r["message_id"] = Value::Null;
                r["summary"]["reply_available"] = json!(false);
            }
            assert!(
                Ledger::load(&ledger_bytes(&old)).is_ok(),
                "{version}/{status}"
            );
            let r = &mut old["records"][0];
            if status == "failed" {
                r["may_have_sent"] = json!(true);
            } else if status == "completed" {
                r["failed_stage"] = json!("running");
            } else if status == "unknown" {
                // Outcome ambiguity is retained, but verified cleanup consumes no slot.
                let f = Fixture::ledgers(None, Some(ledger_bytes(&old)));
                assert!(!f.owner().reserved());
                assert_eq!(f.start(2)["status"], "queued");
                continue;
            } else {
                r["cleanup_required"] = json!(false);
                r["summary"]["cleanup_verified"] = json!(true);
            }
            assert!(
                Ledger::load(&ledger_bytes(&old)).is_err(),
                "{version}/{status}"
            );
        }
    }
}

#[test]
fn reconciled_retains_busy_byte_capacity_and_application_fault_gates() {
    let value = reconciled_fixture(true);
    // Reconciled history does not consume a queue or container slot.
    let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
    f.start(2);
    assert_eq!(f.start(3)["status"], "queued");
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);

    for byte_limit in [false, true] {
        let mut full = value.clone();
        if byte_limit {
            full["records"][0]["reply"] = json!("\u{0001}".repeat(crate::dev_local::MAX_REPLY));
        }
        let record = full["records"][0].clone();
        for n in 2..=if byte_limit { 9 } else { 32 } {
            let mut next = record.clone();
            next["summary"]["request_id"] = json!(id(n));
            next["job"]["request_id"] = json!(id(n));
            full["records"].as_array_mut().unwrap().push(next);
        }
        let bytes = ledger_bytes(&full);
        assert!((bytes.len() as u64) < LIMIT);
        let f = Fixture::ledgers(None, Some(bytes.clone()));
        assert!(!f.owner().reserved());
        if byte_limit {
            assert_eq!(
                f.call("dev_job_start_v4", f.args(33)).unwrap_err().code,
                "CAPACITY"
            );
            assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        } else {
            f.start(33);
            assert_eq!(f.runtime.disk().records.len(), 33);
            assert_eq!(f.runtime.disk().schema_version, 4);
        }
        assert_eq!(f.get(1)["reply"], record["reply"]);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    }
    for fault in [false, true] {
        let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
        let bytes = std::fs::read(&f.runtime.path).unwrap();
        {
            let mut inner = f.app.inner.lock().unwrap();
            inner.fault = fault;
            inner.owned = fault;
        }
        assert!(f.call("dev_job_start_v4", f.args(2)).is_err());
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(f.run().unwrap_err().code, "STATE_UNAVAILABLE");
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn reconciled_does_not_override_other_admission_or_publication_fences() {
    let value = reconciled_fixture(true);
    for status in [
        "unknown",
        "cleanup_unknown",
        "preparing",
        "cloning",
        "ready",
        "running",
    ] {
        let mut combined = value.clone();
        let mut other = value["records"][0].clone();
        other["summary"]["request_id"] = json!(id(2));
        other["job"]["request_id"] = json!(id(2));
        other["summary"]["status"] = json!(status);
        other["cleanup_required"] = json!(status != "unknown");
        other["summary"]["cleanup_verified"] = json!(status == "unknown");
        combined["records"].as_array_mut().unwrap().push(other);
        let bytes = ledger_bytes(&combined);
        let f = Fixture::ledgers(None, Some(bytes.clone()));
        assert!(!f.owner().reserved());
        assert_eq!(f.get(1)["job"]["status"], "reconciled");
        assert_eq!(
            f.get(2)["job"]["status"],
            if active(status) {
                "cleanup_unknown"
            } else {
                status
            }
        );
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(f.start(3)["status"], "queued");
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    }
    {
        let mut legacy: Value = serde_json::from_slice(&legacy_records(1)).unwrap();
        legacy["records"][0]["summary"]["status"] = json!("unknown");
        legacy["records"][0]["may_have_sent"] = json!(true);
        let legacy = ledger_bytes(&legacy);
        let bytes = ledger_bytes(&value);
        let f = Fixture::ledgers(Some(legacy.clone()), Some(bytes.clone()));
        assert_eq!(
            f.call("dev_job_start_v4", f.args(2)).unwrap_err().code,
            "CLEANUP_UNKNOWN"
        );
        assert_eq!(
            f.call("dev_job_start_v4", f.args(100)).unwrap_err().code,
            "REQUEST_EXPIRED"
        );
        assert_eq!(std::fs::read(&f.legacy_path).unwrap(), legacy);
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    }
    let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
    std::fs::set_permissions(&f.runtime.path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        f.call("dev_job_start_v4", f.args(2)).unwrap_err().code,
        "DEV_UNAVAILABLE"
    );
    assert!(f.owner().reserved());
    assert!(f.owner().state.lock().unwrap().publication_ambiguous);
    assert!(f
        .call("dev_job_get_v4", json!({"request_id":id(1)}))
        .is_err());
    assert!(!f.run().unwrap());
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
}

fn deletion(f: &Fixture, n: usize) -> AppResult<Value> {
    f.call(
        "dev_job_delete_v3",
        json!({"request_id":id(n),"instance_id":f.app.instance_id}),
    )
}
fn deletion_query(f: &Fixture, n: usize) -> AppResult<Value> {
    f.call("dev_job_deletion_v3", json!({"request_id":id(n)}))
}

#[test]
fn deletion_schema_is_canonical_versioned_and_count_unbounded_only_in_three() {
    let mut value = reconciled_fixture(true);
    for version in [1, 2, 3, 4] {
        value["schema_version"] = json!(version);
        let mut ordinary = value.clone();
        ordinary["records"][0]["summary"]["status"] = json!("completed");
        ordinary["records"][0]["failed_stage"] = Value::Null;
        assert!(Ledger::load(&ledger_bytes(&ordinary)).is_ok());
        ordinary["records"][0]["deleted"] = json!(false);
        assert!(Ledger::load(&ledger_bytes(&ordinary)).is_err());
        ordinary["records"][0]["deleted"] = json!(true);
        assert_eq!(Ledger::load(&ledger_bytes(&ordinary)).is_ok(), version >= 3);
        assert_eq!(Ledger::load(&ledger_bytes(&value)).is_ok(), version != 1);
    }
    value["schema_version"] = json!(3);
    value["records"][0]["deleted"] = json!(true);
    assert!(Ledger::load(&ledger_bytes(&value)).is_ok());
    for flag in [Value::Null, json!(0), json!("true")] {
        let mut bad = value.clone();
        bad["records"][0]["deleted"] = flag;
        assert!(Ledger::load(&ledger_bytes(&bad)).is_err());
    }
    let record = value["records"][0].clone();
    for n in 2..=40 {
        let mut r = record.clone();
        r["summary"]["request_id"] = json!(id(n));
        r["job"]["request_id"] = json!(id(n));
        value["records"].as_array_mut().unwrap().push(r);
    }
    assert!(Ledger::load(&ledger_bytes(&value)).is_ok());
    for version in [1, 2] {
        let mut old = value.clone();
        old["schema_version"] = json!(version);
        for r in old["records"].as_array_mut().unwrap() {
            r.as_object_mut().unwrap().remove("deleted");
            r["summary"]["status"] = json!("completed");
            r["failed_stage"] = Value::Null;
        }
        assert!(Ledger::load(&ledger_bytes(&old)).is_err());
    }
}

#[test]
fn deletion_preserves_exact_evidence_and_idempotent_reopen_query_and_replay() {
    for status in ["completed", "failed", "reconciled"] {
        for version in [1, 2, 3, 4] {
            if status == "reconciled" && version == 1 {
                continue;
            }
            let mut value = reconciled_fixture(true);
            value["schema_version"] = json!(version);
            let r = &mut value["records"][0];
            r["summary"]["status"] = json!(status);
            if status == "completed" {
                r["failed_stage"] = Value::Null;
            }
            if status == "failed" {
                r["may_have_sent"] = json!(false);
                r["reply"] = Value::Null;
                r["message_id"] = Value::Null;
                r["summary"]["reply_available"] = json!(false);
            }
            let bytes = ledger_bytes(&value);
            let f = Fixture::ledgers(None, Some(bytes.clone()));
            let summary = value["records"][0]["summary"].clone();
            assert_eq!(
                deletion_query(&f, 1).unwrap(),
                json!({"job":summary,"deleted":false})
            );
            assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
            let proof = deletion(&f, 1).unwrap();
            assert_eq!(proof, json!({"job":summary,"deleted":true}));
            value["schema_version"] = json!(version.max(3));
            value["records"][0]["deleted"] = json!(true);
            assert_eq!(serde_json::to_value(f.runtime.disk()).unwrap(), value);
            let saved = std::fs::read(&f.runtime.path).unwrap();
            let inode = std::fs::metadata(&f.runtime.path).unwrap().ino();
            assert_eq!(deletion(&f, 1).unwrap(), proof);
            assert_eq!(deletion_query(&f, 1).unwrap(), proof);
            assert_eq!(std::fs::metadata(&f.runtime.path).unwrap().ino(), inode);
            let state = f.call("dev_jobs_state_v4", json!({})).unwrap();
            assert_eq!(state["jobs"], json!([]));
            assert_eq!(state["used"], 1);
            assert_eq!(
                f.call("dev_job_get_v4", json!({"request_id":id(1)}))
                    .unwrap_err()
                    .code,
                "JOB_DELETED"
            );
            for changed in [false, true] {
                let mut args = f.args(1);
                if changed {
                    args["prompt"] = json!("");
                    args["instance_id"] = json!(id(123));
                    args["expected_config_revision"] = json!("bad");
                }
                assert_eq!(
                    f.call("dev_job_start_v4", args).unwrap_err().code,
                    "JOB_DELETED"
                );
            }
            assert_eq!(deletion_query(&f, 2).unwrap_err().code, "UNKNOWN_REQUEST");
            assert_eq!(deletion(&f, 2).unwrap_err().code, "UNKNOWN_REQUEST");
            assert!(!f.run().unwrap());
            let reopened = Fixture::ledgers(None, Some(saved.clone()));
            assert_eq!(deletion_query(&reopened, 1).unwrap(), proof);
            assert_eq!(deletion(&reopened, 1).unwrap(), proof);
            assert_eq!(std::fs::read(&reopened.runtime.path).unwrap(), saved);
            assert_eq!(std::fs::read(&f.runtime.path).unwrap(), saved);
            assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
            assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
        }
    }
}

#[test]
fn deletion_refuses_unsafe_targets_but_preserves_other_unknown_fences() {
    let source = reconciled_fixture(true);
    for status in [
        "preparing",
        "cloning",
        "ready",
        "running",
        "unknown",
        "cleanup_unknown",
    ] {
        let mut value = source.clone();
        value["schema_version"] = json!(3);
        let mut other = value["records"][0].clone();
        other["summary"]["request_id"] = json!(id(2));
        other["job"]["request_id"] = json!(id(2));
        other["summary"]["status"] = json!(status);
        other["cleanup_required"] = json!(status != "unknown");
        other["summary"]["cleanup_verified"] = json!(status == "unknown");
        let mut hidden = other.clone();
        hidden["deleted"] = json!(true);
        let mut bad = value.clone();
        bad["records"] = json!([hidden]);
        assert!(Ledger::load(&ledger_bytes(&bad)).is_err());
        value["records"].as_array_mut().unwrap().push(other);
        let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
        let bytes = std::fs::read(&f.runtime.path).unwrap();
        assert_eq!(deletion(&f, 2).unwrap_err().code, "JOB_NOT_DELETABLE");
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(deletion_query(&f, 2).unwrap()["deleted"], false);
        deletion(&f, 1).unwrap();
        assert!(!f.owner().reserved());
        assert_eq!(
            f.call("dev_jobs_state_v4", json!({})).unwrap()["jobs"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(f.start(3)["status"], "queued");
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    }
    // A genuinely pending (not restart-interrupted) job is also never a delete target.
    let f = Fixture::new();
    f.start(1);
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    assert_eq!(deletion(&f, 1).unwrap_err().code, "JOB_NOT_DELETABLE");
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    assert!(f.owner().state.lock().unwrap().pending.is_none());
    assert_eq!(f.get(1)["job"]["status"], "queued");
}

#[test]
fn deletion_validates_instance_arguments_ownership_and_publication_faults() {
    let value = reconciled_fixture(true);
    for deleted_flag in [false, true] {
        let mut value = value.clone();
        if deleted_flag {
            value["schema_version"] = json!(3);
            value["records"][0]["deleted"] = json!(true);
        }
        let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
        let bytes = std::fs::read(&f.runtime.path).unwrap();
        for (args, code) in [
            (
                json!({"request_id":id(1),"instance_id":id(99)}),
                "STALE_INSTANCE",
            ),
            (
                json!({"request_id":id(1),"instance_id":"bad"}),
                "INVALID_REQUEST",
            ),
            (
                json!({"request_id":id(1),"instance_id":f.app.instance_id,"extra":true}),
                "INVALID_REQUEST",
            ),
            (json!({"request_id":id(1)}), "INVALID_REQUEST"),
        ] {
            assert_eq!(f.call("dev_job_delete_v3", args).unwrap_err().code, code);
        }
        for args in [
            json!({"request_id":id(1),"instance_id":f.app.instance_id}),
            json!({"request_id":"bad"}),
            json!([id(1)]),
        ] {
            assert!(f.call("dev_job_deletion_v3", args).is_err());
        }
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        for fault in [false, true] {
            {
                let mut inner = f.app.inner.lock().unwrap();
                inner.fault = fault;
                inner.owned = fault;
            }
            assert_eq!(deletion(&f, 1).unwrap_err().code, "STATE_UNAVAILABLE");
            assert_eq!(deletion_query(&f, 1).unwrap_err().code, "STATE_UNAVAILABLE");
        }
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    }
    for drift in [false, true] {
        let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
        if drift {
            filesystem::publish(&f.runtime.path, b"external drift preserved", 0o600, true).unwrap();
        } else {
            std::fs::set_permissions(&f.runtime.path, std::fs::Permissions::from_mode(0o644))
                .unwrap();
        }
        let bytes = std::fs::read(&f.runtime.path).unwrap();
        assert_eq!(deletion(&f, 1).unwrap_err().code, "DEV_UNAVAILABLE");
        let state = f.owner().state.lock().unwrap();
        assert!(state.fault && state.publication_ambiguous && state.ledger.records[0].deleted);
        drop(state);
        assert!(f.app.dev_reserved());
        assert!(deletion_query(&f, 1).is_err());
        assert!(deletion(&f, 1).is_err());
        assert!(f.call("dev_jobs_state_v4", json!({})).is_err());
        assert!(f.call("dev_job_start_v4", f.args(2)).is_err());
        assert!(!f.run().unwrap());
        assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
        assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn deletion_and_query_route_through_validated_app3_envelopes() {
    let f = Fixture::ledgers(None, Some(ledger_bytes(&reconciled_fixture(true))));
    for op in ["dev_job_delete_v3", "dev_job_deletion_v3"] {
        let args = if op == "dev_job_delete_v3" {
            json!({"request_id":id(1),"instance_id":f.app.instance_id})
        } else {
            json!({"request_id":id(1)})
        };
        let mut req = AppRequest {
            schema_version: 3,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: id(999),
            op: op.into(),
            args,
        };
        req.service = "foreign".into();
        assert!(f.app.call(req.clone()).is_err());
        req.service = "smith-next".into();
        let result = f.app.call(req.clone()).unwrap();
        assert_eq!(result["job"]["request_id"], id(1));
        assert_eq!(result["deleted"], true);
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(reply_fits(&result));
        let reply =
            AppReply::from_versioned_result(3, &f.app.instance_id, &req.call_id, Ok(result));
        assert!(serde_json::to_vec(&reply).unwrap().len() <= FRAME_LIMIT);
    }
}

#[test]
fn deletion_byte_exhaustion_is_atomic_and_keeps_result_reservation() {
    let mut value = reconciled_fixture(true);
    value["schema_version"] = json!(3);
    let mut record = value["records"][0].clone();
    record["job"]["prompt"] = json!("x");
    value["records"] = json!((1..=140)
        .map(|n| {
            let mut r = record.clone();
            r["summary"]["request_id"] = json!(id(n));
            r["job"]["request_id"] = json!(id(n));
            r
        })
        .collect::<Vec<_>>());
    let mut remaining = LIMIT as usize - serde_json::to_vec(&value).unwrap().len();
    for r in value["records"].as_array_mut().unwrap() {
        let add = remaining.min(crate::dev_local::MAX_PROMPT - 1);
        r["job"]["prompt"] = json!("x".repeat(1 + add));
        remaining -= add;
    }
    assert_eq!(remaining, 0);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(bytes.len() as u64, LIMIT);
    assert!(Ledger::load(&bytes).is_ok());
    let f = Fixture::ledgers(None, Some(bytes.clone()));
    let inode = std::fs::metadata(&f.runtime.path).unwrap().ino();
    assert_eq!(deletion(&f, 1).unwrap_err().code, "CAPACITY");
    assert_eq!(
        f.call("dev_job_start_v4", f.args(141)).unwrap_err().code,
        "CAPACITY"
    );
    assert_eq!(deletion_query(&f, 1).unwrap()["deleted"], false);
    assert_eq!(std::fs::metadata(&f.runtime.path).unwrap().ino(), inode);
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    assert!(!f.owner().state.lock().unwrap().fault);
    assert!(!f.owner().reserved());
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    let mut oversized = bytes;
    oversized.push(b' ');
    assert!(Ledger::load(&oversized).is_err());
}

#[test]
fn retained_mixed_history_above_32_and_worst_shape_results_fit_actual_envelope() {
    let mut value = reconciled_fixture(true);
    value["schema_version"] = json!(3);
    let mut record = value["records"][0].clone();
    record["summary"]["session_id"] = json!("x".repeat(256));
    record["summary"]["project_key"] = json!("x".repeat(40));
    record["summary"]["created_ms"] = json!(u64::MAX);
    record["reply"] = json!("\u{0001}".repeat(crate::dev_local::MAX_REPLY));
    record["job"]["repositories"][0]["ref"] = json!("x".repeat(255));
    record["repositories"][0]["ref"] = json!("refs/heads/".to_owned() + &"x".repeat(255));
    let url = "https://github.com/".to_owned() + &"x".repeat(2048 - "https://github.com/".len());
    record["job"]["repositories"][0]["url"] = json!(url);
    record["repositories"][0]["url"] = json!(url);
    record["job"]["repositories"] = json!(vec![record["job"]["repositories"][0].clone(); 16]);
    record["repositories"] = json!(vec![record["repositories"][0].clone(); 16]);
    // Keep one maximum reply/detail record; many small retained rows exercise projection.
    value["records"] = json!([record.clone()]);
    record["job"]["repositories"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    record["repositories"].as_array_mut().unwrap().truncate(1);
    record["reply"] = json!("retained reply");
    for n in 2..=70 {
        let mut r = record.clone();
        r["summary"]["request_id"] = json!(id(n));
        r["job"]["request_id"] = json!(id(n));
        if n % 2 == 0 {
            r["deleted"] = json!(true);
        }
        value["records"].as_array_mut().unwrap().push(r);
    }
    let legacy = legacy_records(32);
    let bytes = ledger_bytes(&value);
    assert!(Ledger::load(&bytes).is_ok());
    let f = Fixture::ledgers(Some(legacy.clone()), Some(bytes.clone()));
    for result in [
        f.get(1),
        deletion_query(&f, 1).unwrap(),
        f.call("dev_jobs_state_v4", json!({})).unwrap(),
    ] {
        assert!(reply_fits(&result));
        let reply =
            AppReply::from_versioned_result(3, &"f".repeat(32), &"f".repeat(32), Ok(result));
        assert!(serde_json::to_vec(&reply).unwrap().len() <= FRAME_LIMIT);
    }
    let state = f.call("dev_jobs_state_v4", json!({})).unwrap();
    assert_eq!(state["used"], 102);
    assert_eq!(state["jobs"].as_array().unwrap().len(), 35);
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), bytes);
    f.start(71);
    assert_eq!(f.runtime.disk().schema_version, 4);
    assert_eq!(f.runtime.disk().records.len(), 71);
    assert_eq!(f.call("dev_jobs_state_v4", json!({})).unwrap()["used"], 103);
    assert_eq!(std::fs::read(&f.legacy_path).unwrap(), legacy);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        bounded(json!("x".repeat(FRAME_LIMIT))).unwrap_err().code,
        "CAPACITY"
    );
}

#[test]
fn deletion_preserves_legacy_unknown_and_live_other_pending_ownership() {
    let value = reconciled_fixture(true);
    let mut legacy: Value = serde_json::from_slice(&legacy_records(1)).unwrap();
    legacy["records"][0]["summary"]["status"] = json!("unknown");
    legacy["records"][0]["may_have_sent"] = json!(true);
    let bytes = ledger_bytes(&legacy);
    let f = Fixture::ledgers(Some(bytes.clone()), Some(ledger_bytes(&value)));
    deletion(&f, 1).unwrap();
    assert_eq!(
        f.call("dev_job_start_v4", f.args(2)).unwrap_err().code,
        "CLEANUP_UNKNOWN"
    );
    assert_eq!(std::fs::read(&f.legacy_path).unwrap(), bytes);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);

    let f = Fixture::ledgers(None, Some(ledger_bytes(&value)));
    f.start(2);
    let original_pending = serde_json::to_value(&f.runtime.disk().records[1]).unwrap();
    deletion(&f, 1).unwrap();
    assert_eq!(
        serde_json::to_value(&f.runtime.disk().records[1]).unwrap(),
        original_pending
    );
    assert!(f.owner().state.lock().unwrap().pending.is_none());
    assert!(!f.owner().cancelled.load(Ordering::Acquire));
    assert!(!f.app.dev_reserved());
    assert!(f.app.dev_has_model_dependencies());
    assert_eq!(f.start(3)["status"], "queued");
    let saved = std::fs::read(&f.runtime.path).unwrap();
    f.owner().close();
    assert!(deletion(&f, 1).is_err());
    assert!(deletion_query(&f, 1).is_err());
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), saved);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
}

fn claim_fixture(f: &Fixture, n: usize) {
    f.owner()
        .update(&id(n), |r| {
            r.summary.status = "preparing".into();
            r.summary.message = PREPARING.into();
            r.summary.cleanup_verified = false;
            r.cleanup_required = true;
            Ok(())
        })
        .unwrap();
}

#[path = "app_dev_scheduler_tests.rs"]
mod scheduler_tests;
