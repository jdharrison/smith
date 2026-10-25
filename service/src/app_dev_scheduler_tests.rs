use super::*;
use std::sync::Condvar;

#[test]
fn retained_environments_consume_the_existing_inspection_slots() {
    let f = Fixture::new();
    let retained = f.app.dev_jobs.retained.as_ref().unwrap();
    for _ in 0..3 {
        retained
            .start(retained_jobs::Start {
                operation_id: new_id().unwrap(),
                session_id: new_id().unwrap(),
                expected_revision: retained.revision().unwrap(),
                project: retained_jobs::FrozenProject {
                    project_id: new_id().unwrap(),
                    project_key: "fixture".into(),
                    context: String::new(),
                    repositories: vec![],
                },
                job_type: retained_jobs::JobType::Read,
                prompt: "Inspect".into(),
                created_ms: 1,
            })
            .unwrap();
        retained.prepare_next(0).unwrap().unwrap();
    }
    f.start(1);
    let (wake, _) = mpsc::sync_channel(1);
    assert!(!f.app.dispatch_dev(wake).unwrap());
    assert_eq!(f.get(1)["job"]["status"], "queued");
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.call("dev_jobs_state_v4", json!({})).unwrap()["scheduler"]["occupied"],
        3
    );
}

struct RetainedStepRuntime {
    calls: AtomicUsize,
    fault_during_execute: AtomicBool,
    local: Arc<Owner>,
}
impl retained_jobs::Runtime for RetainedStepRuntime {
    fn execute(&self, c: &retained_jobs::Command) -> retained_jobs::Acknowledgement {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fault_during_execute.load(Ordering::SeqCst) {
            let mut state = self.local.state.lock().unwrap();
            state.fault = true;
            state.publication_ambiguous = true;
        }
        retained_jobs::Acknowledgement {
            session_id: c.session_id.clone(),
            operation_id: c.operation_id.clone(),
            action: c.action,
            outcome: match c.action {
                retained_jobs::Action::Create => retained_jobs::Outcome::Ready,
                retained_jobs::Action::Prompt => retained_jobs::Outcome::Reply("ok".into()),
                retained_jobs::Action::Finish => retained_jobs::Outcome::Finished,
            },
        }
    }
}
fn retained_fixture() -> (Fixture, Arc<RetainedStepRuntime>) {
    let mut f = Fixture::new();
    let local = Arc::new(Owner::open(f.runtime.path.clone(), f.runtime.clone()));
    let runtime = Arc::new(RetainedStepRuntime {
        calls: AtomicUsize::new(0),
        fault_during_execute: AtomicBool::new(false),
        local: local.clone(),
    });
    let mut dev = dev_jobs::Owner::open(f.legacy_path.clone(), Arc::new(NeverLegacy));
    dev.local = Some(local);
    dev.retained_runtime = Some(runtime.clone());
    let dev = Arc::new(dev);
    Arc::get_mut(f.app.owner.as_mut().unwrap())
        .unwrap()
        .dev_jobs = dev.clone();
    f.app.dev_jobs = dev;
    (f, runtime)
}
fn retained_start(f: &Fixture) -> Value {
    let store = f.app.dev_jobs.retained.as_ref().unwrap();
    f.call("dev_session_start_v1",json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,
        "expected_revision":store.revision().unwrap(),"expected_projects_revision":1,"project_id":id(900),"job_type":"read","prompt":"Task"})).unwrap()
}
#[test]
fn inspection_fault_between_create_and_initial_prompt_fences_pending_dispatch() {
    use std::os::unix::fs::PermissionsExt;
    for corruption in ["publication", "content", "mode", "legacy"] {
        let (f, runtime) = retained_fixture();
        let started = retained_start(&f);
        assert!(f.app.run_retained_command().unwrap());
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
        match corruption {
            "publication" => {
                let mut state = f.owner().state.lock().unwrap();
                state.fault = true;
                state.publication_ambiguous = true;
            }
            "content" => {
                filesystem::publish(&f.runtime.path, b"{}", 0o600, false).unwrap();
            }
            "mode" => {
                filesystem::publish(&f.runtime.path, b"{}", 0o644, false).unwrap();
                std::fs::set_permissions(&f.runtime.path, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
            }
            _ => {
                filesystem::publish(&f.legacy_path, b"{}", 0o600, false).unwrap();
            }
        }
        assert!(f.app.run_retained_command().is_err());
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
        let session = f
            .app
            .dev_jobs
            .retained
            .as_ref()
            .unwrap()
            .get(started["job"]["session_id"].as_str().unwrap())
            .unwrap();
        assert_eq!(session.job.status, retained_jobs::Status::Prompting);
        assert!(session.job.retained);
    }
}
#[test]
fn inspection_fault_during_execute_fences_acknowledgement() {
    let (f, runtime) = retained_fixture();
    let started = retained_start(&f);
    runtime.fault_during_execute.store(true, Ordering::SeqCst);
    assert!(f.app.run_retained_command().is_err());
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.app
            .dev_jobs
            .retained
            .as_ref()
            .unwrap()
            .get(started["job"]["session_id"].as_str().unwrap())
            .unwrap()
            .job
            .status,
        retained_jobs::Status::Creating
    );
    assert!(f.app.run_retained_command().is_err());
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn healthy_full_occupancy_still_allows_retained_followup_and_cleanup() {
    let (f, runtime) = retained_fixture();
    let mut starts = Vec::new();
    for _ in 0..3 {
        let s = retained_start(&f);
        f.app.run_retained_command().unwrap();
        f.app.run_retained_command().unwrap();
        starts.push(s);
    }
    let session = starts[0]["job"]["session_id"].as_str().unwrap();
    assert_eq!(f.app.retained_reserved(), 3);
    let store = f.app.dev_jobs.retained.as_ref().unwrap();
    f.call("dev_session_prompt_v1",json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"expected_revision":store.revision().unwrap(),"session_id":session,"prompt":"Next"})).unwrap();
    assert!(f.app.run_retained_command().unwrap());
    f.call("dev_session_finish_v1",json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"expected_revision":store.revision().unwrap(),"session_id":session})).unwrap();
    assert!(f.app.run_retained_command().unwrap());
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 8);
    assert_eq!(store.reserved(), 2);
}

