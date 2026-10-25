use super::*;
use crate::cloud::{
    CredentialRef, CredentialStore, Dependencies, HttpFailure, HttpResponse, HttpTransport, Secret,
};
use crate::providers::{Inventory, Model, Observation};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct Local {
    inventory: AtomicUsize,
    calls: Mutex<Vec<Vec<Message>>>,
    gate: AtomicBool,
    entered: AtomicBool,
}
impl Provider for Local {
    fn inventory(&self) -> AppResult<Inventory> {
        self.inventory.fetch_add(1, Ordering::SeqCst);
        let mut i = Inventory {
            models: crate::catalog::CATALOG[..2]
                .iter()
                .map(|m| Model {
                    name: m.name.into(),
                    digest: m.sha256.into(),
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
    fn chat(&self, _: &str, messages: &[Message]) -> AppResult<String> {
        self.calls.lock().unwrap().push(messages.to_vec());
        self.entered.store(true, Ordering::SeqCst);
        while self.gate.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok("Final only".into())
    }
}
#[derive(Default)]
struct External {
    reads: AtomicUsize,
    saves: AtomicUsize,
    calls: Mutex<Vec<(Vendor, Value)>>,
}
impl CredentialStore for External {
    fn read(&self, _: &CredentialRef, _: Instant, _: &AtomicBool) -> AppResult<Secret> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Secret::new(b"synthetic-key-canary".to_vec())
    }
    fn save(&self, _: &CredentialRef, _: &Secret, _: Instant, _: &AtomicBool) -> AppResult<()> {
        self.saves.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl HttpTransport for External {
    fn post(
        &self,
        vendor: Vendor,
        body: Vec<u8>,
        _: &Secret,
        _: Instant,
        _: Instant,
        _: &AtomicBool,
    ) -> std::result::Result<HttpResponse, HttpFailure> {
        let body: Value = serde_json::from_slice(&body).unwrap();
        let model = body["model"].clone();
        self.calls.lock().unwrap().push((vendor, body));
        let response = match vendor {
            Vendor::Openai => {
                json!({"object":"response","model":model,"status":"completed","error":null,"incomplete_details":null,"output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Cloud final"}]}]})
            }
            Vendor::Anthropic => {
                json!({"type":"message","model":model,"role":"assistant","stop_reason":"end_turn","stop_sequence":null,"content":[{"type":"text","text":"Cloud final"}]})
            }
        };
        Ok(HttpResponse {
            status: 200,
            header_bytes: 0,
            body: serde_json::to_vec(&response).unwrap(),
        })
    }
}
struct F {
    base: PathBuf,
    roots: Roots,
    local: Arc<Local>,
    external: Arc<External>,
}
impl F {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("smith-instructions-{}", new_id().unwrap()));
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
            local: Arc::new(Local::default()),
            external: Arc::new(External::default()),
        }
    }
    fn open(&self) -> Application {
        Application::open_with_cloud(
            &self.roots,
            self.local.clone(),
            Dependencies::new(self.external.clone(), self.external.clone()),
        )
        .unwrap()
    }
    fn path(&self) -> PathBuf {
        self.roots.data.join("smith-next/app-state.json")
    }
    fn configure(&self, a: &Application) {
        invoke(a, 3, "models", json!({"request_id":id(900)})).unwrap();
        done(a, 900);
        invoke(
            a,
            3,
            "configure",
            json!({"model":crate::catalog::CATALOG[0].name,"voice":null}),
        )
        .unwrap();
    }
    fn save(&self, a: &Application, v: Vendor, n: u64) -> String {
        let revision = invoke(a, 3, "route_catalog", json!({})).unwrap()["providers"][v.index()]
            ["revision"]
            .clone();
        let p=invoke(a,3,"provider_prepare",json!({"request_id":id(n),"instance_id":a.instance_id,"provider":v,"action":"save","model":null,"expected_provider_revision":revision})).unwrap();
        invoke(a,3,"provider_apply",json!({"request_id":id(n),"instance_id":a.instance_id,"approval_token":p["approval_token"],"api_key":"synthetic-key-canary"})).unwrap();
        done(a, n)["result"]["provider"]["credential_generation"]
            .as_str()
            .unwrap()
            .into()
    }
}
impl Drop for F {
    fn drop(&mut self) {
        self.local.gate.store(false, Ordering::SeqCst);
        fs::remove_dir_all(&self.base).unwrap();
    }
}
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn invoke(a: &Application, v: u32, op: &str, args: Value) -> AppResult<Value> {
    a.call(AppRequest {
        schema_version: v,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(999),
        op: op.into(),
        args,
    })
}
fn get(a: &Application) -> Value {
    invoke(a, 3, "instructions_get", json!({"route":null})).unwrap()
}
fn settings(text: &str, cloud: bool) -> Value {
    json!({"instructions":text,"personality":"","guardrails":"","cloud_enabled":cloud})
}
fn update_args(a: &Application, n: u64, s: Value) -> Value {
    json!({"request_id":id(n),"instance_id":a.instance_id,"expected_config_revision":get(a)["config_revision"],"settings":s})
}
fn update(a: &Application, n: u64, s: Value) -> Value {
    invoke(a, 3, "instructions_update", update_args(a, n, s)).unwrap()
}
fn done(a: &Application, n: u64) -> Value {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let p = invoke(
            a,
            3,
            "poll",
            json!({"request_id":id(n),"instance_id":a.instance_id}),
        )
        .unwrap();
        if !matches!(p["status"].as_str(), Some("queued" | "running")) {
            assert_eq!(p["status"], "succeeded", "{p}");
            return p;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn routed(a: &Application, n: u64, target: Option<&str>, route: Value) -> Value {
    let history = target.map(|t| {
        invoke(a, 3, "conversation_get", json!({"conversation_id":t})).unwrap()["conversation"]
            ["history_revision"]
            .clone()
    });
    json!({"request_id":id(n),"instance_id":a.instance_id,"conversation_id":target,"text":"Current question","route":route,"expected_config_revision":get(a)["config_revision"],"expected_history_revision":history})
}
fn send(a: &Application, mut args: Value) -> Value {
    let p = invoke(a, 3, "route_prepare", args.clone()).unwrap();
    args["approval_token"] = p["approval_token"].clone();
    args["confirm_transfer"] = p["requires_consent"].clone();
    invoke(a, 3, "submit_routed", args).unwrap()
}

#[test]
fn public_pack_personal_bound_and_fixed_vessel_are_exact() {
    assert!(
        CORE.len() + PRODUCT.len() <= MAX_PUBLIC,
        "public pack bytes: {}",
        CORE.len() + PRODUCT.len()
    );
    let mut s = Settings {
        instructions: "x".repeat(1022),
        personality: "p".into(),
        guardrails: "g".into(),
        cloud_enabled: false,
    };
    assert!(s.valid());
    assert!(
        s.render(true).len() <= 2048,
        "preamble bytes: {}",
        s.render(true).len()
    );
    s.instructions.push('x');
    assert!(!s.valid());
    s.instructions = "\r".into();
    assert!(!s.valid());
    s.instructions = "\n\t🙂".into();
    assert!(s.valid());
    assert!(Snapshot::new(&s).valid());
}
#[test]
fn inspector_is_strict_read_only_and_never_omits_local_only_text() {
    let f = F::new();
    let a = f.open();
    let initial_disk = fs::read(f.path()).unwrap();
    let before = get(&a);
    assert_eq!(fs::read(f.path()).unwrap(), initial_disk);
    assert_eq!(before["vessel"], Value::Null);
    for v in [1, 2] {
        assert_eq!(
            invoke(&a, v, "instructions_get", json!({"route":null}))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }
    assert!(invoke(&a, 3, "instructions_get", json!({})).is_err());
    for args in [
        json!([null]),
        json!({"route":["local",crate::catalog::CATALOG[0].name,null]}),
    ] {
        assert_eq!(
            invoke(&a, 3, "instructions_get", args).unwrap_err().code,
            "INVALID_REQUEST"
        );
    }
    assert_eq!(
        invoke(
            &a,
            3,
            "instructions_update",
            update_args(&a, 99, json!(["canary", "", "", false]))
        )
        .unwrap_err()
        .code,
        "INVALID_REQUEST"
    );
    assert_eq!(fs::read(f.path()).unwrap(), initial_disk);
    update(&a, 1, settings("private-instructions-canary", false));
    let bytes = fs::read(f.path()).unwrap();
    for model in ["gpt-6-astra", "gpt-6-sol"] {
        let p = invoke(
            &a,
            3,
            "instructions_get",
            json!({"route":{"provider":"openai","model":model,"credential_generation":null}}),
        )
        .unwrap();
        assert_eq!(p["vessel"], VESSEL);
        assert_eq!(
            p["local"],
            json!({"text":p["cloud"]["text"],"sha256":p["cloud"]["sha256"]})
        );
        assert!(p["cloud"]["text"]
            .as_str()
            .unwrap()
            .contains("private-instructions-canary"));
        assert_eq!(p["cloud"]["eligible"], false);
        assert_eq!(p["cloud"]["personal_included"], true);
    }
    assert_eq!(bytes, fs::read(f.path()).unwrap());
    assert_eq!(f.local.inventory.load(Ordering::SeqCst), 0);
    assert_eq!(f.external.reads.load(Ordering::SeqCst), 0);
    assert!(f.external.calls.lock().unwrap().is_empty());
}
#[test]
fn updates_are_revision_fenced_idempotent_and_receipts_survive_restart() {
    let f = F::new();
    let a = f.open();
    let original = update_args(&a, 1, settings("first", false));
    let r = invoke(&a, 3, "instructions_update", original.clone()).unwrap();
    assert_eq!(
        r,
        json!({"request_id":id(1),"config_revision":1,"changed":true})
    );
    update(&a, 2, settings("second", false));
    assert_eq!(
        invoke(&a, 3, "instructions_update", original.clone()).unwrap(),
        r
    );
    assert_eq!(get(&a)["settings"]["instructions"], "second");
    let noop = update(&a, 3, settings("second", false));
    assert_eq!(noop["changed"], false);
    assert_eq!(noop["config_revision"], 2);
    let mut conflict = original.clone();
    conflict["settings"] = settings("different", false);
    assert_eq!(
        invoke(&a, 3, "instructions_update", conflict)
            .unwrap_err()
            .code,
        "ID_CONFLICT"
    );
    let mut stale = original.clone();
    stale["request_id"] = json!(id(4));
    assert_eq!(
        invoke(&a, 3, "instructions_update", stale)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    drop(a);
    let a = f.open();
    assert_eq!(invoke(&a, 3, "instructions_update", original).unwrap(), r);
    assert_eq!(
        invoke(&a, 3, "instructions_receipt", json!({"request_id":id(1)})).unwrap(),
        r
    );
    assert_eq!(
        invoke(&a, 3, "instructions_receipt", json!({"request_id":id(88)}))
            .unwrap_err()
            .code,
        "REQUEST_UNKNOWN"
    );
    let mut old = update_args(&a, 5, settings("third", false));
    old["instance_id"] = json!(id(9999));
    assert_eq!(
        invoke(&a, 3, "instructions_update", old).unwrap_err().code,
        "INSTANCE_MISMATCH"
    );
    let disk: Value = serde_json::from_slice(&fs::read(f.path()).unwrap()).unwrap();
    assert!(!disk["instructions"]["receipts"]
        .to_string()
        .contains("first"));
    assert!(!disk["instructions"]["receipts"]
        .to_string()
        .contains("second"));
}
#[test]
fn all_local_admissions_capture_one_system_message_and_diagnostics_stay_synthetic() {
    let f = F::new();
    let a = f.open();
    f.configure(&a);
    update(&a, 1, settings("personal-context", false));
    for v in [1, 2] {
        let n = 10 + u64::from(v);
        let args = if v == 1 {
            json!({"request_id":id(n),"text":"legacy question","output":"chat"})
        } else {
            json!({"request_id":id(n),"text":"new question","output":"chat","expected_config_revision":get(&a)["config_revision"]})
        };
        invoke(&a, v, if v == 1 { "submit" } else { "submit_new" }, args).unwrap();
        done(&a, n);
    }
    let local = json!({"provider":"local","model":crate::catalog::CATALOG[0].name,"credential_generation":null});
    send(&a, routed(&a, 13, None, local));
    done(&a, 13);
    let m = &crate::catalog::CATALOG[0];
    invoke(&a,3,"verify_model",json!({"request_id":id(14),"model":m.name,"sha256":m.sha256,"expected_revision":get(&a)["config_revision"]})).unwrap();
    done(&a, 14);
    let calls = f.local.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    for c in &calls[..3] {
        assert_eq!(c[0].role, "system");
        assert!(c[0].content.contains("personal-context"));
        assert!(c[0].content.ends_with(VESSEL));
        assert_eq!(c.iter().filter(|m| m.role == "system").count(), 1);
    }
    assert_eq!(calls[3].len(), 1);
    assert_eq!(calls[3][0].content, crate::cloud::VALIDATION_TEXT);
    let inner = a.inner.lock().unwrap();
    for n in [11, 12, 13] {
        assert!(inner.durable.instructions.snapshot(&id(n)).is_some());
    }
    assert!(inner.durable.instructions.snapshot(&id(14)).is_none());
}
#[test]
fn busy_updates_do_not_rewrite_accepted_snapshots_or_wake_a_worker() {
    let f = F::new();
    let a = f.open();
    f.configure(&a);
    update(&a, 1, settings("frozen-context", false));
    f.local.gate.store(true, Ordering::SeqCst);
    invoke(&a,2,"submit_new",json!({"request_id":id(2),"text":"question","output":"chat","expected_config_revision":get(&a)["config_revision"]})).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !f.local.entered.load(Ordering::SeqCst) {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    let before = fs::read(f.path()).unwrap();
    assert_eq!(
        invoke(
            &a,
            3,
            "instructions_update",
            update_args(&a, 3, settings("changed", true))
        )
        .unwrap_err()
        .code,
        "BUSY"
    );
    assert_eq!(before, fs::read(f.path()).unwrap());
    f.local.gate.store(false, Ordering::SeqCst);
    done(&a, 2);
    update(&a, 4, settings("changed", true));
    assert!(a
        .inner
        .lock()
        .unwrap()
        .durable
        .instructions
        .snapshot(&id(2))
        .unwrap()
        .text
        .contains("frozen-context"));
}
#[test]
fn cloud_local_only_blocks_before_external_reads_and_both_adapters_get_all_fields() {
    let f = F::new();
    let a = f.open();
    let g = f.save(&a, Vendor::Openai, 1);
    update(&a, 2, settings("secret-personal-canary", false));
    let route = json!({"provider":"openai","model":"gpt-6-astra","credential_generation":g});
    let args = routed(&a, 3, None, route.clone());
    assert_eq!(
        invoke(&a, 3, "route_prepare", args.clone())
            .unwrap_err()
            .code,
        "INSTRUCTIONS_LOCAL_ONLY"
    );
    let mut submit = args;
    submit["approval_token"] = Value::Null;
    submit["confirm_transfer"] = json!(false);
    assert_eq!(
        invoke(&a, 3, "submit_routed", submit).unwrap_err().code,
        "INSTRUCTIONS_LOCAL_ONLY"
    );
    assert_eq!(f.external.reads.load(Ordering::SeqCst), 0);
    assert!(f.external.calls.lock().unwrap().is_empty());
    update(
        &a,
        4,
        json!({"instructions":"secret-personal-canary","personality":"terse","guardrails":"ask first","cloud_enabled":true}),
    );
    for (n, v, m) in [
        (10, Vendor::Openai, "gpt-6-astra"),
        (20, Vendor::Anthropic, "claude-sonnet-5-5"),
    ] {
        let generation = if v == Vendor::Openai {
            g.clone()
        } else {
            f.save(&a, v, 19)
        };
        send(
            &a,
            routed(
                &a,
                n,
                None,
                json!({"provider":v,"model":m,"credential_generation":generation}),
            ),
        );
        done(&a, n);
    }
    let calls = f.external.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    for (v, b) in calls.iter() {
        let p = b[if *v == Vendor::Openai {
            "instructions"
        } else {
            "system"
        }]
        .as_str()
        .unwrap();
        for t in ["secret-personal-canary", "terse", "ask first", VESSEL] {
            assert!(p.contains(t));
        }
        assert!(!p.contains("synthetic-key-canary"));
    }
}
#[test]
fn global_changes_fence_cached_preparations_and_renew_consent_not_model_changes() {
    let f = F::new();
    let a = f.open();
    let g = f.save(&a, Vendor::Openai, 1);
    let route = json!({"provider":"openai","model":"gpt-6-astra","credential_generation":g});
    let first = send(&a, routed(&a, 2, None, route.clone()));
    done(&a, 2);
    let target = first["conversation_id"].as_str().unwrap();
    let old = routed(&a, 3, Some(target), route.clone());
    let p = invoke(&a, 3, "route_prepare", old.clone()).unwrap();
    assert_eq!(p["requires_consent"], false);
    update(&a, 4, settings("new-global", true));
    assert_eq!(
        invoke(&a, 3, "route_prepare", old.clone())
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let mut stale = old;
    stale["approval_token"] = p["approval_token"].clone();
    stale["confirm_transfer"] = json!(false);
    assert_eq!(
        invoke(&a, 3, "submit_routed", stale).unwrap_err().code,
        "STALE_REVISION"
    );
    let fresh = routed(&a, 5, Some(target), route.clone());
    assert_eq!(
        invoke(&a, 3, "route_prepare", fresh.clone()).unwrap()["requires_consent"],
        true
    );
    send(&a, fresh);
    done(&a, 5);
    let mut other = route;
    other["model"] = json!("gpt-6-sol");
    let fresh = routed(&a, 6, Some(target), other);
    assert_eq!(
        invoke(&a, 3, "route_prepare", fresh.clone()).unwrap()["requires_consent"],
        false
    );
    let before = get(&a);
    update(&a, 7, settings("new-global", true));
    assert_eq!(get(&a)["config_revision"], before["config_revision"]);
    assert_eq!(
        invoke(&a, 3, "route_prepare", fresh).unwrap()["requires_consent"],
        false
    );
}
#[test]
fn agents_rich_disk7_migration_preserves_instructions_and_all_opaque_identity() {
    let f = F::new();
    let a = f.open();
    let generation = f.save(&a, Vendor::Openai, 900);
    for n in 1..=33 {
        update(&a, n, settings("retained-global", true));
    }
    send(
        &a,
        routed(
            &a,
            100,
            None,
            json!({"provider":"openai","model":"gpt-6-astra","credential_generation":generation}),
        ),
    );
    done(&a, 100);
    drop(a);
    let mut disk: Value = serde_json::from_slice(&fs::read(f.path()).unwrap()).unwrap();
    assert!(!disk["instructions"]["snapshots"]
        .as_object()
        .unwrap()
        .is_empty());
    assert!(!disk["instructions"]["grants"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        disk["instructions"]["receipts"].as_array().unwrap().len(),
        32
    );
    assert_eq!(disk["instructions"]["expired_ids"], json!([id(1)]));
    let expected_projects = disk["projects"].clone();
    for project in disk["projects"]["projects"].as_array_mut().unwrap() {
        for key in ["key", "description", "prompt"] {
            project.as_object_mut().unwrap().remove(key);
        }
        for row in project["repositories"].as_array_mut().unwrap() {
            row.as_object_mut().unwrap().remove("read_only");
        }
    }
    disk.as_object_mut().unwrap().remove("agents");
    disk["schema_version"] = json!(7);
    let bytes = serde_json::to_vec(&disk).unwrap();
    let (migrated, changed) = Durable::load(&bytes).unwrap();
    assert!(changed);
    migrated.validate().unwrap();
    let migrated = json!(migrated);
    for (key, value) in disk.as_object().unwrap() {
        if !matches!(key.as_str(), "schema_version" | "projects") {
            assert_eq!(&migrated[key], value, "{key}");
        }
    }
    let mut expected_projects = expected_projects;
    for project in expected_projects["projects"].as_array_mut().unwrap() {
        project["key"] = json!(format!(
            "project-{}",
            project["project_id"].as_str().unwrap()
        ));
        project["description"] = json!("");
        project["prompt"] = json!("");
    }
    assert_eq!(migrated["projects"], expected_projects);
    assert_eq!(migrated["agents"], json!(agents::Registry::default()));
    fs::write(f.path(), &bytes).unwrap();
    let counters = (
        f.external.reads.load(Ordering::SeqCst),
        f.external.calls.lock().unwrap().len(),
    );
    let a = f.open();
    assert_eq!(
        invoke(&a, 3, "instructions_receipt", json!({"request_id":id(33)})).unwrap(),
        disk["instructions"]["receipts"][31]["receipt"]
    );
    drop(a);
    let reopened: Value = serde_json::from_slice(&fs::read(f.path()).unwrap()).unwrap();
    for key in [
        "config",
        "instructions",
        "cloud",
        "timestamps",
        "conversations",
        "jobs",
        "expired_ids",
    ] {
        assert_eq!(reopened[key], disk[key], "{key}");
    }
    assert_eq!(reopened["projects"], expected_projects);
    assert_eq!(
        (
            f.external.reads.load(Ordering::SeqCst),
            f.external.calls.lock().unwrap().len()
        ),
        counters
    );
    for key in ["instructions", "config", "projects"] {
        let mut malformed = disk.clone();
        malformed.as_object_mut().unwrap().remove(key);
        let bytes = serde_json::to_vec(&malformed).unwrap();
        fs::write(f.path(), &bytes).unwrap();
        assert!(Application::open(&f.roots, f.local.clone()).is_err());
        assert_eq!(fs::read(f.path()).unwrap(), bytes);
    }
}
#[test]
fn agents_rich_disk9_migration_preserves_instructions_and_all_opaque_identity() {
    let f = F::new();
    let a = f.open();
    let generation = f.save(&a, Vendor::Openai, 900);
    for n in 1..=33 {
        update(&a, n, settings("retained-global", true));
    }
    send(
        &a,
        routed(
            &a,
            100,
            None,
            json!({"provider":"openai","model":"gpt-6-astra","credential_generation":generation}),
        ),
    );
    done(&a, 100);
    drop(a);
    let mut disk: Value = serde_json::from_slice(&fs::read(f.path()).unwrap()).unwrap();
    assert!(!disk["instructions"]["snapshots"]
        .as_object()
        .unwrap()
        .is_empty());
    assert!(!disk["instructions"]["grants"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        disk["instructions"]["receipts"].as_array().unwrap().len(),
        32
    );
    assert_eq!(disk["instructions"]["expired_ids"], json!([id(1)]));
    disk["agents"]["agents"][0]["name"] = json!("Previously saved developer");
    disk["agents"]["agents"][0]["description"] = json!("😀".repeat(256));
    disk["agents"]["revision"] = json!(3);
    disk["agents"]["expired_ids"] = json!([id(800)]);
    disk["agents"]["receipts"] = json!([{"receipt":{"request_id":id(801),"agent_id":"developer","revision":3},"fingerprint":"b".repeat(64)}]);
    disk["schema_version"] = json!(9);
    let bytes = serde_json::to_vec(&disk).unwrap();
    let (migrated, changed) = Durable::load(&bytes).unwrap();
    assert!(changed);
    migrated.validate().unwrap();
    let migrated = json!(migrated);
    for (key, value) in disk.as_object().unwrap() {
        if key != "schema_version" {
            assert_eq!(&migrated[key], value, "{key}");
        }
    }
    assert_eq!(migrated["agents"], disk["agents"]);
    fs::write(f.path(), &bytes).unwrap();
    let counters = (
        f.external.reads.load(Ordering::SeqCst),
        f.external.calls.lock().unwrap().len(),
    );
    let a = f.open();
    assert_eq!(
        invoke(&a, 3, "instructions_receipt", json!({"request_id":id(33)})).unwrap(),
        disk["instructions"]["receipts"][31]["receipt"]
    );
    drop(a);
    let reopened: Value = serde_json::from_slice(&fs::read(f.path()).unwrap()).unwrap();
    for key in [
        "agents",
        "config",
        "instructions",
        "cloud",
        "timestamps",
        "projects",
        "conversations",
        "jobs",
        "expired_ids",
    ] {
        assert_eq!(reopened[key], disk[key], "{key}");
    }
    assert_eq!(
        (
            f.external.reads.load(Ordering::SeqCst),
            f.external.calls.lock().unwrap().len()
        ),
        counters
    );
    for key in ["instructions", "config", "projects"] {
        let mut malformed = disk.clone();
        malformed.as_object_mut().unwrap().remove(key);
        let bytes = serde_json::to_vec(&malformed).unwrap();
        fs::write(f.path(), &bytes).unwrap();
        assert!(Application::open(&f.roots, f.local.clone()).is_err());
        assert_eq!(fs::read(f.path()).unwrap(), bytes);
    }
}
#[test]
fn strict_disk6_migration_and_pack_drift_fence_without_changing_history() {
    let f = F::new();
    let mut disk = serde_json::to_value(Durable::default()).unwrap();
    disk["schema_version"] = json!(6);
    disk.as_object_mut().unwrap().remove("instructions");
    disk.as_object_mut().unwrap().remove("agents");
    disk["config"]["revision"] = json!(9007199254740992u64);
    fs::write(f.path(), serde_json::to_vec(&disk).unwrap()).unwrap();
    fs::set_permissions(f.path(), fs::Permissions::from_mode(0o600)).unwrap();
    let a = f.open();
    assert_eq!(get(&a)["config_revision"], "9007199254740993");
    drop(a);
    let mut current: Value = serde_json::from_slice(&fs::read(f.path()).unwrap()).unwrap();
    assert_eq!(current["schema_version"], 10);
    current["instructions"]["pack_sha256"] = json!("a".repeat(64));
    fs::write(f.path(), serde_json::to_vec(&current).unwrap()).unwrap();
    let a = f.open();
    assert_eq!(get(&a)["config_revision"], "9007199254740994");
    let r = update(&a, 1, settings("changed", false));
    assert_eq!(r["config_revision"], "9007199254740995");
    drop(a);
    disk["config"]["revision"] = json!(u64::MAX);
    let bytes = serde_json::to_vec(&disk).unwrap();
    fs::write(f.path(), &bytes).unwrap();
    assert!(Application::open(&f.roots, f.local.clone()).is_err());
    assert_eq!(fs::read(f.path()).unwrap(), bytes);
    disk["config"]["revision"] = json!(0);
    disk["unexpected"] = json!(true);
    let bytes = serde_json::to_vec(&disk).unwrap();
    fs::write(f.path(), &bytes).unwrap();
    assert!(Application::open(&f.roots, f.local.clone()).is_err());
    assert_eq!(fs::read(f.path()).unwrap(), bytes);
}
#[test]
fn receipt_expiry_capacity_and_publication_failure_never_infer() {
    let f = F::new();
    let a = f.open();
    for n in 1..=33 {
        update(&a, n, settings("", false));
    }
    assert_eq!(
        invoke(&a, 3, "instructions_receipt", json!({"request_id":id(1)}))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(
        invoke(
            &a,
            3,
            "instructions_update",
            update_args(&a, 1, settings("", false))
        )
        .unwrap_err()
        .code,
        "REQUEST_EXPIRED"
    );
    {
        let mut inner = a.inner.lock().unwrap();
        inner.durable.instructions.expired_ids = (100..1124).map(id).collect();
    }
    let before = get(&a);
    assert_eq!(
        invoke(
            &a,
            3,
            "instructions_update",
            update_args(&a, 40, settings("new", false))
        )
        .unwrap_err()
        .code,
        "CAPACITY"
    );
    assert_eq!(get(&a), before);
    {
        let mut inner = a.inner.lock().unwrap();
        inner.durable.instructions.expired_ids.clear();
    }
    fs::remove_file(f.path()).unwrap();
    fs::create_dir(f.path()).unwrap();
    assert_eq!(
        invoke(
            &a,
            3,
            "instructions_update",
            update_args(&a, 41, settings("new", false))
        )
        .unwrap_err()
        .code,
        "STATE_UNAVAILABLE"
    );
    assert_eq!(
        invoke(&a, 3, "instructions_receipt", json!({"request_id":id(41)}))
            .unwrap_err()
            .code,
        "STATE_UNAVAILABLE"
    );
    assert!(f.local.calls.lock().unwrap().is_empty());
    assert!(f.external.calls.lock().unwrap().is_empty());
}
