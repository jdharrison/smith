use super::*;
use std::fs;

#[path = "app_projects_v3_tests.rs"]
mod v3_tests;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn request(version: u32, op: &str, args: Value) -> AppRequest {
    AppRequest {
        schema_version: version,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(99999),
        op: op.into(),
        args,
    }
}
fn call(app: &Application, op: &str, args: Value) -> AppResult<Value> {
    app.call(request(3, op, args))
}
fn create(app: &Application, n: usize, revision: u64) -> Value {
    json!({"request_id":id(n),"instance_id":app.instance_id,"expected_revision":revision,
        "name":"Project","key":format!("project-{n}"),"description":"","prompt":"",
        "repositories":[],"agent":"opencode"})
}
fn update(app: &Application, n: usize, revision: u64, target: &Value) -> Value {
    let mut a = create(app, n, revision);
    a["project_id"] = target.clone();
    if let Some(project) = app
        .inner
        .lock()
        .unwrap()
        .durable
        .projects
        .projects
        .iter()
        .find(|p| json!(p.project_id) == *target)
    {
        a["name"] = json!(project.name);
        a["key"] = json!(project.key);
        a["description"] = json!(project.description);
        a["prompt"] = json!(project.prompt);
    }
    a
}
fn delete(app: &Application, n: usize, revision: u64, target: &Value) -> Value {
    json!({"request_id":id(n),"instance_id":app.instance_id,"expected_revision":revision,"project_id":target})
}
fn list(app: &Application) -> Value {
    call(app, "projects_list_v3", json!({})).unwrap()
}
fn receipt(app: &Application, n: usize) -> AppResult<Value> {
    call(app, "project_receipt", json!({"request_id":id(n)}))
}
#[derive(Default)]
struct Never {
    calls: AtomicUsize,
}
impl Provider for Never {
    fn inventory(&self) -> AppResult<Inventory> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(invalid())
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(invalid())
    }
    fn remove_model(&self, _: &str, _: &str) -> AppResult<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(invalid())
    }
    fn cancel_owned(&self) -> bool {
        true
    }
}
struct Fixture {
    app: Application,
    roots: Roots,
    base: PathBuf,
    provider: Arc<Never>,
}
impl Fixture {
    fn new(durable: Durable) -> Self {
        let base = std::env::temp_dir().join(format!("smith-projects-{}", new_id().unwrap()));
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
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
        let path = roots.data.join("smith-next/app-state.json");
        persist(&path, &durable).unwrap();
        let app = Application {
            inner: Arc::new(Mutex::new(Inner {
                durable,
                inventory: Inventory::default(),
                inventory_observed: false,
                fault: false,
                owned: true,
                cloud: v3::Volatile::default(),
            })),
            owner: None,
            dev_jobs: Arc::new(dev_jobs::Owner::open(
                roots.data.join("smith-next/dev-jobs.json"),
                Arc::new(crate::dev_runtime::DockerRuntime::new(
                    roots.runtime.clone(),
                )),
            )),
            path,
            instance_id: new_id().unwrap(),
            cloud: Arc::new(crate::cloud::Dependencies::disabled()),
        };
        Self {
            app,
            roots,
            base,
            provider: Arc::new(Never::default()),
        }
    }
    fn reopen(&self) -> Result<Application> {
        Application::open(&self.roots, self.provider.clone())
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(&self.app.path).unwrap()
    }
    fn disk(&self) -> Value {
        serde_json::from_slice(&self.bytes()).unwrap()
    }
    fn write(&self, v: &Value) -> Vec<u8> {
        let bytes = serde_json::to_vec(v).unwrap();
        fs::write(&self.app.path, &bytes).unwrap();
        bytes
    }
    fn assert_rejected(&self, op: &str, args: Value, code: &str) {
        let bytes = self.bytes();
        let durable = json!(self.app.inner.lock().unwrap().durable);
        assert_eq!(call(&self.app, op, args).unwrap_err().code, code, "{op}");
        assert_eq!(self.bytes(), bytes);
        let inner = self.app.inner.lock().unwrap();
        assert_eq!(json!(inner.durable), durable);
        assert!(!inner.fault);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.provider.calls.load(Ordering::SeqCst), 0);
        fs::remove_dir_all(&self.base).unwrap();
    }
}

fn fresh_fixture() -> Fixture {
    let mut f = Fixture::new(Durable::default());
    fs::remove_file(&f.app.path).unwrap();
    f.app = f.reopen().unwrap();
    f
}

