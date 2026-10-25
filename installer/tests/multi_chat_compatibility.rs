//! Independent LOCAL-SMITH-NEXT-MULTI-CHAT REQ-004 regression.
//! Real disk3 application/storage and frozen installer v1 seam; no transport,
//! manager, keyring, cloud calls or model weights.
use serde_json::{json, Value};
use smith_installer::{
    setup::{Backend, Native},
    Result,
};
use smith_service::{
    app::{AppError, AppReply, AppRequest, AppResult, Application, Message, Operation},
    providers::{Inventory, Model, Observation as ProviderObservation, Observations, Provider},
    runner, Action, Observation, Roots,
};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

const BOUND: Duration = Duration::from_secs(5);

struct Fixture {
    base: PathBuf,
    roots: Roots,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "smith-installer-multi-chat-{}",
            smith_service::app::new_id().unwrap()
        ));
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        for path in [
            &base,
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
        ] {
            fs::DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        Self { base, roots }
    }
    fn path(&self) -> PathBuf {
        self.roots.data.join("smith-next/app-state.json")
    }
    fn disk(&self) -> Value {
        serde_json::from_slice(&fs::read(self.path()).unwrap()).unwrap()
    }
    fn seed_legacy(&self) {
        let value = json!({"schema_version":1,"config":{"model":null,"voice":null,"revision":0},
            "history":[{"role":"user","content":"Legacy question"},{"role":"assistant","content":"Legacy answer"}],
            "jobs":[],"expired_ids":[]});
        fs::write(self.path(), serde_json::to_vec(&value).unwrap()).unwrap();
        fs::set_permissions(self.path(), fs::Permissions::from_mode(0o600)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}

#[derive(Default)]
struct Gate {
    blocked: bool,
    entered: usize,
    inventories: usize,
    removals: usize,
}
#[derive(Default)]
struct FakeProvider {
    gate: Mutex<Gate>,
    changed: Condvar,
}
impl FakeProvider {
    fn block(&self) -> usize {
        let mut gate = self.gate.lock().unwrap();
        gate.blocked = true;
        gate.entered + 1
    }
    fn wait_entered(&self, count: usize) {
        let (gate, timeout) = self
            .changed
            .wait_timeout_while(self.gate.lock().unwrap(), BOUND, |g| g.entered < count)
            .unwrap();
        assert!(
            !timeout.timed_out() && gate.entered >= count,
            "provider did not enter gate"
        );
    }
    fn release(&self) {
        self.gate.lock().unwrap().blocked = false;
        self.changed.notify_all();
    }
}
impl Provider for FakeProvider {
    fn inventory(&self) -> AppResult<Inventory> {
        self.gate.lock().unwrap().inventories += 1;
        Ok(Inventory {
            models: runner::CATALOG
                .iter()
                .take(2)
                .map(|entry| Model {
                    name: entry.name.into(),
                    digest: entry.sha256.into(),
                })
                .collect(),
            voices: vec![],
            providers: Observations {
                llm: ProviderObservation {
                    ready: true,
                    error: None,
                },
                ..Observations::default()
            },
        })
    }
    fn chat(&self, model: &str, _: &[Message]) -> AppResult<String> {
        assert_eq!(model, runner::CATALOG[0].name);
        let mut gate = self.gate.lock().unwrap();
        gate.entered += 1;
        self.changed.notify_all();
        let (gate, timeout) = self
            .changed
            .wait_timeout_while(gate, BOUND, |g| g.blocked)
            .unwrap();
        if timeout.timed_out() && gate.blocked {
            return Err(AppError::new(
                "FIXTURE_TIMEOUT",
                "Provider gate was not released",
            ));
        }
        Ok("Fixture answer".into())
    }
    fn remove_model(&self, model: &str, sha: &str) -> AppResult<()> {
        assert_eq!(
            (model, sha),
            (runner::CATALOG[1].name, runner::CATALOG[1].sha256)
        );
        self.gate.lock().unwrap().removals += 1;
        Ok(())
    }
}

fn fields(value: &Value, expected: &[&str]) {
    let object = value.as_object().expect("object DTO");
    assert_eq!(object.len(), expected.len(), "unexpected DTO: {value}");
    for key in expected {
        assert!(object.contains_key(*key), "missing {key}: {value}");
    }
}
fn operation(value: &Value) {
    fields(
        value,
        &[
            "request_id",
            "kind",
            "status",
            "phase",
            "config_revision",
            "result",
            "error",
        ],
    );
    let parsed: Operation = serde_json::from_value(value.clone()).unwrap();
    assert!(["running", "succeeded", "failed", "interrupted"].contains(&parsed.status.as_str()));
    assert!([
        "models",
        "submit",
        "verify_model",
        "remove_model",
        "transcribe",
        "speak"
    ]
    .contains(&parsed.kind.as_str()));
}
fn state(value: &Value) {
    fields(
        value,
        &["config", "history", "active", "providers", "retention"],
    );
    fields(&value["config"], &["model", "voice", "revision"]);
    for message in value["history"].as_array().unwrap() {
        fields(message, &["role", "content"]);
    }
    if !value["active"].is_null() {
        operation(&value["active"]);
    }
}

type Trace = Arc<Mutex<Vec<(AppRequest, AppReply)>>>;
struct RealBackend {
    app: Arc<Application>,
    trace: Trace,
}
impl Backend for RealBackend {
    fn inspect(&mut self) -> Result<Observation> {
        panic!("no lifecycle inspection in domain regression")
    }
    fn prepare(&mut self, _: Action) -> Result<Observation> {
        panic!("no lifecycle preparation")
    }
    fn apply(&mut self, _: Action, _: &str) -> Result<Observation> {
        panic!("no lifecycle effects")
    }
    fn app(&mut self, request: AppRequest) -> Result<AppReply> {
        assert_eq!(request.schema_version, 1);
        let reply = AppReply::from_versioned_result(
            request.schema_version,
            &self.app.instance_id,
            &request.call_id,
            self.app.call(request.clone()),
        );
        if let Some(value) = &reply.result {
            match request.op.as_str() {
                "state" => state(value),
                "configure" | "configure_checked" => fields(value, &["model", "voice", "revision"]),
                "models" | "verify_model" | "remove_model" => {
                    fields(value, &["request_id", "status", "phase", "config_revision"]);
                    assert!(["running", "succeeded"].contains(&value["status"].as_str().unwrap()));
                }
                "poll" => operation(value),
                other => panic!("unexpected installer operation {other}"),
            }
        }
        let captured = serde_json::from_value(serde_json::to_value(&reply).unwrap()).unwrap();
        self.trace.lock().unwrap().push((request, captured));
        Ok(reply)
    }
}
fn native(app: &Arc<Application>) -> (Native, Trace) {
    let trace = Arc::new(Mutex::new(vec![]));
    (
        Native::new(Box::new(RealBackend {
            app: app.clone(),
            trace: trace.clone(),
        })),
        trace,
    )
}
fn request(version: u32, op: &str, args: Value) -> AppRequest {
    AppRequest {
        schema_version: version,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: smith_service::app::new_id().unwrap(),
        op: op.into(),
        args,
    }
}
fn call(app: &Application, version: u32, op: &str, args: Value) -> Value {
    app.call(request(version, op, args)).unwrap()
}
fn finished(app: &Application, version: u32, id: &str) -> Value {
    let deadline = Instant::now() + BOUND;
    loop {
        let value = call(
            app,
            version,
            "poll",
            json!({"request_id":id,"instance_id":app.instance_id}),
        );
        if !["queued", "running"].contains(&value["status"].as_str().unwrap()) {
            assert_eq!(value["status"], "succeeded", "{value}");
            return value;
        }
        assert!(Instant::now() < deadline, "terminal publication deadline");
        // The domain has no completion notification seam; yield only for publication.
        std::thread::yield_now();
    }
}
fn submit_new(app: &Application, id: &str, expected_config_revision: u64) -> Value {
    call(
        app,
        2,
        "submit_new",
        json!({"request_id":id,"text":"Other conversation","output":"chat","expected_config_revision":expected_config_revision}),
    )
}
fn initialize(native: &mut Native, expected_revision: u64) {
    assert_eq!(native.configuration().unwrap().revision, expected_revision);
    let names = native.models().unwrap();
    assert_eq!(names.len(), 2);
    assert!(names.contains(runner::CATALOG[0].name));
    assert_eq!(
        native
            .configure_checked(runner::CATALOG[0].name, expected_revision)
            .unwrap(),
        expected_revision + 1
    );
}
fn busy<T: std::fmt::Debug>(result: Result<T>) {
    assert_eq!(
        result.unwrap_err(),
        "App operation failed: BUSY; no automatic resend"
    );
}

#[test]
fn native_v1_management_is_exclusive_with_durable_v2_chat_and_reopens() {
    let fixture = Fixture::new();
    let provider = Arc::new(FakeProvider::default());
    let app = Arc::new(Application::open(&fixture.roots, provider.clone()).unwrap());
    let (mut installer, trace) = native(&app);
    initialize(&mut installer, 0);
    let entered = provider.block();
    let running_id = "a".repeat(32);
    let queued_id = "b".repeat(32);
    submit_new(&app, &running_id, 1);
    provider.wait_entered(entered);
    let queued = submit_new(&app, &queued_id, 1);
    assert_eq!(queued["status"], "queued");
    let before = fixture.disk();
    assert_eq!(before["schema_version"], 10);
    for (id, status) in [(&running_id, "running"), (&queued_id, "queued")] {
        let job = before["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["operation"]["request_id"] == *id)
            .unwrap();
        assert_eq!(job["operation"]["status"], status);
    }
    assert_eq!(installer.configuration().unwrap().revision, 1);
    let projected = installer.app("state", json!({})).unwrap();
    assert_eq!(projected["active"]["request_id"], running_id);
    assert_eq!(projected["active"]["status"], "running");
    assert_eq!(projected["history"], json!([]));
    for id in [&running_id, &queued_id] {
        assert_eq!(
            installer
                .app(
                    "poll",
                    json!({"request_id":id,"instance_id":app.instance_id})
                )
                .unwrap_err(),
            "App operation failed: REQUEST_EXPIRED; no automatic resend",
            "v2 work must not leak a queued status or conversation DTO through v1 poll"
        );
    }
    busy(installer.models());
    busy(installer.configure_checked(runner::CATALOG[0].name, 1));
    busy(installer.configure(runner::CATALOG[0].name));
    busy(installer.model_operation("verify_model", runner::CATALOG[0].name, 1, &|| false));
    busy(installer.model_operation("remove_model", runner::CATALOG[1].name, 1, &|| false));
    assert_eq!(
        fixture.disk(),
        before,
        "BUSY must not admit or mutate global work"
    );
    {
        let gate = provider.gate.lock().unwrap();
        assert_eq!((gate.entered, gate.inventories, gate.removals), (1, 1, 0));
    }
    provider.release();
    finished(&app, 2, &running_id);
    finished(&app, 2, &queued_id);
    installer
        .model_operation("verify_model", runner::CATALOG[0].name, 1, &|| false)
        .unwrap();
    installer
        .model_operation("remove_model", runner::CATALOG[1].name, 1, &|| false)
        .unwrap();
    let terminal = trace
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, p)| {
            r.op == "poll"
                && p.result
                    .as_ref()
                    .is_some_and(|v| v["status"] == "succeeded")
        })
        .map(|(r, p)| (r.args["request_id"].clone(), p.result.clone().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        terminal.len(),
        3,
        "inventory, verification and removal each polled to completion"
    );
    {
        let gate = provider.gate.lock().unwrap();
        assert_eq!((gate.entered, gate.inventories, gate.removals), (3, 2, 1));
    }
    let saved = call(&app, 2, "state", json!({}));
    drop(installer);
    drop(app);
    let reopened = Arc::new(Application::open(&fixture.roots, provider.clone()).unwrap());
    let (mut installer, _) = native(&reopened);
    assert_eq!(installer.configuration().unwrap().revision, 1);
    assert_eq!(
        call(&reopened, 2, "state", json!({}))["conversations"],
        saved["conversations"]
    );
    for (id, expected) in terminal {
        assert_eq!(
            installer
                .app(
                    "poll",
                    json!({"request_id":id,"instance_id":reopened.instance_id})
                )
                .unwrap(),
            expected
        );
    }
    let gate = provider.gate.lock().unwrap();
    assert_eq!(
        (gate.entered, gate.inventories, gate.removals),
        (3, 2, 1),
        "reopen must not replay provider work"
    );
}

#[test]
fn deleting_migrated_legacy_chat_never_remaps_installer_v1_history() {
    let fixture = Fixture::new();
    fixture.seed_legacy();
    let provider = Arc::new(FakeProvider::default());
    let app = Arc::new(Application::open(&fixture.roots, provider.clone()).unwrap());
    let (mut installer, _) = native(&app);
    // Disk1–6 migration advances the configuration revision before installer setup.
    initialize(&mut installer, 1);
    let original = installer.app("state", json!({})).unwrap()["history"].clone();
    assert_eq!(original[0]["content"], "Legacy question");
    assert_eq!(fixture.disk()["schema_version"], 10);
    let legacy = call(&app, 2, "state", json!({}))["conversations"][0].clone();
    let other_id = "c".repeat(32);
    let other = submit_new(&app, &other_id, 2);
    finished(&app, 2, &other_id);
    assert_eq!(
        installer.app("state", json!({})).unwrap()["history"],
        original
    );
    let other_detail = call(
        &app,
        2,
        "conversation_get",
        json!({"conversation_id":other["conversation_id"]}),
    );
    call(
        &app,
        2,
        "conversation_delete",
        json!({"request_id":"d".repeat(32),"conversation_id":legacy["conversation_id"],"expected_metadata_revision":legacy["metadata_revision"],"expected_history_revision":legacy["history_revision"]}),
    );

    assert_eq!(
        installer.app("state", json!({})).unwrap()["history"],
        json!([])
    );
    let error = app.call(request(1, "submit", json!({"request_id":"e".repeat(32),"text":"Must not recreate legacy chat","output":"chat"}))).unwrap_err();
    assert_eq!(error.code, "CONVERSATION_NOT_FOUND");
    drop(installer);
    drop(app);
    let reopened = Arc::new(Application::open(&fixture.roots, provider.clone()).unwrap());
    let (mut installer, _) = native(&reopened);
    assert_eq!(installer.configuration().unwrap().revision, 2);
    assert_eq!(
        installer.app("state", json!({})).unwrap()["history"],
        json!([])
    );
    assert_eq!(
        call(
            &reopened,
            2,
            "conversation_get",
            json!({"conversation_id":other["conversation_id"]})
        ),
        other_detail
    );
    assert_eq!(
        call(&reopened, 2, "state", json!({}))["conversations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let before = fixture.disk();
    let error = reopened.call(request(1, "submit", json!({"request_id":"f".repeat(32),"text":"Still must not recreate legacy chat","output":"chat"}))).unwrap_err();
    assert_eq!(error.code, "CONVERSATION_NOT_FOUND");
    assert_eq!(fixture.disk(), before);
    assert_eq!(provider.gate.lock().unwrap().entered, 1);
}