struct BarrierRuntime {
    entered: mpsc::Sender<String>,
    released: Mutex<HashSet<String>>,
    gate: Condvar,
    active: AtomicUsize,
    peak: AtomicUsize,
    delegate: Arc<FakeRuntime>,
}
impl LocalRuntime for BarrierRuntime {
    fn run(
        &self,
        job: &LocalJob,
        cancel: &AtomicBool,
        event: &mut dyn FnMut(LocalEvent) -> AppResult<()>,
        completion: &mut dyn FnMut(&Value, Instant) -> AppResult<Value>,
    ) -> std::result::Result<LocalReceipt, LocalFailure> {
        let n = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(n, Ordering::SeqCst);
        self.entered.send(job.request_id.clone()).unwrap();
        let mut released = self.released.lock().unwrap();
        while !released.contains(&job.request_id) && !cancel.load(Ordering::Acquire) {
            released = self
                .gate
                .wait_timeout(released, Duration::from_millis(20))
                .unwrap()
                .0;
        }
        drop(released);
        let result = if cancel.load(Ordering::Acquire) {
            Err(LocalFailure {
                error: LocalError::Interrupted,
                may_have_sent: false,
                cleanup_unknown: false,
            })
        } else {
            self.delegate.run(job, cancel, event, completion)
        };
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}
fn pump_until(f: &Fixture, wake: &mpsc::SyncSender<()>, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "scheduler did not make progress");
        f.app.dispatch_dev(wake.clone()).unwrap();
        f.app.run_dev_completion(f.provider.as_ref()).unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn four_jobs_use_three_slots_and_fourth_dispatches_fifo_without_resend() {
    let mut f = Fixture::new();
    let (entered, arrivals) = mpsc::channel();
    let runtime = Arc::new(BarrierRuntime {
        entered,
        released: Mutex::new(HashSet::new()),
        gate: Condvar::new(),
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
        delegate: f.runtime.clone(),
    });
    let mut dev = dev_jobs::Owner::open(f.legacy_path.clone(), Arc::new(NeverLegacy));
    dev.local = Some(Arc::new(Owner::open(
        f.runtime.path.clone(),
        runtime.clone(),
    )));
    let dev = Arc::new(dev);
    Arc::get_mut(f.app.owner.as_mut().unwrap())
        .unwrap()
        .dev_jobs = dev.clone();
    f.app.dev_jobs = dev;
    for n in 1..=4 {
        assert_eq!(f.start(n)["status"], "queued");
    }
    let (wake, _rx) = mpsc::sync_channel(1);
    f.app.dispatch_dev(wake.clone()).unwrap();
    let mut first = (0..3)
        .map(|_| arrivals.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect::<Vec<_>>();
    first.sort();
    assert_eq!(first, vec![id(1), id(2), id(3)]);
    assert!(arrivals.try_recv().is_err());
    let state = f.call("dev_jobs_state_v4", json!({})).unwrap();
    assert_eq!(state.as_object().unwrap().len(), 4);
    assert_eq!(
        state["scheduler"],
        json!({"limit":3,"occupied":3,"queued":1,"accepting":true})
    );
    runtime.released.lock().unwrap().insert(id(2));
    runtime.gate.notify_all();
    pump_until(&f, &wake, || f.get(2)["job"]["status"] == "completed");
    f.app.dispatch_dev(wake.clone()).unwrap();
    assert_eq!(
        arrivals.recv_timeout(Duration::from_secs(5)).unwrap(),
        id(4)
    );
    assert_eq!(runtime.peak.load(Ordering::SeqCst), 3);
    runtime.released.lock().unwrap().extend((1..=4).map(id));
    runtime.gate.notify_all();
    pump_until(&f, &wake, || {
        (1..=4).all(|n| f.get(n)["job"]["status"] == "completed")
    });
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 4);
    assert!(f
        .provider
        .threads
        .lock()
        .unwrap()
        .iter()
        .all(|t| *t == std::thread::current().id()));
    assert_eq!(f.start(4)["status"], "completed");
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn queued_restart_expiry_and_legacy_operations_never_dispatch() {
    let f = Fixture::new();
    f.start(1);
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    let reopened = Fixture::ledgers(None, Some(bytes.clone()));
    assert_eq!(reopened.get(1)["job"]["status"], "failed");
    assert_eq!(reopened.get(1)["failed_stage"], "queued");
    assert_eq!(reopened.get(1)["job"]["message"], QUEUE_INTERRUPTED);
    assert_eq!(std::fs::read(&reopened.runtime.path).unwrap(), bytes);
    assert!(!reopened.run().unwrap());
    assert_eq!(reopened.start(1)["status"], "failed");
    for op in ["dev_jobs_state_v2", "dev_job_get_v2", "dev_job_start_v2"] {
        for args in [json!({}), Value::Null, json!([]), json!("retired")] {
            assert_eq!(f.call(op, args).unwrap_err().code, "UPGRADE_REQUIRED");
        }
    }
    f.owner()
        .update(&id(1), |r| {
            r.summary.created_ms = 0;
            Ok(())
        })
        .unwrap();
    f.run().unwrap();
    assert_eq!(f.get(1)["job"]["message"], QUEUE_EXPIRED);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn terminal_unknown_verified_releases_slot_but_not_identity_or_deletion() {
    let f = Fixture::new();
    f.start(1);
    f.runtime.mode.store(2, Ordering::Release);
    f.run().unwrap();
    assert_eq!(f.get(1)["job"]["status"], "unknown");
    assert!(!f.app.dev_has_model_dependencies());
    assert!(!f.app.dev_model_referenced(crate::catalog::CATALOG[0].name));
    assert!(!f.app.dev_reserved());
    assert_eq!(
        f.call("dev_jobs_state_v4", json!({})).unwrap()["scheduler"]["occupied"],
        0
    );
    assert_eq!(deletion(&f, 1).unwrap_err().code, "JOB_NOT_DELETABLE");
    assert_eq!(f.start(1)["status"], "unknown");
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn cleanup_uncertainty_occupies_only_its_slot_and_reservation_is_per_unfinished_job() {
    let f = Fixture::new();
    f.start(1);
    f.runtime.mode.store(1, Ordering::Release);
    f.run().unwrap();
    assert_eq!(
        f.call("dev_jobs_state_v4", json!({})).unwrap()["scheduler"]["occupied"],
        1
    );
    assert!(!f.app.dev_reserved());
    assert!(!f.app.dev_has_model_dependencies());
    let mut ledger = f.owner().state.lock().unwrap().ledger.clone();
    let historical = ledger.records[0].clone();
    ledger.records = vec![historical; 100];
    assert!(result_capacity(&ledger, 100 * 1024));
    let mut queued = f.owner().state.lock().unwrap().ledger.records[0].clone();
    queued.summary.status = "queued".into();
    ledger.records = vec![queued; 5];
    assert!(result_capacity(&ledger, 100 * 1024));
    ledger.records.push(ledger.records[0].clone());
    assert!(!result_capacity(&ledger, 100 * 1024));
}

#[test]
fn queued_model_drift_fails_before_container_or_provider_dispatch() {
    let f = Fixture::new();
    f.start(1);
    f.app.inner.lock().unwrap().inventory.models.clear();
    assert!(f.run().unwrap());
    let result = f.get(1);
    assert_eq!(result["job"]["status"], "failed");
    assert_eq!(result["job"]["message"], QUEUE_MODEL_CHANGED);
    assert_eq!(result["job"]["cleanup_verified"], true);
    assert_eq!(result["failed_stage"], "queued");
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    assert!(!f.app.dev_has_model_dependencies());
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    let reopened = Fixture::ledgers(None, Some(bytes.clone()));
    assert_eq!(reopened.get(1), result);
    assert_eq!(std::fs::read(&reopened.runtime.path).unwrap(), bytes);
}

#[test]
fn queued_spawn_failure_reopens_without_replay_or_cleanup_reservation() {
    let f = Fixture::new();
    f.start(1);
    // Exercise the durable outcome of a failed thread spawn without creating a worker.
    f.owner()
        .update(&id(1), |r| {
            r.failed_stage = Some("queued".into());
            r.summary.status = "failed".into();
            r.summary.message = SPAWN_FAILED.into();
            Ok(())
        })
        .unwrap();
    let bytes = std::fs::read(&f.runtime.path).unwrap();
    let reopened = Fixture::ledgers(None, Some(bytes.clone()));
    assert_eq!(reopened.get(1)["job"]["message"], SPAWN_FAILED);
    assert_eq!(reopened.start(1)["status"], "failed");
    assert!(!reopened.run().unwrap());
    assert!(!reopened.app.dev_has_model_dependencies());
    assert_eq!(reopened.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(reopened.provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(&reopened.runtime.path).unwrap(), bytes);
}

#[test]
fn five_unfinished_jobs_consume_reserved_results_without_double_charging() {
    let f = Fixture::new();
    for n in 1..=5 {
        f.start(n);
    }
    let before = std::fs::read(&f.runtime.path).unwrap();
    assert_eq!(
        f.call("dev_job_start_v4", f.args(6)).unwrap_err().code,
        "CAPACITY"
    );
    assert_eq!(std::fs::read(&f.runtime.path).unwrap(), before);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), 0);
    let text = "\u{0001}".repeat(crate::dev_local::MAX_REPLY);
    let mut receipts = Vec::new();
    for n in 1..=5 {
        f.owner()
            .update(&id(n), |r| {
                r.summary.status = "preparing".into();
                r.summary.message = PREPARING.into();
                r.summary.cleanup_verified = false;
                r.cleanup_required = true;
                Ok(())
            })
            .unwrap();
        f.owner()
            .event(
                &id(n),
                LocalEvent::Preparing {
                    image_id: format!("sha256:{}", "a".repeat(64)),
                },
            )
            .unwrap();
        f.owner().event(&id(n), LocalEvent::Cloning).unwrap();
        let repositories = vec![Checkout {
            url: "https://github.com/example/fixture".into(),
            reference: "refs/heads/main".into(),
            commit: "b".repeat(40),
        }];
        let session_id = "s".repeat(256);
        let message_id = "m".repeat(256);
        f.owner()
            .event(
                &id(n),
                LocalEvent::Ready {
                    session_id: session_id.clone(),
                    repositories: repositories.clone(),
                },
            )
            .unwrap();
        f.owner().event(&id(n), LocalEvent::Dispatching).unwrap();
        f.owner()
            .event(
                &id(n),
                LocalEvent::Reply {
                    session_id: session_id.clone(),
                    message_id: message_id.clone(),
                    text: text.clone(),
                },
            )
            .unwrap();
        assert_eq!(f.get(n)["reply"], text);
        assert_eq!(f.get(n)["job"]["cleanup_verified"], false);
        receipts.push((
            n,
            LocalReceipt {
                session_id,
                message_id,
                reply: text.clone(),
                repositories,
            },
        ));
        if n == 3 {
            // Three maximum escaped replies coexist with two queued jobs, before cleanup.
            let bytes = std::fs::read(&f.runtime.path).unwrap();
            assert!(bytes.len() + 5 * RESULT_RESERVE > LIMIT as usize);
            assert!(result_capacity(&f.runtime.disk(), bytes.len()));
            for (n, receipt) in receipts.drain(..) {
                f.owner().finish(&id(n), Ok(receipt)).unwrap();
            }
        }
    }
    for (n, receipt) in receipts {
        f.owner().finish(&id(n), Ok(receipt)).unwrap();
    }
    for n in 1..=5 {
        assert_eq!(f.get(n)["job"]["status"], "completed");
        assert_eq!(f.get(n)["reply"], text);
    }
    assert!(!f.owner().reserved());
    assert!(!f.owner().state.lock().unwrap().fault);
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
}

struct InventoryFaultProvider {
    owner: Arc<Owner>,
    inventories: AtomicUsize,
}
impl Provider for InventoryFaultProvider {
    fn inventory(&self) -> AppResult<Inventory> {
        self.inventories.fetch_add(1, Ordering::SeqCst);
        filesystem::publish(&self.owner.path, b"{}", 0o600, false).unwrap();
        let mut state = self.owner.state.lock().unwrap();
        let next = state.ledger.clone();
        assert!(self.owner.commit(&mut state, next).is_err());
        let mut inventory = Inventory::default();
        let model = &crate::catalog::CATALOG[0];
        inventory.providers.llm = crate::providers::Observation {
            ready: true,
            error: None,
        };
        inventory.models.push(crate::providers::Model {
            name: model.name.into(),
            digest: model.sha256.into(),
        });
        Ok(inventory)
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        panic!("Dev integrity fault must fence the subsequent inference phase")
    }
}

#[test]
fn dequeued_routed_work_rechecks_dev_integrity_before_and_between_effects() {
    for during_inventory in [false, true] {
        let f = Fixture::new();
        {
            let mut inner = f.app.inner.lock().unwrap();
            inner.durable.config.revision = 1;
            // This regression persists app work, not the domain fixture's synthetic project.
            inner.durable.projects = projects::Registry::default();
            inner.durable.validate().unwrap();
        }
        f.call("submit_routed", json!({
            "request_id":id(82), "instance_id":f.app.instance_id,
            "conversation_id":null, "text":"bounded task",
            "route":{"provider":"local","model":crate::catalog::CATALOG[0].name,"credential_generation":null},
            "expected_config_revision":1, "expected_history_revision":null,
            "approval_token":null, "confirm_transfer":false,
        })).unwrap();
        let work = f.app.next_work().unwrap().unwrap();
        let provider = InventoryFaultProvider {
            owner: f.app.dev_jobs.local.as_ref().unwrap().clone(),
            inventories: AtomicUsize::new(0),
        };
        if !during_inventory {
            filesystem::publish(&provider.owner.path, b"{}", 0o600, false).unwrap();
            let mut state = provider.owner.state.lock().unwrap();
            let next = state.ledger.clone();
            assert!(provider.owner.commit(&mut state, next).is_err());
        }
        f.app.run(work, &provider);
        let inner = f.app.inner.lock().unwrap();
        let job = inner
            .durable
            .jobs
            .iter()
            .find(|j| j.operation.request_id == id(82))
            .unwrap();
        assert_eq!(job.operation.status, "failed");
        assert_eq!(
            job.operation.error.as_ref().unwrap().code,
            "STATE_UNAVAILABLE"
        );
        assert!(inner
            .durable
            .conversations
            .iter()
            .all(|c| c.history.is_empty()));
        assert!(!inner.fault);
        assert_eq!(
            provider.inventories.load(Ordering::SeqCst),
            usize::from(during_inventory)
        );
    }
}

struct IdleGateProvider {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    first: AtomicBool,
    calls: AtomicUsize,
}
impl Provider for IdleGateProvider {
    fn inventory(&self) -> AppResult<Inventory> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Inventory::default())
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("unexpected".into())
    }
    fn idle_timeout(&self) -> Option<Duration> {
        if self.first.swap(false, Ordering::SeqCst) {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        Some(Duration::from_secs(60))
    }
}

#[test]
fn real_worker_fails_accepted_app_work_after_dev_publication_fault_without_provider_effects() {
    let f = Fixture::new();
    let roots = crate::Roots {
        data: f.base.join("data"),
        config: f.base.join("config"),
        runtime: f.base.join("run"),
    };
    for path in [
        &roots.data,
        &roots.config,
        &roots.runtime,
        &roots.data.join("smith-next"),
    ] {
        filesystem::mkdir(path).unwrap();
    }
    let (entered, arrival) = mpsc::channel();
    let (release, receiver) = mpsc::channel();
    let provider = Arc::new(IdleGateProvider {
        entered,
        release: Mutex::new(receiver),
        first: AtomicBool::new(true),
        calls: AtomicUsize::new(0),
    });
    let app = Application::open_with_cloud(
        &roots,
        provider.clone(),
        crate::cloud::Dependencies::disabled(),
    )
    .unwrap();
    arrival.recv_timeout(Duration::from_secs(5)).unwrap();
    let request = |op: &str, args| AppRequest {
        schema_version: 3,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(800),
        op: op.into(),
        args,
    };
    app.call(request("models", json!({"request_id":id(81)})))
        .unwrap();
    let local = app.dev_jobs.local.as_ref().unwrap();
    filesystem::publish(&local.path, b"{}", 0o600, false).unwrap();
    {
        let mut state = local.state.lock().unwrap();
        let next = state.ledger.clone();
        assert!(local.commit(&mut state, next).is_err());
        assert!(state.publication_ambiguous);
    }
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = app
            .call(request(
                "poll",
                json!({"request_id":id(81),"instance_id":app.instance_id}),
            ))
            .unwrap();
        if result["status"] == "failed" {
            assert_eq!(result["error"]["code"], "STATE_UNAVAILABLE");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(app.next_work().unwrap().is_none());
    assert!(!app.inner.lock().unwrap().fault);
    drop(app);
}

struct QueueClock {
    wall: std::sync::atomic::AtomicU64,
}
impl crate::cloud::Clock for QueueClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wall_ms(&self) -> u64 {
        self.wall.load(Ordering::SeqCst)
    }
}
fn occupy_three_slots(ledger: &mut Ledger) {
    for record in ledger.records.iter_mut().take(3) {
        record.summary.status = "cleanup_unknown".into();
        record.summary.message = RESTARTED.into();
        record.summary.cleanup_verified = false;
        record.cleanup_required = true;
        record.failed_stage = Some("preparing".into());
    }
}

#[test]
fn queue_timeout_tracks_earliest_expiry_and_disarms_when_integrity_blocked() {
    let mut f = Fixture::new();
    assert_eq!(f.app.dev_queue_timeout(), None);
    for n in 1..=5 {
        f.start(n);
    }
    let clock = Arc::new(QueueClock {
        wall: std::sync::atomic::AtomicU64::new(1_000_000),
    });
    f.app.cloud = Arc::new(crate::cloud::Dependencies::disabled().with_clock(clock.clone()));
    {
        let mut state = f.owner().state.lock().unwrap();
        let mut next = state.ledger.clone();
        occupy_three_slots(&mut next);
        next.records[3].summary.created_ms = 950_000;
        next.records[4].summary.created_ms = 400_003;
        f.owner().commit(&mut state, next).unwrap();
    }
    assert_eq!(f.app.dev_queue_timeout(), Some(Duration::from_millis(3)));
    clock.wall.store(1_000_004, Ordering::SeqCst);
    assert_eq!(f.app.dev_queue_timeout(), Some(Duration::from_millis(1)));
    let (sender, _receiver) = mpsc::sync_channel(1);
    assert!(f.app.dispatch_dev(sender).unwrap());
    assert_eq!(f.get(5)["job"]["message"], QUEUE_EXPIRED);
    assert_eq!(f.get(4)["job"]["status"], "queued");
    assert_eq!(
        f.app.dev_queue_timeout(),
        Some(Duration::from_millis(549_996))
    );
    assert_eq!(f.runtime.calls.load(Ordering::SeqCst), 0);
    clock.wall.store(900_000, Ordering::SeqCst);
    assert_eq!(f.app.dev_queue_timeout(), Some(Duration::from_secs(600)));
    {
        let mut state = f.owner().state.lock().unwrap();
        state.publication_ambiguous = true;
    }
    assert_eq!(f.app.dev_queue_timeout(), None);
    {
        let mut state = f.owner().state.lock().unwrap();
        state.publication_ambiguous = false;
        state.fault = true;
    }
    assert_eq!(f.app.dev_queue_timeout(), None);
    f.owner().state.lock().unwrap().fault = false;
    f.app.inner.lock().unwrap().fault = true;
    assert_eq!(f.app.dev_queue_timeout(), None);
    f.app.inner.lock().unwrap().fault = false;
    f.owner().close();
    assert_eq!(f.app.dev_queue_timeout(), None);
}

struct UnloadedIdleProvider {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    first: AtomicBool,
    idled: mpsc::Sender<()>,
}
impl Provider for UnloadedIdleProvider {
    fn inventory(&self) -> AppResult<Inventory> {
        panic!("queue expiry must not observe or load a provider")
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        panic!("queue expiry must not infer")
    }
    fn idle_timeout(&self) -> Option<Duration> {
        if self.first.swap(false, Ordering::SeqCst) {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        None
    }
    fn idle(&self) {
        self.idled.send(()).unwrap();
    }
}

#[test]
fn real_worker_expires_blocked_queue_with_unloaded_provider_without_external_wake() {
    let f = Fixture::new();
    for n in 1..=4 {
        f.start(n);
    }
    let roots = crate::Roots {
        data: f.base.join("data"),
        config: f.base.join("config"),
        runtime: f.base.join("run"),
    };
    for path in [
        &roots.data,
        &roots.config,
        &roots.runtime,
        &roots.data.join("smith-next"),
    ] {
        filesystem::mkdir(path).unwrap();
    }
    let (entered, arrival) = mpsc::channel();
    let (release, receiver) = mpsc::channel();
    let (idled, idle_events) = mpsc::channel();
    let provider = Arc::new(UnloadedIdleProvider {
        entered,
        release: Mutex::new(receiver),
        first: AtomicBool::new(true),
        idled,
    });
    let app = Application::open_with_cloud(
        &roots,
        provider.clone(),
        crate::cloud::Dependencies::disabled(),
    )
    .unwrap();
    arrival.recv_timeout(Duration::from_secs(5)).unwrap();
    let local = app.dev_jobs.local.as_ref().unwrap();
    {
        let mut state = local.state.lock().unwrap();
        let mut next = f.owner().state.lock().unwrap().ledger.clone();
        occupy_three_slots(&mut next);
        // A short remaining age exercises the real ten-minute policy, not a test TTL.
        next.records[3].summary.created_ms = app.cloud.clock.wall_ms() - 600_000 + 150;
        local.commit(&mut state, next).unwrap();
    }
    assert!(!local.scheduler.has_workers().unwrap());
    assert!(app.dev_queue_timeout().is_some());
    app.wake().unwrap(); // The admission wake is the last external wake in this test.
    release.send(()).unwrap();
    idle_events.recv_timeout(Duration::from_secs(5)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = local.state.lock().unwrap();
        if state.ledger.records[3].summary.status == "failed" {
            assert_eq!(state.ledger.records[3].summary.message, QUEUE_EXPIRED);
            assert!(state.ledger.records[3].summary.cleanup_verified);
            break;
        }
        drop(state);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.dev_queue_timeout(), None);
    assert!(idle_events.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(!local.scheduler.has_workers().unwrap());
    assert_eq!(provider.idle_timeout(), None);
    drop(app);
}

#[test]
fn frozen_model_and_config_mutations_are_guarded_while_queued() {
    let f = Fixture::new();
    f.start(1);
    assert!(f.app.dev_model_referenced(crate::catalog::CATALOG[0].name));
    let mut inner = f.app.inner.lock().unwrap();
    let mut next = inner.durable.clone();
    next.config.model = None;
    assert_eq!(f.app.commit(&mut inner, next).unwrap_err().code, "BUSY");
}