#[test]
fn projects_bootstrap_fresh_exact_private_published_state_and_stable_reopen() {
    assert_eq!(
        json!(Registry::default()),
        json!({"revision":0,"projects":[],"receipts":[],"expired_ids":[]})
    );
    let defaults = Durable::default();
    assert_eq!(json!(defaults.projects), json!(Registry::default()));
    let f = fresh_fixture();
    // Inspect disk immediately after open: readiness requires prior publication.
    let bytes = f.bytes();
    assert_eq!(
        fs::metadata(&f.app.path).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let disk = f.disk();
    let registry = &disk["projects"];
    let project = &registry["projects"][0];
    let project_id = project["project_id"].as_str().unwrap();
    assert!(valid_id(project_id));
    assert!(project_id.ends_with("0000000000000001"));
    assert_eq!(
        project,
        &json!({"project_id":project_id,"name":"smith","key":"smith","description":"local autonomy subtrate","prompt":"",
        "repositories":[{"name":"smith","url":"https://github.com/jdharrison/smith","read_only":true,
        "branch":{"base":"main","work":"main","types":{"feature":null,"fix":null,"release":null,"hotfix":null}}}],"agent":"opencode"})
    );
    assert_eq!(registry["projects"].as_array().unwrap().len(), 1);
    assert_eq!(registry["revision"], 1);
    assert_eq!(registry["expired_ids"], json!([]));
    let retained = &registry["receipts"][0];
    assert_eq!(registry["receipts"].as_array().unwrap().len(), 1);
    let request_id = retained["receipt"]["request_id"].as_str().unwrap();
    assert!(valid_id(request_id));
    assert_ne!(request_id, project_id);
    assert_eq!(
        retained["receipt"],
        json!({"request_id":request_id,"project_id":project_id,"revision":1,"deleted":false})
    );
    assert_eq!(
        retained["fingerprint"],
        fingerprint("v3:projects:bootstrap", project).unwrap()
    );
    assert_eq!(
        call(&f.app, "project_receipt", json!({"request_id":request_id})).unwrap(),
        retained["receipt"]
    );
    let (loaded, migrated) = Durable::load(&bytes).unwrap();
    assert!(!migrated);
    loaded.validate().unwrap();
    loaded.preflight().unwrap();
    let mut expected = json!(defaults);
    expected["projects"] = registry.clone();
    expected["revision"] = json!(1);
    assert_eq!(disk, expected);
    assert_eq!(json!(f.app.inner.lock().unwrap().durable), disk);
    for _ in 0..3 {
        let app = f.reopen().unwrap();
        assert_eq!(list(&app), list(&f.app));
        assert_eq!(f.bytes(), bytes);
    }
    let other = fresh_fixture();
    assert_ne!(
        other.disk()["projects"]["projects"][0]["project_id"],
        project["project_id"]
    );
    assert_ne!(
        other.disk()["projects"]["receipts"][0]["receipt"]["request_id"],
        request_id
    );
}

#[test]
fn projects_bootstrap_ordinary_all_fields_clear_delete_and_history_never_resurrect() {
    let mut f = fresh_fixture();
    let initial = f.disk()["projects"]["receipts"][0]["receipt"].clone();
    let target = initial["project_id"].clone();
    let mut a = update(&f.app, 1, 1, &target);
    a["key"] = json!("renamed-starter");
    a["repositories"] = json!([repository()]);
    a["repositories"][0]["branch"]["base"] = json!("stable");
    a["repositories"][0]["branch"]["work"] = json!("develop");
    call(&f.app, "project_update_v3", a).unwrap();
    f.app = f.reopen().unwrap();
    let saved = list(&f.app);
    assert_eq!(saved["projects"][0]["key"], "renamed-starter");
    assert_eq!(saved["projects"][0]["name"], "smith");
    assert_eq!(saved["projects"][0]["project_id"], target);
    let mut expected_repo = repository();
    expected_repo["branch"]["base"] = json!("stable");
    expected_repo["branch"]["work"] = json!("develop");
    assert_eq!(saved["projects"][0]["repositories"], json!([expected_repo]));
    let mut a = update(&f.app, 2, 2, &target);
    let cleared = json!(named_repository(
        "cleared",
        "https://example.invalid/cleared"
    ));
    a["repositories"] = json!([cleared]);
    call(&f.app, "project_update_v3", a).unwrap();
    f.app = f.reopen().unwrap();
    assert_eq!(
        list(&f.app)["projects"][0]["repositories"],
        json!([cleared])
    );
    call(&f.app, "project_update_v3", update(&f.app, 3, 3, &target)).unwrap();
    f.app = f.reopen().unwrap();
    assert_eq!(list(&f.app)["projects"][0]["repositories"], json!([]));
    let deleted = call(&f.app, "project_delete_v3", delete(&f.app, 4, 4, &target)).unwrap();
    let bytes = f.bytes();
    let disk = f.disk();
    for retained in &disk["projects"]["receipts"].as_array().unwrap()[..4] {
        assert!(retained["fingerprint"].is_null());
    }
    assert!(disk["projects"]["receipts"][4]["fingerprint"].is_string());
    for _ in 0..3 {
        f.app = f.reopen().unwrap();
        assert_eq!(list(&f.app), json!({"revision":5,"projects":[]}));
        assert_eq!(f.bytes(), bytes);
        assert_eq!(
            call(
                &f.app,
                "project_receipt",
                json!({"request_id":initial["request_id"]})
            )
            .unwrap(),
            initial
        );
        assert_eq!(receipt(&f.app, 4).unwrap(), deleted);
        for n in 1..=3 {
            assert_eq!(receipt(&f.app, n).unwrap()["project_id"], target);
            assert_eq!(
                call(&f.app, "project_update_v3", update(&f.app, n, 5, &target))
                    .unwrap_err()
                    .code,
                "REQUEST_EXPIRED"
            );
        }
    }
}

#[test]
fn projects_bootstrap_existing_current_empty_populated_full_and_exhausted_are_untouched() {
    let mut populated = Durable::default();
    populated.projects = maximal_registry();
    let mut one = populated.clone();
    one.projects.projects.truncate(1);
    one.projects.receipts.truncate(1);
    one.projects.revision = 1;
    let mut exhausted = populated.clone();
    exhausted.projects.expired_ids = (2000..2000 + MAX_TOMBSTONES).map(id).collect();
    for durable in [Durable::default(), one, populated, exhausted] {
        let f = Fixture::new(durable);
        let before = f.bytes();
        let expected = list(&f.app);
        for _ in 0..2 {
            let app = f.reopen().unwrap();
            assert_eq!(list(&app), expected);
            assert_eq!(f.bytes(), before);
        }
    }
}

#[test]
fn projects_bootstrap_supported_legacy_never_inserts_or_consumes_registry_capacity() {
    for version in 1..=6 {
        let f = Fixture::new(Durable::default());
        let mut old = if version == 1 {
            serde_json::from_slice(include_bytes!("../tests/fixtures/voice-v1.json")).unwrap()
        } else if version == 5 {
            frozen_disk5().0
        } else {
            json!(Durable::default())
        };
        old["schema_version"] = json!(version);
        // This cohort test needs migration headroom; rich frozen fixtures also
        // exercise exhausted revisions in their existing dedicated regressions.
        if version == 5 {
            old["config"]["revision"] = json!(10);
        }
        old.as_object_mut().unwrap().remove("instructions");
        old.as_object_mut().unwrap().remove("agents");
        if version < 5 {
            old.as_object_mut().unwrap().remove("projects");
        }
        if version < 4 {
            old.as_object_mut().unwrap().remove("timestamps");
        }
        if version < 3 {
            old.as_object_mut().unwrap().remove("cloud");
        }
        let bytes = f.write(&old);
        let (converted, migrated) = Durable::load(&bytes).unwrap();
        assert!(migrated);
        let expected = json!(converted.projects);
        let app = f.reopen().unwrap();
        assert_eq!(f.disk()["projects"], expected, "disk{version}");
        assert_eq!(list(&app)["projects"], expected["projects"]);
        drop(app);
        let published = f.bytes();
        let app = f.reopen().unwrap();
        assert_eq!(f.bytes(), published);
        assert_eq!(list(&app)["revision"], expected["revision"]);
    }
}

#[test]
fn projects_bootstrap_corrupt_empty_unsupported_and_unsafe_destinations_are_preserved() {
    for bytes in [
        vec![],
        b"not json".to_vec(),
        b"{}".to_vec(),
        b"{\"schema_version\":999}".to_vec(),
    ] {
        let f = Fixture::new(Durable::default());
        fs::write(&f.app.path, &bytes).unwrap();
        assert!(f.reopen().is_err());
        assert_eq!(f.bytes(), bytes);
    }
    let f = Fixture::new(Durable::default());
    let bytes = f.bytes();
    fs::set_permissions(&f.app.path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), bytes);
    assert_eq!(
        fs::metadata(&f.app.path).unwrap().permissions().mode() & 0o7777,
        0o644
    );
    let preserved = f.app.path.with_extension("preserved");
    fs::rename(&f.app.path, &preserved).unwrap();
    std::os::unix::fs::symlink(&preserved, &f.app.path).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(fs::read_link(&f.app.path).unwrap(), preserved);
    assert_eq!(fs::read(&preserved).unwrap(), bytes);
    fs::remove_file(&f.app.path).unwrap();
    fs::hard_link(&preserved, &f.app.path).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), bytes);
    fs::remove_file(&f.app.path).unwrap();
    fs::create_dir(&f.app.path).unwrap();
    assert!(f.reopen().is_err());
    assert!(f.app.path.is_dir());
    assert_eq!(fs::read(&preserved).unwrap(), bytes);
}

#[test]
fn projects_bootstrap_staging_obstruction_and_appeared_destination_fail_without_clobber() {
    let f = Fixture::new(Durable::default());
    fs::remove_file(&f.app.path).unwrap();
    let staging = f.app.path.parent().unwrap().join(format!(
        ".smith-next-app-state.json-{}-tmp",
        std::process::id()
    ));
    fs::write(&staging, b"staging obstruction").unwrap();
    assert_eq!(f.reopen().err().unwrap(), "Cannot create staging file");
    assert!(!f.app.path.exists());
    assert_eq!(fs::read(&staging).unwrap(), b"staging obstruction");
    fs::remove_file(staging).unwrap();
    // Model a destination appearing after the caller's successful absent read.
    assert!(filesystem::read_file(&f.app.path, 0o600, STATE_LIMIT)
        .unwrap()
        .is_none());
    let existing = b"preserve even invalid newly appeared state";
    fs::write(&f.app.path, existing).unwrap();
    fs::set_permissions(&f.app.path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        bootstrap(&f.app.path).err().unwrap(),
        "Destination exists or cannot publish"
    );
    assert_eq!(f.bytes(), existing);
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), existing);
}

#[test]
fn projects_bootstrap_consumes_ordinary_capacity_and_honors_revoked_ownership() {
    let f = fresh_fixture();
    for revision in 1..MAX_PROJECTS {
        call(
            &f.app,
            "project_create_v3",
            create(&f.app, revision, revision as u64),
        )
        .unwrap();
    }
    assert_eq!(
        list(&f.app)["projects"].as_array().unwrap().len(),
        MAX_PROJECTS
    );
    f.assert_rejected(
        "project_create_v3",
        create(&f.app, 100, MAX_PROJECTS as u64),
        "CAPACITY",
    );
    let app = f.reopen().unwrap();
    assert_eq!(list(&app), list(&f.app));
    f.app.inner.lock().unwrap().owned = false;
    let target = list(&app)["projects"][0]["project_id"].clone();
    f.assert_rejected(
        "project_delete_v3",
        delete(&f.app, 101, MAX_PROJECTS as u64, &target),
        "STATE_UNAVAILABLE",
    );
    assert_eq!(
        list(&app)["projects"].as_array().unwrap().len(),
        MAX_PROJECTS
    );
}

#[test]
fn projects_locator_grammar_accepts_only_the_three_inert_ascii_forms() {
    for locator in [
        "https://example.invalid/a",
        "https://EXAMPLE.invalid:65535/Team/Repo.git",
        "https://a:1/._~-a0/..git",
        "https://127.0.0.1:00443/repo",
        "ssh://git@example.invalid/repo.git",
        "ssh://git@a-b.invalid:22/team/repo",
        "git@example.invalid:team/repo.git",
        "git@a:repo",
    ] {
        assert!(valid_locator(locator), "{locator}");
    }
    for locator in [
        "",
        "example.invalid/repo",
        "/repo",
        "./repo",
        "../repo",
        "file:///repo",
        "http://example.invalid/repo",
        "HTTPS://example.invalid/repo",
        "git://example.invalid/repo",
        "https://git@example.invalid/repo",
        "https://user:placeholder@example.invalid/repo",
        "ssh://user@example.invalid/repo",
        "ssh://git:placeholder@example.invalid/repo",
        "ssh://example.invalid/repo",
        "user@example.invalid:repo",
        "git@:repo",
        "https://example.invalid",
        "https://example.invalid/",
        "git@example.invalid:",
        "https://example.invalid/a/",
        "https://example.invalid//a",
        "https://example.invalid/a//b",
        "https://example.invalid/.",
        "https://example.invalid/..",
        "git@a:a/../b",
        "git@a:./b",
        "https://example.invalid/a?x",
        "https://example.invalid/a#x",
        "https://example.invalid/a%20b",
        "https://example.invalid/a;b",
        "https://example.invalid/a&b",
        "https://example.invalid/a|b",
        "https://example.invalid/a\\b",
        "https://example.invalid/a:b",
        "https://example.invalid/a@b",
        "https://example.invalid/a'b",
        "https://example.invalid/a\"b",
        "https://example.invalid/a`b",
        "https://example.invalid/a(b)",
        "https://example.invalid/a<b",
        "https://example.invalid/a>b",
        "https://example.invalid/a+b",
        "https://example.invalid/a=b",
        "https://example.invalid/a*b",
        "https://example.invalid/a$b",
        "https://example.invalid/a b",
        "https://example.invalid/a\nb",
        "https://example.invalid/a\0b",
        "https://example.invalid/répô",
        "https://é.invalid/repo",
        "https://.example.invalid/repo",
        "https://example..invalid/repo",
        "https://example.invalid./repo",
        "https://-example.invalid/repo",
        "https://example-.invalid/repo",
        "https://a_b.invalid/repo",
        "https://[::1]/repo",
        "https://a:0/repo",
        "https://a:65536/repo",
        "https://a:/repo",
        "https://a:-1/repo",
        "https://a:+1/repo",
        "https://a:1.0/repo",
        "https://a:1:2/repo",
        " https://a/repo",
        "https://a/repo ",
        "git@a:/repo",
        "git@a:a:b",
    ] {
        assert!(!valid_locator(locator), "{locator:?}");
    }
    let host = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    assert_eq!(host.len(), 253);
    assert!(valid_locator(&format!("https://{host}/repo")));
    assert!(!valid_locator(&format!("https://{host}d/repo")));
    assert!(!valid_locator(&format!(
        "https://{}.invalid/repo",
        "a".repeat(64)
    )));
    let locator = format!("https://a/{}", "b".repeat(MAX_LOCATOR_BYTES - 10));
    assert_eq!(locator.len(), MAX_LOCATOR_BYTES);
    assert!(valid_locator(&locator));
    assert!(!valid_locator(&(locator + "b")));
}

