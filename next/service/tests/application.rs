use serde_json::{json, Value};
use smith_service::app::{AppError, AppRequest, AppResult, Application, Message, MAX_CONTEXT};
use smith_service::providers::{
    Inventory, Model, Observation, Observations, Provider, SttObservation,
};
use smith_service::Roots;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    roots: Roots,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "smith-app-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        fs::create_dir(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        for p in [
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
        ] {
            fs::create_dir(p).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self { base, roots }
    }
    fn state_path(&self) -> PathBuf {
        self.roots.data.join("smith-next/app-state.json")
    }
    fn write(&self, bytes: &[u8]) {
        fs::write(self.state_path(), bytes).unwrap();
        fs::set_permissions(self.state_path(), fs::Permissions::from_mode(0o600)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn request(op: &str, args: Value) -> AppRequest {
    AppRequest {
        schema_version: 1,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(90000),
        op: op.into(),
        args,
    }
}
fn call(app: &Application, op: &str, args: Value) -> Value {
    app.call(request(op, args)).unwrap()
}
fn poll(app: &Application, n: usize) -> Value {
    call(
        app,
        "poll",
        json!({"request_id":id(n),"instance_id":app.instance_id}),
    )
}
fn finished(app: &Application, n: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let v = poll(app, n);
        if v["status"] != "running" {
            return v;
        }
        assert!(Instant::now() < deadline, "operation completion deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn setup(app: &Application) {
    call(app, "models", json!({"request_id":id(1)}));
    let terminal = finished(app, 1);
    assert_eq!(terminal["status"], "succeeded");
    call(
        app,
        "configure",
        json!({"model":"local:exact","voice":null}),
    );
}
struct Fake {
    inventories: AtomicUsize,
    chats: AtomicUsize,
    missing_speech: bool,
    fail_llm: bool,
    answer: String,
    gated: Mutex<bool>,
    gate: Condvar,
    started: AtomicBool,
    inventory_override: Option<Inventory>,
    messages: Mutex<Vec<Message>>,
    calls: Mutex<Vec<Vec<Message>>>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            inventories: AtomicUsize::new(0),
            chats: AtomicUsize::new(0),
            missing_speech: false,
            fail_llm: false,
            answer: "Assistant answer".into(),
            gated: Mutex::new(false),
            gate: Condvar::new(),
            started: AtomicBool::new(false),
            inventory_override: None,
            messages: Mutex::new(vec![]),
            calls: Mutex::new(vec![]),
        }
    }
}
impl Fake {
    fn release(&self) {
        *self.gated.lock().unwrap() = false;
        self.gate.notify_all();
    }
    fn wait_started(&self) {
        let end = Instant::now() + Duration::from_secs(3);
        while !self.started.load(Ordering::SeqCst) {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Provider for Fake {
    fn inventory(&self) -> AppResult<Inventory> {
        self.inventories.fetch_add(1, Ordering::SeqCst);
        if let Some(inventory) = &self.inventory_override {
            return Ok(inventory.clone());
        }
        Ok(Inventory {
            models: vec![Model {
                name: "local:exact".into(),
                digest: "a".repeat(64),
            }],
            voices: if self.missing_speech {
                vec![]
            } else {
                vec!["observed_voice".into()]
            },
            providers: Observations {
                llm: Observation {
                    ready: true,
                    error: None,
                },
                stt: SttObservation {
                    ready: !self.missing_speech,
                    error: self
                        .missing_speech
                        .then(|| AppError::new("STT_UNAVAILABLE", "Fake STT unavailable")),
                    model: Some("local-whisper".into()),
                },
                tts: Observation {
                    ready: !self.missing_speech,
                    error: self
                        .missing_speech
                        .then(|| AppError::new("TTS_UNAVAILABLE", "Fake TTS unavailable")),
                },
            },
        })
    }
    fn chat(&self, model: &str, messages: &[Message]) -> AppResult<String> {
        assert_eq!(model, "local:exact");
        assert!(messages.iter().map(|m| m.content.len()).sum::<usize>() <= MAX_CONTEXT);
        *self.messages.lock().unwrap() = messages.to_vec();
        self.calls.lock().unwrap().push(messages.to_vec());
        self.chats.fetch_add(1, Ordering::SeqCst);
        self.started.store(true, Ordering::SeqCst);
        let mut blocked = self.gated.lock().unwrap();
        while *blocked {
            blocked = self.gate.wait(blocked).unwrap();
        }
        if self.fail_llm {
            Err(AppError::new("LLM_UNAVAILABLE", "fake generation failure"))
        } else {
            Ok(self.answer.clone())
        }
    }
}

#[test]
fn model_verification_is_durable_bound_and_does_not_append_history() {
    let f = Fixture::new();
    let provider = Arc::new(Fake::default());
    let app = Application::open(&f.roots, provider.clone()).unwrap();
    setup(&app);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"prior turn","output":"chat"}),
    );
    finished(&app, 2);
    let before = call(&app, "state", json!({}));
    let args = json!({"request_id":id(3),"model":"local:exact","sha256":"a".repeat(64),"expected_revision":1});
    call(&app, "verify_model", args.clone());
    let terminal = finished(&app, 3);
    let messages = provider.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Say hello in one short sentence.");
    drop(messages);
    assert_eq!(
        terminal["result"],
        json!({"model":"local:exact","sha256":"a".repeat(64),"verified":true,"assistant_text":"Assistant answer"})
    );
    assert_eq!(call(&app, "state", json!({}))["history"], before["history"]);
    call(&app, "verify_model", args.clone());
    assert_eq!(provider.chats.load(Ordering::SeqCst), 2);
    let mut changed = args.clone();
    changed["sha256"] = json!("b".repeat(64));
    assert_eq!(
        app.call(request("verify_model", changed)).unwrap_err().code,
        "ID_CONFLICT"
    );
    drop(app);
    let reopened = Application::open(&f.roots, provider.clone()).unwrap();
    call(&reopened, "verify_model", args);
    assert_eq!(finished(&reopened, 3), terminal);
    assert_eq!(provider.chats.load(Ordering::SeqCst), 2);
}

#[test]
fn successful_removal_uses_serial_worker_and_durable_duplicate_result() {
    struct Remover {
        fake: Fake,
        removals: AtomicUsize,
    }
    impl Provider for Remover {
        fn inventory(&self) -> AppResult<Inventory> {
            self.fake.inventory()
        }
        fn chat(&self, model: &str, messages: &[Message]) -> AppResult<String> {
            self.fake.chat(model, messages)
        }
        fn remove_model(&self, model: &str, sha: &str) -> AppResult<()> {
            assert_eq!(model, "other:exact");
            assert_eq!(sha, "b".repeat(64));
            self.removals.fetch_add(1, Ordering::SeqCst);
            self.fake.started.store(true, Ordering::SeqCst);
            let mut blocked = self.fake.gated.lock().unwrap();
            while *blocked {
                blocked = self.fake.gate.wait(blocked).unwrap();
            }
            Ok(())
        }
    }
    let f = Fixture::new();
    let provider = Arc::new(Remover {
        fake: Fake {
            gated: Mutex::new(true),
            ..Fake::default()
        },
        removals: AtomicUsize::new(0),
    });
    let app = Application::open(&f.roots, provider.clone()).unwrap();
    setup(&app);
    let before = call(&app, "state", json!({}));
    let args = json!({"request_id":id(3),"model":"other:exact","sha256":"b".repeat(64),"expected_revision":1});
    call(&app, "remove_model", args.clone());
    provider.fake.wait_started();
    assert_eq!(poll(&app, 3)["status"], "running");
    assert_eq!(
        call(&app, "state", json!({}))["active"]["kind"],
        "remove_model"
    );
    assert_eq!(
        app.call(request(
            "configure",
            json!({"model":"local:exact","voice":null})
        ))
        .unwrap_err()
        .code,
        "BUSY"
    );
    assert_eq!(
        app.call(request("models", json!({"request_id":id(4)})))
            .unwrap_err()
            .code,
        "BUSY"
    );
    call(&app, "remove_model", args.clone());
    assert_eq!(provider.removals.load(Ordering::SeqCst), 1);
    provider.fake.release();
    let terminal = finished(&app, 3);
    assert_eq!(
        terminal["result"],
        json!({"model":"other:exact","sha256":"b".repeat(64),"removed":true})
    );
    let after = call(&app, "state", json!({}));
    assert_eq!(after["history"], before["history"]);
    assert_eq!(after["config"], before["config"]);
    drop(app);
    let app = Application::open(&f.roots, provider.clone()).unwrap();
    call(&app, "remove_model", args);
    assert_eq!(finished(&app, 3), terminal);
    assert_eq!(provider.removals.load(Ordering::SeqCst), 1);
}

#[test]
fn new_running_model_kinds_interrupt_without_replay_on_restart() {
    for (kind, phase) in [
        ("verify_model", "verification"),
        ("remove_model", "removal"),
    ] {
        let f = Fixture::new();
        let provider = Arc::new(Fake::default());
        let app = Application::open(&f.roots, provider.clone()).unwrap();
        setup(&app);
        drop(app);
        let mut state: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
        state["jobs"].as_array_mut().unwrap().push(json!({
            "fingerprint":"b".repeat(64),
            "version":1,"conversation_id":null,"history_revision":null,"task":null,"accepted_ms":0,
            "operation":{"request_id":id(9),"kind":kind,"status":"running","phase":phase,"config_revision":1,"result":null,"error":null}
        }));
        f.write(&serde_json::to_vec(&state).unwrap());
        let app = Application::open(&f.roots, provider.clone()).unwrap();
        let operation = finished(&app, 9);
        assert_eq!(operation["status"], "interrupted");
        assert_eq!(operation["error"]["code"], "INTERRUPTED");
        assert_eq!(provider.chats.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn checked_configuration_and_removal_fail_closed_before_dispatch() {
    let f = Fixture::new();
    let app = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    setup(&app);
    let checked = json!({"model":"local:exact","voice":null,"expected_revision":0});
    assert_eq!(
        app.call(request("configure_checked", checked))
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert!(app
        .call(request(
            "configure",
            json!({"model":"local:exact","voice":null,"expected_revision":1})
        ))
        .is_err());
    call(
        &app,
        "configure_checked",
        json!({"model":"local:exact","voice":null,"expected_revision":1}),
    );
    let mut args = json!({"request_id":id(4),"model":"local:exact","sha256":"a".repeat(64),"expected_revision":1});
    assert_eq!(
        app.call(request("remove_model", args.clone()))
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    args["expected_revision"] = json!(2);
    assert_eq!(
        app.call(request("remove_model", args.clone()))
            .unwrap_err()
            .code,
        "MODEL_SELECTED"
    );
    args["model"] = json!("other:exact");
    call(&app, "remove_model", args);
    assert_eq!(finished(&app, 4)["error"]["code"], "UNSUPPORTED");
    assert_eq!(call(&app, "state", json!({}))["config"]["revision"], 2);
}

#[test]
fn verification_rejects_empty_final_and_wrong_digest() {
    for answer in ["", "   "] {
        let f = Fixture::new();
        let app = Application::open(
            &f.roots,
            Arc::new(Fake {
                answer: answer.into(),
                ..Fake::default()
            }),
        )
        .unwrap();
        setup(&app);
        call(
            &app,
            "verify_model",
            json!({"request_id":id(3),"model":"local:exact","sha256":"a".repeat(64),"expected_revision":1}),
        );
        assert_eq!(finished(&app, 3)["error"]["code"], "MODEL_RESULT_LIMIT");
        call(
            &app,
            "verify_model",
            json!({"request_id":id(4),"model":"local:exact","sha256":"b".repeat(64),"expected_revision":1}),
        );
        assert_eq!(finished(&app, 4)["error"]["code"], "MODEL_MISMATCH");
        assert_eq!(call(&app, "state", json!({}))["history"], json!([]));
    }
}

#[test]
fn dropped_gated_worker_cannot_publish_into_reopened_roots() {
    let f = Fixture::new();
    let old_provider = Arc::new(Fake {
        gated: Mutex::new(true),
        ..Fake::default()
    });
    let old = Application::open(&f.roots, old_provider.clone()).unwrap();
    setup(&old);
    call(
        &old,
        "submit",
        json!({"request_id":id(2),"text":"old pending input","output":"chat"}),
    );
    old_provider.wait_started();
    let last_handle = old.clone();
    drop(old);
    assert_eq!(poll(&last_handle, 2)["status"], "running");
    let started = Instant::now();
    drop(last_handle);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "shutdown must not wait for a blocked provider"
    );
    let new_provider = Arc::new(Fake {
        answer: "New owner answer".into(),
        ..Fake::default()
    });
    let new = Application::open(&f.roots, new_provider).unwrap();
    assert_eq!(poll(&new, 2)["status"], "interrupted");
    call(&new, "models", json!({"request_id":id(3)}));
    finished(&new, 3);
    call(
        &new,
        "configure",
        json!({"model":"local:exact","voice":null}),
    );
    call(
        &new,
        "submit",
        json!({"request_id":id(4),"text":"new owner input","output":"chat"}),
    );
    finished(&new, 4);
    let expected_bytes = fs::read(f.state_path()).unwrap();
    let expected_state = call(&new, "state", json!({}));
    old_provider.release();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Arc::strong_count(&old_provider) != 1 {
        assert!(
            Instant::now() < deadline,
            "old worker did not finish after release"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(fs::read(f.state_path()).unwrap(), expected_bytes);
    assert_eq!(call(&new, "state", json!({})), expected_state);
    assert_eq!(expected_state["config"]["revision"], 2);
    assert_eq!(expected_state["history"][1]["content"], "New owner answer");
    drop(new);
    let restored = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(
        call(&restored, "state", json!({}))["config"],
        expected_state["config"]
    );
    assert_eq!(poll(&restored, 2)["status"], "interrupted");
    assert_eq!(
        poll(&restored, 4)["result"]["assistant_text"],
        "New owner answer"
    );
}
#[test]
fn multiple_running_jobs_are_corrupt_before_reconciliation_and_preserved() {
    let source = Fixture::new();
    let app = Application::open(&source.roots, Arc::new(Fake::default())).unwrap();
    for n in 1..=2 {
        call(&app, "models", json!({"request_id":id(n)}));
        finished(&app, n);
    }
    let mut disk: Value = serde_json::from_slice(&fs::read(source.state_path()).unwrap()).unwrap();
    for job in disk["jobs"].as_array_mut().unwrap() {
        job["operation"]["status"] = json!("running");
        job["operation"]["phase"] = json!("inventory");
        job["operation"]["result"] = Value::Null;
        job["operation"]["error"] = Value::Null;
    }
    let target = Fixture::new();
    let bytes = serde_json::to_vec(&disk).unwrap();
    target.write(&bytes);
    assert!(Application::open(&target.roots, Arc::new(Fake::default())).is_err());
    assert_eq!(fs::read(target.state_path()).unwrap(), bytes);
}
#[test]
fn nested_inventory_semantic_type_and_size_corruption_is_preserved() {
    let source = Fixture::new();
    let app = Application::open(&source.roots, Arc::new(Fake::default())).unwrap();
    setup(&app);
    let disk: Value = serde_json::from_slice(&fs::read(source.state_path()).unwrap()).unwrap();
    let baseline = json!(Fake::default().inventory().unwrap());
    let mut cases = Vec::new();
    for (pointer, value) in [
        ("/models/0/digest", json!("z".repeat(64))),
        ("/models/0/name", json!("cloud-model:tag")),
        ("/models/0/extra", json!(true)),
        ("/models", json!(vec![baseline["models"][0].clone(); 257])),
        (
            "/models",
            json!([baseline["models"][0], baseline["models"][0]]),
        ),
        ("/voices", json!([""])),
        ("/voices", json!([])),
        ("/voices", json!([{"name":"voice"}])),
        ("/voices/0", json!("\n")),
        ("/voices", json!(["duplicate", "duplicate"])),
        ("/voices", json!(vec!["voice"; 257])),
        ("/voices/0", json!("v".repeat(257))),
        ("/providers/stt/model", json!("m".repeat(257))),
        ("/providers/stt/model", Value::Null),
        ("/providers/stt/model", json!("\n")),
        ("/providers/stt/ready", json!("true")),
        ("/providers/tts/extra", json!(true)),
    ] {
        let mut invalid = baseline.clone();
        set_pointer(&mut invalid, pointer, value);
        cases.push(invalid);
    }
    for provider in ["llm", "stt", "tts"] {
        for error in [
            json!({"code":"BAD","message":"e".repeat(5*1024*1024)}),
            json!({"code":"BAD","message":"e".repeat(1025)}),
            json!({"code":"","message":"bad"}),
            json!({"code":"lowercase","message":"bad"}),
            json!({"code":"BAD","message":""}),
            json!({"code":5,"message":"bad"}),
            json!({"code":"BAD","message":"bad","extra":true}),
            Value::Null,
        ] {
            let mut invalid = baseline.clone();
            invalid["providers"][provider]["ready"] = json!(false);
            invalid["providers"][provider]["error"] = error;
            if provider == "llm" {
                invalid["models"] = json!([]);
            }
            if provider == "tts" {
                invalid["voices"] = json!([]);
            }
            cases.push(invalid);
        }
        let mut invalid = baseline.clone();
        invalid["providers"][provider]["error"] =
            json!({"code":"BAD","message":"Ready provider cannot also fail"});
        cases.push(invalid);
        let mut missing = baseline.clone();
        missing["providers"][provider]
            .as_object_mut()
            .unwrap()
            .remove("error");
        cases.push(missing);
        if provider == "stt" {
            let mut missing = baseline.clone();
            missing["providers"][provider]
                .as_object_mut()
                .unwrap()
                .remove("model");
            cases.push(missing);
        }
        if provider != "stt" {
            let mut inconsistent = baseline.clone();
            inconsistent["providers"][provider]["ready"] = json!(false);
            inconsistent["providers"][provider]["error"] =
                json!({"code":"NOT_READY","message":"Unavailable"});
            cases.push(inconsistent);
        }
    }
    for invalid in cases {
        let target = Fixture::new();
        let mut bad = disk.clone();
        bad["jobs"][0]["operation"]["result"] = invalid;
        let bytes = serde_json::to_vec(&bad).unwrap();
        target.write(&bytes);
        assert!(Application::open(&target.roots, Arc::new(Fake::default())).is_err());
        assert_eq!(fs::read(target.state_path()).unwrap(), bytes);
    }
}
fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let target = value.pointer_mut(parent).unwrap();
    if let Some(array) = target.as_array_mut() {
        array[key.parse::<usize>().unwrap()] = replacement;
    } else {
        target[key] = replacement;
    }
}
#[test]
fn invalid_live_inventory_is_bounded_failure_not_persisted_corruption() {
    for huge_error in [false, true] {
        let mut inventory = Fake::default().inventory().unwrap();
        if huge_error {
            inventory.providers.stt.ready = false;
            inventory.providers.stt.error =
                Some(AppError::new("BAD", &"e".repeat(5 * 1024 * 1024)));
        } else {
            inventory.models[0].digest = "z".repeat(64);
        }
        let f = Fixture::new();
        let app = Application::open(
            &f.roots,
            Arc::new(Fake {
                inventory_override: Some(inventory),
                ..Fake::default()
            }),
        )
        .unwrap();
        call(&app, "models", json!({"request_id":id(1)}));
        let operation = finished(&app, 1);
        assert_eq!(operation["status"], "failed");
        assert_eq!(operation["error"]["code"], "PROVIDER_PROTOCOL");
        let reply =
            smith_service::app::AppReply::from_result(&app.instance_id, &id(9), Ok(operation));
        assert!(serde_json::to_vec(&reply).unwrap().len() <= smith_service::app::FRAME_LIMIT);
        let restored = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
        assert_eq!(poll(&restored, 1)["status"], "failed");
        assert!(call(&restored, "state", json!({}))["active"].is_null());
    }
}
#[test]
fn maximal_valid_inventory_metadata_stays_pollable_across_restart() {
    let mut inventory = Fake::default().inventory().unwrap();
    inventory.models = (0..256)
        .map(|n| Model {
            name: format!("{}{:03}:exact", "m".repeat(245), n),
            digest: "A".repeat(64),
        })
        .collect();
    inventory.voices = (0..256)
        .map(|n| format!("{}{:04}", "v".repeat(252), n))
        .collect();
    inventory.providers.stt = SttObservation {
        ready: false,
        error: Some(AppError::new(&"A".repeat(256), &"\"".repeat(1024))),
        model: Some("m".repeat(256)),
    };
    let legacy = Fixture::new();
    let mut disk = legacy_state();
    disk["jobs"][0]["operation"]["result"] = json!(inventory);
    legacy.write(&serde_json::to_vec(&disk).unwrap());
    let preserved = Application::open(&legacy.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(poll(&preserved, 1)["result"], json!(inventory));
    assert_eq!(
        poll(&preserved, 1)["result"]["voices"]
            .as_array()
            .unwrap()
            .len(),
        256
    );
    let legacy_reply = smith_service::app::AppReply::from_result(
        &preserved.instance_id,
        &id(9),
        Ok(poll(&preserved, 1)),
    );
    assert!(serde_json::to_vec(&legacy_reply).unwrap().len() <= smith_service::app::FRAME_LIMIT);
    let f = Fixture::new();
    let app = Application::open(
        &f.roots,
        Arc::new(Fake {
            inventory_override: Some(inventory),
            ..Fake::default()
        }),
    )
    .unwrap();
    call(&app, "models", json!({"request_id":id(1)}));
    assert_eq!(finished(&app, 1)["status"], "succeeded");
    drop(app);
    let restored = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    for value in [poll(&restored, 1), call(&restored, "state", json!({}))] {
        let reply =
            smith_service::app::AppReply::from_result(&restored.instance_id, &id(9), Ok(value));
        assert!(serde_json::to_vec(&reply).unwrap().len() <= smith_service::app::FRAME_LIMIT);
    }
    assert_eq!(
        poll(&restored, 1)["result"]["models"]
            .as_array()
            .unwrap()
            .len(),
        256
    );
}
#[test]
fn operation_and_speech_errors_require_bounded_semantics_and_nullable_fields() {
    let source = Fixture::new();
    let app = Application::open(
        &source.roots,
        Arc::new(Fake {
            missing_speech: true,
            ..Fake::default()
        }),
    )
    .unwrap();
    setup(&app);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"answer","output":"chat"}),
    );
    finished(&app, 2);
    let mut baseline: Value =
        serde_json::from_slice(&fs::read(source.state_path()).unwrap()).unwrap();
    baseline["jobs"][1]["operation"]["result"]["speech_error"] =
        json!({"code":"AUDIO_EXPIRED","message":"Audio was transient"});
    for (pointer, value) in [
        ("/result/speech_error/code", json!("")),
        ("/result/speech_error/code", json!("5")),
        ("/result/speech_error/message", json!("e".repeat(1025))),
        (
            "/error",
            json!({"code":"BAD","message":"e".repeat(5*1024*1024)}),
        ),
    ] {
        let target = Fixture::new();
        let mut disk = baseline.clone();
        set_pointer(&mut disk["jobs"][1]["operation"], pointer, value);
        let bytes = serde_json::to_vec(&disk).unwrap();
        target.write(&bytes);
        assert!(Application::open(&target.roots, Arc::new(Fake::default())).is_err());
        assert_eq!(fs::read(target.state_path()).unwrap(), bytes);
    }
    for (parent, key) in [
        ("/jobs/1/operation", "error"),
        ("/jobs/1/operation", "result"),
        ("/jobs/1/operation/result", "audio"),
        ("/jobs/1/operation/result", "speech_error"),
    ] {
        let target = Fixture::new();
        let mut disk = baseline.clone();
        disk.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        let bytes = serde_json::to_vec(&disk).unwrap();
        target.write(&bytes);
        assert!(Application::open(&target.roots, Arc::new(Fake::default())).is_err());
        assert_eq!(fs::read(target.state_path()).unwrap(), bytes);
    }
}
#[test]
fn durable_identifier_capacity_is_explicit_not_silent_forgetting() {
    let f = Fixture::new();
    let app = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    for n in 1..=32 {
        call(&app, "models", json!({"request_id":id(n)}));
        finished(&app, n);
    }
    let mut disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    disk["expired_ids"] = json!((10000..11024).map(id).collect::<Vec<_>>());
    let restart = Fixture::new();
    restart.write(&serde_json::to_vec(&disk).unwrap());
    let restored = Application::open(&restart.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(
        restored
            .call(request("models", json!({"request_id":id(33)})))
            .unwrap_err()
            .code,
        "CAPACITY"
    );
    assert_eq!(
        call(&restored, "models", json!({"request_id":id(32)}))["status"],
        "succeeded"
    );
    assert_eq!(
        restored
            .call(request("models", json!({"request_id":id(10000)})))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    assert!(call(&restored, "state", json!({}))["active"].is_null());
}
#[test]
fn corrupt_nested_result_and_inconsistent_operation_status_are_preserved() {
    let f = Fixture::new();
    let app = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    setup(&app);
    let disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    for (key, value) in [
        ("phase", json!("generation")),
        ("status", json!("unknown")),
        ("result", json!({"audio":{"data_base64":"should-not-load"}})),
    ] {
        let target = Fixture::new();
        let mut bad = disk.clone();
        bad["jobs"][0]["operation"][key] = value;
        let bytes = serde_json::to_vec(&bad).unwrap();
        target.write(&bytes);
        assert!(Application::open(&target.roots, Arc::new(Fake::default())).is_err());
        assert_eq!(fs::read(target.state_path()).unwrap(), bytes);
    }
}
#[test]
fn long_text_answer_is_not_truncated_or_regenerated() {
    let f = Fixture::new();
    let fake = Arc::new(Fake {
        answer: "a".repeat(4097),
        ..Fake::default()
    });
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"hello","output":"chat"}),
    );
    let job = finished(&app, 2);
    assert_eq!(job["status"], "succeeded");
    assert!(job["result"]["speech_error"].is_null());
    assert_eq!(
        job["result"]["assistant_text"].as_str().unwrap().len(),
        4097
    );
    assert_eq!(fake.chats.load(Ordering::SeqCst), 1);
}
#[test]
fn active_worker_keeps_state_poll_responsive_and_blocks_queue_and_config() {
    let f = Fixture::new();
    let fake = Arc::new(Fake {
        gated: Mutex::new(true),
        ..Fake::default()
    });
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app);
    let args = json!({"request_id":id(2),"text":"exact","output":"chat"});
    call(&app, "submit", args.clone());
    fake.wait_started();
    let end = Instant::now();
    assert_eq!(
        call(&app, "state", json!({}))["active"]["phase"],
        "generation"
    );
    assert_eq!(poll(&app, 2)["status"], "running");
    assert!(end.elapsed() < Duration::from_secs(1));
    assert_eq!(call(&app, "submit", args.clone())["status"], "running");
    assert_eq!(
        app.call(request(
            "submit",
            json!({"request_id":id(2),"text":"changed","output":"chat"})
        ))
        .unwrap_err()
        .code,
        "ID_CONFLICT"
    );
    assert_eq!(
        app.call(request("models", json!({"request_id":id(3)})))
            .unwrap_err()
            .code,
        "BUSY"
    );
    assert_eq!(
        app.call(request(
            "configure",
            json!({"model":"local:exact","voice":null})
        ))
        .unwrap_err()
        .code,
        "BUSY"
    );
    let disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    assert_eq!(disk["jobs"][1]["operation"]["status"], "running");
    fake.release();
    assert_eq!(finished(&app, 2)["status"], "succeeded");
    assert_eq!(call(&app, "submit", args)["status"], "succeeded");
    assert_eq!(fake.chats.load(Ordering::SeqCst), 1);
}
#[test]
fn restart_interrupts_pending_preserves_terminal_and_never_replays() {
    let f = Fixture::new();
    let fake = Arc::new(Fake {
        gated: Mutex::new(true),
        ..Fake::default()
    });
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app);
    let args = json!({"request_id":id(2),"text":"pending","output":"chat"});
    call(&app, "submit", args.clone());
    fake.wait_started();
    let snapshot = fs::read(f.state_path()).unwrap();
    let restart = Fixture::new();
    restart.write(&snapshot);
    let after = Arc::new(Fake::default());
    let restored = Application::open(&restart.roots, after.clone()).unwrap();
    assert_ne!(app.instance_id, restored.instance_id);
    assert_eq!(poll(&restored, 2)["status"], "interrupted");
    assert_eq!(poll(&restored, 2)["error"]["code"], "INTERRUPTED");
    assert_eq!(poll(&restored, 1)["status"], "succeeded");
    assert_eq!(call(&restored, "submit", args)["status"], "interrupted");
    assert_eq!(after.chats.load(Ordering::SeqCst), 0);
    assert_eq!(
        restored
            .call(request(
                "poll",
                json!({"request_id":id(2),"instance_id":app.instance_id})
            ))
            .unwrap_err()
            .code,
        "INSTANCE_MISMATCH"
    );
    fake.release();
    finished(&app, 2);
}
#[test]
fn history_is_bounded_in_complete_pairs_and_evicted_ids_cannot_resurrect() {
    let f = Fixture::new();
    let app = Application::open(
        &f.roots,
        Arc::new(Fake {
            answer: "a".repeat(4000),
            ..Fake::default()
        }),
    )
    .unwrap();
    setup(&app);
    for n in 2..36 {
        call(
            &app,
            "submit",
            json!({"request_id":id(n),"text":format!("turn {n}"),"output":"chat"}),
        );
        finished(&app, n);
    }
    let state = call(&app, "state", json!({}));
    let history = state["history"].as_array().unwrap();
    assert_eq!(history.len() % 2, 0);
    assert!(history.len() <= 32);
    assert!(
        history
            .iter()
            .map(|v| v["content"].as_str().unwrap().len())
            .sum::<usize>()
            <= MAX_CONTEXT
    );
    assert_eq!(history.last().unwrap()["role"], "assistant");
    assert_eq!(
        app.call(request(
            "poll",
            json!({"request_id":id(1),"instance_id":app.instance_id})
        ))
        .unwrap_err()
        .code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(
        app.call(request("models", json!({"request_id":id(1)})))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    let restored = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(
        call(&restored, "state", json!({}))["history"],
        state["history"]
    );
    assert_eq!(
        restored
            .call(request("models", json!({"request_id":id(1)})))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
}
#[test]
fn failed_generation_does_not_append_partial_history() {
    let f = Fixture::new();
    let fake = Arc::new(Fake {
        fail_llm: true,
        ..Fake::default()
    });
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"no partial","output":"chat"}),
    );
    assert_eq!(finished(&app, 2)["status"], "failed");
    assert_eq!(call(&app, "state", json!({}))["history"], json!([]));
}
#[test]
fn strict_requests_ids_exact_tags_and_text_limits() {
    let f = Fixture::new();
    let app = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(
        app.call(request("state", json!({"unknown":true})))
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    assert_eq!(
        app.call(request("arbitrary_tool", json!({})))
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    let mut req = request("state", json!({}));
    req.schema_version = 3;
    assert!(app.call(req).is_err());
    for bad in [
        "ABCDEFABCDEFABCDEFABCDEFABCDEFABCD",
        "00000000-0000-0000-0000-000000000000",
        "short",
    ] {
        assert!(app
            .call(request("models", json!({"request_id":bad})))
            .is_err());
    }
    for (op, args) in [
        (
            "submit",
            json!({"request_id":id(2),"text":"a".repeat(16385),"output":"chat"}),
        ),
        ("speak", json!({"request_id":id(2),"text":"a".repeat(4097)})),
        (
            "submit",
            json!({"request_id":id(2),"text":"hello","output":"tools"}),
        ),
    ] {
        assert!(app.call(request(op, args)).is_err());
    }
    setup(&app);
    assert_eq!(
        app.call(request("configure", json!({"model":"local","voice":null})))
            .unwrap_err()
            .code,
        "MODEL_UNAVAILABLE"
    );
    assert_eq!(
        app.call(request(
            "configure",
            json!({"model":"local:exact","voice":"invented"})
        ))
        .unwrap_err()
        .code,
        "RETIRED"
    );
    let raw = json!({"schema_version":1,"service":"smith-next","protocol":"app","call_id":id(1),"op":"state","args":{},"extra":0});
    assert!(serde_json::from_value::<AppRequest>(raw).is_err());
}
#[test]
fn corrupt_unsupported_symlink_and_unsafe_app_state_preserved() {
    for bytes in [b"not json".as_slice(),br#"{"schema_version":99,"config":{"model":null,"voice":null,"revision":0},"history":[],"jobs":[],"expired_ids":[]}"#.as_slice()] {
        let f=Fixture::new();f.write(bytes);assert!(Application::open(&f.roots,Arc::new(Fake::default())).is_err());assert_eq!(fs::read(f.state_path()).unwrap(),bytes);
    }
    let f = Fixture::new();
    let target = f.base.join("foreign");
    fs::write(&target, b"preserve").unwrap();
    symlink(&target, f.state_path()).unwrap();
    assert!(Application::open(&f.roots, Arc::new(Fake::default())).is_err());
    assert_eq!(fs::read(target).unwrap(), b"preserve");
    let f = Fixture::new();
    f.write(b"{}");
    fs::set_permissions(f.state_path(), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Application::open(&f.roots, Arc::new(Fake::default())).is_err());
}
#[test]
fn unsafe_destination_change_blocks_admission_without_inference() {
    let f = Fixture::new();
    let fake = Arc::new(Fake::default());
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app);
    fs::set_permissions(f.state_path(), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        app.call(request(
            "submit",
            json!({"request_id":id(2),"text":"not accepted","output":"chat"})
        ))
        .unwrap_err()
        .code,
        "STATE_UNAVAILABLE"
    );
    assert_eq!(fake.chats.load(Ordering::SeqCst), 0);
    assert_eq!(
        app.call(request("state", json!({}))).unwrap_err().code,
        "STATE_UNAVAILABLE"
    );
}
fn request2(op: &str, args: Value) -> AppRequest {
    let mut r = request(op, args);
    r.schema_version = 2;
    r
}
fn call2(app: &Application, op: &str, args: Value) -> Value {
    app.call(request2(op, args)).unwrap()
}
fn done2(app: &Application, n: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let v = call2(
            app,
            "poll",
            json!({"request_id":id(n),"instance_id":app.instance_id}),
        );
        if !["running", "queued"].contains(&v["status"].as_str().unwrap()) {
            return v;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn new2(app: &Application, n: usize, text: &str) -> Value {
    call2(
        app,
        "submit_new",
        json!({"request_id":id(n),"text":text,"output":"chat","expected_config_revision":1}),
    )
}
fn get2(app: &Application, target: &Value) -> Value {
    call2(app, "conversation_get", json!({"conversation_id":target}))
}

#[test]
fn multi_chat_title_derived_once_preserves_full_prompt_and_restart() {
    let f = Fixture::new();
    let provider = Arc::new(Fake::default());
    let app = Application::open(&f.roots, provider.clone()).unwrap();
    setup(&app);
    let prompt = format!(" \tHello\n\u{202e}{} original suffix\r\n", "🦀".repeat(45));
    let expected = format!("Hello {}", "🦀".repeat(34));
    let a = new2(&app, 100, &prompt);
    assert_eq!(done2(&app, 100)["status"], "succeeded");
    let target = &a["conversation_id"];
    let detail = get2(&app, target);
    assert_eq!(detail["conversation"]["title"], expected);
    assert_eq!(detail["history"][0]["content"], prompt);
    assert_eq!(provider.calls.lock().unwrap()[0][0].content, prompt);
    let disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    let job = disk["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["operation"]["request_id"] == id(100))
        .unwrap();
    assert_eq!(job["task"]["Submit"]["text"], prompt);
    let replay = new2(&app, 100, &prompt);
    assert_eq!(replay["conversation_id"], *target);
    assert_eq!(provider.chats.load(Ordering::SeqCst), 1);
    call2(
        &app,
        "submit",
        json!({"request_id":id(101),"conversation_id":target,
        "text":"Followup must not rename","output":"chat","expected_config_revision":1,"expected_history_revision":1}),
    );
    done2(&app, 101);
    assert_eq!(get2(&app, target)["conversation"]["title"], expected);
    drop(app);
    let app = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(get2(&app, target)["conversation"]["title"], expected);
}

#[test]
fn multi_chat_atomic_admission_fifo_capacity_and_isolated_context() {
    let f = Fixture::new();
    let provider = Arc::new(Fake::default());
    let app = Application::open(&f.roots, provider.clone()).unwrap();
    setup(&app);
    *provider.gated.lock().unwrap() = true;
    let a = new2(&app, 100, "private A");
    provider.wait_started();
    let b = new2(&app, 101, "private B");
    let c = new2(&app, 102, "private C");
    let d = new2(&app, 103, "private D");
    for v in [&b, &c, &d] {
        assert_eq!(v["status"], "queued");
    }
    assert_eq!(provider.chats.load(Ordering::SeqCst), 1);
    let before = fs::read(f.state_path()).unwrap();
    assert_eq!(
        app.call(request2(
            "submit_new",
            json!({"request_id":id(104),"text":"E","output":"chat","expected_config_revision":1})
        ))
        .unwrap_err()
        .code,
        "BUSY"
    );
    assert_eq!(before, fs::read(f.state_path()).unwrap());
    assert_eq!(
        call2(&app, "state", json!({}))["conversations"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    for req in [
        request("models", json!({"request_id":id(105)})),
        request("configure", json!({"model":"local:exact","voice":null})),
        request(
            "submit",
            json!({"request_id":id(106),"text":"v1","output":"chat"}),
        ),
    ] {
        assert_eq!(app.call(req).unwrap_err().code, "BUSY");
    }
    provider.release();
    for n in 100..104 {
        assert_eq!(done2(&app, n)["status"], "succeeded");
    }
    let calls = provider.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    for (i, s) in ["private A", "private B", "private C", "private D"]
        .iter()
        .enumerate()
    {
        assert_eq!(calls[i].len(), 1);
        assert_eq!(calls[i][0].content, *s);
    }
    drop(calls);
    let target = &a["conversation_id"];
    call2(
        &app,
        "submit",
        json!({"request_id":id(107),"conversation_id":target,"text":"A followup","output":"chat","expected_config_revision":1,"expected_history_revision":1}),
    );
    done2(&app, 107);
    let calls = provider.calls.lock().unwrap();
    assert_eq!(calls[4].len(), 3);
    assert_eq!(calls[4][0].content, "private A");
    assert_eq!(get2(&app, target)["conversation"]["history_revision"], 2);
}
#[test]
fn multi_chat_lifecycle_receipts_delete_residue_and_no_missing_target_alias() {
    let f = Fixture::new();
    let provider = Arc::new(Fake::default());
    let app = Application::open(&f.roots, provider.clone()).unwrap();
    setup(&app);
    let a = new2(&app, 100, "erase this prompt");
    done2(&app, 100);
    let target = a["conversation_id"].clone();
    let b = new2(&app, 101, "keep this prompt");
    done2(&app, 101);
    let rename = json!({"request_id":id(102),"conversation_id":target,"expected_metadata_revision":0,"title":"Erase this title"});
    let receipt = call2(&app, "conversation_rename", rename.clone());
    assert_eq!(receipt, call2(&app, "conversation_rename", rename));
    assert_eq!(done2(&app, 102)["result"], receipt);
    call2(
        &app,
        "conversation_archive",
        json!({"request_id":id(103),"conversation_id":target,"expected_metadata_revision":1}),
    );
    assert_eq!(app.call(request2("submit",json!({"request_id":id(104),"conversation_id":target,"text":"no","output":"chat","expected_config_revision":1,"expected_history_revision":1}))).unwrap_err().code,"CONVERSATION_ARCHIVED");
    call2(
        &app,
        "conversation_restore",
        json!({"request_id":id(105),"conversation_id":target,"expected_metadata_revision":2}),
    );
    let stale = json!({"request_id":id(106),"conversation_id":target,"expected_metadata_revision":2,"expected_history_revision":1});
    assert_eq!(
        app.call(request2("conversation_delete", stale))
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let delete = json!({"request_id":id(107),"conversation_id":target,"expected_metadata_revision":3,"expected_history_revision":1});
    let receipt = call2(&app, "conversation_delete", delete.clone());
    assert_eq!(
        receipt,
        json!({"request_id":id(107),"conversation_id":target,"deleted":true})
    );
    assert_eq!(receipt, call2(&app, "conversation_delete", delete));
    assert_eq!(done2(&app, 107)["result"], receipt);
    assert!(app
        .call(request2(
            "conversation_get",
            json!({"conversation_id":target})
        ))
        .is_err());
    assert_eq!(
        get2(&app, &b["conversation_id"])["history"][0]["content"],
        "keep this prompt"
    );
    let bytes = fs::read_to_string(f.state_path()).unwrap();
    assert!(!bytes.contains("erase this prompt"));
    assert!(!bytes.contains("Erase this title"));
    let disk: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(disk["expired_ids"].as_array().unwrap().len(), 4);
    assert_eq!(app.call(request2("submit_new",json!({"request_id":id(100),"text":"erase this prompt","output":"chat","expected_config_revision":1}))).unwrap_err().code,"REQUEST_EXPIRED");
    drop(app);
    let app = Application::open(&f.roots, provider).unwrap();
    assert_eq!(done2(&app, 107)["result"], receipt);
}
#[test]
fn multi_chat_target_busy_rename_and_unrelated_management() {
    let f = Fixture::new();
    let p = Arc::new(Fake::default());
    let app = Application::open(&f.roots, p.clone()).unwrap();
    setup(&app);
    let a = new2(&app, 100, "A");
    done2(&app, 100);
    *p.gated.lock().unwrap() = true;
    let b = new2(&app, 101, "B");
    call2(
        &app,
        "conversation_archive",
        json!({"request_id":id(102),"conversation_id":a["conversation_id"],"expected_metadata_revision":0}),
    );
    for op in ["conversation_archive", "conversation_delete"] {
        let mut args = json!({"request_id":id(103),"conversation_id":b["conversation_id"],"expected_metadata_revision":0});
        if op == "conversation_delete" {
            args["expected_history_revision"] = json!(0);
        }
        assert_eq!(app.call(request2(op, args)).unwrap_err().code, "BUSY");
    }
    call2(
        &app,
        "conversation_rename",
        json!({"request_id":id(104),"conversation_id":b["conversation_id"],"expected_metadata_revision":0,"title":"Renamed while working"}),
    );
    p.release();
    assert_eq!(done2(&app, 101)["status"], "succeeded");
    assert_eq!(
        get2(&app, &b["conversation_id"])["conversation"]["title"],
        "Renamed while working"
    );
}
#[test]
fn multi_chat_restart_interrupts_every_pending_and_resets_inventory() {
    let f = Fixture::new();
    let p = Arc::new(Fake::default());
    let app = Application::open(&f.roots, p.clone()).unwrap();
    setup(&app);
    *p.gated.lock().unwrap() = true;
    new2(&app, 100, "A");
    p.wait_started();
    for n in 101..104 {
        new2(&app, n, "queued secret");
    }
    assert_eq!(call2(&app, "state", json!({}))["inventory_observed"], true);
    drop(app);
    let p2 = Arc::new(Fake::default());
    let app = Application::open(&f.roots, p2.clone()).unwrap();
    for n in 100..104 {
        assert_eq!(done2(&app, n)["status"], "interrupted");
    }
    let before = fs::read(f.state_path()).unwrap();
    p.release();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(before, fs::read(f.state_path()).unwrap());
    assert_eq!(p2.chats.load(Ordering::SeqCst), 0);
    assert_eq!(call2(&app, "state", json!({}))["inventory_observed"], false);
    assert!(fs::read_to_string(f.state_path())
        .unwrap()
        .contains("queued secret"));
}
#[test]
fn multi_chat_strict_corruption_preserved_and_first_submit_rejection_atomic() {
    let f = Fixture::new();
    let p = Arc::new(Fake::default());
    let app = Application::open(&f.roots, p.clone()).unwrap();
    assert_eq!(
        app.call(request2(
            "submit_new",
            json!({"request_id":id(100),"text":"A","output":"chat","expected_config_revision":0})
        ))
        .unwrap_err()
        .code,
        "MODEL_UNAVAILABLE"
    );
    assert!(call2(&app, "state", json!({}))["conversations"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!f.state_path().exists());
    setup(&app);
    let a = new2(&app, 101, "A");
    done2(&app, 101);
    for title in [" ", "a\nb", "a\u{202e}b", &"é".repeat(129)] {
        assert!(app.call(request2("conversation_rename",json!({"request_id":id(102),"conversation_id":a["conversation_id"],"expected_metadata_revision":0,"title":title}))).is_err());
    }
    drop(app);
    let disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    for (pointer, value) in [
        ("/conversations/0/history_revision", json!(-1)),
        ("/conversations/0/title", json!("bad\nname")),
        ("/jobs/1/conversation_id", json!(id(999))),
        ("/jobs/1/operation/kind", json!("unknown")),
        ("/jobs/1/operation/status", json!("unknown")),
    ] {
        let mut bad = disk.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        let bytes = serde_json::to_vec(&bad).unwrap();
        f.write(&bytes);
        assert!(Application::open(&f.roots, p.clone()).is_err(), "{pointer}");
        assert_eq!(bytes, fs::read(f.state_path()).unwrap());
    }
}
#[test]
fn multi_chat_delete_preflights_tombstones_and_conversation_limit() {
    let f = Fixture::new();
    let p = Arc::new(Fake::default());
    let app = Application::open(&f.roots, p.clone()).unwrap();
    setup(&app);
    let a = new2(&app, 100, "retained secret");
    done2(&app, 100);
    drop(app);
    let mut disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    disk["expired_ids"] = json!((1000..2024).map(id).collect::<Vec<_>>());
    f.write(&serde_json::to_vec(&disk).unwrap());
    let app = Application::open(&f.roots, p).unwrap();
    let before = fs::read(f.state_path()).unwrap();
    assert_eq!(app.call(request2("conversation_delete",json!({"request_id":id(102),"conversation_id":a["conversation_id"],"expected_metadata_revision":0,"expected_history_revision":1}))).unwrap_err().code,"CAPACITY");
    assert_eq!(before, fs::read(f.state_path()).unwrap());
    assert_eq!(
        get2(&app, &a["conversation_id"])["history"][0]["content"],
        "retained secret"
    );
}
#[test]
fn multi_chat_duplicate_versions_and_v1_projection_remain_exact() {
    let f = Fixture::new();
    let p = Arc::new(Fake::default());
    let app = Application::open(&f.roots, p.clone()).unwrap();
    setup(&app);
    let a = new2(&app, 100, "A");
    done2(&app, 100);
    assert_eq!(
        new2(&app, 100, "A")["conversation_id"],
        a["conversation_id"]
    );
    assert_eq!(p.chats.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.call(request2("models", json!({"request_id":id(1)})))
            .unwrap_err()
            .code,
        "ID_CONFLICT"
    );
    let state = call(&app, "state", json!({}));
    let mut keys = state
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec!["active", "config", "history", "providers", "retention"]
    );
    let state = call2(&app, "state", json!({}));
    let mut keys = state
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "config",
            "conversations",
            "inventory",
            "inventory_observed",
            "operations",
            "retention",
            "revision"
        ]
    );
    for summary in state["operations"].as_array().unwrap() {
        assert_eq!(summary.as_object().unwrap().len(), 6);
        assert!(summary.get("result").is_none());
    }
    call(
        &app,
        "submit",
        json!({"request_id":id(101),"text":"legacy","output":"chat"}),
    );
    finished(&app, 101);
    let legacy = call(&app, "state", json!({}));
    assert_eq!(legacy["history"][0]["content"], "legacy");
    assert_eq!(
        get2(&app, &a["conversation_id"])["history"][0]["content"],
        "A"
    );
}

fn legacy_requests() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/voice-v1-requests.json")).unwrap()
}
fn legacy_state() -> Value {
    serde_json::from_str(include_str!("fixtures/voice-v1.json")).unwrap()
}
#[test]
fn voice_v1_migration_preserves_preferences_history_results_ids_and_no_replay() {
    let f = Fixture::new();
    let baseline = legacy_state();
    f.write(&serde_json::to_vec(&baseline).unwrap());
    let fake = Arc::new(Fake::default());
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    let state = call(&app, "state", json!({}));
    assert_eq!(state["config"], baseline["config"]);
    assert_eq!(state["history"], baseline["history"]);
    assert!(state["active"].is_null());
    for n in 1..=5 {
        assert_eq!(poll(&app, n), baseline["jobs"][n - 1]["operation"]);
    }
    assert_eq!(poll(&app, 6)["status"], "interrupted");
    assert_eq!(poll(&app, 6)["error"]["code"], "INTERRUPTED");
    let disk: Value = serde_json::from_slice(&fs::read(f.state_path()).unwrap()).unwrap();
    assert_eq!(disk["expired_ids"], baseline["expired_ids"]);
    for n in 0..6 {
        assert_eq!(
            disk["jobs"][n]["fingerprint"],
            baseline["jobs"][n]["fingerprint"]
        );
    }
    assert_eq!(fake.chats.load(Ordering::SeqCst), 0);
    assert_eq!(fs::metadata(f.state_path()).unwrap().mode() & 0o777, 0o600);
    let before = fs::read(f.state_path()).unwrap();
    for req in legacy_requests() {
        let kind = req["kind"].as_str().unwrap();
        let args = req["args"].clone();
        let n = usize::from_str_radix(args["request_id"].as_str().unwrap(), 16).unwrap();
        let expected = poll(&app, n);
        let admission = call(&app, kind, args.clone());
        assert_eq!(admission["status"], expected["status"]);
        assert_eq!(admission["phase"], expected["phase"]);
        let mut changed = args;
        changed["extra"] = json!(true);
        assert_eq!(
            app.call(request(kind, changed)).unwrap_err().code,
            "ID_CONFLICT"
        );
    }
    assert_eq!(fs::read(f.state_path()).unwrap(), before);
    assert_eq!(fake.chats.load(Ordering::SeqCst), 0);
    assert_eq!(fake.inventories.load(Ordering::SeqCst), 0);
    // Fresh text uses the preserved exact model, never the inert old voice preference.
    call(
        &app,
        "submit",
        json!({"request_id":id(7),"text":"new text","output":"chat"}),
    );
    assert_eq!(
        finished(&app, 7)["result"],
        json!({"assistant_text":"Assistant answer","audio":null,"speech_error":null})
    );
    assert_eq!(call(&app, "state", json!({}))["config"], baseline["config"]);
    drop(app);
    let restored = Application::open(&f.roots, fake.clone()).unwrap();
    assert_eq!(poll(&restored, 6)["status"], "interrupted");
    assert_eq!(poll(&restored, 7)["status"], "succeeded");
    assert_eq!(fake.chats.load(Ordering::SeqCst), 1);
    call(&restored, "models", json!({"request_id":id(8)}));
    finished(&restored, 8);
    let config = call(
        &restored,
        "configure",
        json!({"model":"local:exact","voice":null}),
    );
    assert_eq!(config["revision"], 8);
    assert!(config["voice"].is_null());
    assert_eq!(poll(&restored, 1), baseline["jobs"][0]["operation"]);
    assert_eq!(poll(&restored, 2), baseline["jobs"][1]["operation"]);
}
#[test]
fn every_legacy_running_kind_interrupts_without_replay() {
    for (kind, phase) in [
        ("models", "inventory"),
        ("transcribe", "transcription"),
        ("speak", "synthesis"),
        ("submit", "generation"),
        ("submit", "synthesis"),
    ] {
        let f = Fixture::new();
        let mut disk = legacy_state();
        disk["jobs"][5]["operation"]["kind"] = json!(kind);
        disk["jobs"][5]["operation"]["phase"] = json!(phase);
        f.write(&serde_json::to_vec(&disk).unwrap());
        let fake = Arc::new(Fake::default());
        let app = Application::open(&f.roots, fake.clone()).unwrap();
        assert_eq!(poll(&app, 6)["status"], "interrupted");
        assert_eq!(fake.chats.load(Ordering::SeqCst), 0);
        assert_eq!(call(&app, "state", json!({}))["history"], disk["history"]);
    }
}
#[test]
fn retired_requests_reject_without_execution_and_tombstones_win() {
    let f = Fixture::new();
    f.write(&serde_json::to_vec(&legacy_state()).unwrap());
    let fake = Arc::new(Fake::default());
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    let before = fs::read(f.state_path()).unwrap();
    for req in legacy_requests()
        .into_iter()
        .filter(|r| r["kind"] != "models")
    {
        let kind = req["kind"].as_str().unwrap();
        let mut args = req["args"].clone();
        args["request_id"] = json!(id(80));
        assert_eq!(
            app.call(request(kind, args.clone())).unwrap_err().code,
            "RETIRED"
        );
        args["request_id"] = json!(id(99));
        assert_eq!(
            app.call(request(kind, args)).unwrap_err().code,
            "REQUEST_EXPIRED"
        );
    }
    assert_eq!(
        app.call(request(
            "configure",
            json!({"model":"local:exact","voice":"observed_voice"})
        ))
        .unwrap_err()
        .code,
        "RETIRED"
    );
    assert_eq!(fs::read(f.state_path()).unwrap(), before);
    assert_eq!(fake.chats.load(Ordering::SeqCst), 0);
    assert_eq!(fake.inventories.load(Ordering::SeqCst), 0);
}
#[test]
fn live_text_inventory_retires_speech_and_configure_requires_explicit_null() {
    let f = Fixture::new();
    let fake = Arc::new(Fake::default());
    let app = Application::open(&f.roots, fake).unwrap();
    let unobserved = call(&app, "state", json!({}));
    assert_eq!(unobserved["providers"]["stt"]["error"]["code"], "RETIRED");
    assert_eq!(unobserved["providers"]["tts"]["error"]["code"], "RETIRED");
    setup(&app);
    let inventory = poll(&app, 1)["result"].clone();
    assert_eq!(inventory["voices"], json!([]));
    assert_eq!(
        inventory["providers"]["stt"],
        unobserved["providers"]["stt"]
    );
    assert_eq!(
        inventory["providers"]["tts"],
        unobserved["providers"]["tts"]
    );
    assert!(inventory["providers"]["stt"]["model"].is_null());
    assert_eq!(
        app.call(request("configure", json!({"model":"local:exact"})))
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    assert_eq!(
        app.call(request(
            "configure",
            json!({"model":"local:exact","voice":"observed_voice"})
        ))
        .unwrap_err()
        .code,
        "RETIRED"
    );
    assert!(call(&app, "state", json!({}))["config"]["voice"].is_null());
    assert!(serde_json::from_slice::<Value>(&fs::read(f.state_path()).unwrap()).is_ok());
    assert!(!fs::read_to_string(f.state_path())
        .unwrap()
        .contains("data_base64"));
}
#[test]
fn legacy_null_audio_is_strict_and_corrupt_migration_never_rewrites() {
    for (pointer, value) in [
        (
            "/jobs/1/operation/result/audio",
            json!({"mime":"audio/wav","data_base64":"not-durable"}),
        ),
        ("/config/voice", json!("v".repeat(257))),
        ("/jobs/3/operation/result/duration_ms", json!(60001)),
        (
            "/jobs/5/operation/result",
            json!({"unexpected":"running result"}),
        ),
    ] {
        let f = Fixture::new();
        let mut disk = legacy_state();
        *disk.pointer_mut(pointer).unwrap() = value;
        let bytes = serde_json::to_vec(&disk).unwrap();
        f.write(&bytes);
        assert!(Application::open(&f.roots, Arc::new(Fake::default())).is_err());
        assert_eq!(fs::read(f.state_path()).unwrap(), bytes);
    }
}
#[test]
fn production_source_has_no_speech_transport_or_audio_execution() {
    let providers = include_str!("../src/providers.rs");
    let app = include_str!("../src/app.rs");
    for forbidden in [
        ":4890",
        ":4891",
        "validate_wav",
        "data_base64",
        "base64::",
        "fn transcribe(",
        "fn speak(",
        "11434",
        "reqwest",
        "http://",
    ] {
        assert!(!providers.contains(forbidden), "{forbidden}");
        assert!(!app.contains(forbidden), "{forbidden}");
    }
    let cargo = include_str!("../Cargo.toml");
    assert!(!cargo
        .lines()
        .any(|line| line.starts_with("base64") || line.starts_with("reqwest")));
    let runner = include_str!("../src/runner.rs");
    assert!(!runner.contains("TcpStream") && !runner.contains("http://"));
}
