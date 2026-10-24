use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn create(app: &Application, n: usize, revision: u64) -> Value {
    json!({"request_id":id(n),"instance_id":app.instance_id,"expected_revision":revision,"name":"Custom","description":"Saved profile","prompt":" exact\n\ttext "})
}
fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn call(app: &Application, op: &str, args: Value) -> AppResult<Value> {
    app.call(AppRequest {
        schema_version: 3,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(99999),
        op: op.into(),
        args,
    })
}
fn update(app: &Application, n: usize, revision: u64) -> Value {
    let agent = json!(app.inner.lock().unwrap().durable.agents)["agents"][0].clone();
    json!({"request_id":id(n),"instance_id":app.instance_id,"expected_revision":revision,"agent_id":"developer","name":agent["name"],"description":agent["description"],"prompt":"exact\n\ttext"})
}
struct Never;
impl Provider for Never {
    fn inventory(&self) -> AppResult<Inventory> {
        panic!("metadata must not observe providers")
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        panic!("metadata must not infer")
    }
}
struct F {
    app: Application,
    roots: Roots,
    base: PathBuf,
}
impl F {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("smith-agents-{}", new_id().unwrap()));
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
        let app = Application::open(&roots, Arc::new(Never)).unwrap();
        Self { app, roots, base }
    }
    fn disk(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.app.path).unwrap()).unwrap()
    }
    fn reject(&self, args: Value, code: &str) {
        let before = fs::read(&self.app.path).unwrap();
        assert_eq!(
            call(&self.app, "agent_update_v2", args).unwrap_err().code,
            code
        );
        assert_eq!(fs::read(&self.app.path).unwrap(), before);
    }
}
impl Drop for F {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
#[test]
fn agents_custom_create_update_reopen_and_lost_ack_receipt() {
    let mut f = F::new();
    let before = f.disk();
    let args = create(&f.app, 1, 1);
    let receipt = call(&f.app, "agent_create_v1", args.clone()).unwrap();
    let target = receipt["agent_id"].as_str().unwrap();
    assert!(valid_id(target));
    assert_eq!(receipt.as_object().unwrap().len(), 3);
    let after = f.disk();
    for (key, value) in before.as_object().unwrap() {
        if !matches!(key.as_str(), "agents" | "revision") {
            assert_eq!(after[key], *value, "{key}");
        }
    }
    assert_eq!(after["agents"]["agents"][0], before["agents"]["agents"][0]);
    assert_eq!(after["agents"]["agents"][1], before["agents"]["agents"][1]);
    assert_eq!(
        after["agents"]["agents"][2],
        json!({"agent_id":target,"name":"Custom","description":"Saved profile","prompt":" exact\n\ttext "})
    );
    let bytes = fs::read(&f.app.path).unwrap();
    assert_eq!(
        call(&f.app, "agent_create_v1", args.clone()).unwrap(),
        receipt
    );
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    for (key, value) in [
        ("name", json!("Other")),
        ("description", json!("Other")),
        ("prompt", json!("Other")),
        ("instance_id", json!(id(55))),
        ("expected_revision", json!(2)),
    ] {
        let mut conflict = args.clone();
        conflict[key] = value;
        assert_eq!(
            call(&f.app, "agent_create_v1", conflict).unwrap_err().code,
            "ID_CONFLICT"
        );
    }
    let mut edit = update(&f.app, 1, 2);
    edit["agent_id"] = json!(target);
    f.reject(edit, "ID_CONFLICT");
    drop(f.app.owner.take());
    let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
    // Resolve a lost acknowledgement through the retained receipt, not fields.
    assert_eq!(
        call(&app, "agent_receipt", json!({"request_id":id(1)})).unwrap(),
        receipt
    );
    assert_eq!(call(&app, "agent_create_v1", args).unwrap(), receipt);
    let mut edit = update(&app, 2, 2);
    edit["agent_id"] = json!(target);
    edit["name"] = json!("Renamed");
    edit["description"] = json!("");
    call(&app, "agent_update_v2", edit).unwrap();
    let list = call(&app, "agents_list_v2", json!({})).unwrap();
    assert_eq!(list["agents"].as_array().unwrap().len(), 3);
    assert_eq!(list["agents"][2]["agent_id"], target);
    assert_eq!(list["agents"][2]["name"], "Renamed");
}
#[test]
fn agents_builtin_metadata_is_immutable_with_legacy_prompt_editing() {
    let mut f = F::new();
    let mut disk = f.disk();
    disk["agents"]["agents"][0]["name"] = json!("Retained saved name");
    disk["agents"]["agents"][0]["description"] = json!("😀".repeat(256));
    drop(f.app.owner.take());
    fs::write(&f.app.path, serde_json::to_vec(&disk).unwrap()).unwrap();
    let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
    for index in 0..2 {
        let agent = &disk["agents"]["agents"][index];
        let mut edit = json!({"request_id":id(index+1),"instance_id":app.instance_id,"expected_revision":index+1,"agent_id":agent["agent_id"],"name":agent["name"],"description":agent["description"],"prompt":"new prompt"});
        for key in ["name", "description"] {
            let mut bad = edit.clone();
            bad[key] = json!("Changed");
            let before = fs::read(&app.path).unwrap();
            assert_eq!(
                call(&app, "agent_update_v2", bad).unwrap_err().code,
                "INVALID_REQUEST"
            );
            assert_eq!(fs::read(&app.path).unwrap(), before);
        }
        edit["prompt"] = json!("😀".repeat(1024));
        call(&app, "agent_update_v2", edit).unwrap();
        let list = call(&app, "agents_list_v2", json!({})).unwrap();
        assert_eq!(list["agents"][index]["name"], agent["name"]);
        assert_eq!(list["agents"][index]["description"], agent["description"]);
    }
}
#[test]
fn agents_custom_strict_bounds_origin_and_total_capacity() {
    let f = F::new();
    let good = create(&f.app, 1, 1);
    for key in good.as_object().unwrap().keys() {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert_eq!(
            call(&f.app, "agent_create_v1", bad).unwrap_err().code,
            "INVALID_REQUEST"
        );
    }
    for (key, value, code) in [
        ("agent_id", json!(id(44)), "INVALID_REQUEST"),
        ("name", json!("😀".repeat(41)), "INVALID_REQUEST"),
        ("description", json!("😀".repeat(41)), "INVALID_REQUEST"),
        ("description", json!("a\u{2028}b"), "INVALID_REQUEST"),
        (
            "prompt",
            json!(format!("{}x", "😀".repeat(1024))),
            "INVALID_REQUEST",
        ),
        ("instance_id", json!(id(44)), "INSTANCE_MISMATCH"),
        ("expected_revision", json!(0), "STALE_REVISION"),
    ] {
        let before = fs::read(&f.app.path).unwrap();
        let mut bad = good.clone();
        bad[key] = value;
        assert_eq!(call(&f.app, "agent_create_v1", bad).unwrap_err().code, code);
        assert_eq!(fs::read(&f.app.path).unwrap(), before);
    }
    let mut ids = HashSet::new();
    for n in 1..=30 {
        let mut args = create(&f.app, n, n as u64);
        args["name"] = json!("😀".repeat(40));
        args["description"] = json!("😀".repeat(40));
        args["prompt"] = json!("😀".repeat(1024));
        let receipt = call(&f.app, "agent_create_v1", args.clone()).unwrap();
        assert!(ids.insert(receipt["agent_id"].as_str().unwrap().to_owned()));
        assert_eq!(
            f.disk()["agents"]["agents"][n + 1]["agent_id"],
            receipt["agent_id"]
        );
        if n == 30 {
            assert_eq!(call(&f.app, "agent_create_v1", args).unwrap(), receipt);
        }
    }
    let before = fs::read(&f.app.path).unwrap();
    assert_eq!(
        call(&f.app, "agent_create_v1", create(&f.app, 31, 31))
            .unwrap_err()
            .code,
        "CAPACITY"
    );
    assert_eq!(fs::read(&f.app.path).unwrap(), before);
}
#[test]
fn agents_legacy_receipts_share_namespace_without_legacy_replay() {
    let mut f = F::new();
    let mut old = f.disk();
    old["schema_version"] = json!(9);
    let args = update(&f.app, 1, 1);
    let receipt = json!({"request_id":id(1),"agent_id":"developer","revision":2});
    old["agents"]["revision"] = json!(2);
    old["agents"]["receipts"] = json!([{"receipt":receipt,"fingerprint":fingerprint("v3:agents:agent_update_v1",&args).unwrap()}]);
    drop(f.app.owner.take());
    fs::write(&f.app.path, serde_json::to_vec(&old).unwrap()).unwrap();
    let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
    let bytes = fs::read(&f.app.path).unwrap();
    assert_eq!(
        call(&app, "agent_receipt", json!({"request_id":id(1)})).unwrap(),
        receipt
    );
    assert_eq!(
        call(&app, "agent_update_v1", args.clone())
            .unwrap_err()
            .code,
        "UPGRADE_REQUIRED"
    );
    assert_eq!(
        call(&app, "agent_update_v2", args).unwrap_err().code,
        "ID_CONFLICT"
    );
    assert_eq!(
        call(&app, "agent_create_v1", create(&app, 1, 2))
            .unwrap_err()
            .code,
        "ID_CONFLICT"
    );
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
}
#[test]
fn agents_retired_versions_refuse_before_replay_or_admission() {
    let f = F::new();
    let args = update(&f.app, 1, 1);
    call(&f.app, "agent_update_v2", args.clone()).unwrap();
    let before = fs::read(&f.app.path).unwrap();
    for (op, args) in [
        ("agent_update_v1", args),
        ("agent_update_v1", json!([])),
        ("agents_list_v1", json!({})),
        ("agents_list_v1", json!(null)),
    ] {
        assert_eq!(call(&f.app, op, args).unwrap_err().code, "UPGRADE_REQUIRED");
    }
    assert_eq!(fs::read(&f.app.path).unwrap(), before);
}
#[test]
fn agents_frozen_disk8_and_disk9_refuse_widened_or_corrupt_legacy() {
    for version in [8, 9] {
        let mut f = F::new();
        let mut good = legacy_disk(version);
        good["agents"]["agents"][0]["description"] = json!("😀".repeat(256));
        let (converted, changed) = Durable::load(&serde_json::to_vec(&good).unwrap()).unwrap();
        assert!(changed);
        converted.validate().unwrap();
        assert_eq!(json!(converted.agents), good["agents"]);
        drop(f.app.owner.take());
        for mutation in 0..8 {
            let mut bad = good.clone();
            match mutation {
                0 => bad["agents"]["agents"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"agent_id":id(88),"name":"Custom","description":"","prompt":""})),
                1 => bad["agents"]["agents"][0]["agent_id"] = json!(id(88)),
                2 => bad["agents"]["agents"][0]["description"] = json!("😀".repeat(257)),
                3 => bad["agents"]["agents"].as_array_mut().unwrap().swap(0, 1),
                4 => bad["agents"]["agents"][0]["extra"] = json!(true),
                5 => bad["agents"]["agents"][0] = json!(["developer", "Name", "", ""]),
                6 => {
                    bad["agents"]["revision"] = json!(2);
                    bad["agents"]["receipts"] = json!([{"receipt":{"request_id":id(4),"agent_id":id(88),"revision":2},"fingerprint":"a".repeat(64)}]);
                }
                _ => {
                    bad.as_object_mut().unwrap().remove("instructions");
                }
            }
            let bytes = serde_json::to_vec(&bad).unwrap();
            assert!(
                Durable::load(&bytes).is_err(),
                "disk{version} mutation{mutation}"
            );
            fs::write(&f.app.path, &bytes).unwrap();
            assert!(Application::open(&f.roots, Arc::new(Never)).is_err());
            assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
        }
    }
}
#[test]
fn agents_create_publication_failure_has_no_partial_record_id_or_receipt() {
    let mut f = F::new();
    let before = fs::read(&f.app.path).unwrap();
    let memory = json!(f.app.inner.lock().unwrap().durable);
    let staging = f.app.path.parent().unwrap().join(format!(
        ".smith-next-app-state.json-{}-tmp",
        std::process::id()
    ));
    fs::write(&staging, b"publication blocker").unwrap();
    let args = create(&f.app, 1, 1);
    assert_eq!(
        call(&f.app, "agent_create_v1", args).unwrap_err().code,
        "STATE_UNAVAILABLE"
    );
    assert_eq!(json!(f.app.inner.lock().unwrap().durable), memory);
    assert_eq!(fs::read(&f.app.path).unwrap(), before);
    assert_eq!(
        call(&f.app, "agents_list_v2", json!({})).unwrap_err().code,
        "STATE_UNAVAILABLE"
    );
    fs::remove_file(staging).unwrap();
    drop(f.app.owner.take());
    let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
    assert_eq!(
        call(&app, "agent_receipt", json!({"request_id":id(1)}))
            .unwrap_err()
            .code,
        "REQUEST_UNKNOWN"
    );
    assert_eq!(
        call(&app, "agents_list_v2", json!({})).unwrap()["agents"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn agents_defaults_exact_roles_and_text_bounds() {
    let r = Registry::default();
    r.validate().unwrap();
    assert_eq!(r.revision, 1);
    assert!(r.receipts.is_empty());
    assert_eq!(json!(r.agents[0])["agent_id"], "developer");
    assert_eq!(json!(r.agents[1])["agent_id"], "assurance");
    for a in &r.agents {
        assert_eq!(json!(a).as_object().unwrap().len(), 4);
        for heading in [
            "Purpose\n",
            "Inputs\n",
            "Constraints\n",
            "Expected output\n",
        ] {
            assert!(a.prompt.contains(heading));
        }
    }
    let mut a = r.agents[0].clone();
    a.name = "😀".repeat(40);
    a.description = "😀".repeat(256);
    a.prompt = "😀".repeat(1024);
    assert!(a.valid());
    a.name.push('x');
    assert!(!a.valid());
    a.name.pop();
    a.description.push('x');
    assert!(!a.valid());
    a.description.pop();
    a.prompt.push('x');
    assert!(!a.valid());
    for bad in [" x", "x ", "\u{feff}x", "x\u{0000}", ""] {
        a.name = bad.into();
        assert!(!a.valid());
    }
    a.name = "Valid".into();
    a.prompt = "\n\t exact \n".into();
    for bad in [
        " x",
        "x ",
        "x\ntext",
        "x\u{2028}text",
        "x\u{2029}text",
        "x\ttext",
        "x\u{0085}text",
    ] {
        a.description = bad.into();
        assert!(!a.valid());
    }
    a.description.clear();
    assert!(a.valid());
    for bad in ["\r", "\0", "\u{0085}", "\u{007f}"] {
        a.prompt = bad.into();
        assert!(!a.valid());
    }
    a.prompt.clear();
    assert!(a.valid());
}
#[test]
fn agents_atomic_update_replay_reopen_and_no_chat_or_config_effects() {
    let mut f = F::new();
    let before = f.disk();
    assert_eq!(before["schema_version"], 10);
    assert_eq!(before["projects"]["projects"][0]["name"], "smith");
    let args = update(&f.app, 1, 1);
    let receipt = call(&f.app, "agent_update_v2", args.clone()).unwrap();
    assert_eq!(
        receipt,
        json!({"request_id":id(1),"agent_id":"developer","revision":2})
    );
    let after = f.disk();
    for key in [
        "config",
        "instructions",
        "cloud",
        "timestamps",
        "projects",
        "jobs",
        "conversations",
        "expired_ids",
    ] {
        assert_eq!(before[key], after[key], "{key}");
    }
    let bytes = fs::read(&f.app.path).unwrap();
    assert_eq!(
        call(&f.app, "agent_update_v2", args.clone()).unwrap(),
        receipt
    );
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    let mut conflict = args.clone();
    conflict["prompt"] = json!("other");
    f.reject(conflict, "ID_CONFLICT");

    // Release the owner, retaining the fixture's private paths and state.
    drop(f.app.owner.take());
    let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
    assert_eq!(call(&app, "agent_update_v2", args).unwrap(), receipt);
    assert_eq!(
        call(&app, "agent_receipt", json!({"request_id":id(1)})).unwrap(),
        receipt
    );
    assert_eq!(
        call(&app, "agents_list_v2", json!({})).unwrap()["agents"][0]["prompt"],
        "exact\n\ttext"
    );
    drop(app);
}
#[test]
fn agents_strict_requests_and_storage_corruption() {
    let mut f = F::new();
    let good = update(&f.app, 1, 1);
    for key in good.as_object().unwrap().keys() {
        let mut a = good.clone();
        a.as_object_mut().unwrap().remove(key);
        f.reject(a, "INVALID_REQUEST");
    }
    let mut a = good.clone();
    a["extra"] = json!(true);
    f.reject(a, "INVALID_REQUEST");
    for role in [
        "",
        "planner",
        "Developer",
        "00000000000000000000000000000000",
    ] {
        let mut a = good.clone();
        a["agent_id"] = json!(role);
        f.reject(a, "INVALID_REQUEST");
    }
    let mut a = good.clone();
    a["expected_revision"] = json!(0);
    f.reject(a, "STALE_REVISION");
    let mut a = good;
    a["instance_id"] = json!(id(55));
    f.reject(a, "INSTANCE_MISMATCH");
    let baseline = f.disk();
    drop(f.app.owner.take());
    for mutation in 0..8 {
        let mut d = baseline.clone();
        match mutation {
            0 => {
                d.as_object_mut().unwrap().remove("agents");
            }
            1 => d["agents"]["agents"] = json!([]),
            2 => d["agents"]["agents"][1] = d["agents"]["agents"][0].clone(),
            3 => d["agents"]["agents"][0]["agent_id"] = json!("planner"),
            4 => {
                d["agents"]["agents"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("prompt");
            }
            5 => d["agents"]["agents"][0]["icon"] = json!("code"),
            6 => d["agents"]["agents"][0] = json!(["developer", "Developer", "", ""]),
            _ => d["agents"]["revision"] = json!(0),
        }
        let bytes = serde_json::to_vec(&d).unwrap();
        assert!(
            Durable::load(&bytes)
                .and_then(|(d, _)| d.validate())
                .is_err(),
            "{mutation}"
        );
        fs::write(&f.app.path, &bytes).unwrap();
        assert!(Application::open(&f.roots, Arc::new(Never)).is_err());
        assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    }
    let bytes = serde_json::to_string(&baseline).unwrap().replace(
        "\"agent_id\":\"developer\"",
        "\"agent_id\":\"developer\",\"agent_id\":\"developer\"",
    );
    assert!(Durable::load(bytes.as_bytes()).is_err());
    assert!(crate::cloud::unique_json(br#"{"prompt":"\ud800"}"#).is_err());
}
#[test]
fn agents_expiry_capacity_and_ownership_fences() {
    let f = F::new();
    call(&f.app, "agent_create_v1", create(&f.app, 1, 1)).unwrap();
    for n in 2..=1056 {
        call(&f.app, "agent_update_v2", update(&f.app, n, n as u64)).unwrap();
    }
    let bytes = fs::read(&f.app.path).unwrap();
    assert_eq!(
        call(&f.app, "agent_create_v1", create(&f.app, 1, 1))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(
        call(&f.app, "agent_create_v1", create(&f.app, 1057, 1057))
            .unwrap_err()
            .code,
        "CAPACITY"
    );
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    f.reject(update(&f.app, 1057, 1057), "CAPACITY");
    f.reject(update(&f.app, 1, 1), "REQUEST_EXPIRED");
    assert_eq!(
        call(&f.app, "agent_receipt", json!({"request_id":id(1)}))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(
        call(&f.app, "agent_receipt", json!({"request_id":id(9999)}))
            .unwrap_err()
            .code,
        "REQUEST_UNKNOWN"
    );
    f.app.inner.lock().unwrap().owned = false;
    assert_eq!(
        call(&f.app, "agents_list_v2", json!({})).unwrap_err().code,
        "STATE_UNAVAILABLE"
    );
    f.reject(update(&f.app, 1057, 1057), "STATE_UNAVAILABLE");
}

fn legacy_disk(version: u64) -> Value {
    let mut disk = if version == 1 {
        json!(LegacyDurable::default())
    } else {
        json!(Durable::default())
    };
    disk["schema_version"] = json!(version);
    disk["config"]["revision"] = json!(42);
    disk["expired_ids"] = json!([id(8000)]);
    for (key, introduced) in [
        ("cloud", 3),
        ("timestamps", 4),
        ("projects", 5),
        ("instructions", 7),
        ("agents", 8),
    ] {
        if version < introduced {
            disk.as_object_mut().unwrap().remove(key);
        }
    }
    disk
}

#[test]
fn agents_disk1_through_disk7_initialize_once_without_project_backfill() {
    for version in 1..=7 {
        let mut f = F::new();
        drop(f.app.owner.take());
        let old = legacy_disk(version);
        let bytes = serde_json::to_vec(&old).unwrap();
        let (loaded, changed) = Durable::load(&bytes).unwrap();
        assert!(changed);
        loaded.validate().unwrap();
        assert_eq!(json!(loaded.agents), json!(Registry::default()));
        assert_eq!(loaded.config.revision, if version == 7 { 42 } else { 43 });
        assert_eq!(json!(loaded.projects), json!(projects::Registry::default()));
        assert_eq!(loaded.expired_ids, vec![id(8000)]);
        fs::write(&f.app.path, bytes).unwrap();
        let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
        assert_eq!(f.disk()["schema_version"], 10);
        let mut args = update(&app, 1, 1);
        args["prompt"] = json!("");
        call(&app, "agent_update_v2", args).unwrap();
        drop(app);
        let saved = fs::read(&f.app.path).unwrap();
        let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
        assert_eq!(
            call(&app, "agents_list_v2", json!({})).unwrap()["agents"][0]["prompt"],
            ""
        );
        assert_eq!(fs::read(&f.app.path).unwrap(), saved);
        drop(app);
    }
}

#[test]
fn agents_failed_migration_publication_preserves_legacy_bytes() {
    for version in 1..=9 {
        let mut f = F::new();
        drop(f.app.owner.take());
        let bytes = serde_json::to_vec(&legacy_disk(version)).unwrap();
        fs::write(&f.app.path, &bytes).unwrap();
        // Occupying the existing publisher's exclusive staging path makes the
        // failure deterministic, including when tests run with elevated UID.
        let staging = f.app.path.parent().unwrap().join(format!(
            ".smith-next-app-state.json-{}-tmp",
            std::process::id()
        ));
        fs::write(&staging, b"publication blocker").unwrap();
        assert!(Application::open(&f.roots, Arc::new(Never)).is_err());
        assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
        assert_eq!(fs::read(staging).unwrap(), b"publication blocker");
    }
}

#[test]
fn agents_failed_update_publication_fences_all_operations_until_reopen() {
    let mut f = F::new();
    let first_args = update(&f.app, 1, 1);
    let first = call(&f.app, "agent_update_v2", first_args.clone()).unwrap();
    let bytes = fs::read(&f.app.path).unwrap();
    let in_memory = json!(f.app.inner.lock().unwrap().durable);
    let staging = f.app.path.parent().unwrap().join(format!(
        ".smith-next-app-state.json-{}-tmp",
        std::process::id()
    ));
    fs::write(&staging, b"publication blocker").unwrap();
    f.reject(update(&f.app, 2, 2), "STATE_UNAVAILABLE");
    assert!(f.app.inner.lock().unwrap().fault);
    assert_eq!(json!(f.app.inner.lock().unwrap().durable), in_memory);
    for (op, args) in [
        ("agents_list_v2", json!({})),
        ("agent_receipt", json!({"request_id":id(1)})),
        ("agent_update_v2", first_args),
    ] {
        assert_eq!(
            call(&f.app, op, args).unwrap_err().code,
            "STATE_UNAVAILABLE"
        );
    }
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    fs::remove_file(staging).unwrap();
    drop(f.app.owner.take());
    let app = Application::open(&f.roots, Arc::new(Never)).unwrap();
    assert_eq!(
        call(&app, "agent_receipt", json!({"request_id":id(1)})).unwrap(),
        first
    );
    assert_eq!(
        call(&app, "agent_receipt", json!({"request_id":id(2)}))
            .unwrap_err()
            .code,
        "REQUEST_UNKNOWN"
    );
    assert_eq!(
        call(&app, "agents_list_v2", json!({})).unwrap()["revision"],
        2
    );
    drop(app);
}

#[test]
fn agents_strict_shapes_fingerprints_and_peer_versions() {
    let f = F::new();
    let args = update(&f.app, 1, 1);
    for (key, value) in [
        ("request_id", json!("invalid")),
        ("instance_id", json!("invalid")),
        ("expected_revision", json!(v2::MAX_REVISION + 1)),
        ("expected_revision", json!(-1)),
        ("expected_revision", json!(1.5)),
        ("name", json!(null)),
        ("description", json!([])),
        ("prompt", json!({})),
    ] {
        let mut bad = args.clone();
        bad[key] = value;
        f.reject(bad, "INVALID_REQUEST");
    }
    for op in ["agents_list_v2", "agent_update_v2", "agent_receipt"] {
        for bad in [json!([]), json!(null), json!({"extra":true})] {
            assert_eq!(call(&f.app, op, bad).unwrap_err().code, "INVALID_REQUEST");
        }
        for version in [1, 2] {
            let req = AppRequest {
                schema_version: version,
                service: "smith-next".into(),
                protocol: "app".into(),
                call_id: id(999),
                op: op.into(),
                args: json!({}),
            };
            assert!(f.app.call(req).is_err());
        }
    }
    call(&f.app, "agent_update_v2", args.clone()).unwrap();
    for (key, value) in [
        ("instance_id", json!(id(88))),
        ("expected_revision", json!(2)),
        ("agent_id", json!("assurance")),
        ("name", json!("Other")),
        ("description", json!("Other")),
        ("prompt", json!("Other")),
    ] {
        let mut conflict = args.clone();
        conflict[key] = value;
        f.reject(conflict, "ID_CONFLICT");
    }
    let mut assurance = update(&f.app, 2, 2);
    assurance["agent_id"] = json!("assurance");
    assurance["name"] = json!("Assurance");
    assurance["description"] = json!(Registry::default().agents[1].description);
    call(&f.app, "agent_update_v2", assurance).unwrap();
    let list = call(&f.app, "agents_list_v2", json!({})).unwrap();
    assert_eq!(list["agents"][0]["name"], "Developer");
    assert_eq!(list["agents"][1]["name"], "Assurance");
}

#[test]
fn agents_corrupt_ledger_shapes_are_preserved_on_open() {
    let mut f = F::new();
    call(&f.app, "agent_update_v2", update(&f.app, 1, 1)).unwrap();
    let baseline = f.disk();
    drop(f.app.owner.take());
    for mutation in 0..12 {
        let mut disk = baseline.clone();
        let registry = &mut disk["agents"];
        match mutation {
            0 => registry["receipts"][0]["fingerprint"] = json!("bad"),
            1 => registry["receipts"][0]["receipt"]["revision"] = json!(1),
            2 => registry["expired_ids"] = json!([id(1)]),
            3 => registry["receipts"][0]["receipt"]["agent_id"] = json!("other"),
            4 => registry["receipts"][0]["receipt"]["request_id"] = json!("bad"),
            5 => registry["receipts"][0]["receipt"]["prompt"] = json!("leak"),
            6 => registry["receipts"][0] = json!([]),
            7 => registry["receipts"][0]["receipt"] = json!([id(1), "developer", 2]),
            8 => registry["revision"] = json!(v2::MAX_REVISION + 1),
            9 => registry["agents"].as_array_mut().unwrap().swap(0, 1),
            10 => {
                registry.as_object_mut().unwrap().remove("expired_ids");
            }
            _ => disk["agents"] = json!([2, [], [], []]),
        }
        let bytes = serde_json::to_vec(&disk).unwrap();
        fs::write(&f.app.path, &bytes).unwrap();
        assert!(
            Application::open(&f.roots, Arc::new(Never)).is_err(),
            "{mutation}"
        );
        assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    }
}
