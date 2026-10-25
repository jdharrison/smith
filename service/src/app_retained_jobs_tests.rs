use super::*;

#[derive(Default)]
struct FakeRetained {
    commands: Mutex<Vec<retained_jobs::Command>>,
    mismatch: std::sync::atomic::AtomicBool,
}
impl retained_jobs::Runtime for FakeRetained {
    fn execute(&self, c: &retained_jobs::Command) -> retained_jobs::Acknowledgement {
        self.commands.lock().unwrap().push(c.clone());
        let outcome = match c.action {
            retained_jobs::Action::Create => retained_jobs::Outcome::Ready,
            retained_jobs::Action::Prompt if c.prompt.as_deref() == Some("Initial task") => {
                retained_jobs::Outcome::Failed
            }
            retained_jobs::Action::Prompt => {
                retained_jobs::Outcome::Reply("Follow-up succeeded".into())
            }
            retained_jobs::Action::Finish => retained_jobs::Outcome::Finished,
        };
        retained_jobs::Acknowledgement {
            session_id: c.session_id.clone(),
            operation_id: if self.mismatch.load(std::sync::atomic::Ordering::SeqCst) {
                new_id().unwrap()
            } else {
                c.operation_id.clone()
            },
            action: c.action,
            outcome,
        }
    }
}
fn wait_job(f: &Fixture, session: &str, status: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let job = f
            .call("dev_session_get_v1", json!({"session_id":session}))
            .unwrap();
        if job["job"]["status"] == status {
            return job;
        }
        assert!(Instant::now() < deadline, "waiting for {status}: {job}");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn send_current(f: &Fixture, op: &str, mut args: Value) -> Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        args["expected_revision"] =
            f.call("dev_sessions_state_v1", json!({})).unwrap()["revision"].clone();
        match f.call(op, args.clone()) {
            Ok(value) => return value,
            Err(e) if e.code == "STALE_REVISION" && Instant::now() < deadline => continue,
            Err(e) => panic!("{op}: {e:?}"),
        }
    }
}
fn mutation(f: &Fixture, session: &str) -> Value {
    json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"session_id":session,"expected_revision":0})
}
#[test]
fn composed_input_is_rejected_before_start_or_followup_publication() {
    let runtime = Arc::new(FakeRetained::default());
    let f = Fixture::with_runtime(Some(runtime.clone()));
    let mut args = f.start_args();
    args["prompt"] = json!("x".repeat(retained_jobs::MAX_PROMPT));
    let e = f.call("dev_session_start_v1", args.clone()).unwrap_err();
    assert_eq!(e.code, "CONTEXT_LIMIT");
    assert!(!e.message.contains(&"x".repeat(32)));
    assert_eq!(
        f.app
            .dev_jobs
            .retained
            .as_ref()
            .unwrap()
            .revision()
            .unwrap(),
        0
    );
    assert!(runtime.commands.lock().unwrap().is_empty());
    let frozen = {
        let inner = f.app.inner.lock().unwrap();
        inner
            .durable
            .projects
            .freeze_retained(
                args["project_id"].as_str().unwrap(),
                args["expected_projects_revision"].as_u64().unwrap(),
                &inner.durable.instructions.dev_settings_text(),
            )
            .unwrap()
    };
    let room = retained_jobs::MAX_PROMPT - frozen.context.len() - "\nTask:\n".len();
    args["prompt"] = json!("x".repeat(room));
    let started = f.call("dev_session_start_v1", args).unwrap();
    let session = started["job"]["session_id"].as_str().unwrap();
    wait_job(&f, session, "ready");
    let path = f.roots.data.join("smith-next/dev-retained-jobs-v1.json");
    let before = std::fs::read(&path).unwrap();
    let count = runtime.commands.lock().unwrap().len();
    for prompt in ["x".repeat(retained_jobs::MAX_PROMPT), "\u{1}".repeat(6000)] {
        let mut followup = mutation(&f, session);
        followup["expected_revision"] =
            f.call("dev_sessions_state_v1", json!({})).unwrap()["revision"].clone();
        followup["prompt"] = json!(prompt);
        assert_eq!(
            f.call("dev_session_prompt_v1", followup).unwrap_err().code,
            "CONTEXT_LIMIT"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(runtime.commands.lock().unwrap().len(), count);
    }
}

#[test]
fn retained_app3_connected_worker_lifecycle_queue_and_query_only_receipts() {
    let runtime = Arc::new(FakeRetained::default());
    let f = Fixture::with_runtime(Some(runtime.clone()));
    let projects = f.call("projects_list_v3", json!({})).unwrap();
    let repo = |name: &str, read_only: bool| {
        json!({"name":name,"url":format!("https://example.invalid/{name}.git"),"read_only":read_only,
        "branch":{"base":"main","work":null,"types":{"feature":null,"fix":null,"release":null,"hotfix":null}}})
    };
    let created = f.call("project_create_v3",json!({"request_id":new_id().unwrap(),"instance_id":f.app.instance_id,
        "expected_revision":projects["revision"],"name":"Fixture","key":"runtime-fixture","description":"","prompt":" exact project context ",
        "repositories":[repo("source",false),repo("reference",true)],"agent":"opencode"})).unwrap();
    let mut receipts = Vec::new();
    for i in 0..4 {
        let args = json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"expected_revision":0,
            "expected_projects_revision":created["revision"],"project_id":created["project_id"],"job_type":if i==0 {"write"} else {"read"},"prompt":"Initial task"});
        let admitted = send_current(&f, "dev_session_start_v1", args);
        let session = admitted["receipt"]["session_id"].as_str().unwrap();
        if i < 3 {
            wait_job(&f, session, "failed");
        } else {
            wait_job(&f, session, "queued");
        }
        receipts.push(admitted["receipt"].clone());
    }
    let state = f.call("dev_sessions_state_v1", json!({})).unwrap();
    assert_eq!(state["capability"]["available"], true);
    assert_eq!(
        state["scheduler"],
        json!({"limit":3,"occupied":3,"queued":1,"accepting":true})
    );
    let first = receipts[0]["session_id"].as_str().unwrap();
    let commands = runtime.commands.lock().unwrap().clone();
    assert_eq!(commands.len(), 6);
    assert_eq!(commands[0].action, retained_jobs::Action::Create);
    assert_eq!(commands[1].action, retained_jobs::Action::Prompt);
    assert_eq!(commands[0].operation_id, commands[1].operation_id);
    assert!(!commands[0].project.repositories[0].read_only);
    assert!(commands[0].project.repositories[1].read_only);
    assert!(commands[2].project.repositories.iter().all(|r| r.read_only));
    let global = f
        .app
        .inner
        .lock()
        .unwrap()
        .durable
        .instructions
        .dev_settings_text();
    assert_eq!(
        commands[0].project.context,
        format!("Global:\n{global}\nProject:\n exact project context ")
    );
    let mut followup = mutation(&f, first);
    followup["prompt"] = json!("Fix the known failure");
    let reply = send_current(&f, "dev_session_prompt_v1", followup);
    let detail = wait_job(&f, first, "ready");
    assert_eq!(detail["turns"][0]["outcome"], "failed");
    assert_eq!(detail["turns"][1]["reply"], "Follow-up succeeded");
    let before = std::fs::read(f.roots.data.join("smith-next/dev-retained-jobs-v1.json")).unwrap();
    let count = runtime.commands.lock().unwrap().len();
    let queried = f
        .call(
            "dev_session_receipt_v1",
            json!({"operation_id":reply["receipt"]["operation_id"]}),
        )
        .unwrap();
    assert_eq!(queried["receipt"], reply["receipt"]);
    assert_eq!(queried["job"], detail["job"]);
    assert_eq!(
        before,
        std::fs::read(f.roots.data.join("smith-next/dev-retained-jobs-v1.json")).unwrap()
    );
    assert_eq!(runtime.commands.lock().unwrap().len(), count);
    f.call("project_update_v3", json!({"request_id":new_id().unwrap(),"instance_id":f.app.instance_id,
        "expected_revision":created["revision"],"project_id":created["project_id"],"name":"Fixture","key":"runtime-fixture",
        "description":"","prompt":"Changed after admission","repositories":[repo("source",true),repo("reference",true)],"agent":"opencode"})).unwrap();
    let finish = send_current(&f, "dev_session_finish_v1", mutation(&f, first));
    wait_job(&f, first, "finished");
    wait_job(&f, receipts[3]["session_id"].as_str().unwrap(), "failed");
    let queued_create = runtime
        .commands
        .lock()
        .unwrap()
        .iter()
        .find(|c| {
            c.session_id == receipts[3]["session_id"].as_str().unwrap()
                && c.action == retained_jobs::Action::Create
        })
        .unwrap()
        .clone();
    assert_eq!(queued_create.project.context, commands[0].project.context);
    assert!(queued_create
        .project
        .repositories
        .iter()
        .all(|r| r.read_only));
    assert_eq!(
        f.call(
            "dev_session_receipt_v1",
            json!({"operation_id":finish["receipt"]["operation_id"]})
        )
        .unwrap()["job"]["retained"],
        false
    );
    let deletion = send_current(&f, "dev_session_delete_v1", mutation(&f, first));
    let queried = f
        .call(
            "dev_session_receipt_v1",
            json!({"operation_id":deletion["receipt"]["operation_id"]}),
        )
        .unwrap();
    assert_eq!(queried, deletion);
    assert_eq!(queried["job"]["deleted"], true);
    let restored =
        retained_jobs::Store::open(f.roots.data.join("smith-next/dev-retained-jobs-v1.json"))
            .unwrap();
    assert_eq!(
        restored
            .receipt(deletion["receipt"]["operation_id"].as_str().unwrap())
            .unwrap()
            .session_id,
        first
    );
}
#[test]
fn retained_app3_mismatched_runtime_ack_is_unknown_and_never_replayed() {
    let runtime = Arc::new(FakeRetained::default());
    runtime
        .mismatch
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let f = Fixture::with_runtime(Some(runtime.clone()));
    let start = send_current(&f, "dev_session_start_v1", f.start_args());
    let id = start["job"]["session_id"].as_str().unwrap();
    wait_job(&f, id, "unknown");
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
    let mut args = mutation(&f, id);
    args["prompt"] = json!("Do not replay");
    args["expected_revision"] =
        f.call("dev_sessions_state_v1", json!({})).unwrap()["revision"].clone();
    assert_eq!(
        f.call("dev_session_prompt_v1", args).unwrap_err().code,
        "JOB_BLOCKED"
    );
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
}

