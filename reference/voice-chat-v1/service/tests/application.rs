use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use smith_service::app::{AppError, AppRequest, AppResult, Application, Message, MAX_CONTEXT};
use smith_service::providers::{
    eligible_tags, validate_wav, Audio, Inventory, Model, Observation, Observations, Provider,
    SttObservation,
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
fn setup(app: &Application, voice: bool) {
    call(app, "models", json!({"request_id":id(1)}));
    let terminal = finished(app, 1);
    assert_eq!(terminal["status"], "succeeded");
    call(
        app,
        "configure",
        json!({"model":"local:exact","voice":if voice {Some("observed_voice")} else {None}}),
    );
}
fn wav(rate: u32, ms: usize) -> Vec<u8> {
    let n = rate as usize * 2 * ms / 1000;
    let mut v = Vec::new();
    v.extend(b"RIFF");
    v.extend(((36 + n) as u32).to_le_bytes());
    v.extend(b"WAVEfmt ");
    v.extend(16u32.to_le_bytes());
    v.extend(1u16.to_le_bytes());
    v.extend(1u16.to_le_bytes());
    v.extend(rate.to_le_bytes());
    v.extend((rate * 2).to_le_bytes());
    v.extend(2u16.to_le_bytes());
    v.extend(16u16.to_le_bytes());
    v.extend(b"data");
    v.extend((n as u32).to_le_bytes());
    v.resize(44 + n, 0);
    v
}
struct Fake {
    chats: AtomicUsize,
    syntheses: AtomicUsize,
    transcriptions: AtomicUsize,
    missing_speech: bool,
    fail_tts: bool,
    fail_llm: bool,
    answer: String,
    gated: Mutex<bool>,
    gate: Condvar,
    started: AtomicBool,
    inventory_override: Option<Inventory>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            chats: AtomicUsize::new(0),
            syntheses: AtomicUsize::new(0),
            transcriptions: AtomicUsize::new(0),
            missing_speech: false,
            fail_tts: false,
            fail_llm: false,
            answer: "Assistant answer".into(),
            gated: Mutex::new(false),
            gate: Condvar::new(),
            started: AtomicBool::new(false),
            inventory_override: None,
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
    fn transcribe(&self, audio: &[u8]) -> AppResult<Value> {
        let duration = validate_wav(audio, 16000, 2 * 1024 * 1024)?;
        self.transcriptions.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"text":"Editable transcript","language":"en","duration_ms":duration}))
    }
    fn speak(&self, text: &str, voice: Option<&str>) -> AppResult<Audio> {
        assert!(text.len() <= 4096);
        self.syntheses.fetch_add(1, Ordering::SeqCst);
        if self.missing_speech || self.fail_tts {
            Err(AppError::new("TTS_UNAVAILABLE", "fake synthesis failure"))
        } else {
            assert_eq!(voice, Some("observed_voice"));
            Audio::output(&wav(24000, 10))
        }
    }
}
#[test]
fn dropped_gated_worker_cannot_publish_into_reopened_roots_or_start_synthesis() {
    let f = Fixture::new();
    let old_provider = Arc::new(Fake {
        gated: Mutex::new(true),
        ..Fake::default()
    });
    let old = Application::open(&f.roots, old_provider.clone()).unwrap();
    setup(&old, true);
    call(
        &old,
        "submit",
        json!({"request_id":id(2),"text":"old pending input","output":"speech"}),
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
    assert_eq!(old_provider.syntheses.load(Ordering::SeqCst), 0);
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
    setup(&app, true);
    let disk: Value = serde_json::from_slice(&fs::read(source.state_path()).unwrap()).unwrap();
    let baseline = disk["jobs"][0]["operation"]["result"].clone();
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
    setup(&app, false);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"answer","output":"speech"}),
    );
    finished(&app, 2);
    let baseline: Value = serde_json::from_slice(&fs::read(source.state_path()).unwrap()).unwrap();
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
    setup(&app, true);
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
fn all_four_paths_explicit_transcription_does_not_generate() {
    let f = Fixture::new();
    let fake = Arc::new(Fake::default());
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app, true);
    for (n, output) in [(2, "chat"), (3, "speech")] {
        call(
            &app,
            "submit",
            json!({"request_id":id(n),"text":"typed message","output":output}),
        );
        let job = finished(&app, n);
        assert_eq!(job["status"], "succeeded");
        assert_eq!(job["result"]["assistant_text"], "Assistant answer");
        assert_eq!(job["result"]["audio"].is_null(), output == "chat");
    }
    let audio = json!({"mime":"audio/wav","data_base64":STANDARD.encode(wav(16000,10))});
    for (n, output) in [(4, "chat"), (6, "speech")] {
        call(
            &app,
            "transcribe",
            json!({"request_id":id(n),"audio":audio}),
        );
        let transcript = finished(&app, n);
        assert_eq!(transcript["result"]["text"], "Editable transcript");
        assert_eq!(
            fake.chats.load(Ordering::SeqCst),
            if n == 4 { 2 } else { 3 }
        );
        call(
            &app,
            "submit",
            json!({"request_id":id(n+1),"text":"edited transcript","output":output}),
        );
        assert_eq!(finished(&app, n + 1)["status"], "succeeded");
    }
    call(
        &app,
        "speak",
        json!({"request_id":id(8),"text":"Deliberate replay"}),
    );
    assert_eq!(finished(&app, 8)["result"]["audio"]["sample_rate"], 24000);
    assert_eq!(fake.chats.load(Ordering::SeqCst), 4);
    assert_eq!(fake.transcriptions.load(Ordering::SeqCst), 2);
    assert_eq!(fake.syntheses.load(Ordering::SeqCst), 3);
    let state = call(&app, "state", json!({}));
    assert_eq!(state["history"].as_array().unwrap().len(), 8);
    let disk = fs::read_to_string(f.state_path()).unwrap();
    assert!(!disk.contains("data_base64"));
    assert!(!disk.contains(&STANDARD.encode(wav(16000, 10))));
    assert_eq!(fs::metadata(f.state_path()).unwrap().mode() & 0o777, 0o600);
}
#[test]
fn missing_speech_inventory_and_tts_failure_preserve_text() {
    let f = Fixture::new();
    let fake = Arc::new(Fake {
        missing_speech: true,
        ..Fake::default()
    });
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app, false);
    assert_eq!(poll(&app, 1)["result"]["providers"]["llm"]["ready"], true);
    assert_eq!(poll(&app, 1)["result"]["providers"]["tts"]["ready"], false);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"keep me","output":"speech"}),
    );
    let job = finished(&app, 2);
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["result"]["speech_error"]["code"], "TTS_UNAVAILABLE");
    assert_eq!(job["result"]["assistant_text"], "Assistant answer");
    assert_eq!(
        call(&app, "state", json!({}))["history"][1]["content"],
        "Assistant answer"
    );
    assert_eq!(fake.chats.load(Ordering::SeqCst), 1);
    let restored = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    assert_eq!(
        poll(&restored, 2)["result"]["assistant_text"],
        "Assistant answer"
    );
    assert_eq!(
        call(&restored, "state", json!({}))["config"]["model"],
        "local:exact"
    );
}
#[test]
fn oversized_tts_answer_is_not_truncated_or_regenerated() {
    let f = Fixture::new();
    let fake = Arc::new(Fake {
        answer: "a".repeat(4097),
        ..Fake::default()
    });
    let app = Application::open(&f.roots, fake.clone()).unwrap();
    setup(&app, true);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"hello","output":"speech"}),
    );
    let job = finished(&app, 2);
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["result"]["speech_error"]["code"], "TTS_INPUT_LIMIT");
    assert_eq!(
        job["result"]["assistant_text"].as_str().unwrap().len(),
        4097
    );
    assert_eq!(fake.syntheses.load(Ordering::SeqCst), 0);
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
    setup(&app, true);
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
    setup(&app, true);
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
fn persisted_audio_expires_but_completed_text_survives() {
    let f = Fixture::new();
    let app = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    setup(&app, true);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"speech","output":"speech"}),
    );
    assert!(!finished(&app, 2)["result"]["audio"].is_null());
    call(&app, "speak", json!({"request_id":id(3),"text":"replay"}));
    finished(&app, 3);
    let restored = Application::open(&f.roots, Arc::new(Fake::default())).unwrap();
    let job = poll(&restored, 2);
    assert_eq!(job["status"], "succeeded");
    assert!(job["result"]["audio"].is_null());
    assert_eq!(job["result"]["speech_error"]["code"], "AUDIO_EXPIRED");
    assert_eq!(job["result"]["assistant_text"], "Assistant answer");
    assert_eq!(poll(&restored, 3)["error"]["code"], "AUDIO_EXPIRED");
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
    setup(&app, true);
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
    setup(&app, true);
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"no partial","output":"speech"}),
    );
    assert_eq!(finished(&app, 2)["status"], "failed");
    assert_eq!(fake.syntheses.load(Ordering::SeqCst), 0);
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
    req.schema_version = 2;
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
    setup(&app, true);
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
        "VOICE_UNAVAILABLE"
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
    setup(&app, true);
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
#[test]
fn wav_bounds_reject_stereo_rates_compression_truncation_and_duration() {
    assert_eq!(
        validate_wav(&wav(16000, 60_000), 16000, 2 * 1024 * 1024).unwrap(),
        60_000
    );
    assert_eq!(
        validate_wav(&wav(24000, 60_000), 24000, 2_880_044).unwrap(),
        60_000
    );
    assert!(validate_wav(&wav(16000, 60_001), 16000, 2 * 1024 * 1024).is_err());
    assert!(validate_wav(&wav(24000, 60_001), 24000, 2_880_044).is_err());
    assert!(validate_wav(&wav(24000, 10), 16000, 2 * 1024 * 1024).is_err());
    for index in [20, 22, 28, 32, 34] {
        let mut bad = wav(16000, 10);
        bad[index] = 9;
        assert!(validate_wav(&bad, 16000, 2 * 1024 * 1024).is_err());
    }
    let mut bad = wav(16000, 10);
    bad.pop();
    assert!(validate_wav(&bad, 16000, 2 * 1024 * 1024).is_err());
    let bad = Audio {
        mime: "audio/wav".into(),
        data_base64: "not base64".into(),
        sample_rate: None,
    };
    assert!(bad.input_bytes().is_err());
}
#[test]
fn installed_models_reject_cloud_remote_substitution_and_malformed_digest() {
    let local = json!({"name":"local:exact","model":"local:exact","digest":"a".repeat(64),"details":{"format":"gguf","family":"llama"}});
    let mut entries = vec![local.clone()];
    for (name, key, value) in [
        ("cloud:tag", "remote_host", json!("https://cloud")),
        ("bad:tag", "remote_model", json!("other")),
        ("another:tag", "cloud", json!(true)),
    ] {
        let mut m = local.clone();
        m["name"] = json!(name);
        m["model"] = json!(name);
        m[key] = value;
        entries.push(m);
    }
    let mut nested = local.clone();
    nested["name"] = json!("nested:tag");
    nested["model"] = json!("nested:tag");
    nested["details"]["remote_host"] = json!("remote");
    entries.push(nested);
    let models = eligible_tags(&json!({"models":entries})).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].name, "local:exact");
    let mut bad = local;
    bad["digest"] = json!("bad");
    assert!(eligible_tags(&json!({"models":[bad]})).is_err());
}