#[test]
fn projects_name_repository_and_agent_limits_are_strict_without_normalization() {
    for name in ["A", "two words", "é", "🚀", "A  B"] {
        assert!(valid_name(name));
    }
    assert!(valid_name(&"🚀".repeat(40)));
    assert!(!valid_name(&"🚀".repeat(41)));
    assert!(valid_name(&"e\u{0301}".repeat(20)));
    assert!(!valid_name(&"e\u{0301}".repeat(21)));
    for name in [
        "",
        " ",
        " A",
        "A ",
        "A\tB",
        "A\nB",
        "A\0B",
        "A\u{007f}B",
        "A\u{0085}B",
        "\u{2003}A",
        "A\u{feff}",
    ] {
        assert!(!valid_name(name), "{name:?}");
    }
    let repositories: Vec<_> = (0..16)
        .map(|n| Repository::unknown(format!("https://example.invalid/repo{n}")))
        .collect();
    assert!(valid_metadata("A", &repositories, "opencode"));
    let mut too_many = repositories.clone();
    too_many.push(Repository::unknown("https://example.invalid/other".into()));
    assert!(!valid_metadata("A", &too_many, "opencode"));
    let mut duplicate = repositories;
    duplicate[1] = duplicate[0].clone();
    assert!(!valid_metadata("A", &duplicate, "opencode"));
    assert!(valid_metadata(
        "A",
        &[
            Repository::unknown("https://a/Repo".into()),
            Repository::unknown("https://a/repo".into()),
            Repository::unknown("https://A/repo".into())
        ],
        "opencode"
    ));
    for agent in ["", "OpenCode", "other", "opencode "] {
        assert!(!valid_metadata("A", &[], agent));
    }
}

#[test]
fn projects_crud_receipts_and_empty_registry_survive_restart_without_provider_calls() {
    let f = Fixture::new(Durable::default());
    let app = f.reopen().unwrap();
    assert_eq!(list(&app), json!({"revision":0,"projects":[]}));
    let mut a = create(&app, 1, 0);
    a["name"] = json!("First 🚀");
    a["repositories"] = json!([
        named_repository("repo", "https://example.invalid/Team/repo.git"),
        named_repository("other", "git@example.invalid:other")
    ]);
    let first = call(&app, "project_create_v3", a.clone()).unwrap();
    assert_eq!(first.as_object().unwrap().len(), 4);
    assert_eq!(first["request_id"], id(1));
    assert!(valid_id(first["project_id"].as_str().unwrap()));
    assert_eq!(first["revision"], 1);
    assert_eq!(first["deleted"], false);
    assert_eq!(
        list(&app),
        json!({"revision":1,"projects":[{"project_id":first["project_id"],"name":a["name"],
            "key":a["key"],"description":a["description"],"prompt":a["prompt"],
            "repositories":a["repositories"],"agent":"opencode"}]})
    );
    assert_eq!(receipt(&app, 1).unwrap(), first);
    let second = call(&app, "project_create_v3", create(&app, 2, 1)).unwrap();
    assert_ne!(second["project_id"], first["project_id"]);
    let mut b = update(&app, 3, 2, &first["project_id"]);
    b["key"] = json!("renamed");
    let edited = call(&app, "project_update_v3", b).unwrap();
    assert_eq!(edited["revision"], 3);
    assert_eq!(list(&app)["projects"][0]["key"], "renamed");
    assert_eq!(list(&app)["projects"][0]["name"], "First 🚀");
    assert_eq!(list(&app)["projects"][0]["repositories"], json!([]));
    let before = list(&app);
    drop(app);
    let app = f.reopen().unwrap();
    assert_eq!(list(&app), before);
    assert_eq!(receipt(&app, 1).unwrap(), first);
    let d = delete(&app, 4, 3, &first["project_id"]);
    let removed = call(&app, "project_delete_v3", d.clone()).unwrap();
    assert_eq!(
        removed,
        json!({"request_id":id(4),"project_id":first["project_id"],"revision":4,"deleted":true})
    );
    assert_eq!(call(&app, "project_delete_v3", d).unwrap(), removed);
    call(
        &app,
        "project_delete_v3",
        delete(&app, 5, 4, &second["project_id"]),
    )
    .unwrap();
    assert_eq!(list(&app), json!({"revision":5,"projects":[]}));
    drop(app);
    let app = f.reopen().unwrap();
    assert_eq!(list(&app), json!({"revision":5,"projects":[]}));
    assert_eq!(receipt(&app, 4).unwrap(), removed);
    let third = call(&app, "project_create_v3", create(&app, 6, 5)).unwrap();
    assert_ne!(third["project_id"], first["project_id"]);
    assert_ne!(third["project_id"], second["project_id"]);
    assert_eq!(f.disk()["schema_version"], 10);
}

#[test]
fn projects_replay_precedes_fresh_fences_and_conflicts_bind_action_and_all_arguments() {
    let f = Fixture::new(Durable::default());
    let a = create(&f.app, 1, 0);
    let first = call(&f.app, "project_create_v3", a.clone()).unwrap();
    call(&f.app, "project_create_v3", create(&f.app, 2, 1)).unwrap();
    let bytes = f.bytes();
    assert_eq!(call(&f.app, "project_create_v3", a.clone()).unwrap(), first);
    assert_eq!(f.bytes(), bytes);
    for (key, value) in [
        ("name", json!("Changed")),
        ("repositories", json!(["https://a/repo"])),
        ("agent", json!("other")),
        ("instance_id", json!(id(987))),
        ("expected_revision", json!(2)),
        ("extra", json!(true)),
    ] {
        let mut changed = a.clone();
        changed[key] = value;
        f.assert_rejected("project_create_v3", changed, "ID_CONFLICT");
    }
    f.assert_rejected(
        "project_update_v3",
        update(&f.app, 1, 2, &first["project_id"]),
        "ID_CONFLICT",
    );
    f.assert_rejected(
        "project_delete_v3",
        delete(&f.app, 1, 2, &first["project_id"]),
        "ID_CONFLICT",
    );
    let app = f.reopen().unwrap();
    assert_ne!(app.instance_id, f.app.instance_id);
    assert_eq!(call(&app, "project_create_v3", a).unwrap(), first);
    assert_eq!(f.bytes(), bytes);
    let mut fresh = create(&app, 3, 2);
    fresh["instance_id"] = json!(f.app.instance_id);
    assert_eq!(
        call(&app, "project_create_v3", fresh).unwrap_err().code,
        "INSTANCE_MISMATCH"
    );
    assert_eq!(receipt(&app, 1).unwrap(), first);
    assert_eq!(receipt(&app, 999).unwrap_err().code, "REQUEST_UNKNOWN");
}

