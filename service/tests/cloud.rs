//! Synthetic only: no production keyring, public transport, keys, model or service.
use serde_json::{json, Value};
use smith_service::app::{new_id, AppError, AppRequest, AppResult, Application, Message};
use smith_service::cloud::{
    self, Clock, CredentialRef, CredentialStore, Dependencies, HttpFailure, HttpResponse,
    HttpTransport, Secret, Vendor,
};
use smith_service::providers::{Inventory, Model, Observation, Provider};
use smith_service::{catalog::CATALOG, Roots};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn req(op: &str, args: Value) -> AppRequest {
    AppRequest {
        schema_version: 3,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(99999),
        op: op.into(),
        args,
    }
}
fn call(app: &Application, op: &str, args: Value) -> Value {
    app.call(req(op, args)).unwrap()
}
fn done(app: &Application, n: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let v = call(
            app,
            "poll",
            json!({"request_id":id(n),"instance_id":app.instance_id}),
        );
        if v["status"] != "running" && v["status"] != "queued" {
            return v;
        }
        assert!(Instant::now() < deadline, "job did not terminate: {v}");
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[derive(Default)]
struct FakeClock {
    offset: AtomicUsize,
    jump_after_marker: Mutex<Option<PathBuf>>,
}
impl FakeClock {
    fn advance(&self, seconds: usize) {
        self.offset.fetch_add(seconds, Ordering::SeqCst);
    }
}
impl Clock for FakeClock {
    fn now(&self) -> Instant {
        if self
            .jump_after_marker
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|p| {
                fs::read_to_string(p).is_ok_and(|s| s.contains("\"delivery\":\"may_have_sent\""))
            })
        {
            self.offset.store(601, Ordering::SeqCst);
        }
        Instant::now() + Duration::from_secs(self.offset.load(Ordering::SeqCst) as u64)
    }
    fn wall_ms(&self) -> u64 {
        1_800_000_000_000 + self.offset.load(Ordering::SeqCst) as u64 * 1000
    }
}
#[derive(Default)]
struct Keys {
    items: Mutex<HashMap<String, Vec<u8>>>,
    reads: AtomicUsize,
    identities: Mutex<Vec<CredentialRef>>,
    saves: AtomicUsize,
    fail_activation: Mutex<Option<PathBuf>>,
    failure: Mutex<Option<String>>,
    gate: AtomicBool,
    entered: AtomicBool,
}
impl CredentialStore for Keys {
    fn read(&self, c: &CredentialRef, _: Instant, cancel: &AtomicBool) -> AppResult<Secret> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.identities.lock().unwrap().push(c.clone());
        self.entered.store(true, Ordering::SeqCst);
        while self.gate.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::new("INTERRUPTED", "fake cancelled"));
        }
        if let Some(code) = self.failure.lock().unwrap().as_ref() {
            return Err(AppError::new(code, "synthetic-keyring-error-canary"));
        }
        Secret::new(
            self.items
                .lock()
                .unwrap()
                .get(&c.generation)
                .cloned()
                .ok_or_else(|| AppError::new("KEYRING_MISSING", "synthetic missing"))?,
        )
    }
    fn save(&self, c: &CredentialRef, s: &Secret, _: Instant, _: &AtomicBool) -> AppResult<()> {
        self.saves.fetch_add(1, Ordering::SeqCst);
        if let Some(code) = self.failure.lock().unwrap().as_ref() {
            return Err(AppError::new(code, "synthetic-keyring-error-canary"));
        }
        assert!(self
            .items
            .lock()
            .unwrap()
            .insert(c.generation.clone(), s.expose().to_vec())
            .is_none());
        if let Some(path) = self.fail_activation.lock().unwrap().as_ref() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        Ok(())
    }
}
#[derive(Default)]
struct Http {
    calls: Mutex<Vec<(Vendor, Value)>>,
    status: Mutex<Option<u16>>,
    answer: Mutex<Option<String>>,
    fail_completion: Mutex<Option<PathBuf>>,
    failure: AtomicBool,
    gate: AtomicBool,
    entered: AtomicBool,
}
fn success(v: Vendor, model: &str, text: &str) -> Value {
    match v {
        Vendor::Openai => {
            json!({"object":"response","model":model,"status":"completed","error":null,"incomplete_details":null,"output":[{"type":"reasoning","summary":["reasoning-canary"]},{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":text}]}]})
        }
        Vendor::Anthropic => {
            json!({"type":"message","role":"assistant","model":model,"stop_reason":"end_turn","stop_sequence":null,"content":[{"type":"thinking","thinking":"reasoning-canary"},{"type":"text","text":text}]})
        }
    }
}
impl HttpTransport for Http {
    fn post(
        &self,
        v: Vendor,
        body: Vec<u8>,
        s: &Secret,
        _: Instant,
        _: Instant,
        cancel: &AtomicBool,
    ) -> Result<HttpResponse, HttpFailure> {
        assert_eq!(s.expose(), b"synthetic-secret-canary");
        let body: Value = serde_json::from_slice(&body).unwrap();
        self.calls.lock().unwrap().push((v, body.clone()));
        self.entered.store(true, Ordering::SeqCst);
        while self.gate.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if self.failure.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
            return Err(HttpFailure {
                error: AppError::new("REMOTE_OUTCOME_UNKNOWN", "synthetic-remote-error-canary"),
                not_sent: false,
            });
        }
        let value = success(
            v,
            body["model"].as_str().unwrap(),
            self.answer
                .lock()
                .unwrap()
                .as_deref()
                .unwrap_or("Cloud final answer"),
        );
        if let Some(path) = self.fail_completion.lock().unwrap().as_ref() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        Ok(HttpResponse {
            status: self.status.lock().unwrap().unwrap_or(200),
            header_bytes: 80,
            body: serde_json::to_vec(&value).unwrap(),
        })
    }
}
#[derive(Default)]
struct Local {
    calls: Mutex<Vec<(String, Vec<Message>)>>,
    gate: AtomicBool,
    entered: AtomicBool,
}
impl Provider for Local {
    fn inventory(&self) -> AppResult<Inventory> {
        let mut i = Inventory {
            models: CATALOG[..2]
                .iter()
                .map(|e| Model {
                    name: e.name.into(),
                    digest: e.sha256.into(),
                })
                .collect(),
            ..Inventory::default()
        };
        i.providers.llm = Observation {
            ready: true,
            error: None,
        };
        Ok(i)
    }
    fn chat(&self, m: &str, history: &[Message]) -> AppResult<String> {
        self.calls
            .lock()
            .unwrap()
            .push((m.into(), history.to_vec()));
        self.entered.store(true, Ordering::SeqCst);
        while self.gate.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok("Local final answer".into())
    }
}
struct F {
    base: PathBuf,
    roots: Roots,
    keys: Arc<Keys>,
    http: Arc<Http>,
    local: Arc<Local>,
    clock: Arc<FakeClock>,
}
impl F {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("smith-cloud-{}", new_id().unwrap()));
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("runtime"),
        };
        for p in [
            &base,
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
        ] {
            fs::create_dir(p).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            base,
            roots,
            keys: Arc::new(Keys::default()),
            http: Arc::new(Http::default()),
            local: Arc::new(Local::default()),
            clock: Arc::new(FakeClock::default()),
        }
    }
    fn open(&self) -> Application {
        Application::open_with_cloud(
            &self.roots,
            self.local.clone(),
            Dependencies::new(self.keys.clone(), self.http.clone()).with_clock(self.clock.clone()),
        )
        .unwrap()
    }
    fn disk(&self) -> PathBuf {
        self.roots.data.join("smith-next/app-state.json")
    }
    fn save(&self, app: &Application, v: Vendor, n: usize) -> String {
        let catalog = call(app, "route_catalog", json!({}));
        let revision = catalog["providers"][v.index()]["revision"].clone();
        let p = call(
            app,
            "provider_prepare",
            json!({"request_id":id(n),"instance_id":app.instance_id,"provider":v,"action":"save","model":null,"expected_provider_revision":revision}),
        );
        let a = json!({"request_id":id(n),"instance_id":app.instance_id,"approval_token":p["approval_token"],"api_key":"synthetic-secret-canary"});
        call(app, "provider_apply", a);
        let r = done(app, n);
        assert_eq!(r["status"], "succeeded", "{r}");
        r["result"]["provider"]["credential_generation"]
            .as_str()
            .unwrap()
            .into()
    }
    fn begin_validation(&self, app: &Application, v: Vendor, model: &str, n: usize) {
        let c = call(app, "route_catalog", json!({}));
        let p = call(
            app,
            "provider_prepare",
            json!({"request_id":id(n),"instance_id":app.instance_id,"provider":v,"action":"validate","model":model,"expected_provider_revision":c["providers"][v.index()]["revision"]}),
        );
        call(
            app,
            "provider_apply",
            json!({"request_id":id(n),"instance_id":app.instance_id,"approval_token":p["approval_token"],"api_key":null}),
        );
    }
    fn validate(&self, app: &Application, v: Vendor, model: &str, n: usize) -> Value {
        self.begin_validation(app, v, model, n);
        done(app, n)
    }
    fn configured(&self, app: &Application, v: Vendor, n: usize) -> String {
        let generation = self.save(app, v, n);
        let m = cloud::MODELS.iter().find(|m| m.vendor == v).unwrap();
        assert_eq!(self.validate(app, v, m.id, n + 1)["status"], "succeeded");
        generation
    }
    fn local(&self, app: &Application) {
        call(app, "models", json!({"request_id":id(9900)}));
        assert_eq!(done(app, 9900)["status"], "succeeded");
    }
}
impl Drop for F {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
fn route(v: Vendor, generation: &str, model: &str) -> Value {
    json!({"provider":v,"model":model,"credential_generation":generation})
}
fn args(app: &Application, n: usize, target: Option<&str>, text: &str, route: Value) -> Value {
    let revision = target.map(|id| {
        call(app, "conversation_get", json!({"conversation_id":id}))["conversation"]
            ["history_revision"]
            .clone()
    });
    json!({"request_id":id(n),"instance_id":app.instance_id,"conversation_id":target,"text":text,"route":route,"expected_config_revision":call(app,"state",json!({}))["config"]["revision"],"expected_history_revision":revision})
}
fn send(app: &Application, mut a: Value) -> Value {
    if a["route"]["provider"] != "local" {
        let p = call(app, "route_prepare", a.clone());
        a["approval_token"] = p["approval_token"].clone();
        a["confirm_transfer"] = p["requires_consent"].clone();
    } else {
        a["approval_token"] = Value::Null;
        a["confirm_transfer"] = json!(false);
    }
    call(app, "submit_routed", a)
}
fn wait(flag: &AtomicBool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !flag.load(Ordering::SeqCst) {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn catalog_queries_and_unconfigured_probe_have_zero_external_effects() {
    let f = F::new();
    let app = f.open();
    let c = call(&app, "route_catalog", json!({}));
    assert_eq!(c.as_object().unwrap().len(), 4);
    assert_eq!(c["providers"].as_array().unwrap().len(), 2);
    let models = c["models"].as_array().unwrap();
    assert_eq!(models.len(), cloud::MODELS.len());
    for (choice, model) in models.iter().zip(cloud::MODELS) {
        assert_eq!(choice["group"], "cloud");
        assert_eq!(choice["label"], model.label);
        assert_eq!(choice["route"]["provider"], model.vendor.name());
        assert_eq!(choice["route"]["model"], model.id);
        assert!(choice["route"]["credential_generation"].is_null());
        assert_eq!(choice["available"], false);
        assert_eq!(choice["validated_model"], false);
        assert_eq!(
            choice["unavailable_reason"]["code"],
            "PROVIDER_NOT_CONFIGURED"
        );
    }
    assert_eq!(call(&app, "route_catalog", json!({})), c);
    call(
        &app,
        "provider_probe",
        json!({"request_id":id(1),"instance_id":app.instance_id,"provider":"openai","expected_provider_revision":0}),
    );
    assert_eq!(
        done(&app, 1)["result"]["provider"]["credential_state"],
        "not_configured"
    );
    assert_eq!(f.keys.reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.keys.saves.load(Ordering::SeqCst), 0);
    assert!(f.http.calls.lock().unwrap().is_empty());
    assert!(f.local.calls.lock().unwrap().is_empty());
}
#[test]
fn saved_credentials_enable_attempts_without_validation_or_inference() {
    let f = F::new();
    let app = f.open();
    for (i, vendor) in Vendor::ALL.into_iter().enumerate() {
        f.save(&app, vendor, i + 1);
    }
    let reads = f.keys.reads.load(Ordering::SeqCst);
    let disk = fs::read(f.disk()).unwrap();
    let c = call(&app, "route_catalog", json!({}));
    assert_eq!(c["models"].as_array().unwrap().len(), cloud::MODELS.len());
    for choice in c["models"].as_array().unwrap() {
        assert_eq!(choice["available"], true);
        assert_eq!(choice["validated_model"], false);
        assert!(choice["unavailable_reason"].is_null());
        assert!(choice["route"]["credential_generation"].is_string());
    }
    for p in c["providers"].as_array().unwrap() {
        assert_eq!(p["credential_state"], "available");
        assert!(p["validation"].is_null());
    }
    assert_eq!(call(&app, "route_catalog", json!({})), c);
    assert_eq!(fs::read(f.disk()).unwrap(), disk);
    assert_eq!(f.keys.reads.load(Ordering::SeqCst), reads);
    assert!(f.http.calls.lock().unwrap().is_empty());
    assert!(f.local.calls.lock().unwrap().is_empty());
    assert_eq!(call(&app, "state", json!({}))["conversations"], json!([]));
}
fn default_model(vendor: Vendor) -> &'static str {
    cloud::MODELS
        .iter()
        .find(|m| m.vendor == vendor)
        .unwrap()
        .id
}
fn assert_attemptable(catalog: &Value, vendor: Vendor, generation: &str) {
    let provider = &catalog["providers"][vendor.index()];
    assert_eq!(provider["credential_generation"], generation);
    let choices: Vec<_> = catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["route"]["provider"] == vendor.name())
        .collect();
    assert_eq!(
        choices.len(),
        cloud::MODELS.iter().filter(|m| m.vendor == vendor).count()
    );
    for choice in choices {
        assert_eq!(choice["route"]["credential_generation"], generation);
        assert_eq!(choice["available"], true, "{choice}");
        assert!(choice["unavailable_reason"].is_null());
        assert_eq!(
            choice["validated_model"],
            provider["validation"]["status"] == "succeeded"
                && provider["validation"]["model"] == choice["route"]["model"]
        );
    }
}
#[test]
fn saved_keys_after_restart_allow_send_and_validation_without_probe() {
    for vendor in Vendor::ALL {
        for validate in [false, true] {
            let f = F::new();
            let app = f.open();
            let generation = f.save(&app, vendor, 1);
            drop(app);
            let before = fs::read(f.disk()).unwrap();
            let app = f.open();
            let catalog = call(&app, "route_catalog", json!({}));
            let p = &catalog["providers"][vendor.index()];
            assert_eq!(p["credential_state"], "not_observed");
            assert_eq!(p["credential_error"]["code"], "NOT_OBSERVED");
            assert!(p["validation"].is_null());
            assert_attemptable(&catalog, vendor, &generation);
            assert_eq!(fs::read(f.disk()).unwrap(), before);
            assert_eq!(f.keys.reads.load(Ordering::SeqCst), 0);
            assert!(f.http.calls.lock().unwrap().is_empty());
            let result = if validate {
                f.validate(&app, vendor, default_model(vendor), 2)
            } else {
                send(
                    &app,
                    args(
                        &app,
                        2,
                        None,
                        "first explicit send",
                        route(vendor, &generation, default_model(vendor)),
                    ),
                );
                done(&app, 2)
            };
            assert_eq!(result["status"], "succeeded", "{result}");
            assert_eq!(f.keys.reads.load(Ordering::SeqCst), 1);
            assert_eq!(
                *f.keys.identities.lock().unwrap(),
                vec![CredentialRef {
                    provider: vendor,
                    generation: generation.clone(),
                    save_request_id: id(1),
                }]
            );
            let calls = f.http.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, vendor);
            assert_eq!(calls[0].1["model"], default_model(vendor));
            drop(calls);
            let catalog = call(&app, "route_catalog", json!({}));
            assert_eq!(
                catalog["providers"][vendor.index()]["credential_state"],
                "available"
            );
            assert_eq!(
                catalog["providers"][vendor.index()]["validation"].is_null(),
                !validate
            );
            assert_attemptable(&catalog, vendor, &generation);
            let conversations = call(&app, "state", json!({}))["conversations"].clone();
            assert_eq!(
                conversations.as_array().unwrap().len(),
                usize::from(!validate)
            );
            assert!(f.local.calls.lock().unwrap().is_empty());
        }
    }
}
#[test]
fn saved_routes_ignore_cached_observations_and_validation_without_changing_facts() {
    for vendor in Vendor::ALL {
        for validation_status in [None, Some("failed"), Some("interrupted"), Some("succeeded")] {
            let f = F::new();
            let mut app = f.open();
            let generation = f.save(&app, vendor, 1);
            match validation_status {
                Some("interrupted") => {
                    f.http.gate.store(true, Ordering::SeqCst);
                    f.begin_validation(&app, vendor, default_model(vendor), 2);
                    wait(&f.http.entered);
                    drop(app);
                    f.http.gate.store(false, Ordering::SeqCst);
                    app = f.open();
                    assert_eq!(done(&app, 2)["status"], "interrupted");
                }
                Some(status) => {
                    if status == "failed" {
                        *f.http.status.lock().unwrap() = Some(401);
                    }
                    assert_eq!(
                        f.validate(&app, vendor, default_model(vendor), 2)["status"],
                        status
                    );
                }
                None => {}
            }
            let validation = call(&app, "route_catalog", json!({}))["providers"][vendor.index()]
                ["validation"]
                .clone();
            for (i, (state, error)) in [
                ("not_observed", None),
                ("available", None),
                ("locked", Some("KEYRING_LOCKED")),
                ("missing", Some("KEYRING_MISSING")),
                ("conflict", Some("KEYRING_CONFLICT")),
                ("unavailable", Some("KEYRING_UNAVAILABLE")),
            ]
            .into_iter()
            .enumerate()
            {
                if state == "not_observed" {
                    drop(app);
                    app = f.open();
                } else {
                    *f.keys.failure.lock().unwrap() = error.map(str::to_owned);
                    call(
                        &app,
                        "provider_probe",
                        json!({"request_id":id(10+i),"instance_id":app.instance_id,"provider":vendor,"expected_provider_revision":1}),
                    );
                    assert_eq!(done(&app, 10 + i)["status"], "succeeded");
                }
                let reads = f.keys.reads.load(Ordering::SeqCst);
                let http = f.http.calls.lock().unwrap().len();
                let before = fs::read(f.disk()).unwrap();
                let catalog = call(&app, "route_catalog", json!({}));
                assert_attemptable(&catalog, vendor, &generation);
                let p = &catalog["providers"][vendor.index()];
                assert_eq!(p["credential_state"], state);
                assert_eq!(p["validation"], validation);
                assert_eq!(p["validation"]["status"].as_str(), validation_status);
                if let Some(error) = error {
                    assert_eq!(p["credential_error"]["code"], error);
                }
                call(
                    &app,
                    "route_prepare",
                    args(
                        &app,
                        30 + i,
                        None,
                        "prepare only",
                        route(vendor, &generation, default_model(vendor)),
                    ),
                );
                call(
                    &app,
                    "provider_prepare",
                    json!({"request_id":id(40+i),"instance_id":app.instance_id,"provider":vendor,"action":"validate","model":default_model(vendor),"expected_provider_revision":1}),
                );
                assert_eq!(call(&app, "route_catalog", json!({})), catalog);
                assert_eq!(fs::read(f.disk()).unwrap(), before);
                assert_eq!(f.keys.reads.load(Ordering::SeqCst), reads);
                assert_eq!(f.http.calls.lock().unwrap().len(), http);
                assert!(call(&app, "state", json!({}))["conversations"]
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
        }
    }
}
#[test]
fn saved_only_routes_keep_missing_stale_model_instance_and_consent_guards() {
    for vendor in Vendor::ALL {
        let f = F::new();
        let app = f.open();
        let other = Vendor::ALL[1 - vendor.index()];
        let generation = f.save(&app, vendor, 1);
        drop(app);
        let app = f.open();
        for (candidate, code) in [
            (
                route(vendor, &id(900), default_model(vendor)),
                "CREDENTIAL_CHANGED",
            ),
            (
                route(vendor, &generation, "unknown-model"),
                "ROUTE_UNAVAILABLE",
            ),
            (
                route(vendor, &generation, default_model(other)),
                "ROUTE_UNAVAILABLE",
            ),
            (
                route(other, &generation, default_model(other)),
                "PROVIDER_NOT_CONFIGURED",
            ),
            (
                json!({"provider":vendor,"model":default_model(vendor),"credential_generation":null}),
                "INVALID_REQUEST",
            ),
            (
                json!({"provider":"local","model":CATALOG[0].name,"credential_generation":null}),
                "ROUTE_UNAVAILABLE",
            ),
        ] {
            assert_eq!(
                app.call(req(
                    "route_prepare",
                    args(&app, 10, None, "reject", candidate)
                ))
                .unwrap_err()
                .code,
                code
            );
        }
        let mut a = args(
            &app,
            20,
            None,
            "requires consent",
            route(vendor, &generation, default_model(vendor)),
        );
        let mut stale = a.clone();
        stale["instance_id"] = json!(id(901));
        assert_eq!(
            app.call(req("route_prepare", stale)).unwrap_err().code,
            "INSTANCE_MISMATCH"
        );
        let preparation = call(&app, "route_prepare", a.clone());
        assert_eq!(preparation["requires_consent"], true);
        a["approval_token"] = preparation["approval_token"].clone();
        a["confirm_transfer"] = json!(false);
        assert_eq!(
            app.call(req("submit_routed", a)).unwrap_err().code,
            "CONSENT_REQUIRED"
        );
        assert_eq!(f.keys.reads.load(Ordering::SeqCst), 0);
        assert!(f.http.calls.lock().unwrap().is_empty());
        assert_eq!(call(&app, "state", json!({}))["conversations"], json!([]));
    }
}
#[test]
fn save_is_local_validation_is_exact_synthetic_and_no_secret_is_durable() {
    let f = F::new();
    let app = f.open();
    let generation = f.save(&app, Vendor::Openai, 1);
    assert!(f.http.calls.lock().unwrap().is_empty());
    let r = f.validate(&app, Vendor::Openai, "gpt-6-astra", 2);
    assert_eq!(r["status"], "succeeded");
    assert_eq!(
        r["result"]["provider"]["validation"]["credential_generation"],
        generation
    );
    assert_eq!(call(&app, "state", json!({}))["conversations"], json!([]));
    let calls = f.http.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].1["input"],
        json!([{"role":"user","content":[{"type":"input_text","text":cloud::VALIDATION_TEXT}]}])
    );
    assert_eq!(calls[0].1["max_output_tokens"], 8192);
    assert_eq!(calls[0].1["store"], false);
    let disk = fs::read_to_string(f.disk()).unwrap();
    for forbidden in [
        "synthetic-secret-canary",
        "reasoning-canary",
        "approval_token",
        "api_key",
    ] {
        assert!(!disk.contains(forbidden));
    }
    assert_eq!(
        serde_json::from_str::<Value>(&disk).unwrap()["schema_version"],
        4
    );
}
#[test]
fn both_providers_and_same_destination_models_are_available_but_exact_check_is_labelled() {
    let f = F::new();
    let app = f.open();
    let generations = [
        f.configured(&app, Vendor::Openai, 1),
        f.configured(&app, Vendor::Anthropic, 3),
    ];
    let reads = f.keys.reads.load(Ordering::SeqCst);
    let c = call(&app, "route_catalog", json!({}));
    let choices = c["models"].as_array().unwrap();
    assert_eq!(choices.len(), cloud::MODELS.len());
    for (choice, model) in choices.iter().zip(cloud::MODELS) {
        assert_eq!(choice["route"]["provider"], model.vendor.name());
        assert_eq!(choice["route"]["model"], model.id);
        assert_eq!(
            choice["route"]["credential_generation"],
            generations[model.vendor.index()]
        );
        assert_eq!(choice["available"], true);
        assert!(choice["unavailable_reason"].is_null());
        assert_eq!(
            choice["validated_model"],
            matches!(model.id, "gpt-6-astra" | "claude-sonnet-5-5")
        );
    }
    assert_eq!(
        choices
            .iter()
            .filter(|m| m["validated_model"] == true)
            .count(),
        2
    );
    assert_eq!(call(&app, "route_catalog", json!({})), c);
    assert_eq!(f.keys.reads.load(Ordering::SeqCst), reads);
    assert_eq!(f.http.calls.lock().unwrap().len(), 2);
    assert!(f.local.calls.lock().unwrap().is_empty());
}
#[test]
fn every_catalog_model_sends_on_validated_generation_without_extra_validation() {
    let f = F::new();
    let app = f.open();
    let generations = [
        f.configured(&app, Vendor::Openai, 1),
        f.configured(&app, Vendor::Anthropic, 3),
    ];
    let mut target = None::<String>;
    let mut destinations = std::collections::HashSet::new();
    for (i, model) in cloud::MODELS.iter().enumerate() {
        let n = i + 10;
        let a = args(
            &app,
            n,
            target.as_deref(),
            "explicit catalog send",
            route(model.vendor, &generations[model.vendor.index()], model.id),
        );
        let reads = f.keys.reads.load(Ordering::SeqCst);
        let prepared = call(&app, "route_prepare", a.clone());
        assert_eq!(
            prepared["requires_consent"],
            destinations.insert(model.vendor)
        );
        assert_eq!(f.keys.reads.load(Ordering::SeqCst), reads);
        assert_eq!(f.http.calls.lock().unwrap().len(), i + 2);
        let receipt = send(&app, a);
        target = Some(receipt["conversation_id"].as_str().unwrap().into());
        assert_eq!(done(&app, n)["status"], "succeeded");
        let routing = call(
            &app,
            "request_routing",
            json!({"request_id":id(n),"instance_id":app.instance_id}),
        );
        assert_eq!(routing["route"]["model"], model.id);
        assert_eq!(routing["delivery"], "received");
        let calls = f.http.calls.lock().unwrap();
        assert_eq!(calls.len(), i + 3);
        assert_eq!(calls.last().unwrap().0, model.vendor);
        assert_eq!(calls.last().unwrap().1["model"], model.id);
    }
    assert!(f.local.calls.lock().unwrap().is_empty());
    let c = call(&app, "route_catalog", json!({}));
    assert_eq!(
        c["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["validated_model"] == true)
            .count(),
        2
    );
}
#[test]
fn unverified_catalog_model_failure_never_retries_or_falls_back() {
    let f = F::new();
    let app = f.open();
    let generations = [
        f.configured(&app, Vendor::Openai, 1),
        f.configured(&app, Vendor::Anthropic, 3),
    ];
    for (i, (vendor, model, status, code)) in [
        (Vendor::Openai, "gpt-6-sol", 403, "CLOUD_AUTH"),
        (
            Vendor::Anthropic,
            "claude-opus-5-5",
            503,
            "CLOUD_UNAVAILABLE",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        *f.http.status.lock().unwrap() = Some(status);
        let n = i + 10;
        let expected_route = route(vendor, &generations[vendor.index()], model);
        let receipt = send(
            &app,
            args(
                &app,
                n,
                None,
                "explicit unverified send",
                expected_route.clone(),
            ),
        );
        let result = done(&app, n);
        assert_eq!(result["status"], "failed");
        assert_eq!(result["error"]["code"], code);
        let routing = call(
            &app,
            "request_routing",
            json!({"request_id":id(n),"instance_id":app.instance_id}),
        );
        assert_eq!(routing["route"], expected_route);
        assert_eq!(routing["delivery"], "received");
        assert_eq!(
            call(
                &app,
                "conversation_get",
                json!({"conversation_id":receipt["conversation_id"]})
            )["history"],
            json!([])
        );
        let calls = f.http.calls.lock().unwrap();
        assert_eq!(calls.len(), i + 3);
        assert_eq!(calls.last().unwrap().0, vendor);
        assert_eq!(calls.last().unwrap().1["model"], model);
    }
    assert!(f.local.calls.lock().unwrap().is_empty());
}
#[test]
fn first_transfer_exact_binding_replay_and_legacy_fences() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    let original = args(
        &app,
        3,
        None,
        "my private prompt",
        route(Vendor::Openai, &g, "gpt-6-astra"),
    );
    let p = call(&app, "route_prepare", original.clone());
    assert_eq!(p["requires_consent"], true);
    assert_eq!(p.as_object().unwrap().len(), 12);
    assert_eq!(call(&app, "route_prepare", original.clone()), p);
    let mut a = original.clone();
    a["approval_token"] = p["approval_token"].clone();
    a["confirm_transfer"] = json!(false);
    assert_eq!(
        app.call(req("submit_routed", a.clone())).unwrap_err().code,
        "CONSENT_REQUIRED"
    );
    a["confirm_transfer"] = json!(true);
    let mut changed = a.clone();
    changed["text"] = json!("different");
    assert_eq!(
        app.call(req("submit_routed", changed)).unwrap_err().code,
        "CONSENT_MISMATCH"
    );
    let receipt = call(&app, "submit_routed", a.clone());
    assert_eq!(receipt.as_object().unwrap().len(), 6);
    let target = receipt["conversation_id"].as_str().unwrap();
    assert_eq!(done(&app, 3)["status"], "succeeded");
    f.clock.advance(700);
    assert_eq!(call(&app, "submit_routed", a)["conversation_id"], target);
    assert_eq!(f.http.calls.lock().unwrap().len(), 2);
    for version in [1, 2] {
        let mut r = req("state", json!({}));
        r.schema_version = version;
        assert_eq!(app.call(r).unwrap_err().code, "UPGRADE_REQUIRED");
        let mut r = req(
            "poll",
            json!({"request_id":id(3),"instance_id":app.instance_id}),
        );
        r.schema_version = version;
        assert_eq!(app.call(r).unwrap_err().code, "UPGRADE_REQUIRED");
    }
    assert_eq!(
        call(
            &app,
            "request_routing",
            json!({"request_id":id(3),"instance_id":app.instance_id})
        )["delivery"],
        "received"
    );
}
#[test]
fn mixed_local_cloud_same_chat_and_model_switch_preserve_context_provenance() {
    let f = F::new();
    let app = f.open();
    f.local(&app);
    let openai = f.configured(&app, Vendor::Openai, 1);
    let anthropic = f.configured(&app, Vendor::Anthropic, 3);
    let local = json!({"provider":"local","model":CATALOG[0].name,"credential_generation":null});
    let r = send(&app, args(&app, 10, None, "local one", local.clone()));
    let target = r["conversation_id"].as_str().unwrap();
    assert_eq!(done(&app, 10)["status"], "succeeded");
    send(
        &app,
        args(
            &app,
            11,
            Some(target),
            "openai two",
            route(Vendor::Openai, &openai, "gpt-6-astra"),
        ),
    );
    assert_eq!(done(&app, 11)["status"], "succeeded");
    send(
        &app,
        args(
            &app,
            12,
            Some(target),
            "claude three",
            route(Vendor::Anthropic, &anthropic, "claude-sonnet-5-5"),
        ),
    );
    assert_eq!(done(&app, 12)["status"], "succeeded");
    let a = args(
        &app,
        13,
        Some(target),
        "haiku four",
        route(Vendor::Anthropic, &anthropic, "claude-haiku-5-5"),
    );
    assert_eq!(
        call(&app, "route_prepare", a.clone())["requires_consent"],
        false
    );
    send(&app, a);
    assert_eq!(done(&app, 13)["status"], "succeeded");
    send(&app, args(&app, 14, Some(target), "local five", local));
    assert_eq!(done(&app, 14)["status"], "succeeded");
    let calls = f.local.calls.lock().unwrap();
    assert_eq!(calls[1].1.len(), 9);
    assert_eq!(calls[1].1[3].content, "Cloud final answer");
    drop(calls);
    let r = call(
        &app,
        "conversation_routing",
        json!({"conversation_id":target}),
    );
    assert_eq!(r["approved_destinations"].as_array().unwrap().len(), 2);
    assert_eq!(r["turn_routes"].as_array().unwrap().len(), 5);
    assert_eq!(r["turn_routes"][3]["route"]["model"], "claude-haiku-5-5");
    let back = args(
        &app,
        15,
        Some(target),
        "back to approved",
        route(Vendor::Openai, &openai, "gpt-6-astra"),
    );
    assert_eq!(call(&app, "route_prepare", back)["requires_consent"], false);
}
#[test]
fn key_replacement_is_fresh_destination_without_deleting_history() {
    let f = F::new();
    let app = f.open();
    let old = f.configured(&app, Vendor::Openai, 1);
    let r = send(
        &app,
        args(
            &app,
            3,
            None,
            "old",
            route(Vendor::Openai, &old, "gpt-6-astra"),
        ),
    );
    assert_eq!(done(&app, 3)["status"], "succeeded");
    let target = r["conversation_id"].as_str().unwrap();
    let new = f.configured(&app, Vendor::Openai, 4);
    assert_ne!(old, new);
    assert_eq!(f.keys.items.lock().unwrap().len(), 2);
    let a = args(
        &app,
        6,
        Some(target),
        "new",
        route(Vendor::Openai, &new, "gpt-6-astra"),
    );
    assert_eq!(call(&app, "route_prepare", a)["requires_consent"], true);
    assert_eq!(
        call(&app, "conversation_get", json!({"conversation_id":target}))["history"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        app.call(req(
            "route_prepare",
            args(
                &app,
                7,
                Some(target),
                "old",
                route(Vendor::Openai, &old, "gpt-6-astra")
            )
        ))
        .unwrap_err()
        .code,
        "CREDENTIAL_CHANGED"
    );
}
#[test]
fn provider_prepare_expiry_is_monotonic_never_renewed() {
    let f = F::new();
    let app = f.open();
    let a = json!({"request_id":id(1),"instance_id":app.instance_id,"provider":"openai","action":"save","model":null,"expected_provider_revision":0});
    let p = call(&app, "provider_prepare", a.clone());
    f.clock.advance(60);
    assert_eq!(
        app.call(req("provider_prepare", a)).unwrap_err().code,
        "CONSENT_EXPIRED"
    );
    assert_eq!(app.call(req("provider_apply",json!({"request_id":id(1),"instance_id":app.instance_id,"approval_token":p["approval_token"],"api_key":"synthetic-secret-canary"}))).unwrap_err().code,"CONSENT_EXPIRED");
    assert_eq!(f.keys.saves.load(Ordering::SeqCst), 0);
}
#[test]
fn preparation_capacity_and_no_empty_chats() {
    let f = F::new();
    let app = f.open();
    for n in 1..=32 {
        call(
            &app,
            "provider_prepare",
            json!({"request_id":id(n),"instance_id":app.instance_id,"provider":"openai","action":"save","model":null,"expected_provider_revision":0}),
        );
    }
    assert_eq!(app.call(req("provider_prepare",json!({"request_id":id(33),"instance_id":app.instance_id,"provider":"openai","action":"save","model":null,"expected_provider_revision":0}))).unwrap_err().code,"CAPACITY");
    assert_eq!(call(&app, "state", json!({}))["conversations"], json!([]));
    assert!(!f.disk().exists());
}
#[test]
fn save_duplicate_public_binding_never_reads_or_changes_replacement_key() {
    let f = F::new();
    let app = f.open();
    let p = call(
        &app,
        "provider_prepare",
        json!({"request_id":id(1),"instance_id":app.instance_id,"provider":"openai","action":"save","model":null,"expected_provider_revision":0}),
    );
    let mut a = json!({"request_id":id(1),"instance_id":app.instance_id,"approval_token":p["approval_token"],"api_key":"synthetic-secret-canary"});
    call(&app, "provider_apply", a.clone());
    assert_eq!(done(&app, 1)["status"], "succeeded");
    a["api_key"] = json!(" invalid replacement\n");
    assert_eq!(
        call(&app, "provider_apply", a.clone())["status"],
        "succeeded"
    );
    assert_eq!(f.keys.saves.load(Ordering::SeqCst), 1);
    a["approval_token"] = json!(id(888));
    assert_eq!(
        app.call(req("provider_apply", a)).unwrap_err().code,
        "ID_CONFLICT"
    );
}
#[test]
fn keyring_failures_are_sanitized_probe_is_observation_and_no_http() {
    for code in [
        "KEYRING_LOCKED",
        "KEYRING_MISSING",
        "KEYRING_CONFLICT",
        "KEYRING_UNAVAILABLE",
        "KEYRING_PROMPT_REQUIRED",
        "KEYRING_TIMEOUT",
    ] {
        let f = F::new();
        let app = f.open();
        f.save(&app, Vendor::Openai, 1);
        *f.keys.failure.lock().unwrap() = Some(code.into());
        call(
            &app,
            "provider_probe",
            json!({"request_id":id(2),"instance_id":app.instance_id,"provider":"openai","expected_provider_revision":1}),
        );
        let r = done(&app, 2);
        assert_eq!(r["status"], "succeeded");
        assert_eq!(r["result"]["provider"]["credential_error"]["code"], code);
        assert!(!r.to_string().contains("synthetic-keyring-error-canary"));
        assert!(f.http.calls.lock().unwrap().is_empty());
        assert!(!fs::read_to_string(f.disk())
            .unwrap()
            .contains("synthetic-secret-canary"));
    }
}
#[test]
fn key_read_failures_stop_send_and_validation_before_http_then_repair_without_probe() {
    for vendor in Vendor::ALL {
        for code in [
            "KEYRING_LOCKED",
            "KEYRING_MISSING",
            "KEYRING_CONFLICT",
            "KEYRING_UNAVAILABLE",
            "KEYRING_PROMPT_REQUIRED",
            "KEYRING_TIMEOUT",
        ] {
            for validate in [false, true] {
                let f = F::new();
                let app = f.open();
                let generation = f.save(&app, vendor, 1);
                drop(app);
                let app = f.open();
                *f.keys.failure.lock().unwrap() = Some(code.into());
                let result = if validate {
                    f.validate(&app, vendor, default_model(vendor), 2)
                } else {
                    send(
                        &app,
                        args(
                            &app,
                            2,
                            None,
                            "unreadable key",
                            route(vendor, &generation, default_model(vendor)),
                        ),
                    );
                    done(&app, 2)
                };
                assert_eq!(result["status"], "failed");
                assert_eq!(result["error"]["code"], code);
                assert!(!result
                    .to_string()
                    .contains("synthetic-keyring-error-canary"));
                let routing = call(
                    &app,
                    "request_routing",
                    json!({"request_id":id(2),"instance_id":app.instance_id}),
                );
                assert_eq!(
                    routing["route"],
                    route(vendor, &generation, default_model(vendor))
                );
                assert_eq!(routing["delivery"], "not_sent");
                let catalog = call(&app, "route_catalog", json!({}));
                assert_eq!(
                    catalog["providers"][vendor.index()]["credential_error"]["code"],
                    code
                );
                assert_attemptable(&catalog, vendor, &generation);
                if validate {
                    assert_eq!(
                        catalog["providers"][vendor.index()]["validation"]["status"],
                        "failed"
                    );
                }
                assert_eq!(f.keys.reads.load(Ordering::SeqCst), 1);
                assert!(f.http.calls.lock().unwrap().is_empty());
                assert_eq!(done(&app, 2), result);
                *f.keys.failure.lock().unwrap() = None;
                let target = result["conversation_id"].as_str();
                let repaired = if validate {
                    f.validate(&app, vendor, default_model(vendor), 3)
                } else {
                    assert_eq!(
                        call(&app, "conversation_get", json!({"conversation_id":target}))
                            ["history"],
                        json!([])
                    );
                    send(
                        &app,
                        args(
                            &app,
                            3,
                            target,
                            "external repair; new attempt",
                            route(vendor, &generation, default_model(vendor)),
                        ),
                    );
                    done(&app, 3)
                };
                assert_eq!(repaired["status"], "succeeded", "{repaired}");
                assert_eq!(f.keys.reads.load(Ordering::SeqCst), 2);
                let identity = CredentialRef {
                    provider: vendor,
                    generation: generation.clone(),
                    save_request_id: id(1),
                };
                assert_eq!(
                    *f.keys.identities.lock().unwrap(),
                    vec![identity.clone(), identity]
                );
                assert_eq!(f.http.calls.lock().unwrap().len(), 1);
                assert_eq!(f.keys.saves.load(Ordering::SeqCst), 1);
                assert!(f.local.calls.lock().unwrap().is_empty());
                assert!(!fs::read_to_string(f.disk())
                    .unwrap()
                    .contains("synthetic-secret-canary"));
            }
        }
    }
}
#[test]
fn failed_validation_keeps_routes_attemptable_and_auth_quota_fail_once_per_attempt() {
    for vendor in Vendor::ALL {
        let f = F::new();
        let app = f.open();
        let generation = f.save(&app, vendor, 1);
        let mut attempts = 0;
        for (status, code) in [(401, "CLOUD_AUTH"), (402, "CLOUD_QUOTA")] {
            *f.http.status.lock().unwrap() = Some(status);
            let n = 2 + attempts;
            let result = f.validate(&app, vendor, default_model(vendor), n);
            attempts += 1;
            assert_eq!(result["status"], "failed");
            assert_eq!(result["error"]["code"], code);
            let catalog = call(&app, "route_catalog", json!({}));
            let validation = catalog["providers"][vendor.index()]["validation"].clone();
            assert_eq!(validation["status"], "failed");
            assert_eq!(validation["delivery"], "received");
            assert_attemptable(&catalog, vendor, &generation);
            assert_eq!(f.http.calls.lock().unwrap().len(), attempts);
            let n = 2 + attempts;
            send(
                &app,
                args(
                    &app,
                    n,
                    None,
                    "explicit send after failed validation",
                    route(vendor, &generation, default_model(vendor)),
                ),
            );
            let result = done(&app, n);
            attempts += 1;
            assert_eq!(result["status"], "failed");
            assert_eq!(result["error"]["code"], code);
            assert_eq!(
                call(
                    &app,
                    "request_routing",
                    json!({"request_id":id(n),"instance_id":app.instance_id})
                )["delivery"],
                "received"
            );
            assert_eq!(done(&app, n), result);
            let catalog = call(&app, "route_catalog", json!({}));
            assert_eq!(
                catalog["providers"][vendor.index()]["validation"],
                validation
            );
            assert_attemptable(&catalog, vendor, &generation);
            assert_eq!(f.http.calls.lock().unwrap().len(), attempts);
            assert_eq!(f.keys.reads.load(Ordering::SeqCst), attempts);
        }
        assert_eq!(f.keys.items.lock().unwrap().len(), 1);
        assert!(f.local.calls.lock().unwrap().is_empty());
    }
}
#[test]
fn queued_routes_are_immutable_global_management_busy_and_no_expired_dispatch() {
    let f = F::new();
    let app = f.open();
    f.local(&app);
    let g = f.configured(&app, Vendor::Openai, 1);
    f.local.gate.store(true, Ordering::SeqCst);
    let local = json!({"provider":"local","model":CATALOG[1].name,"credential_generation":null});
    send(&app, args(&app, 3, None, "block worker", local));
    wait(&f.local.entered);
    let cloud = route(Vendor::Openai, &g, "gpt-6-astra");
    let queued = send(&app, args(&app, 4, None, "queued", cloud.clone()));
    assert_eq!(queued["status"], "queued");
    for n in 5..=6 {
        send(&app, args(&app, n, None, "queued", cloud.clone()));
    }
    assert_eq!(
        app.call(req("route_prepare", args(&app, 7, None, "full", cloud)))
            .unwrap_err()
            .code,
        "BUSY"
    );
    assert_eq!(app.call(req("provider_prepare",json!({"request_id":id(8),"instance_id":app.instance_id,"provider":"openai","action":"save","model":null,"expected_provider_revision":1}))).unwrap_err().code,"BUSY");
    assert_eq!(
        call(
            &app,
            "request_routing",
            json!({"request_id":id(4),"instance_id":app.instance_id})
        )["route"]["model"],
        "gpt-6-astra"
    );
    f.clock.advance(601);
    f.local.gate.store(false, Ordering::SeqCst);
    assert_eq!(done(&app, 3)["status"], "succeeded");
    for n in 4..=6 {
        assert_eq!(done(&app, n)["error"]["code"], "PERMISSION_EXPIRED");
    }
    assert_eq!(f.http.calls.lock().unwrap().len(), 1);
    let routing = call(
        &app,
        "conversation_routing",
        json!({"conversation_id":queued["conversation_id"]}),
    );
    assert_eq!(
        routing["approved_destinations"].as_array().unwrap().len(),
        1
    );
}
#[test]
fn restart_preserves_history_validation_and_unknown_delivery_without_replay() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    f.http.gate.store(true, Ordering::SeqCst);
    f.http.entered.store(false, Ordering::SeqCst);
    let receipt = send(
        &app,
        args(
            &app,
            3,
            None,
            "ambiguous",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    wait(&f.http.entered);
    assert_eq!(
        call(
            &app,
            "request_routing",
            json!({"request_id":id(3),"instance_id":app.instance_id})
        )["delivery"],
        "may_have_sent"
    );
    let before = Instant::now();
    drop(app);
    assert!(before.elapsed() < Duration::from_secs(2));
    f.http.gate.store(false, Ordering::SeqCst);
    let app = f.open();
    let r = done(&app, 3);
    assert_eq!(r["status"], "interrupted");
    assert_eq!(r["error"]["code"], "REMOTE_OUTCOME_UNKNOWN");
    assert_eq!(f.http.calls.lock().unwrap().len(), 2);
    let c = call(&app, "route_catalog", json!({}));
    assert_eq!(c["providers"][0]["credential_state"], "not_observed");
    assert_attemptable(&c, Vendor::Openai, &g);
    assert_eq!(
        call(
            &app,
            "conversation_get",
            json!({"conversation_id":receipt["conversation_id"]})
        )["history"],
        json!([])
    );
}
fn interrupted_validation(f: &F) -> Application {
    let app = f.open();
    f.save(&app, Vendor::Openai, 1);
    f.http.gate.store(true, Ordering::SeqCst);
    f.begin_validation(&app, Vendor::Openai, "gpt-6-astra", 2);
    wait(&f.http.entered);
    drop(app);
    f.http.gate.store(false, Ordering::SeqCst);
    f.clock.advance(11);
    let app = f.open();
    let result = done(&app, 2);
    assert_eq!(result["status"], "interrupted");
    assert_eq!(result["error"]["code"], "REMOTE_OUTCOME_UNKNOWN");
    let validation = call(&app, "route_catalog", json!({}))["providers"][0]["validation"].clone();
    assert_eq!(validation["request_id"], id(2));
    assert_eq!(validation["status"], "interrupted");
    assert_eq!(validation["completed_ms"], f.clock.wall_ms());
    assert_eq!(validation["delivery"], "may_have_sent");
    assert_eq!(f.http.calls.lock().unwrap().len(), 1);
    app
}
fn assert_restart_validation_unchanged(f: &F, app: Application) {
    let validation = call(&app, "route_catalog", json!({}))["providers"][0]["validation"].clone();
    let bytes = fs::read(f.disk()).unwrap();
    let calls = f.http.calls.lock().unwrap().len();
    let reads = f.keys.reads.load(Ordering::SeqCst);
    drop(app);
    for _ in 0..3 {
        f.clock.advance(17);
        let app = f.open();
        assert_eq!(
            call(&app, "route_catalog", json!({}))["providers"][0]["validation"],
            validation
        );
        assert_eq!(fs::read(f.disk()).unwrap(), bytes);
        assert_eq!(f.http.calls.lock().unwrap().len(), calls);
        assert_eq!(f.keys.reads.load(Ordering::SeqCst), reads);
    }
}
#[test]
fn interrupted_validation_restart_preserves_completion_time_and_durable_bytes() {
    let f = F::new();
    let app = interrupted_validation(&f);
    assert_restart_validation_unchanged(&f, app);
}
#[test]
fn older_interrupted_validation_never_overwrites_newer_success_on_restart() {
    let f = F::new();
    let app = interrupted_validation(&f);
    f.clock.advance(13);
    assert_eq!(
        f.validate(&app, Vendor::Openai, "gpt-6-astra", 3)["status"],
        "succeeded"
    );
    let validation = call(&app, "route_catalog", json!({}))["providers"][0]["validation"].clone();
    assert_eq!(validation["request_id"], id(3));
    assert_eq!(validation["status"], "succeeded");
    assert_eq!(validation["completed_ms"], f.clock.wall_ms());
    assert_restart_validation_unchanged(&f, app);
}
#[test]
fn older_interrupted_validation_never_overwrites_newer_failure_on_restart() {
    let f = F::new();
    let app = interrupted_validation(&f);
    f.clock.advance(13);
    *f.http.status.lock().unwrap() = Some(401);
    assert_eq!(
        f.validate(&app, Vendor::Openai, "gpt-6-astra", 3)["status"],
        "failed"
    );
    let validation = call(&app, "route_catalog", json!({}))["providers"][0]["validation"].clone();
    assert_eq!(validation["request_id"], id(3));
    assert_eq!(validation["status"], "failed");
    assert_eq!(validation["error"]["code"], "CLOUD_AUTH");
    assert_eq!(validation["completed_ms"], f.clock.wall_ms());
    assert_restart_validation_unchanged(&f, app);
}
#[test]
fn storage_failure_before_marker_means_zero_chat_http_and_fenced_state() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    f.keys.entered.store(false, Ordering::SeqCst);
    f.keys.gate.store(true, Ordering::SeqCst);
    send(
        &app,
        args(
            &app,
            3,
            None,
            "never transmitted",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    wait(&f.keys.entered);
    fs::set_permissions(f.disk(), fs::Permissions::from_mode(0o644)).unwrap();
    f.keys.gate.store(false, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if app.call(req("state", json!({}))).is_err() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(f.http.calls.lock().unwrap().len(), 1);
}
#[test]
fn logical_delete_removes_all_routing_context_and_replay_content() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    let r = send(
        &app,
        args(
            &app,
            3,
            None,
            "unique-delete-canary",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    assert_eq!(done(&app, 3)["status"], "succeeded");
    call(
        &app,
        "conversation_delete",
        json!({"request_id":id(4),"conversation_id":r["conversation_id"],"expected_metadata_revision":0,"expected_history_revision":1}),
    );
    let disk = fs::read_to_string(f.disk()).unwrap();
    assert!(!disk.contains("unique-delete-canary"));
    assert_eq!(
        serde_json::from_str::<Value>(&disk).unwrap()["cloud"]["conversations"],
        json!({})
    );
    assert_eq!(
        app.call(req(
            "request_routing",
            json!({"request_id":id(3),"instance_id":app.instance_id})
        ))
        .unwrap_err()
        .code,
        "REQUEST_EXPIRED"
    );
}
#[test]
fn v2_disk_migration_preserves_unknown_models_u64_and_rejects_missing_v3_extension() {
    let f = F::new();
    let old = json!({"schema_version":2,"config":{"model":"historical:unavailable","voice":null,"revision":u64::MAX},"conversations":[],"jobs":[],"expired_ids":[],"legacy_conversation_id":null,"revision":0,"next_sequence":0});
    fs::write(f.disk(), serde_json::to_vec(&old).unwrap()).unwrap();
    fs::set_permissions(f.disk(), fs::Permissions::from_mode(0o600)).unwrap();
    let app = f.open();
    assert_eq!(
        call(&app, "state", json!({}))["config"]["revision"],
        u64::MAX.to_string()
    );
    drop(app);
    let mut disk: Value = serde_json::from_slice(&fs::read(f.disk()).unwrap()).unwrap();
    assert_eq!(disk["schema_version"], 4);
    disk.as_object_mut().unwrap().remove("cloud");
    let bytes = serde_json::to_vec(&disk).unwrap();
    fs::write(f.disk(), &bytes).unwrap();
    assert!(Application::open(&f.roots, f.local.clone()).is_err());
    assert_eq!(fs::read(f.disk()).unwrap(), bytes);
}
#[test]
fn expiry_after_durable_marker_still_prevents_http_and_retains_destination_approval() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    *f.clock.jump_after_marker.lock().unwrap() = Some(f.disk());
    let r = send(
        &app,
        args(
            &app,
            3,
            None,
            "expire during publication",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    assert_eq!(done(&app, 3)["error"]["code"], "PERMISSION_EXPIRED");
    assert_eq!(f.http.calls.lock().unwrap().len(), 1);
    assert_eq!(
        call(
            &app,
            "request_routing",
            json!({"request_id":id(3),"instance_id":app.instance_id})
        )["delivery"],
        "not_sent"
    );
    assert_eq!(
        call(
            &app,
            "conversation_routing",
            json!({"conversation_id":r["conversation_id"]})
        )["approved_destinations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn failed_credential_activation_preserves_old_generation_and_never_adopts_orphan() {
    let f = F::new();
    let app = f.open();
    let old = f.save(&app, Vendor::Openai, 1);
    let p = call(
        &app,
        "provider_prepare",
        json!({"request_id":id(2),"instance_id":app.instance_id,"provider":"openai","action":"save","model":null,"expected_provider_revision":1}),
    );
    *f.keys.fail_activation.lock().unwrap() = Some(f.disk());
    call(
        &app,
        "provider_apply",
        json!({"request_id":id(2),"instance_id":app.instance_id,"approval_token":p["approval_token"],"api_key":"synthetic-secret-canary"}),
    );
    let end = Instant::now() + Duration::from_secs(3);
    while app.call(req("state", json!({}))).is_ok() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    drop(app);
    fs::set_permissions(f.disk(), fs::Permissions::from_mode(0o600)).unwrap();
    let app = f.open();
    assert_eq!(done(&app, 2)["error"]["code"], "CREDENTIAL_OUTCOME_UNKNOWN");
    assert_eq!(
        call(&app, "route_catalog", json!({}))["providers"][0]["credential_generation"],
        old
    );
    assert_eq!(f.keys.items.lock().unwrap().len(), 2);
    assert_eq!(f.keys.saves.load(Ordering::SeqCst), 2);
}
#[test]
fn remote_reply_before_failed_commit_is_unknown_after_restart_and_not_replayed() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    *f.http.fail_completion.lock().unwrap() = Some(f.disk());
    let receipt = send(
        &app,
        args(
            &app,
            3,
            None,
            "remote reply not committed",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    let end = Instant::now() + Duration::from_secs(3);
    while app.call(req("state", json!({}))).is_ok() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    drop(app);
    fs::set_permissions(f.disk(), fs::Permissions::from_mode(0o600)).unwrap();
    let app = f.open();
    assert_eq!(done(&app, 3)["error"]["code"], "REMOTE_OUTCOME_UNKNOWN");
    assert_eq!(f.http.calls.lock().unwrap().len(), 2);
    assert_eq!(
        call(
            &app,
            "conversation_get",
            json!({"conversation_id":receipt["conversation_id"]})
        )["history"],
        json!([])
    );
}
#[test]
fn vendor_cannot_publish_the_authentication_secret_as_final_text() {
    let f = F::new();
    let app = f.open();
    f.save(&app, Vendor::Openai, 1);
    *f.http.answer.lock().unwrap() = Some("synthetic-secret-canary".into());
    let r = f.validate(&app, Vendor::Openai, "gpt-6-astra", 2);
    assert_eq!(r["error"]["code"], "CLOUD_PROTOCOL");
    assert!(!fs::read_to_string(f.disk())
        .unwrap()
        .contains("synthetic-secret-canary"));
}
#[test]
fn strict_v3_keys_and_secret_redacted_debug() {
    let f = F::new();
    let app = f.open();
    for op in [
        "route_catalog",
        "conversation_routing",
        "provider_probe",
        "provider_prepare",
        "provider_apply",
        "route_prepare",
        "submit_routed",
        "request_routing",
    ] {
        assert!(app.call(req(op, json!({"extra":true}))).is_err());
    }
    let request = req(
        "provider_apply",
        json!({"api_key":"synthetic-secret-canary"}),
    );
    assert!(!format!("{request:?}").contains("synthetic-secret-canary"));
    assert_eq!(
        Secret::new(b"invalid key".to_vec()).err().unwrap().code,
        "INVALID_REQUEST"
    );
}

#[test]
fn timestamps_routed_local_cloud_clocks_context_and_observation_are_independent() {
    use sha2::Digest;
    let f = F::new();
    let app = f.open();
    f.local(&app);
    let g = f.configured(&app, Vendor::Openai, 1);
    let local = json!({"provider":"local","model":CATALOG[0].name,"credential_generation":null});
    f.local.gate.store(true, Ordering::SeqCst);
    let local_admission = f.clock.wall_ms();
    let receipt = send(&app, args(&app, 10, None, "local first", local));
    let target = receipt["conversation_id"].as_str().unwrap();
    wait(&f.local.entered);
    let query = || {
        call(
            &app,
            "conversation_timestamps",
            json!({"conversation_id":target}),
        )
    };
    assert_eq!(query()["turns"], json!([]));
    f.clock.advance(7);
    f.local.gate.store(false, Ordering::SeqCst);
    assert_eq!(done(&app, 10)["status"], "succeeded");
    let local_completion = f.clock.wall_ms();
    assert_eq!(
        query()["turns"],
        json!([{"user_at_ms":local_admission,"assistant_at_ms":local_completion}])
    );

    let mut a = args(
        &app,
        11,
        Some(target),
        "cloud second",
        route(Vendor::Openai, &g, "gpt-6-astra"),
    );
    let prepared = call(&app, "route_prepare", a.clone());
    let mut context: Vec<Message> = serde_json::from_value(
        call(&app, "conversation_get", json!({"conversation_id":target}))["history"].clone(),
    )
    .unwrap();
    context.push(Message {
        role: "user".into(),
        content: "cloud second".into(),
    });
    let hash = format!(
        "{:x}",
        sha2::Sha256::digest(serde_json::to_vec(&context).unwrap())
    );
    assert_eq!(prepared["context_sha256"], hash);
    a["approval_token"] = prepared["approval_token"].clone();
    a["confirm_transfer"] = prepared["requires_consent"].clone();
    f.http.gate.store(true, Ordering::SeqCst);
    f.http.entered.store(false, Ordering::SeqCst);
    let cloud_admission = f.clock.wall_ms();
    call(&app, "submit_routed", a.clone());
    wait(&f.http.entered);
    assert_eq!(query()["turns"].as_array().unwrap().len(), 1);
    f.clock.advance(5);
    f.http.gate.store(false, Ordering::SeqCst);
    assert_eq!(done(&app, 11)["status"], "succeeded");
    let expected = json!({"conversation_id":target,"history_revision":2,"turns":[
        {"user_at_ms":local_admission,"assistant_at_ms":local_completion},
        {"user_at_ms":cloud_admission,"assistant_at_ms":f.clock.wall_ms()}
    ]});
    assert_eq!(query(), expected);
    let expected_body: Value = serde_json::from_slice(
        &cloud::build_body(Vendor::Openai, "gpt-6-astra", &context).unwrap(),
    )
    .unwrap();
    assert_eq!(
        f.http.calls.lock().unwrap().last().unwrap().1,
        expected_body
    );
    let routed = call(
        &app,
        "request_routing",
        json!({"request_id":id(11),"instance_id":app.instance_id}),
    );
    assert_eq!(routed["context_sha256"], hash);
    let disk = fs::read(f.disk()).unwrap();
    let reads = f.keys.reads.load(Ordering::SeqCst);
    let saves = f.keys.saves.load(Ordering::SeqCst);
    let http_calls = f.http.calls.lock().unwrap().len();
    f.clock.advance(19);
    assert_eq!(call(&app, "submit_routed", a)["status"], "succeeded");
    for _ in 0..3 {
        assert_eq!(query(), expected);
    }
    assert_eq!(fs::read(f.disk()).unwrap(), disk);
    let target = target.to_owned();
    drop(app);
    let app = f.open();
    assert_eq!(
        call(
            &app,
            "conversation_timestamps",
            json!({"conversation_id":target})
        ),
        expected
    );
    assert_eq!(fs::read(f.disk()).unwrap(), disk);
    assert_eq!(f.keys.reads.load(Ordering::SeqCst), reads);
    assert_eq!(f.keys.saves.load(Ordering::SeqCst), saves);
    assert_eq!(f.http.calls.lock().unwrap().len(), http_calls);
}

#[test]
fn timestamps_disk3_migration_preserves_routed_receipts_and_never_dates_old_pairs() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    let r = send(
        &app,
        args(
            &app,
            3,
            None,
            "old routed chat",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    let terminal = done(&app, 3);
    assert_eq!(terminal["status"], "succeeded");
    let target = &r["conversation_id"];
    let history = call(&app, "conversation_get", json!({"conversation_id":target}));
    let routing = call(
        &app,
        "conversation_routing",
        json!({"conversation_id":target}),
    );
    drop(app);
    let mut old: Value = serde_json::from_slice(&fs::read(f.disk()).unwrap()).unwrap();
    old["schema_version"] = json!(3);
    old.as_object_mut().unwrap().remove("timestamps");
    fs::write(f.disk(), serde_json::to_vec(&old).unwrap()).unwrap();
    let calls = f.http.calls.lock().unwrap().len();
    f.clock.advance(100);
    let app = f.open();
    assert_eq!(done(&app, 3), terminal);
    assert_eq!(
        call(&app, "conversation_get", json!({"conversation_id":target})),
        history
    );
    assert_eq!(
        call(
            &app,
            "conversation_routing",
            json!({"conversation_id":target})
        ),
        routing
    );
    assert_eq!(
        call(
            &app,
            "conversation_timestamps",
            json!({"conversation_id":target})
        )["turns"],
        json!([{"user_at_ms":null,"assistant_at_ms":null}])
    );
    assert_eq!(f.http.calls.lock().unwrap().len(), calls);
    let saved: Value = serde_json::from_slice(&fs::read(f.disk()).unwrap()).unwrap();
    assert_eq!(saved["schema_version"], 4);
    assert_eq!(saved["cloud"], old["cloud"]);
    assert_eq!(saved["jobs"], old["jobs"]);
}

#[test]
fn timestamps_failed_cloud_completion_never_appends_or_restamps_history() {
    let f = F::new();
    let app = f.open();
    let g = f.configured(&app, Vendor::Openai, 1);
    let r = send(
        &app,
        args(
            &app,
            3,
            None,
            "retained",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    assert_eq!(done(&app, 3)["status"], "succeeded");
    let target = r["conversation_id"].as_str().unwrap();
    let original = call(
        &app,
        "conversation_timestamps",
        json!({"conversation_id":target}),
    );
    f.http.failure.store(true, Ordering::SeqCst);
    f.clock.advance(3);
    send(
        &app,
        args(
            &app,
            4,
            Some(target),
            "fails",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    assert_eq!(done(&app, 4)["status"], "failed");
    assert_eq!(
        call(
            &app,
            "conversation_timestamps",
            json!({"conversation_id":target})
        ),
        original
    );
    f.http.failure.store(false, Ordering::SeqCst);
    *f.http.fail_completion.lock().unwrap() = Some(f.disk());
    send(
        &app,
        args(
            &app,
            5,
            Some(target),
            "unpublished",
            route(Vendor::Openai, &g, "gpt-6-astra"),
        ),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.call(req("state", json!({}))).is_ok() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    drop(app);
    fs::set_permissions(f.disk(), fs::Permissions::from_mode(0o600)).unwrap();
    let app = f.open();
    assert_eq!(done(&app, 5)["status"], "interrupted");
    assert_eq!(
        call(
            &app,
            "conversation_timestamps",
            json!({"conversation_id":target})
        ),
        original
    );
}