struct Never;
impl Provider for Never {
    fn inventory(&self) -> AppResult<Inventory> {
        panic!("retained API must not observe providers")
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        panic!("retained API must not infer")
    }
}
struct Fixture {
    app: Application,
    dir: PathBuf,
    roots: Roots,
}
impl Fixture {
    fn new() -> Self {
        Self::with_runtime(None)
    }
    fn with_runtime(runtime: Option<Arc<dyn retained_jobs::Runtime>>) -> Self {
        let dir = std::env::temp_dir().join(format!("smith-retained-api-{}", new_id().unwrap()));
        let roots = Roots {
            data: dir.join("data"),
            config: dir.join("config"),
            runtime: dir.join("runtime"),
        };
        for p in [
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
        ] {
            filesystem::mkdir(p).unwrap();
        }
        let app = match runtime {
            Some(runtime) => Application::open_with_retained_runtime(
                &roots,
                Arc::new(Never),
                crate::cloud::Dependencies::disabled(),
                runtime,
            )
            .unwrap(),
            None => Application::open(&roots, Arc::new(Never)).unwrap(),
        };
        Self { app, dir, roots }
    }
    fn call(&self, op: &str, args: Value) -> AppResult<Value> {
        self.app.call(AppRequest {
            schema_version: 3,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: new_id().unwrap(),
            op: op.into(),
            args,
        })
    }
    fn start_args(&self) -> Value {
        let inner = self.app.inner.lock().unwrap();
        let p = json!(inner.durable.projects);
        json!({"operation_id":new_id().unwrap(),"instance_id":self.app.instance_id,"expected_revision":0,
            "expected_projects_revision":p["revision"],"project_id":p["projects"][0]["project_id"],"job_type":"write","prompt":"Implement a change"})
    }
    fn seed(&self) -> retained_jobs::Receipt {
        self.app
            .dev_jobs
            .retained
            .as_ref()
            .unwrap()
            .start(retained_jobs::Start {
                operation_id: new_id().unwrap(),
                session_id: new_id().unwrap(),
                expected_revision: 0,
                project: retained_jobs::FrozenProject {
                    project_id: new_id().unwrap(),
                    project_key: "fixture".into(),
                    context: "context".into(),
                    repositories: vec![],
                },
                job_type: retained_jobs::JobType::Read,
                prompt: "Inspect".into(),
                created_ms: 1,
            })
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.app.inner.lock().unwrap().owned = false;
        let _ = self.app.wake();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn retained_app3_capability_is_truthfully_disabled_and_never_admits_start() {
    let f = Fixture::new();
    let state = f.call("dev_sessions_state_v1", json!({})).unwrap();
    assert_eq!(state["revision"], 0);
    assert_eq!(state["capability"]["available"], false);
    assert_eq!(
        state["scheduler"],
        json!({"limit":3,"occupied":0,"queued":0,"accepting":false})
    );
    let args = f.start_args();
    assert_eq!(
        f.call("dev_session_start_v1", args.clone())
            .unwrap_err()
            .code,
        "DEV_RUNTIME_UNAVAILABLE"
    );
    assert_eq!(
        f.call("dev_session_start_v1", args).unwrap_err().code,
        "DEV_RUNTIME_UNAVAILABLE"
    );
    assert!(!f
        .roots
        .data
        .join("smith-next/dev-retained-jobs-v1.json")
        .exists());
}
#[test]
fn retained_app3_rejects_stale_and_malformed_start_before_capability_check() {
    let f = Fixture::new();
    let mut args = f.start_args();
    args["expected_revision"] = json!(1);
    assert_eq!(
        f.call("dev_session_start_v1", args).unwrap_err().code,
        "STALE_REVISION"
    );
    let mut args = f.start_args();
    args["job_type"] = json!("admin");
    assert!(f.call("dev_session_start_v1", args).is_err());
    let mut args = f.start_args();
    args["model"] = json!("ambient");
    assert!(f.call("dev_session_start_v1", args).is_err());
    assert!(f
        .call("dev_sessions_state_v1", json!({"auto_start":true}))
        .is_err());
}
#[test]
fn retained_app3_observation_and_queued_finish_delete_are_durable() {
    let f = Fixture::new();
    let r = f.seed();
    let args = json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"expected_revision":1,"session_id":r.session_id});
    let finish = f.call("dev_session_finish_v1", args.clone()).unwrap();
    assert_eq!(finish["job"]["status"], "finished");
    assert_eq!(
        f.call("dev_session_finish_v1", args).unwrap()["receipt"],
        finish["receipt"]
    );
    let args = json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"expected_revision":2,"session_id":r.session_id});
    let deleted = f.call("dev_session_delete_v1", args).unwrap();
    assert_eq!(deleted["job"]["deleted"], true);
    assert_eq!(
        f.call("dev_sessions_state_v1", json!({})).unwrap()["jobs"],
        json!([])
    );
    let s = f
        .call("dev_session_get_v1", json!({"session_id":r.session_id}))
        .unwrap();
    assert_eq!(s["job"]["deleted"], true);
    assert_eq!(s["turns"][0]["prompt"], "Inspect");
}
#[test]
fn retained_app3_unknown_environment_reservation_is_visible_and_cannot_be_finished() {
    let f = Fixture::new();
    let r = f.seed();
    let store = f.app.dev_jobs.retained.as_ref().unwrap();
    let c = store.prepare_next(0).unwrap().unwrap();
    store
        .acknowledge(retained_jobs::Acknowledgement {
            session_id: c.session_id,
            operation_id: c.operation_id,
            action: c.action,
            outcome: retained_jobs::Outcome::Unknown,
        })
        .unwrap();
    assert_eq!(
        f.call("dev_sessions_state_v1", json!({})).unwrap()["scheduler"]["occupied"],
        1
    );
    let args = json!({"operation_id":new_id().unwrap(),"instance_id":f.app.instance_id,"expected_revision":store.revision().unwrap(),"session_id":r.session_id});
    assert_eq!(
        f.call("dev_session_finish_v1", args).unwrap_err().code,
        "DEV_RUNTIME_UNAVAILABLE"
    );
    assert_eq!(store.reserved(), 1);
}