#[test]
fn projects_delete_scrubs_every_old_fingerprint_but_keeps_content_free_historical_receipts() {
    let f = Fixture::new(Durable::default());
    let mut a = create(&f.app, 1, 0);
    a["name"] = json!("old-name-canary");
    a["repositories"] = json!([named_repository(
        "canary",
        "https://example.invalid/repository-canary"
    )]);
    let first = call(&f.app, "project_create_v3", a.clone()).unwrap();
    let mut b = update(&f.app, 2, 1, &first["project_id"]);
    b["key"] = json!("new-name-canary");
    let edited = call(&f.app, "project_update_v3", b.clone()).unwrap();
    let unrelated = call(&f.app, "project_create_v3", create(&f.app, 3, 2)).unwrap();
    let previous_fingerprints = f.disk()["projects"]["receipts"]
        .as_array()
        .unwrap()
        .iter()
        .take(2)
        .map(|r| r["fingerprint"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let d = delete(&f.app, 4, 3, &first["project_id"]);
    let removed = call(&f.app, "project_delete_v3", d.clone()).unwrap();
    let bytes = f.bytes();
    let text = String::from_utf8(bytes.clone()).unwrap();
    for absent in [
        "old-name-canary",
        "new-name-canary",
        "repository-canary",
        &previous_fingerprints[0],
        &previous_fingerprints[1],
    ] {
        assert!(!text.contains(absent), "{absent}");
    }
    assert_eq!(receipt(&f.app, 1).unwrap(), first);
    assert_eq!(receipt(&f.app, 2).unwrap(), edited);
    for (op, args) in [
        ("project_create_v3", a.clone()),
        ("project_update_v3", b.clone()),
    ] {
        f.assert_rejected(op, args, "REQUEST_EXPIRED");
    }
    f.assert_rejected("project_create_v3", create(&f.app, 2, 4), "REQUEST_EXPIRED");
    assert_eq!(list(&f.app)["projects"].as_array().unwrap().len(), 1);
    assert_eq!(
        list(&f.app)["projects"][0]["project_id"],
        unrelated["project_id"]
    );
    let app = f.reopen().unwrap();
    assert_eq!(receipt(&app, 1).unwrap(), first);
    assert_eq!(receipt(&app, 2).unwrap(), edited);
    assert_eq!(call(&app, "project_delete_v3", d).unwrap(), removed);
    assert_eq!(
        call(&app, "project_create_v3", a).unwrap_err().code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(
        call(&app, "project_update_v3", b).unwrap_err().code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_strict_request_shapes_syntax_and_stale_fences_never_partially_publish() {
    let f = Fixture::new(Durable::default());
    let first = call(&f.app, "project_create_v3", create(&f.app, 1, 0)).unwrap();
    for (op, good) in [
        ("project_create_v3", create(&f.app, 2, 1)),
        (
            "project_update_v3",
            update(&f.app, 2, 1, &first["project_id"]),
        ),
        (
            "project_delete_v3",
            delete(&f.app, 2, 1, &first["project_id"]),
        ),
        ("project_receipt", json!({"request_id":id(1)})),
    ] {
        for key in good.as_object().unwrap().keys() {
            let mut missing = good.clone();
            missing.as_object_mut().unwrap().remove(key);
            f.assert_rejected(op, missing, "INVALID_REQUEST");
            let mut null = good.clone();
            null[key] = Value::Null;
            f.assert_rejected(op, null, "INVALID_REQUEST");
        }
        let mut extra = good.clone();
        extra["extra"] = json!(0);
        f.assert_rejected(op, extra, "INVALID_REQUEST");
        for args in [Value::Null, json!([]), json!("text"), json!(1)] {
            f.assert_rejected(op, args, "INVALID_REQUEST");
        }
    }
    for value in [
        json!(-1),
        json!(1.5),
        json!("1"),
        json!(true),
        json!(v2::MAX_REVISION + 1),
    ] {
        let mut a = create(&f.app, 2, 1);
        a["expected_revision"] = value;
        f.assert_rejected("project_create_v3", a, "INVALID_REQUEST");
    }
    for (key, values) in [
        (
            "name",
            vec![
                json!(""),
                json!(" leading"),
                json!("trailing "),
                json!("a\nb"),
                json!("🚀".repeat(41)),
                json!(1),
            ],
        ),
        (
            "repositories",
            vec![
                json!("https://a/repo"),
                json!([null]),
                json!(["https://a/repo", "https://a/repo"]),
                json!(["https://user:placeholder@a/repo"]),
                json!(["/local/path"]),
                json!((0..17)
                    .map(|n| format!("https://a/{n}"))
                    .collect::<Vec<_>>()),
            ],
        ),
        (
            "agent",
            vec![json!("other"), json!("OpenCode"), json!(true)],
        ),
        (
            "request_id",
            vec![json!("bad"), json!("A".repeat(32)), json!(2)],
        ),
        ("instance_id", vec![json!("bad"), json!(2)]),
    ] {
        for value in values {
            let mut a = create(&f.app, 2, 1);
            a[key] = value;
            f.assert_rejected("project_create_v3", a, "INVALID_REQUEST");
        }
    }
    f.assert_rejected("projects_list_v3", json!({"extra":0}), "INVALID_REQUEST");
    f.assert_rejected("projects_list_v3", json!([]), "INVALID_REQUEST");
    for (op, mut a) in [
        ("project_create_v3", create(&f.app, 2, 0)),
        (
            "project_update_v3",
            update(&f.app, 2, 0, &first["project_id"]),
        ),
        (
            "project_delete_v3",
            delete(&f.app, 2, 0, &first["project_id"]),
        ),
    ] {
        f.assert_rejected(op, a.clone(), "STALE_REVISION");
        a["expected_revision"] = json!(1);
        a["instance_id"] = json!(id(987));
        f.assert_rejected(op, a, "INSTANCE_MISMATCH");
    }
    for (op, a) in [
        ("project_update_v3", update(&f.app, 2, 1, &json!(id(987)))),
        ("project_delete_v3", delete(&f.app, 2, 1, &json!(id(987)))),
    ] {
        f.assert_rejected(op, a, "PROJECT_NOT_FOUND");
    }
    let mut invalid_target = delete(&f.app, 2, 1, &json!("bad"));
    f.assert_rejected(
        "project_delete_v3",
        invalid_target.clone(),
        "INVALID_REQUEST",
    );
    invalid_target["name"] = json!("Project");
    f.assert_rejected("project_create_v3", invalid_target, "INVALID_REQUEST");
    assert_eq!(list(&f.app)["revision"], 1);
}

fn legacy_terminal() -> Durable {
    let (mut d, _) = Durable::load(include_bytes!("../tests/fixtures/voice-v1.json")).unwrap();
    for j in &mut d.jobs {
        if v2::pending(&j.operation) {
            j.operation.status = "interrupted".into();
            j.operation.phase = "complete".into();
            j.operation.error = Some(AppError::new("INTERRUPTED", "Interrupted without replay"));
        }
    }
    d.config.revision = u64::MAX;
    d.revision = 10;
    d.validate().unwrap();
    d
}
fn rich_durable() -> Durable {
    let mut value = json!(legacy_terminal());
    let target = value["conversations"][0]["conversation_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let pairs = value["conversations"][0]["history"]
        .as_array()
        .unwrap()
        .len()
        / 2;
    value["cloud"]["providers"][0]["revision"] = json!(7);
    value["cloud"]["providers"][0]["credential"] =
        json!({"provider":"openai","generation":id(700),"save_request_id":id(701)});
    value["cloud"]["providers"][0]["validation"] = json!({"request_id":id(702),"model":"gpt-6-astra","credential_generation":id(700),"status":"succeeded","completed_ms":1000,"error":null,"delivery":"received"});
    let route = json!({"provider":"openai","model":"gpt-6-astra","credential_generation":id(700)});
    let provenance: Vec<_> = (0..pairs)
        .map(|n| json!({"request_id":id(800+n),"route":route}))
        .collect();
    value["cloud"]["conversations"][&target] = json!({"last_route":route,"approved_destinations":[{"provider":"openai","credential_generation":id(700)}],"turn_routes":provenance});
    value["timestamps"][&target] =
        json!(vec![json!({"user_at_ms":123,"assistant_at_ms":456}); pairs]);
    let d: Durable = serde_json::from_value(value).unwrap();
    d.validate().unwrap();
    d
}

#[test]
fn projects_disk1_through_frozen_disk4_migrate_empty_preserving_all_existing_data() {
    for version in 1..=4 {
        let f = Fixture::new(Durable::default());
        let mut old = if version == 1 {
            serde_json::from_slice(include_bytes!("../tests/fixtures/voice-v1.json")).unwrap()
        } else {
            json!(rich_durable())
        };
        old["schema_version"] = json!(version);
        if version > 1 {
            old.as_object_mut().unwrap().remove("projects");
            old.as_object_mut().unwrap().remove("instructions");
            old.as_object_mut().unwrap().remove("agents");
            old["config"]["revision"] = json!(u64::MAX - 1);
            if version < 4 {
                old.as_object_mut().unwrap().remove("timestamps");
            }
            if version < 3 {
                old.as_object_mut().unwrap().remove("cloud");
            }
        }
        let bytes = f.write(&old);
        let (d, migrated) = Durable::load(&bytes).unwrap();
        assert!(migrated);
        d.validate().unwrap();
        assert_eq!(json!(d.projects), json!(Registry::default()));
        let mut expected_config = old["config"].clone();
        expected_config["revision"] = json!(old["config"]["revision"].as_u64().unwrap() + 1);
        if version == 1 {
            let mut expected = old.clone();
            expected["config"] = expected_config.clone();
            for job in expected["jobs"].as_array_mut().unwrap() {
                if matches!(
                    job["operation"]["status"].as_str(),
                    Some("queued" | "running")
                ) {
                    job["operation"]["status"] = json!("interrupted");
                    job["operation"]["phase"] = json!("complete");
                    job["operation"]["result"] = Value::Null;
                    job["operation"]["error"] = json!({"code":"INTERRUPTED","message":"Operation interrupted; it was not replayed"});
                }
            }
            assert_eq!(json!(d.legacy()), expected);
        } else {
            let encoded = json!(d);
            for (key, value) in old.as_object().unwrap() {
                if !matches!(key.as_str(), "schema_version" | "config") {
                    assert_eq!(encoded[key], *value, "disk{version} {key}");
                }
            }
            assert_eq!(encoded["config"], expected_config);
        }
        let app = f.reopen().unwrap();
        assert_eq!(list(&app), json!({"revision":0,"projects":[]}));
        let disk = f.disk();
        assert_eq!(disk["schema_version"], 10);
        assert_eq!(disk["config"], expected_config);
        assert_eq!(disk["expired_ids"], old["expired_ids"]);
        if version > 1 {
            for key in [
                "conversations",
                "jobs",
                "legacy_conversation_id",
                "next_sequence",
            ] {
                assert_eq!(disk[key], old[key], "disk{version} {key}");
            }
            assert_eq!(disk["revision"], old["revision"].as_u64().unwrap() + 1);
        }
        if version >= 3 {
            assert_eq!(disk["cloud"], old["cloud"]);
        }
        if version == 4 {
            assert_eq!(disk["timestamps"], old["timestamps"]);
        }
        drop(app);
        let before = f.bytes();
        let app = f.reopen().unwrap();
        assert_eq!(list(&app)["revision"], 0);
        assert_eq!(f.bytes(), before);
    }
}

#[test]
fn projects_disk6_and_frozen_legacy_schemas_reject_corruption_without_rewriting_bytes() {
    let f = Fixture::new(rich_durable());
    let first = call(&f.app, "project_create_v3", create(&f.app, 1, 0)).unwrap();
    let good = f.disk();
    let mut bad = vec![];
    for version in [0, 1, 2, 3, 4, 7] {
        let mut v = good.clone();
        v["schema_version"] = json!(version);
        bad.push(v);
    }
    for key in ["projects", "cloud", "timestamps", "legacy_conversation_id"] {
        let mut v = good.clone();
        v.as_object_mut().unwrap().remove(key);
        bad.push(v);
    }
    for key in ["revision", "projects", "receipts", "expired_ids"] {
        let mut v = good.clone();
        v["projects"].as_object_mut().unwrap().remove(key);
        bad.push(v);
    }
    for (pointer, value) in [
        ("/projects/extra", json!(0)),
        ("/projects/revision", json!(v2::MAX_REVISION + 1)),
        ("/projects/revision", json!(0)),
        ("/projects/projects/0/project_id", json!("bad")),
        ("/projects/projects/0/name", json!("untrimmed ")),
        (
            "/projects/projects/0/repositories",
            json!(["https://user:placeholder@a/repo"]),
        ),
        ("/projects/projects/0/agent", json!("other")),
        ("/projects/projects/0/extra", json!(0)),
        ("/projects/receipts/0/fingerprint", Value::Null),
        ("/projects/receipts/0/fingerprint", json!("not-a-digest")),
        ("/projects/receipts/0/receipt/project_id", json!(id(987))),
        ("/projects/receipts/0/receipt/request_id", json!("bad")),
        ("/projects/receipts/0/receipt/deleted", json!(true)),
        ("/projects/receipts/0/receipt/revision", json!(0)),
        ("/projects/receipts/0/receipt/revision", json!(2)),
        ("/projects/expired_ids", json!([id(1)])),
        ("/projects/expired_ids", json!([id(987), id(987)])),
        ("/projects/expired_ids", json!(["bad"])),
    ] {
        let mut v = good.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
        bad.push(v);
    }
    for (parent, key) in [
        ("/projects/projects/0", "agent"),
        ("/projects/receipts/0", "fingerprint"),
        ("/projects/receipts/0/receipt", "deleted"),
    ] {
        let mut v = good.clone();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        bad.push(v);
    }
    for field in ["projects", "receipts"] {
        let mut v = good.clone();
        let entry = v["projects"][field][0].clone();
        v["projects"][field].as_array_mut().unwrap().push(entry);
        bad.push(v);
    }
    for key in ["cloud", "timestamps", "legacy_conversation_id"] {
        let mut v = good.clone();
        v["schema_version"] = json!(4);
        v.as_object_mut().unwrap().remove("projects");
        v.as_object_mut().unwrap().remove(key);
        bad.push(v);
    }
    let registry = &good["projects"];
    let project = &registry["projects"][0];
    let retained = &registry["receipts"][0];
    let receipt_fields = &retained["receipt"];
    for (pointer, value) in [
        (
            "/projects",
            json!([
                registry["revision"],
                registry["projects"],
                registry["receipts"],
                registry["expired_ids"]
            ]),
        ),
        (
            "/projects/projects/0",
            json!([
                project["project_id"],
                project["name"],
                project["repositories"],
                project["agent"]
            ]),
        ),
        (
            "/projects/receipts/0",
            json!([retained["receipt"], retained["fingerprint"]]),
        ),
        (
            "/projects/receipts/0/receipt",
            json!([
                receipt_fields["request_id"],
                receipt_fields["project_id"],
                receipt_fields["revision"],
                receipt_fields["deleted"]
            ]),
        ),
    ] {
        let mut v = good.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        bad.push(v);
    }
    for v in bad {
        let bytes = f.write(&v);
        assert!(f.reopen().is_err(), "corrupt fixture accepted");
        assert_eq!(f.bytes(), bytes);
    }
    let duplicate = serde_json::to_string(&good).unwrap().replacen(
        "\"revision\":1",
        "\"revision\":1,\"revision\":1",
        1,
    );
    fs::write(&f.app.path, duplicate.as_bytes()).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), duplicate.as_bytes());
    f.write(&good);
    let app = f.reopen().unwrap();
    assert_eq!(receipt(&app, 1).unwrap(), first);
}

#[test]
fn projects_are_app3_only_keep_old_chat_replies_and_never_use_chat_receipt_capacity() {
    let f = Fixture::new(legacy_terminal());
    let states: Vec<_> = (1..=3)
        .map(|v| f.app.call(request(v, "state", json!({}))).unwrap())
        .collect();
    let before = f.disk();
    // The same opaque ID is deliberately valid in both independent families.
    let chat_id = before["jobs"][0]["operation"]["request_id"]
        .as_str()
        .unwrap();
    let mut a = create(&f.app, 900, 0);
    a["request_id"] = json!(chat_id);
    let first = call(&f.app, "project_create_v3", a).unwrap();
    assert_eq!(list(&f.app)["revision"], 1);
    assert_eq!(f.disk()["jobs"], before["jobs"]);
    assert_eq!(f.disk()["expired_ids"], before["expired_ids"]);
    for version in 1..=3 {
        let mut after = f.app.call(request(version, "state", json!({}))).unwrap();
        let expected = &states[version as usize - 1];
        if version > 1 {
            assert_eq!(
                after["revision"],
                expected["revision"].as_u64().unwrap() + 1
            );
            after["revision"] = expected["revision"].clone();
        }
        assert_eq!(after, *expected);
    }
    for version in [1, 2, 4] {
        for (op, args) in [
            ("projects_list_v3", json!({})),
            ("project_create_v3", create(&f.app, 901, 1)),
            (
                "project_update_v3",
                update(&f.app, 901, 1, &first["project_id"]),
            ),
            (
                "project_delete_v3",
                delete(&f.app, 901, 1, &first["project_id"]),
            ),
            ("project_receipt", json!({"request_id":chat_id})),
        ] {
            assert_eq!(
                f.app.call(request(version, op, args)).unwrap_err().code,
                "INVALID_REQUEST"
            );
        }
    }
    call(
        &f.app,
        "project_update_v3",
        update(&f.app, 901, 1, &first["project_id"]),
    )
    .unwrap();
    call(
        &f.app,
        "project_delete_v3",
        delete(&f.app, 902, 2, &first["project_id"]),
    )
    .unwrap();
    let after = f.disk();
    for key in [
        "config",
        "conversations",
        "jobs",
        "expired_ids",
        "cloud",
        "timestamps",
        "legacy_conversation_id",
        "next_sequence",
    ] {
        assert_eq!(after[key], before[key], "{key}");
    }
}

#[test]
fn projects_global_revision_is_independent_of_chat_mutations_and_pending_work() {
    let mut d = legacy_terminal();
    let target = d.conversations[0].conversation_id.clone();
    d.jobs.clear();
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(100),
            kind: "submit".into(),
            status: "running".into(),
            phase: "generation".into(),
            config_revision: d.config.revision,
            result: None,
            error: None,
        },
        fingerprint: "a".repeat(64),
        version: 2,
        conversation_id: Some(target.clone()),
        history_revision: Some(0),
        task: Some(Task::Submit {
            text: "chat-only input".into(),
        }),
        accepted_ms: 0,
    });
    d.instructions.capture(&id(100));
    let f = Fixture::new(d);
    let before = f.disk();
    let first = call(&f.app, "project_create_v3", create(&f.app, 1, 0)).unwrap();
    assert_eq!(first["revision"], 1);
    assert_eq!(f.disk()["jobs"], before["jobs"]);
    f.app.call(request(2,"conversation_rename",json!({"request_id":id(101),"conversation_id":target,"expected_metadata_revision":0,"title":"Chat rename"}))).unwrap();
    assert_eq!(list(&f.app)["revision"], 1);
    assert_eq!(f.disk()["revision"], 12);
    call(
        &f.app,
        "project_update_v3",
        update(&f.app, 2, 1, &first["project_id"]),
    )
    .unwrap();
    assert_eq!(list(&f.app)["revision"], 2);
    assert_eq!(f.disk()["jobs"][0], before["jobs"][0]);
}

#[test]
fn projects_receipt_eviction_tombstones_and_project_count_are_atomic_and_replayable() {
    let f = Fixture::new(Durable::default());
    let first_args = create(&f.app, 1, 0);
    let first = call(&f.app, "project_create_v3", first_args.clone()).unwrap();
    for n in 2..=32 {
        call(
            &f.app,
            "project_create_v3",
            create(&f.app, n, (n - 1) as u64),
        )
        .unwrap();
    }
    f.assert_rejected("project_create_v3", create(&f.app, 33, 32), "CAPACITY");
    let edited_args = update(&f.app, 33, 32, &first["project_id"]);
    let edited = call(&f.app, "project_update_v3", edited_args.clone()).unwrap();
    assert_eq!(
        f.disk()["projects"]["receipts"].as_array().unwrap().len(),
        32
    );
    assert_eq!(f.disk()["projects"]["expired_ids"], json!([id(1)]));
    assert_eq!(receipt(&f.app, 1).unwrap_err().code, "REQUEST_EXPIRED");
    f.assert_rejected("project_create_v3", first_args.clone(), "REQUEST_EXPIRED");
    {
        let mut inner = f.app.inner.lock().unwrap();
        inner
            .durable
            .projects
            .expired_ids
            .extend((1000..2023).map(id));
        inner.durable.projects.validate().unwrap();
        persist(&f.app.path, &inner.durable).unwrap();
    }
    let bytes = f.bytes();
    assert_eq!(
        call(&f.app, "project_update_v3", edited_args.clone()).unwrap(),
        edited
    );
    f.assert_rejected(
        "project_update_v3",
        update(&f.app, 34, 33, &first["project_id"]),
        "CAPACITY",
    );
    f.assert_rejected(
        "project_delete_v3",
        delete(&f.app, 34, 33, &first["project_id"]),
        "CAPACITY",
    );
    assert_eq!(f.bytes(), bytes);
    let app = f.reopen().unwrap();
    assert_eq!(
        call(&app, "project_update_v3", edited_args).unwrap(),
        edited
    );
    assert_eq!(receipt(&app, 1000).unwrap_err().code, "REQUEST_EXPIRED");
    assert_eq!(
        call(&app, "project_create_v3", create(&app, 1000, 33))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(list(&app)["projects"].as_array().unwrap().len(), 32);
    assert_eq!(f.disk()["jobs"], json!([]));
    assert_eq!(f.disk()["expired_ids"], json!([]));
}

#[test]
fn projects_safe_revision_and_shared_application_transition_headroom_refuse_before_effects() {
    let f = Fixture::new(Durable::default());
    let a = create(&f.app, 1, 0);
    let first = call(&f.app, "project_create_v3", a.clone()).unwrap();
    {
        let mut inner = f.app.inner.lock().unwrap();
        inner.durable.projects.revision = v2::MAX_REVISION;
        inner.durable.projects.receipts[0].receipt.revision = v2::MAX_REVISION;
        persist(&f.app.path, &inner.durable).unwrap();
    }
    f.assert_rejected(
        "project_create_v3",
        create(&f.app, 2, v2::MAX_REVISION),
        "CAPACITY",
    );
    f.assert_rejected(
        "project_update_v3",
        update(&f.app, 2, v2::MAX_REVISION, &first["project_id"]),
        "CAPACITY",
    );
    f.assert_rejected(
        "project_delete_v3",
        delete(&f.app, 2, v2::MAX_REVISION, &first["project_id"]),
        "CAPACITY",
    );
    {
        let mut inner = f.app.inner.lock().unwrap();
        inner.durable.projects.revision = 1;
        inner.durable.projects.receipts[0].receipt.revision = 1;
        inner.durable.revision = v2::MAX_REVISION;
        persist(&f.app.path, &inner.durable).unwrap();
    }
    let bytes = f.bytes();
    assert_eq!(call(&f.app, "project_create_v3", a).unwrap(), first);
    f.assert_rejected("project_create_v3", create(&f.app, 2, 1), "CAPACITY");
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_concurrent_creates_and_update_delete_races_have_one_revision_winner() {
    let f = Fixture::new(Durable::default());
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (1..=2)
        .map(|n| {
            let app = f.app.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let a = create(&app, n, 0);
                barrier.wait();
                call(&app, "project_create_v3", a)
            })
        })
        .collect();
    let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| r.as_ref().is_err_and(|e| e.code == "STALE_REVISION"))
            .count(),
        1
    );
    let first = outcomes.into_iter().find_map(|r| r.ok()).unwrap();
    assert_eq!(list(&f.app)["revision"], 1);
    assert_eq!(list(&f.app)["projects"].as_array().unwrap().len(), 1);
    let threads: Vec<_> = ["project_update_v3", "project_delete_v3"]
        .into_iter()
        .enumerate()
        .map(|(index, op)| {
            let app = f.app.clone();
            let barrier = barrier.clone();
            let target = first["project_id"].clone();
            std::thread::spawn(move || {
                let args = if op == "project_update_v3" {
                    update(&app, index + 3, 1, &target)
                } else {
                    delete(&app, index + 3, 1, &target)
                };
                barrier.wait();
                call(&app, op, args)
            })
        })
        .collect();
    let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| r.as_ref().is_err_and(|e| e.code == "STALE_REVISION"))
            .count(),
        1
    );
    assert_eq!(list(&f.app)["revision"], 2);
    assert_eq!(
        f.disk()["projects"]["receipts"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn projects_full_receipt_lifecycle_never_forgets_any_of_1024_expired_ids() {
    let f = Fixture::new(Durable::default());
    let app = f.reopen().unwrap();
    let a = create(&app, 1, 0);
    let first = call(&app, "project_create_v3", a.clone()).unwrap();
    let bound = MAX_RECEIPTS + MAX_TOMBSTONES;
    for n in 2..=bound {
        call(
            &app,
            "project_update_v3",
            update(&app, n, (n - 1) as u64, &first["project_id"]),
        )
        .unwrap();
    }
    let registry = &f.disk()["projects"];
    assert_eq!(registry["revision"], bound);
    assert_eq!(registry["receipts"].as_array().unwrap().len(), MAX_RECEIPTS);
    assert_eq!(
        registry["expired_ids"],
        json!((1..=MAX_TOMBSTONES).map(id).collect::<Vec<_>>())
    );
    for n in 1..=MAX_TOMBSTONES {
        assert_eq!(receipt(&app, n).unwrap_err().code, "REQUEST_EXPIRED");
    }
    let bytes = f.bytes();
    assert_eq!(
        call(&app, "project_create_v3", a).unwrap_err().code,
        "REQUEST_EXPIRED"
    );
    for (op, args) in [
        ("project_create_v3", create(&app, bound + 1, bound as u64)),
        (
            "project_update_v3",
            update(&app, bound + 1, bound as u64, &first["project_id"]),
        ),
        (
            "project_delete_v3",
            delete(&app, bound + 1, bound as u64, &first["project_id"]),
        ),
    ] {
        assert_eq!(call(&app, op, args).unwrap_err().code, "CAPACITY");
        assert_eq!(f.bytes(), bytes);
    }
    let latest = receipt(&app, bound).unwrap();
    let last_args = update(&app, bound, (bound - 1) as u64, &first["project_id"]);
    assert_eq!(
        call(&app, "project_update_v3", last_args.clone()).unwrap(),
        latest
    );
    drop(app);
    let app = f.reopen().unwrap();
    assert_eq!(call(&app, "project_update_v3", last_args).unwrap(), latest);
    assert_eq!(receipt(&app, 1).unwrap_err().code, "REQUEST_EXPIRED");
    assert_eq!(
        receipt(&app, bound + 1).unwrap_err().code,
        "REQUEST_UNKNOWN"
    );
    assert_eq!(f.bytes(), bytes);
}

fn maximal_repository(n: usize, url: String) -> Repository {
    let mut r = Repository::unknown(url);
    r.name = Some(format!("{n}{}", "🚀".repeat(38)));
    r.branch.base = Some("a".repeat(255));
    r.branch.work = Some("b".repeat(255));
    let pattern = Some(format!("{}/*", "c".repeat(253)));
    r.branch.types = BranchTypes {
        feature: pattern.clone(),
        fix: pattern.clone(),
        release: pattern.clone(),
        hotfix: pattern,
    };
    r
}
fn maximal_registry() -> Registry {
    let repositories: Vec<_> = (0..16)
        .map(|n| {
            let prefix = format!("https://example.invalid/{n}/");
            maximal_repository(
                n,
                format!("{prefix}{}", "a".repeat(MAX_LOCATOR_BYTES - prefix.len())),
            )
        })
        .collect();
    let registry = Registry {
        revision: 32,
        projects: (1..=32)
            .map(|n| Project {
                project_id: id(n),
                name: "🚀".repeat(40),
                key: format!("{n:040}"),
                description: "🚀".repeat(MAX_DESCRIPTION_SCALARS),
                prompt: "🚀".repeat(MAX_PROMPT_SCALARS),
                repositories: repositories.clone(),
                agent: "opencode".into(),
            })
            .collect(),
        receipts: (1..=32)
            .map(|n| RetainedReceipt {
                receipt: Receipt {
                    request_id: id(n + 100),
                    project_id: id(n),
                    revision: n as u64,
                    deleted: false,
                },
                fingerprint: Some("a".repeat(64)),
            })
            .collect(),
        expired_ids: vec![],
    };
    registry.validate().unwrap();
    registry
}
#[test]
fn projects_maximal_registry_is_frame_safe_and_counted_in_shared_encoded_capacity() {
    let mut d = Durable::default();
    d.projects = maximal_registry();
    let f = Fixture::new(d);
    let result = list(&f.app);
    assert!(reply_fits(&result));
    assert_eq!(result["projects"].as_array().unwrap().len(), 32);
    let app = f.reopen().unwrap();
    assert_eq!(list(&app), result);
    drop(app);
    let mut d = Durable::default();
    for n in 1..=32 {
        let target = d.allocate(None, "Bounded history".into()).unwrap();
        d.conversations[n - 1].history = vec![
            Message {
                role: "user".into(),
                content: "\u{0001}".repeat(MAX_TEXT),
            },
            Message {
                role: "assistant".into(),
                content: "\u{0002}".repeat(MAX_CONTEXT - MAX_TEXT),
            },
        ];
        if n <= 10 {
            d.jobs.push(Job { operation:Operation { request_id:id(1000+n),kind:"submit".into(),status:"succeeded".into(),phase:"complete".into(),config_revision:0,result:Some(json!({"assistant_text":"\u{0003}".repeat(MAX_CONTEXT),"audio":null,"speech_error":null})),error:None },fingerprint:"a".repeat(64),version:2,conversation_id:Some(target),history_revision:Some(0),task:Some(Task::Submit { text:"old input".into() }),accepted_ms:0 });
        }
    }
    d.validate().unwrap();
    let without = d.reserved_bytes().unwrap();
    d.projects = maximal_registry();
    assert!(d.reserved_bytes().unwrap() > without + 1024 * 1024);
    assert_eq!(d.preflight().unwrap_err().code, "CAPACITY");
    let bytes = f.bytes();
    let mut inner = f.app.inner.lock().unwrap();
    assert_eq!(f.app.commit(&mut inner, d).unwrap_err().code, "CAPACITY");
    assert!(!inner.fault);
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_publication_failure_fences_reads_and_writes_preserving_previous_transaction() {
    for op in [
        "project_create_v3",
        "project_update_v3",
        "project_delete_v3",
    ] {
        let f = Fixture::new(Durable::default());
        let first = call(&f.app, "project_create_v3", create(&f.app, 1, 0)).unwrap();
        let before = f.bytes();
        let args = match op {
            "project_create_v3" => create(&f.app, 2, 1),
            "project_update_v3" => update(&f.app, 2, 1, &first["project_id"]),
            _ => delete(&f.app, 2, 1, &first["project_id"]),
        };
        let preserved = f.app.path.with_extension("preserved");
        fs::rename(&f.app.path, &preserved).unwrap();
        fs::create_dir(&f.app.path).unwrap();
        assert_eq!(
            call(&f.app, op, args.clone()).unwrap_err().code,
            "STATE_UNAVAILABLE"
        );
        assert!(f.app.inner.lock().unwrap().fault);
        assert_eq!(
            call(&f.app, "projects_list_v3", json!({}))
                .unwrap_err()
                .code,
            "STATE_UNAVAILABLE"
        );
        assert_eq!(receipt(&f.app, 1).unwrap_err().code, "STATE_UNAVAILABLE");
        assert_eq!(
            call(&f.app, op, args).unwrap_err().code,
            "STATE_UNAVAILABLE"
        );
        fs::remove_dir(&f.app.path).unwrap();
        fs::rename(preserved, &f.app.path).unwrap();
        assert_eq!(f.bytes(), before);
        let app = f.reopen().unwrap();
        assert_eq!(receipt(&app, 1).unwrap(), first);
        assert_eq!(receipt(&app, 2).unwrap_err().code, "REQUEST_UNKNOWN");
        assert_eq!(list(&app)["revision"], 1);
        assert_eq!(list(&app)["projects"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn projects_revoked_owner_cannot_publish_or_recreate_after_reopen() {
    let f = Fixture::new(Durable::default());
    let a = create(&f.app, 1, 0);
    let first = call(&f.app, "project_create_v3", a.clone()).unwrap();
    let stale_update = update(&f.app, 2, 1, &first["project_id"]);
    f.app.inner.lock().unwrap().owned = false;
    let app = f.reopen().unwrap();
    call(
        &app,
        "project_delete_v3",
        delete(&app, 3, 1, &first["project_id"]),
    )
    .unwrap();
    let bytes = f.bytes();
    assert_eq!(
        call(&f.app, "project_update_v3", stale_update)
            .unwrap_err()
            .code,
        "STATE_UNAVAILABLE"
    );
    assert_eq!(
        call(&f.app, "project_create_v3", a).unwrap_err().code,
        "STATE_UNAVAILABLE"
    );
    assert_eq!(list(&app), json!({"revision":2,"projects":[]}));
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_branch_and_pattern_pure_grammar_boundaries() {
    for value in [
        "main", "dev", "head", "Main", "a/b", "a.b_c-d", "a/1", "a.locked",
    ] {
        assert!(valid_branch(value), "{value}");
        assert!(valid_pattern(&format!("{value}/*")), "{value}");
    }
    assert!(valid_branch(&"a".repeat(255)));
    assert!(!valid_branch(&"a".repeat(256)));
    assert!(valid_pattern(&format!("{}/*", "a".repeat(253))));
    assert!(!valid_pattern(&format!("{}/*", "a".repeat(254))));
    for value in [
        "",
        "HEAD",
        "refs/main",
        "refs/heads/main",
        "/main",
        "main/",
        "a//b",
        ".a",
        "_a",
        "-a",
        "a/.b",
        "a/..",
        "a..b",
        "a.",
        "a./b",
        "a.lock",
        "a.LoCk/b",
        "é",
        "a b",
        "a\nb",
        "a\0b",
        "a\\b",
        "a@{b",
        "a:b",
        "a?b",
        "a[b",
        "a*b",
        "a~b",
        "a^b",
    ] {
        assert!(!valid_branch(value), "{value:?}");
        assert!(!valid_pattern(&format!("{value}/*")), "{value:?}");
    }
    for value in [
        "", "main", "*", "/*", "a*", "a/**", "a/*/b", "a/*/*", "a/?", "a/[x]", "a/* ",
    ] {
        assert!(!valid_pattern(value), "{value:?}");
    }
}

fn named_repository(name: &str, url: &str) -> Repository {
    let mut repository = Repository::unknown(url.into());
    repository.name = Some(name.into());
    repository
}

fn repository() -> Value {
    json!({"name":"backend","url":"https://example.invalid/backend","read_only":true,
        "branch":{"base":"main","work":"main","types":{"feature":"feature/*",
        "fix":"fix/*","release":"release/*","hotfix":"hotfix/*"}}})
}

#[test]
fn projects_nested_objects_require_exact_keys_on_wire_and_disk() {
    let f = Fixture::new(Durable::default());
    let mut args = create(&f.app, 1, 0);
    args["repositories"] = json!([repository()]);
    call(&f.app, "project_create_v3", args).unwrap();
    let good = f.disk();
    let mut bad = vec![
        json!("https://a/repo"),
        Value::Null,
        json!([]),
        json!([null, "https://a/repo", {}]),
    ];
    for parent in ["", "/branch", "/branch/types"] {
        let original = repository();
        let fields = original.pointer(parent).unwrap().as_object().unwrap();
        for key in fields.keys() {
            let mut missing = original.clone();
            missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            bad.push(missing);
        }
        for value in [
            Value::Null,
            json!([]),
            json!(fields.values().collect::<Vec<_>>()),
            json!("text"),
        ] {
            let mut changed = original.clone();
            *changed.pointer_mut(parent).unwrap() = value;
            bad.push(changed);
        }
        let mut extra = original;
        extra
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(0));
        bad.push(extra);
    }
    for path in [
        "/name",
        "/url",
        "/branch/base",
        "/branch/work",
        "/branch/types/feature",
        "/branch/types/fix",
        "/branch/types/release",
        "/branch/types/hotfix",
    ] {
        for value in [json!(""), json!(42), json!(true), json!([]), json!({})] {
            let mut changed = repository();
            *changed.pointer_mut(path).unwrap() = value;
            bad.push(changed);
        }
    }
    let mut null_url = repository();
    null_url["url"] = Value::Null;
    bad.push(null_url);
    for repository in bad {
        let mut args = create(&f.app, 2, 1);
        args["repositories"] = json!([repository]);
        f.assert_rejected("project_create_v3", args, "INVALID_REQUEST");
        let mut disk = good.clone();
        disk["projects"]["projects"][0]["repositories"] = json!([repository]);
        let bytes = f.write(&disk);
        assert!(f.reopen().is_err(), "accepted {repository}");
        assert_eq!(f.bytes(), bytes);
        f.write(&good);
    }
    // Duplicate nested keys must be rejected before serde can collapse them.
    let duplicate = serde_json::to_string(&good)
        .unwrap()
        .replace("\"base\":\"main\"", "\"base\":\"main\",\"base\":\"main\"");
    assert_ne!(duplicate.as_bytes(), serde_json::to_vec(&good).unwrap());
    fs::write(&f.app.path, duplicate.as_bytes()).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), duplicate.as_bytes());
}

#[test]
fn projects_nullable_leaves_uniqueness_and_nested_fingerprints_are_exact() {
    let f = Fixture::new(Durable::default());
    let mut args = create(&f.app, 1, 0);
    args["repositories"] = json!([repository()]);
    let first = call(&f.app, "project_create_v3", args.clone()).unwrap();
    let paths = [
        "/name",
        "/branch/base",
        "/branch/work",
        "/branch/types/feature",
        "/branch/types/fix",
        "/branch/types/release",
        "/branch/types/hotfix",
    ];
    for path in paths {
        let mut conflict = args.clone();
        *conflict["repositories"][0].pointer_mut(path).unwrap() = Value::Null;
        f.assert_rejected("project_create_v3", conflict, "ID_CONFLICT");
    }
    let mut conflict = args.clone();
    conflict["repositories"][0]["url"] = json!("https://a/other");
    f.assert_rejected("project_create_v3", conflict, "ID_CONFLICT");
    let mut revision = 1;
    for path in &paths[1..] {
        for value in [Value::Null, repository().pointer(path).unwrap().clone()] {
            let mut update = update(
                &f.app,
                100 + revision,
                revision as u64,
                &first["project_id"],
            );
            let mut row = repository();
            *row.pointer_mut(path).unwrap() = value;
            update["repositories"] = json!([row]);
            call(&f.app, "project_update_v3", update.clone()).unwrap();
            assert_eq!(
                list(&f.app)["projects"][0]["repositories"],
                update["repositories"]
            );
            revision += 1;
        }
    }
    let mut a = create(&f.app, 999, revision as u64);
    let mut other = repository();
    other["url"] = json!("https://a/other");
    a["repositories"] = json!([repository(), other]);
    f.assert_rejected("project_create_v3", a.clone(), "INVALID_REQUEST");
    a["repositories"][1]["name"] = json!("other-backend");
    call(&f.app, "project_create_v3", a).unwrap();
    revision += 1;
    let mut a = create(&f.app, 1000, revision as u64);
    let r = named_repository("repo", "https://a/repo");
    a["repositories"] = json!([r, r]);
    f.assert_rejected("project_create_v3", a.clone(), "INVALID_REQUEST");
    a["repositories"][1]["url"] = json!("https://a/Repo");
    a["repositories"][1]["name"] = json!("other-repo");
    call(&f.app, "project_create_v3", a).unwrap();
    let app = f.reopen().unwrap();
    // Original retained v2 replay wins even after a new instance and revisions.
    assert_eq!(call(&app, "project_create_v3", args).unwrap(), first);
    assert_eq!(list(&app)["projects"][2]["repositories"][0], json!(r));
}

fn frozen_disk5() -> (Value, Value) {
    let mut disk = json!(rich_durable());
    let args = json!({"request_id":id(504),"instance_id":id(500),"expected_revision":3,
        "name":"Legacy","repositories":["git@example.invalid:Team/a.git","https://a/Repo"],"agent":"opencode"});
    let receipt = |n: usize, project: usize, deleted: bool| json!({"request_id":id(500+n),"project_id":id(project),"revision":n,"deleted":deleted});
    disk["schema_version"] = json!(5);
    disk.as_object_mut().unwrap().remove("instructions");
    disk.as_object_mut().unwrap().remove("agents");
    disk["config"]["revision"] = json!(u64::MAX - 1);
    disk["projects"] = json!({"revision":4,"projects":[
        {"project_id":id(502),"name":"Second","repositories":[],"agent":"opencode"},
        {"project_id":id(503),"name":"Legacy","repositories":args["repositories"],"agent":"opencode"}],
        "receipts":[
            {"receipt":receipt(1,501,false),"fingerprint":null},
            {"receipt":receipt(2,501,true),"fingerprint":"a".repeat(64)},
            {"receipt":receipt(3,502,false),"fingerprint":"b".repeat(64)},
            {"receipt":receipt(4,503,false),"fingerprint":fingerprint("v3:projects:project_create", &args).unwrap()}],
        "expired_ids":[id(599)]});
    (disk, args)
}

#[test]
fn projects_actual_disk5_migration_preserves_opaque_state_and_legacy_requests_never_replay() {
    let f = Fixture::new(Durable::default());
    let (old, legacy_args) = frozen_disk5();
    let bytes = f.write(&old);
    let (d, migrated) = Durable::load(&bytes).unwrap();
    assert!(migrated);
    d.validate().unwrap();
    let converted = json!(d);
    for (key, value) in old.as_object().unwrap() {
        if !matches!(key.as_str(), "projects" | "schema_version" | "config") {
            assert_eq!(converted[key], *value, "{key}");
        }
    }
    for key in ["revision", "receipts", "expired_ids"] {
        assert_eq!(converted["projects"][key], old["projects"][key]);
    }
    for (index, project) in old["projects"]["projects"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut expected = project.clone();
        expected["key"] = json!(format!(
            "project-{}",
            project["project_id"].as_str().unwrap()
        ));
        expected["description"] = json!("");
        expected["prompt"] = json!("");
        expected["repositories"] = json!(project["repositories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|url| Repository::unknown(url.as_str().unwrap().into()))
            .collect::<Vec<_>>());
        assert_eq!(converted["projects"]["projects"][index], expected);
    }
    let app = f.reopen().unwrap();
    assert_eq!(f.disk()["schema_version"], 10);
    assert_eq!(f.disk()["projects"], converted["projects"]);
    for n in 1..=4 {
        assert_eq!(
            receipt(&app, 500 + n).unwrap(),
            old["projects"]["receipts"][n - 1]["receipt"]
        );
    }
    assert_eq!(receipt(&app, 599).unwrap_err().code, "REQUEST_EXPIRED");
    let before = f.bytes();
    for op in [
        "projects_list",
        "project_create",
        "project_update",
        "project_delete",
    ] {
        for args in [
            json!({}),
            json!([]),
            Value::Null,
            legacy_args.clone(),
            update(&app, 600, 4, &json!(id(503))),
            delete(&app, 601, 4, &json!(id(503))),
        ] {
            assert_eq!(
                call(&app, op, args).unwrap_err().code,
                "UPGRADE_REQUIRED",
                "{op}"
            );
            assert_eq!(f.bytes(), before);
        }
    }
    assert_eq!(
        call(&app, "project_create_v3", legacy_args)
            .unwrap_err()
            .code,
        "ID_CONFLICT"
    );
    assert_eq!(
        call(&app, "project_create_v3", create(&app, 501, 4))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    assert_eq!(
        call(&app, "project_create_v3", create(&app, 599, 4))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    let mut edit = update(&app, 600, 4, &json!(id(503)));
    edit["repositories"] = converted["projects"]["projects"][1]["repositories"].clone();
    edit["repositories"][0]["branch"]["base"] = json!("main");
    call(&app, "project_update_v3", edit).unwrap();
    call(
        &app,
        "project_delete_v3",
        delete(&app, 601, 5, &json!(id(503))),
    )
    .unwrap();
    assert!(f.disk()["projects"]["receipts"][3]["fingerprint"].is_null());
    assert_eq!(
        receipt(&app, 504).unwrap(),
        old["projects"]["receipts"][3]["receipt"]
    );
    drop(app);
    let before = f.bytes();
    f.reopen().unwrap();
    assert_eq!(f.bytes(), before);
    // Empty registries also reject legacy operations unconditionally.
    let empty = Fixture::new(Durable::default());
    for op in [
        "projects_list",
        "project_create",
        "project_update",
        "project_delete",
    ] {
        empty.assert_rejected(op, json!({}), "UPGRADE_REQUIRED");
    }
}

#[test]
fn projects_frozen_disk5_rejects_old_shape_corruption_without_publication() {
    let f = Fixture::new(Durable::default());
    let (good, _) = frozen_disk5();
    for url_list in [
        json!([repository()]),
        json!([null]),
        json!(["https://a/repo", "https://a/repo"]),
        json!(["https://a/../repo"]),
        json!("https://a/repo"),
        json!(vec!["https://a/repo"; 17]),
    ] {
        let mut bad = good.clone();
        bad["projects"]["projects"][1]["repositories"] = url_list;
        let bytes = f.write(&bad);
        assert!(f.reopen().is_err());
        assert_eq!(f.bytes(), bytes);
    }
    for path in [
        "/projects",
        "/projects/projects/1",
        "/projects/receipts/3",
        "/projects/receipts/3/receipt",
    ] {
        for key in good.pointer(path).unwrap().as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let bytes = f.write(&bad);
            assert!(f.reopen().is_err(), "{path}/{key}");
            assert_eq!(f.bytes(), bytes);
        }
        let mut bad = good.clone();
        bad.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(0));
        let bytes = f.write(&bad);
        assert!(f.reopen().is_err());
        assert_eq!(f.bytes(), bytes);
    }
}
