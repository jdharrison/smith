use smith_service::{
    request, Action, Controller, Manager, ManagerFacts, Observation, Result, Roots,
};
use std::cell::RefCell;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

// CLOEXEC closes inherited writers at exec, not at fork. A concurrent child can
// retain a copy/publication writer after its parent closes it and cause ETXTBSY.
// Fence only those short write/spawn windows, not daemon lifetime or whole tests.
static EXECUTABLE_IO: Mutex<()> = Mutex::new(());
fn executable_io<T>(action: impl FnOnce() -> T) -> T {
    let _guard = EXECUTABLE_IO.lock().unwrap();
    action()
}
fn copy_executable(destination: &std::path::Path) {
    executable_io(|| {
        fs::copy(env!("CARGO_BIN_EXE_smith-service"), destination).unwrap();
        fs::set_permissions(destination, fs::Permissions::from_mode(0o755)).unwrap();
    });
}
fn spawn_fixture(command: &mut Command) -> std::io::Result<Child> {
    executable_io(|| command.spawn())
}
fn output_fixture(command: &mut Command) -> std::io::Result<Output> {
    spawn_fixture(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?
    .wait_with_output()
}
fn apply_action(c: &Controller<FakeManager>, action: Action, token: &str) -> Observation {
    if action == Action::InstallBinary {
        executable_io(|| c.apply(action, token))
    } else {
        c.apply(action, token)
    }
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    roots: Roots,
}
fn directory(path: &std::path::Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        // SAFETY: geteuid has no arguments or memory access.
        assert_ne!(unsafe { libc::geteuid() }, 0, "Compiled daemon integration tests require a non-root user; do not bypass production preflight in containers");
        let base = std::env::temp_dir().join(format!(
            "smith-next-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        directory(&base);
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        for path in [&roots.data, &roots.config, &roots.runtime] {
            directory(path);
        }
        copy_executable(&base.join("helper"));
        Self { base, roots }
    }
    fn controller(&self) -> Controller<FakeManager> {
        Controller {
            roots: self.roots.clone(),
            source: self.base.join("helper"),
            manager: FakeManager::new(),
        }
    }
    fn installed(&self) {
        directory(&self.roots.data.join("smith-next"));
        directory(&self.roots.data.join("smith-next/bin"));
        copy_executable(&self.roots.binary());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
struct Daemon(Child);
impl Daemon {
    fn start(roots: &Roots) -> Self {
        let child = spawn_fixture(
            Command::new(roots.binary())
                .args(["serve", "--data-root"])
                .arg(&roots.data)
                .arg("--config-root")
                .arg(&roots.config)
                .arg("--runtime-root")
                .arg(&roots.runtime)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null()),
        )
        .unwrap();
        let mut daemon = Self(child);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if request(roots, daemon.0.id(), "identity").is_ok() {
                break;
            }
            assert!(
                daemon.0.try_wait().unwrap().is_none(),
                "daemon exited before ready"
            );
            assert!(Instant::now() < deadline, "daemon readiness deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
        daemon
    }
    fn stop(&mut self) {
        if self.0.try_wait().unwrap().is_some() {
            return;
        }
        // SAFETY: signal only the captured test child.
        unsafe {
            libc::kill(self.0.id() as i32, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.0.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                self.0.kill().unwrap();
                self.0.wait().unwrap();
                panic!("daemon cleanup exceeded budget");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}
struct FakeManager {
    facts: RefCell<ManagerFacts>,
    calls: RefCell<Vec<&'static str>>,
    child: RefCell<Option<Daemon>>,
    unavailable: RefCell<bool>,
    fail_reload: RefCell<bool>,
    fail_start: RefCell<bool>,
}
impl FakeManager {
    fn new() -> Self {
        Self {
            facts: RefCell::new(ManagerFacts {
                load: "not-found".into(),
                active: "inactive".into(),
                sub: "dead".into(),
                pid: 0,
                fragment: String::new(),
                drop_ins: String::new(),
                need_reload: false,
                transient: false,
                unit_file_state: String::new(),
            }),
            calls: RefCell::new(Vec::new()),
            child: RefCell::new(None),
            unavailable: RefCell::new(false),
            fail_reload: RefCell::new(false),
            fail_start: RefCell::new(false),
        }
    }
}
impl Manager for FakeManager {
    fn inspect(&self, _: &Roots) -> Result<ManagerFacts> {
        self.calls.borrow_mut().push("inspect");
        if *self.unavailable.borrow() {
            Err("fake offline".into())
        } else {
            Ok(self.facts.borrow().clone())
        }
    }
    fn reload(&self, roots: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("reload");
        if *self.fail_reload.borrow() {
            return Err("fake reload failed".into());
        }
        let mut f = self.facts.borrow_mut();
        f.load = "loaded".into();
        f.fragment = roots.unit().to_string_lossy().into();
        f.need_reload = false;
        f.unit_file_state = "disabled".into();
        Ok(())
    }
    fn enable(&self, roots: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("enable");
        let link = roots.enablement_link();
        let parent = link.parent().unwrap();
        if !parent.exists() {
            directory(parent);
        }
        symlink(roots.unit(), link).map_err(|_| "fake enable failed")?;
        self.facts.borrow_mut().unit_file_state = "enabled".into();
        Ok(())
    }
    fn disable(&self, roots: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("disable");
        fs::remove_file(roots.enablement_link()).map_err(|_| "fake disable failed")?;
        self.facts.borrow_mut().unit_file_state = "disabled".into();
        Ok(())
    }
    fn start(&self, roots: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("start");
        if *self.fail_start.borrow() {
            return Err("fake start failed".into());
        }
        assert!(self.child.borrow().is_none(), "duplicate fake start");
        let daemon = Daemon::start(roots);
        let mut f = self.facts.borrow_mut();
        f.active = "active".into();
        f.sub = "running".into();
        f.pid = daemon.0.id();
        *self.child.borrow_mut() = Some(daemon);
        Ok(())
    }
}
fn apply(c: &Controller<FakeManager>, action: Action) -> Observation {
    let prepare = c.prepare(action);
    assert!(prepare.ok, "{prepare:?}");
    let result = apply_action(c, action, prepare.approval_token.as_ref().unwrap());
    assert!(result.ok, "{result:?}");
    result
}

#[test]
fn stale_app_path_blocks_start_without_adoption_or_removal() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    directory(&fixture.roots.runtime.join("smith-next"));
    fs::write(fixture.roots.app_socket(), b"foreign-preserve").unwrap();
    assert_eq!(c.inspect().runtime_state, "conflict");
    assert!(!c.prepare(Action::Start).ok);
    assert_eq!(
        fs::read(fixture.roots.app_socket()).unwrap(),
        b"foreign-preserve"
    );
    assert!(!c.manager.calls.borrow().contains(&"start"));
}
#[test]
fn app_cli_stdin_envelope_is_separate_and_never_uses_endpoint_argv() {
    let mut child = spawn_fixture(
        Command::new(env!("CARGO_BIN_EXE_smith-service"))
            .arg("app")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped()),
    )
    .unwrap();
    let mut bytes = serde_json::to_vec(&app_state_request()).unwrap();
    bytes.push(b'\n');
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let reply: smith_service::app::AppReply = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply.protocol, "app");
    assert_eq!(reply.call_id, app_state_request().call_id);
    assert_eq!(reply.error.unwrap().code, "APP_UNAVAILABLE");
    let output = output_fixture(
        Command::new(env!("CARGO_BIN_EXE_smith-service"))
            .args(["app", "--endpoint", "http://127.0.0.1:1"])
            .env_clear(),
    )
    .unwrap();
    assert!(!output.status.success());
    let reply: smith_service::app::AppReply = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!reply.ok);
}
#[test]
fn app_v2_v3_helper_correlates_preflight_and_strict_decode_failures() {
    for version in [2, 3] {
        for (malformed, suffix) in [(false, ""), (true, ""), (false, "\n"), (false, "{}\n")] {
            let mut request = serde_json::to_value(app_state_request()).unwrap();
            request["schema_version"] = serde_json::json!(version);
            if malformed {
                request["unknown"] = serde_json::json!(true);
            }
            let mut child = spawn_fixture(
                Command::new(env!("CARGO_BIN_EXE_smith-service"))
                    .arg("app")
                    .env_clear()
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped()),
            )
            .unwrap();
            let mut bytes = serde_json::to_vec(&request).unwrap();
            bytes.push(b'\n');
            bytes.extend_from_slice(suffix.as_bytes());
            child.stdin.take().unwrap().write_all(&bytes).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(!output.status.success());
            let reply: smith_service::app::AppReply =
                serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(reply.schema_version, version);
            assert_eq!(reply.call_id, app_state_request().call_id);
            assert!(!reply.ok);
        }
    }
}
#[test]
fn app_v3_cached_catalog_and_preparation_use_the_actual_socket_without_cloud_effects() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    apply(&c, Action::Start);
    let state_path = fixture.roots.data.join("smith-next/app-state.json");
    let before = std::fs::read(&state_path).unwrap();
    let mut request = app_state_request();
    request.schema_version = 3;
    request.op = "route_catalog".into();
    let reply = c.app(request.clone()).unwrap();
    assert!(reply.ok);
    assert_eq!(reply.schema_version, 3);
    let instance = reply.instance_id;
    let catalog = reply.result.unwrap();
    assert_eq!(catalog["providers"].as_array().unwrap().len(), 2);
    let models = catalog["models"].as_array().unwrap();
    assert_eq!(models.len(), smith_service::cloud::MODELS.len());
    for (choice, model) in models.iter().zip(smith_service::cloud::MODELS) {
        assert_eq!(choice["route"]["provider"], model.vendor.name());
        assert_eq!(choice["route"]["model"], model.id);
        assert_eq!(choice["label"], model.label);
        assert_eq!(choice["available"], false);
        assert_eq!(choice["validated_model"], false);
    }
    request.op = "provider_prepare".into();
    request.args = serde_json::json!({"request_id":"11111111111111111111111111111111","instance_id":instance,"provider":"openai","action":"save","model":null,"expected_provider_revision":0});
    let reply = c.app(request).unwrap();
    assert!(reply.ok);
    assert!(reply.result.unwrap()["endpoint"].is_null());
    assert_eq!(std::fs::read(&state_path).unwrap(), before);
}
#[test]
fn v2_trailing_frames_execute_neither_request_and_keep_correlation() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    apply(&c, Action::Start);
    for suffix in ["\n", "{\"schema_version\":2,\"op\":\"models\"}\n"] {
        let mut request = app_state_request();
        request.schema_version = 2;
        request.op = "models".into();
        request.args = serde_json::json!({"request_id":"11111111111111111111111111111111"});
        let mut bytes = serde_json::to_vec(&request).unwrap();
        bytes.push(b'\n');
        bytes.extend_from_slice(suffix.as_bytes());
        let mut stream = UnixStream::connect(fixture.roots.app_socket()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream.write_all(&bytes).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let reply: smith_service::app::AppReply = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reply.schema_version, 2);
        assert_eq!(reply.call_id, request.call_id);
        assert_eq!(reply.error.unwrap().code, "INVALID_REQUEST");
        let mut state = app_state_request();
        state.schema_version = 2;
        assert_eq!(
            c.app(state).unwrap().result.unwrap()["operations"],
            serde_json::json!([])
        );
    }
}
fn app_state_request() -> smith_service::app::AppRequest {
    smith_service::app::AppRequest {
        schema_version: 1,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: "0123456789abcdef0123456789abcdef".into(),
        op: "state".into(),
        args: serde_json::json!({}),
    }
}

#[test]
fn separate_app_socket_verified_by_current_controller_facts_preserves_control() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    assert!(c.app(app_state_request()).is_err());
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    apply(&c, Action::Start);
    let reply = c.app(app_state_request()).unwrap();
    assert!(reply.ok);
    assert_eq!(reply.pid, c.manager.facts.borrow().pid);
    assert_eq!(reply.protocol, "app");
    assert_eq!(reply.instance_id.len(), 32);
    assert_eq!(reply.result.unwrap()["history"], serde_json::json!([]));
    let mut v2 = app_state_request();
    v2.schema_version = 2;
    let reply = c.app(v2.clone()).unwrap();
    assert_eq!(reply.schema_version, 2);
    assert!(reply.ok);
    assert_eq!(
        reply.result.unwrap()["conversations"],
        serde_json::json!([])
    );
    v2.op = "conversation_get".into();
    v2.args = serde_json::json!({"conversation_id":"bad"});
    let reply = c.app(v2).unwrap();
    assert_eq!(reply.schema_version, 2);
    assert_eq!(reply.error.unwrap().code, "INVALID_REQUEST");
    assert_eq!(
        fs::metadata(fixture.roots.app_socket()).unwrap().mode() & 0o777,
        0o600
    );
    let slow = UnixStream::connect(fixture.roots.app_socket()).unwrap();
    let started = Instant::now();
    assert_eq!(c.inspect().bridge_state, "ready");
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(slow);
    let mut request = app_state_request();
    request.op = "models".into();
    request.args = serde_json::json!({"request_id":"INVALID"});
    let rejected = c.app(request).unwrap();
    assert!(!rejected.ok);
    assert_eq!(rejected.error.unwrap().code, "INVALID_REQUEST");
    c.manager.facts.borrow_mut().drop_ins = "foreign.conf".into();
    assert!(c.app(app_state_request()).is_err());
    c.manager.facts.borrow_mut().drop_ins.clear();
    assert!(c.app(app_state_request()).unwrap().ok);
    assert!(c
        .manager
        .calls
        .borrow()
        .iter()
        .all(|call| ["inspect", "reload", "enable", "start"].contains(call)));
}

#[test]
fn app_peer_conflict_and_corrupt_state_fail_closed_without_changing_core() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    let app_state = fixture.roots.data.join("smith-next/app-state.json");
    fs::write(&app_state, b"corrupt-preserve").unwrap();
    fs::set_permissions(&app_state, fs::Permissions::from_mode(0o600)).unwrap();
    apply(&c, Action::Start);
    let reply = c.app(app_state_request()).unwrap();
    assert!(!reply.ok);
    assert_eq!(reply.error.unwrap().code, "STATE_UNAVAILABLE");
    let mut v2 = app_state_request();
    v2.schema_version = 2;
    let reply = c.app(v2).unwrap();
    assert_eq!(reply.schema_version, 2);
    assert_eq!(reply.error.unwrap().code, "STATE_UNAVAILABLE");
    assert_eq!(fs::read(&app_state).unwrap(), b"corrupt-preserve");
    assert_eq!(c.inspect().bridge_state, "ready");
    fs::remove_file(fixture.roots.app_socket()).unwrap();
    let _foreign = std::os::unix::net::UnixListener::bind(fixture.roots.app_socket()).unwrap();
    fs::set_permissions(
        fixture.roots.app_socket(),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(c.app(app_state_request()).unwrap_err().contains("peer"));
    assert_eq!(c.inspect().bridge_state, "ready");
}

#[test]
fn app_buffered_large_frame_and_unknown_fields_are_rejected_without_provider_work() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    apply(&c, Action::Start);
    let mut stream = UnixStream::connect(fixture.roots.app_socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut request = app_state_request();
    request.args = serde_json::json!({"unexpected":"x".repeat(512*1024)});
    let mut bytes = serde_json::to_vec(&request).unwrap();
    bytes.push(b'\n');
    stream.write_all(&bytes).unwrap();
    let mut response = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(stream), &mut response).unwrap();
    let reply: smith_service::app::AppReply = serde_json::from_str(&response).unwrap();
    assert!(!reply.ok);
    assert_eq!(reply.error.unwrap().code, "INVALID_REQUEST");
    assert_eq!(reply.call_id, request.call_id);
    assert_eq!(c.inspect().bridge_state, "ready");
}

#[test]
fn separate_consented_lifecycle_and_exact_json_contract() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    let observation = c.inspect();
    assert_eq!(observation.binary_state, "missing");
    assert_eq!(observation.unit_state, "missing");
    assert_eq!(observation.enablement_state, "missing");
    assert_eq!(observation.runtime_state, "stopped");
    assert_eq!(observation.setup_complete, Some(false));
    assert_eq!(fs::read_dir(&fixture.roots.data).unwrap().count(), 0);
    assert!(c.manager.calls.borrow().iter().all(|c| *c == "inspect"));
    let json = serde_json::to_value(&observation).unwrap();
    let mut keys: Vec<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "binary_state",
            "bridge_state",
            "effects",
            "enablement_state",
            "error",
            "message",
            "ok",
            "runtime_state",
            "schema_version",
            "service",
            "setup_complete",
            "unit_state"
        ]
    );
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["service"], "smith-next");
    assert_eq!(json["effects"].as_object().unwrap().len(), 5);
    let o = apply(&c, Action::InstallBinary);
    assert_eq!(o.binary_state, "installed");
    assert_eq!(o.unit_state, "missing");
    assert!(!c.manager.calls.borrow().contains(&"reload"));
    assert!(!c.manager.calls.borrow().contains(&"start"));
    let o = apply(&c, Action::InstallUnit);
    assert_eq!(o.unit_state, "installed");
    assert_eq!(o.runtime_state, "stopped");
    let unit = fs::read_to_string(fixture.roots.unit()).unwrap();
    assert_eq!(unit, fixture.roots.unit_text());
    assert!(unit.contains("Restart=no"));
    assert!(unit.ends_with("\n[Install]\nWantedBy=default.target\n"));
    assert_eq!(o.enablement_state, "disabled");
    assert!(!c.prepare(Action::Start).ok);
    let o = apply(&c, Action::Enable);
    assert_eq!(o.enablement_state, "enabled");
    assert_eq!(
        fs::read_link(fixture.roots.enablement_link()).unwrap(),
        fixture.roots.unit()
    );
    assert!(!c.manager.calls.borrow().contains(&"start"));
    let o = apply(&c, Action::Start);
    assert_eq!(o.bridge_state, "ready");
    assert_eq!(o.setup_complete, Some(false));
    assert!(!c.prepare(Action::Start).ok);
    assert_eq!(apply(&c, Action::CompleteSetup).setup_complete, Some(true));
    assert!(!c.prepare(Action::CompleteSetup).ok);
    *c.manager.unavailable.borrow_mut() = true;
    let outage = c.inspect();
    assert_eq!(outage.runtime_state, "unavailable");
    assert_eq!(outage.setup_complete, Some(true));
    *c.manager.unavailable.borrow_mut() = false;
    assert_eq!(
        fs::read_to_string(fixture.roots.state()).unwrap(),
        "{\"setup_complete\":true}\n"
    );
    assert_eq!(
        fs::metadata(fixture.roots.state()).unwrap().mode() & 0o777,
        0o600
    );
    let mut daemon = c.manager.child.borrow_mut().take().unwrap();
    daemon.stop();
    assert!(!fixture.roots.socket().exists());
    let _restarted = Daemon::start(&fixture.roots);
    assert_eq!(
        request(&fixture.roots, _restarted.0.id(), "get_state")
            .unwrap()
            .setup_complete,
        Some(true)
    );
}

#[test]
fn strict_enablement_rejects_runtime_link_alias_mask_indirect_and_disagreement() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    assert_eq!(c.inspect().enablement_state, "disabled");

    for state in [
        "enabled-runtime",
        "linked",
        "linked-runtime",
        "alias",
        "masked",
        "masked-runtime",
        "indirect",
        "static",
        "generated",
        "transient",
        "bad",
        "foreign",
    ] {
        c.manager.facts.borrow_mut().unit_file_state = state.into();
        let observation = c.inspect();
        assert_eq!(observation.enablement_state, "conflict", "{state}");
        assert!(!c.prepare(Action::Enable).ok, "{state}");
        assert!(!c.prepare(Action::Start).ok, "{state}");
    }

    c.manager.facts.borrow_mut().unit_file_state = "enabled".into();
    assert_eq!(c.inspect().enablement_state, "conflict");
    let link = fixture.roots.enablement_link();
    directory(link.parent().unwrap());
    symlink(fixture.roots.unit(), &link).unwrap();
    c.manager.facts.borrow_mut().unit_file_state = "disabled".into();
    assert_eq!(c.inspect().enablement_state, "conflict");
    fs::remove_file(&link).unwrap();
    symlink(fixture.base.join("foreign.service"), &link).unwrap();
    assert_eq!(c.inspect().enablement_state, "conflict");
    fs::remove_file(&link).unwrap();
    fs::write(&link, b"not a symlink").unwrap();
    assert_eq!(c.inspect().enablement_state, "conflict");

    fs::remove_file(&link).unwrap();
    *c.manager.unavailable.borrow_mut() = true;
    assert_eq!(c.inspect().enablement_state, "unknown");
    assert!(!c.prepare(Action::Enable).ok);
    assert!(!c.prepare(Action::Start).ok);
}

#[test]
fn action_time_source_paths_snapshot_and_replay_bindings() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    let p = c.prepare(Action::InstallBinary);
    let token = p.approval_token.unwrap();
    for root in [
        &fixture.roots.data,
        &fixture.roots.config,
        &fixture.roots.runtime,
    ] {
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }
    assert!(!apply_action(&c, Action::InstallUnit, &token).ok);
    let (time, hash) = token.split_once('.').unwrap();
    let expired = format!("{}.{}", time.parse::<u64>().unwrap() - 60, hash);
    assert!(!apply_action(&c, Action::InstallBinary, &expired).ok);
    let future = format!("{}.{}", time.parse::<u64>().unwrap() + 60, hash);
    assert!(!apply_action(&c, Action::InstallBinary, &future).ok);
    assert!(!apply_action(&c, Action::InstallBinary, "garbage").ok);
    let other = Fixture::new();
    let other_c = other.controller();
    assert!(!apply_action(&other_c, Action::InstallBinary, &token).ok);
    c.manager.facts.borrow_mut().active = "activating".into();
    assert!(!apply_action(&c, Action::InstallBinary, &token).ok);
    c.manager.facts.borrow_mut().active = "inactive".into();
    assert!(apply_action(&c, Action::InstallBinary, &token).ok);
    assert!(!apply_action(&c, Action::InstallBinary, &token).ok);
    let source_fixture = Fixture::new();
    source_fixture.installed();
    let source = source_fixture.roots.binary();
    let fresh = Fixture::new();
    let mut changed = fresh.controller();
    changed.source = source.clone();
    let token = changed
        .prepare(Action::InstallBinary)
        .approval_token
        .unwrap();
    let mut bytes = fs::read(&source).unwrap();
    bytes.push(0);
    executable_io(|| fs::write(&source, bytes)).unwrap();
    assert!(!apply_action(&changed, Action::InstallBinary, &token).ok);
    assert!(!fresh.roots.binary().exists());
}

#[test]
fn unsafe_roots_files_units_sockets_and_corrupt_state_fail_closed() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    fs::set_permissions(&fixture.roots.runtime, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!c.prepare(Action::InstallBinary).ok);
    fs::set_permissions(&fixture.roots.runtime, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&fixture.roots.config, fixture.roots.data.join("smith-next")).unwrap();
    assert!(!c.prepare(Action::InstallBinary).ok);
    fs::remove_file(fixture.roots.data.join("smith-next")).unwrap();
    fixture.installed();
    fs::write(fixture.roots.state(), b"{broken}").unwrap();
    fs::set_permissions(fixture.roots.state(), fs::Permissions::from_mode(0o600)).unwrap();
    let o = c.inspect();
    assert!(!o.ok);
    assert_eq!(o.setup_complete, None);
    assert!(!c.prepare(Action::InstallUnit).ok);
    assert_eq!(fs::read(fixture.roots.state()).unwrap(), b"{broken}");
    fs::remove_file(fixture.roots.state()).unwrap();
    directory(&fixture.roots.config.join("systemd"));
    directory(&fixture.roots.config.join("systemd/user"));
    fs::write(fixture.roots.unit(), b"unrelated unit").unwrap();
    fs::set_permissions(fixture.roots.unit(), fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(c.inspect().unit_state, "conflict");
    assert!(!c.prepare(Action::InstallUnit).ok);
    fs::remove_file(fixture.roots.unit()).unwrap();
    fs::set_permissions(fixture.roots.binary(), fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(c.inspect().binary_state, "conflict");
    fs::set_permissions(fixture.roots.binary(), fs::Permissions::from_mode(0o755)).unwrap();
    directory(&fixture.roots.runtime.join("smith-next"));
    fs::write(fixture.roots.socket(), b"unrelated socket path").unwrap();
    assert!(smith_service::serve(&fixture.roots).is_err());
    assert_eq!(c.inspect().runtime_state, "conflict");
    assert!(request(&fixture.roots, std::process::id(), "get_state").is_err());
    assert_eq!(
        fs::read(fixture.roots.socket()).unwrap(),
        b"unrelated socket path"
    );
}

#[test]
fn real_socket_rejects_wrong_pid_bounded_frames_and_persists_on_client_detach() {
    let fixture = Fixture::new();
    fixture.installed();
    let mut daemon = Daemon::start(&fixture.roots);
    assert!(
        request(&fixture.roots, std::process::id(), "complete_setup")
            .unwrap_err()
            .contains("UID/PID")
    );
    assert!(!fixture.roots.state().exists());
    {
        let reply = request(&fixture.roots, daemon.0.id(), "get_state").unwrap();
        assert_eq!(reply.setup_complete, Some(false));
    }
    assert!(daemon.0.try_wait().unwrap().is_none());
    let mut stream = UnixStream::connect(fixture.roots.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    stream.write_all(&vec![b'x'; 4096]).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["ok"], false);
    let mut idle = UnixStream::connect(fixture.roots.socket()).unwrap();
    idle.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    let start = Instant::now();
    let mut response = String::new();
    idle.read_to_string(&mut response).unwrap();
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&response).unwrap()["ok"],
        false
    );
    assert!(request(&fixture.roots, daemon.0.id(), "get_state").is_ok());
    fs::write(
        fixture.roots.state(),
        b"{\"setup_complete\":false,\"extra\":true}",
    )
    .unwrap();
    fs::set_permissions(fixture.roots.state(), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(request(&fixture.roots, daemon.0.id(), "complete_setup").is_err());
    assert!(fs::read_to_string(fixture.roots.state())
        .unwrap()
        .contains("extra"));
}

#[test]
fn partial_manager_failure_preserves_files_and_never_falls_back() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    *c.manager.fail_reload.borrow_mut() = true;
    let p = c.prepare(Action::InstallUnit);
    assert!(p.ok);
    let o = apply_action(&c, Action::InstallUnit, p.approval_token.as_ref().unwrap());
    assert!(!o.ok);
    assert_eq!(o.unit_state, "installed");
    assert_eq!(o.runtime_state, "stopped");
    assert!(!c.manager.calls.borrow().contains(&"start"));
    assert!(c.manager.child.borrow().is_none());
    *c.manager.fail_reload.borrow_mut() = false;
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    *c.manager.fail_start.borrow_mut() = true;
    let p = c.prepare(Action::Start);
    let o = apply_action(&c, Action::Start, p.approval_token.as_ref().unwrap());
    assert!(!o.ok);
    assert_eq!(o.runtime_state, "stopped");
    assert!(c.manager.child.borrow().is_none());
    *c.manager.unavailable.borrow_mut() = true;
    assert_eq!(c.inspect().runtime_state, "unavailable");
    assert!(!c.prepare(Action::Start).ok);
}

#[test]
fn loaded_pending_reload_repairs_only_with_separate_consent_and_retries_failure() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    c.manager.facts.borrow_mut().need_reload = true;
    let before = fs::metadata(fixture.roots.unit()).unwrap();
    let bytes = fs::read(fixture.roots.unit()).unwrap();
    let parent = fixture.roots.unit().parent().unwrap().to_owned();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    let o = c.inspect();
    assert_eq!(o.unit_state, "installed");
    assert_eq!(o.runtime_state, "stopped");
    assert!(o.message.contains("reload-only"));
    assert!(!c.prepare(Action::Start).ok);
    let p = c.prepare(Action::InstallUnit);
    assert!(p.ok, "{p:?}");
    assert!(p.effects.description.contains("Reload only"));
    assert!(!p.effects.description.contains("Publish"));
    *c.manager.fail_reload.borrow_mut() = true;
    let failed = apply_action(&c, Action::InstallUnit, p.approval_token.as_ref().unwrap());
    assert!(!failed.ok);
    assert!(c.manager.facts.borrow().need_reload);
    assert!(!c.prepare(Action::Start).ok);
    *c.manager.fail_reload.borrow_mut() = false;
    let repaired = apply(&c, Action::InstallUnit);
    assert!(repaired.effects.description.contains("Reload only"));
    assert!(!c.manager.facts.borrow().need_reload);
    assert!(!c.prepare(Action::Start).ok);
    assert!(c.prepare(Action::Enable).ok);
    let after = fs::metadata(fixture.roots.unit()).unwrap();
    assert_eq!(after.ino(), before.ino());
    assert_eq!(after.mtime(), before.mtime());
    assert_eq!(after.mtime_nsec(), before.mtime_nsec());
    assert_eq!(fs::read(fixture.roots.unit()).unwrap(), bytes);
    assert_eq!(fs::metadata(parent).unwrap().mode() & 0o777, 0o755);
    assert!(!c.manager.calls.borrow().contains(&"start"));
}

#[test]
fn pending_reload_rejects_foreign_overridden_transient_and_ambiguous_runtime() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    c.manager.facts.borrow_mut().need_reload = true;
    let baseline = c.manager.facts.borrow().clone();
    for changed in 0..6 {
        *c.manager.facts.borrow_mut() = baseline.clone();
        let mut f = c.manager.facts.borrow_mut();
        match changed {
            0 => f.fragment = "/foreign/unit.service".into(),
            1 => f.drop_ins = "/foreign/override.conf".into(),
            2 => f.transient = true,
            3 => f.active = "activating".into(),
            4 => f.pid = std::process::id(),
            _ => {
                f.active = "failed".into();
                f.sub = "failed".into();
            }
        }
        drop(f);
        assert!(
            !c.prepare(Action::InstallUnit).ok,
            "unsafe variant {changed}"
        );
        assert!(!c.prepare(Action::Start).ok);
    }
    *c.manager.facts.borrow_mut() = baseline;
    fs::write(fixture.roots.unit(), b"foreign content").unwrap();
    assert!(!c.prepare(Action::InstallUnit).ok);
    assert_eq!(fs::read(fixture.roots.unit()).unwrap(), b"foreign content");
}

#[test]
fn manager_fragment_binary_and_pid_drift_block_bridge_adoption() {
    let fixture = Fixture::new();
    let c = fixture.controller();
    apply(&c, Action::InstallBinary);
    apply(&c, Action::InstallUnit);
    apply(&c, Action::Enable);
    apply(&c, Action::Start);
    c.manager.facts.borrow_mut().drop_ins = "/foreign/service.d/override.conf".into();
    assert_eq!(c.inspect().bridge_state, "conflict");
    c.manager.facts.borrow_mut().drop_ins.clear();
    c.manager.facts.borrow_mut().need_reload = true;
    assert_eq!(c.inspect().bridge_state, "conflict");
    c.manager.facts.borrow_mut().need_reload = false;
    c.manager.facts.borrow_mut().transient = true;
    assert_eq!(c.inspect().bridge_state, "conflict");
    c.manager.facts.borrow_mut().transient = false;
    c.manager.facts.borrow_mut().pid = std::process::id();
    let o = c.inspect();
    assert_eq!(o.runtime_state, "conflict");
    assert_eq!(o.bridge_state, "conflict");
    assert!(!c.prepare(Action::CompleteSetup).ok);
    c.manager.facts.borrow_mut().pid = c.manager.child.borrow().as_ref().unwrap().0.id();
    c.manager.facts.borrow_mut().fragment = "/unrelated/smith.service".into();
    assert_eq!(c.inspect().bridge_state, "conflict");
    c.manager.facts.borrow_mut().fragment = fixture.roots.unit().to_string_lossy().into();
    let mut unit = fs::read(fixture.roots.unit()).unwrap();
    unit.extend_from_slice(b"WantedBy=default.target\n");
    fs::write(fixture.roots.unit(), unit).unwrap();
    assert_eq!(c.inspect().unit_state, "conflict");
    assert_eq!(c.inspect().bridge_state, "conflict");
}

#[test]
fn concurrent_apply_is_rejected_without_effects() {
    use std::os::fd::AsRawFd;
    let fixture = Fixture::new();
    let c = fixture.controller();
    let token = c.prepare(Action::InstallBinary).approval_token.unwrap();
    let lock = fs::File::open(&fixture.roots.data).unwrap();
    // SAFETY: flock operates on this test's private open directory.
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert!(!apply_action(&c, Action::InstallBinary, &token).ok);
    assert!(!fixture.roots.binary().exists());
    // SAFETY: releasing only the private test descriptor's lock.
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_UN) }, 0);
    assert!(apply_action(&c, Action::InstallBinary, &token).ok);
}

#[test]
fn fixture_spawn_waits_until_executable_writer_is_closed() {
    let fixture = Fixture::new();
    let helper = fixture.base.join("helper");
    let fence = EXECUTABLE_IO.lock().unwrap();
    let writer = fs::OpenOptions::new().write(true).open(&helper).unwrap();
    let (attempt_sender, attempt_receiver) = mpsc::channel();
    let (result_sender, result_receiver) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        attempt_sender.send(()).unwrap();
        let result = output_fixture(Command::new(helper).arg("--version").env_clear());
        result_sender.send(result).unwrap();
    });
    let attempted = attempt_receiver.recv_timeout(Duration::from_secs(2));
    let premature = result_receiver.recv_timeout(Duration::from_millis(100));
    drop(writer);
    drop(fence);
    // Always release the writer/fence and join before assertions, even if a
    // broken spawn fence returns ETXTBSY instead of waiting.
    let output = match premature {
        Ok(result) => {
            thread.join().unwrap();
            panic!("fixture spawn escaped the executable writer fence: {result:?}");
        }
        Err(mpsc::RecvTimeoutError::Timeout) => result_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        Err(error) => panic!("fixture result channel failed: {error}"),
    };
    thread.join().unwrap();
    assert!(attempted.is_ok());
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        smith_service::IDENTITY
    );
}
#[test]
fn duplicate_daemon_does_not_adopt_or_remove_verified_socket() {
    let fixture = Fixture::new();
    fixture.installed();
    let daemon = Daemon::start(&fixture.roots);
    let before = fs::metadata(fixture.roots.socket()).unwrap().ino();
    assert!(smith_service::serve(&fixture.roots).is_err());
    assert_eq!(fs::metadata(fixture.roots.socket()).unwrap().ino(), before);
    assert!(request(&fixture.roots, daemon.0.id(), "get_state").is_ok());
}

#[test]
fn root_normalization_state_types_and_links_fail_closed() {
    let fixture = Fixture::new();
    let mut c = fixture.controller();
    c.roots.runtime = fixture.roots.runtime.join(".");
    assert!(!c.prepare(Action::InstallBinary).ok);
    c.roots = fixture.roots.clone();
    fixture.installed();
    directory(&fixture.roots.state());
    assert_eq!(c.inspect().setup_complete, None);
    fs::remove_dir(fixture.roots.state()).unwrap();
    symlink(fixture.base.join("missing"), fixture.roots.state()).unwrap();
    assert_eq!(c.inspect().setup_complete, None);
    fs::remove_file(fixture.roots.state()).unwrap();
    fs::write(fixture.roots.state(), b"{\"setup_complete\":false}").unwrap();
    fs::set_permissions(fixture.roots.state(), fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(fixture.roots.state(), fixture.base.join("state-link")).unwrap();
    assert_eq!(c.inspect().setup_complete, None);
    fs::remove_file(fixture.base.join("state-link")).unwrap();
    fs::set_permissions(fixture.roots.state(), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(c.inspect().setup_complete, None);
}

#[test]
fn compiled_identity_is_finite_and_ambient_manager_override_is_not_a_control_option() {
    let output = output_fixture(
        Command::new(env!("CARGO_BIN_EXE_smith-service"))
            .arg("--version")
            .env_clear(),
    )
    .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        smith_service::IDENTITY
    );
    let output = output_fixture(
        Command::new(env!("CARGO_BIN_EXE_smith-service"))
            .args(["inspect", "--systemctl", "/fake/systemctl"])
            .env_clear(),
    )
    .unwrap();
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], false);
}

// Test-only executable seam consumed by Flutter's mandatory Dagger gate. None
// of these controls are linked into smith-service or consult the host keyring.
mod dart_cloud {
    use super::*;
    use serde_json::{json, Value};
    use smith_service::app::{AppReply, AppRequest, AppResult, Application, Message};
    use smith_service::cloud::{
        Clock, CredentialRef, CredentialStore, Dependencies, HttpFailure, HttpResponse,
        HttpTransport, Secret, Vendor,
    };
    use smith_service::providers::{
        Inventory, Model, Observation as ProviderObservation, Observations, Provider,
    };
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[derive(Default)]
    struct Controls {
        hold: AtomicBool,
        fail_http: AtomicBool,
        offset: AtomicU64,
        local_calls: AtomicU64,
        http_calls: AtomicU64,
        key_reads: AtomicU64,
        messages: Mutex<Vec<Value>>,
    }
    impl Controls {
        fn wait(&self) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while self.hold.load(Ordering::SeqCst) {
                assert!(
                    Instant::now() < deadline,
                    "fixture hold exceeded 10 seconds"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    impl Clock for Controls {
        fn now(&self) -> Instant {
            Instant::now() + Duration::from_millis(self.offset.load(Ordering::SeqCst))
        }
        fn wall_ms(&self) -> u64 {
            smith_service::cloud::SystemClock.wall_ms() + self.offset.load(Ordering::SeqCst)
        }
    }
    struct Keys {
        controls: Arc<Controls>,
        items: Mutex<HashMap<String, Vec<u8>>>,
    }
    impl CredentialStore for Keys {
        fn read(&self, identity: &CredentialRef, _: Instant, _: &AtomicBool) -> AppResult<Secret> {
            self.controls.key_reads.fetch_add(1, Ordering::SeqCst);
            let items = self.items.lock().unwrap();
            Secret::new(
                items
                    .get(&identity.generation)
                    .expect("only saved fake generations")
                    .clone(),
            )
        }
        fn save(
            &self,
            identity: &CredentialRef,
            secret: &Secret,
            _: Instant,
            _: &AtomicBool,
        ) -> AppResult<()> {
            self.items
                .lock()
                .unwrap()
                .insert(identity.generation.clone(), secret.expose().to_vec());
            Ok(())
        }
    }
    struct Local(Arc<Controls>);
    impl Provider for Local {
        fn inventory(&self) -> AppResult<Inventory> {
            Ok(Inventory {
                models: vec![Model {
                    name: smith_service::catalog::CATALOG[0].name.into(),
                    digest: smith_service::catalog::CATALOG[0].sha256.into(),
                }],
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
        fn chat(&self, _: &str, messages: &[Message]) -> AppResult<String> {
            self.0.local_calls.fetch_add(1, Ordering::SeqCst);
            self.0
                .messages
                .lock()
                .unwrap()
                .push(serde_json::to_value(messages).unwrap());
            self.0.wait();
            Ok("Synthetic local answer".into())
        }
    }
    struct Http(Arc<Controls>);
    impl HttpTransport for Http {
        fn post(
            &self,
            vendor: Vendor,
            body: Vec<u8>,
            _: &Secret,
            _: Instant,
            _: Instant,
            _: &AtomicBool,
        ) -> std::result::Result<HttpResponse, HttpFailure> {
            self.0.http_calls.fetch_add(1, Ordering::SeqCst);
            self.0.wait();
            let input: Value = serde_json::from_slice(&body).unwrap();
            self.0.messages.lock().unwrap().push(input.clone());
            if self.0.fail_http.swap(false, Ordering::SeqCst) {
                return Ok(HttpResponse {
                    status: 503,
                    header_bytes: 0,
                    body: b"{}".to_vec(),
                });
            }
            let result = match vendor {
                Vendor::Openai => {
                    json!({"object":"response","model":input["model"],"status":"completed","error":null,"incomplete_details":null,"output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Synthetic cloud answer","annotations":[]}]}]})
                }
                Vendor::Anthropic => {
                    json!({"type":"message","role":"assistant","model":input["model"],"content":[{"type":"text","text":"Synthetic cloud answer"}],"stop_reason":"end_turn","stop_sequence":null})
                }
            };
            Ok(HttpResponse {
                status: 200,
                header_bytes: 0,
                body: serde_json::to_vec(&result).unwrap(),
            })
        }
    }
    fn call(app: &Application, op: &str, args: Value) -> Value {
        app.call(AppRequest {
            schema_version: 3,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: smith_service::app::new_id().unwrap(),
            op: op.into(),
            args,
        })
        .unwrap()
    }
    fn wait(app: &Application, request: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let v = call(
                app,
                "poll",
                json!({"request_id":request,"instance_id":app.instance_id}),
            );
            if v["status"] != "running" && v["status"] != "queued" {
                assert_eq!(v["status"], "succeeded", "{v}");
                return;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn open(roots: &Roots, keys: &Arc<Keys>, controls: &Arc<Controls>) -> Application {
        Application::open_with_cloud(
            roots,
            Arc::new(Local(controls.clone())),
            Dependencies::new(keys.clone(), Arc::new(Http(controls.clone())))
                .with_clock(controls.clone()),
        )
        .unwrap()
    }
    #[derive(Default)]
    struct RetainedFixtureState {
        commands: Vec<Value>,
        live: std::collections::HashSet<String>,
        peak: usize,
    }
    #[derive(Default)]
    struct RetainedFixtureRuntime {
        state: Mutex<RetainedFixtureState>,
        cancelled: AtomicBool,
    }
    impl smith_service::app::retained_jobs::Runtime for RetainedFixtureRuntime {
        fn execute(
            &self,
            command: &smith_service::app::retained_jobs::Command,
        ) -> smith_service::app::retained_jobs::Acknowledgement {
            use smith_service::app::retained_jobs::{Acknowledgement, Action, Outcome};
            let mut state = self.state.lock().unwrap();
            assert!(
                state.commands.len() < 256,
                "bounded synthetic retained fixture"
            );
            let outcome = if self.cancelled.load(Ordering::SeqCst) {
                Outcome::Unknown
            } else {
                match command.action {
                    Action::Create => {
                        assert!(state.live.insert(command.session_id.clone()));
                        state.peak = state.peak.max(state.live.len());
                        assert!(state.peak <= 3, "retained slot limit exceeded");
                        Outcome::Ready
                    }
                    Action::Prompt => {
                        assert!(state.live.contains(&command.session_id));
                        let previous = state.commands.iter().any(|c| {
                            c["session_id"] == command.session_id && c["action"] == "prompt"
                        });
                        if previous {
                            Outcome::Reply("Synthetic retained follow-up\nUnicode ✓".into())
                        } else {
                            Outcome::Failed
                        }
                    }
                    Action::Finish => {
                        assert!(state.live.remove(&command.session_id));
                        Outcome::Finished
                    }
                }
            };
            state.commands.push(json!({"session_id":command.session_id,"operation_id":command.operation_id,"action":command.action}));
            Acknowledgement {
                session_id: command.session_id.clone(),
                operation_id: command.operation_id.clone(),
                action: command.action,
                outcome,
            }
        }
        fn cancel(&self) {
            self.cancelled.store(true, Ordering::SeqCst);
        }
    }
    struct NoRetainedFixtureProvider;
    impl Provider for NoRetainedFixtureProvider {
        fn inventory(&self) -> AppResult<Inventory> {
            panic!("retained fixture must not observe a provider")
        }
        fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
            panic!("retained fixture must not infer through a provider")
        }
    }
    /// Separate test-binary entry point, NOT a production environment selector.
    /// No inspection/provider request reaches Application; no fake failure can
    /// fall through to a Docker, Dagger, source resolver or provider implementation.
    #[test]
    #[ignore = "launched explicitly by the isolated Dart retained-jobs protocol test"]
    fn dart_retained_fixture() {
        let base = PathBuf::from(
            std::env::var_os("SMITH_DART_CLOUD_ROOT").expect("explicit private fixture root"),
        );
        assert!(base.is_absolute());
        let metadata = fs::symlink_metadata(&base).unwrap();
        assert!(
            metadata.is_dir()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o7777 == 0o700
        );
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("runtime"),
        };
        for path in [&roots.data, &roots.config, &roots.runtime] {
            directory(path);
        }
        directory(&roots.data.join("smith-next"));
        let runtime = Arc::new(RetainedFixtureRuntime::default());
        let app = Application::open_with_retained_runtime(
            &roots,
            Arc::new(NoRetainedFixtureProvider),
            Dependencies::disabled(),
            runtime.clone(),
        )
        .unwrap();
        let socket = base.join("app.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut exchanges = 0;
        loop {
            assert!(
                Instant::now() < deadline,
                "retained Dart fixture exceeded 90 seconds"
            );
            let (mut stream, _) = match listener.accept() {
                Ok(peer) => peer,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(_) => panic!("retained fixture socket failed"),
            };
            exchanges += 1;
            assert!(exchanges <= 2048);
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut frame = String::new();
            BufReader::new((&stream).take(4 * 1024 * 1024 + 1))
                .read_line(&mut frame)
                .unwrap();
            assert!(frame.len() <= 4 * 1024 * 1024 && frame.ends_with('\n'));
            let value: Value = serde_json::from_str(&frame).unwrap();
            let stop = value == json!({"fixture":"shutdown"});
            let response = if value.get("fixture").is_some() {
                assert!(stop || value == json!({"fixture":"stats"}));
                let state = runtime.state.lock().unwrap();
                json!({"local_calls":0,"http_calls":0,"key_reads":0,"messages":[],"retained_commands":state.commands,"retained_peak":state.peak,"retained_live":state.live.len()})
            } else {
                let call_id = value["call_id"]
                    .as_str()
                    .unwrap_or("00000000000000000000000000000000")
                    .to_owned();
                let result = match serde_json::from_value::<AppRequest>(value) {
                    Ok(req)
                        if matches!(
                            req.op.as_str(),
                            "projects_list_v3"
                                | "project_create_v3"
                                | "dev_sessions_state_v1"
                                | "dev_session_get_v1"
                                | "dev_session_start_v1"
                                | "dev_session_prompt_v1"
                                | "dev_session_receipt_v1"
                                | "dev_session_finish_v1"
                                | "dev_session_delete_v1"
                        ) =>
                    {
                        app.call(req)
                    }
                    _ => Err(smith_service::app::AppError::new(
                        "FIXTURE_OPERATION_DENIED",
                        "Operation excluded from isolated retained fixture",
                    )),
                };
                serde_json::to_value(AppReply::from_versioned_result(
                    3,
                    &app.instance_id,
                    &call_id,
                    result,
                ))
                .unwrap()
            };
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
            if stop {
                break;
            }
        }
        drop(app);
        for name in ["dev-local-jobs.json", "dev-jobs.json"] {
            assert!(
                !roots.data.join("smith-next").join(name).exists(),
                "isolated fixture dispatched an inspection mutation"
            );
        }
    }

    // Called only after dropping the fixture app. No production constructor or
    // environment override can seed application state this way.
    fn seed_disk5_projects(roots: &Roots, instance_id: &str) -> Value {
        let path = roots.data.join("smith-next/app-state.json");
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() < 16 * 1024 * 1024);
        let mut disk: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(disk["schema_version"], 10);
        assert_eq!(
            disk["projects"],
            json!({"revision":0,"projects":[],"receipts":[],"expired_ids":[]})
        );
        assert!(disk["jobs"].as_array().unwrap().iter().all(|j| !matches!(
            j["operation"]["status"].as_str(),
            Some("queued" | "running")
        )));
        let project_id = "d5000000000000000000000000000001";
        let request_id = "d5000000000000000000000000000002";
        let expired_id = "d5000000000000000000000000000003";
        let url = "git@example.invalid:fixture/legacy.git";
        let args = json!({"request_id":request_id,"instance_id":instance_id,"expected_revision":0,
            "name":"Legacy fixture","repositories":[url],"agent":"opencode"});
        use sha2::Digest;
        let fingerprint = format!(
            "{:x}",
            sha2::Sha256::digest(
                serde_json::to_vec(&json!({"kind":"v3:projects:project_create","args":args}))
                    .unwrap(),
            )
        );
        let receipt =
            json!({"request_id":request_id,"project_id":project_id,"revision":1,"deleted":false});
        // Frozen disk5 has no instruction sidecar. Only downgrade this
        // synthetic, unused default; never carry disk7-only fields into disk5.
        let instructions = disk
            .as_object_mut()
            .unwrap()
            .remove("instructions")
            .unwrap();
        assert_eq!(instructions["snapshots"], json!({}));
        assert_eq!(instructions["grants"], json!([]));
        assert_eq!(instructions["receipts"], json!([]));
        assert_eq!(instructions["expired_ids"], json!([]));
        disk.as_object_mut().unwrap().remove("agents");
        disk["schema_version"] = json!(5);
        disk["projects"] = json!({"revision":1,"projects":[{"project_id":project_id,
            "name":"Legacy fixture","repositories":[url],"agent":"opencode"}],
            "receipts":[{"receipt":receipt,"fingerprint":fingerprint}],"expired_ids":[expired_id]});
        fs::write(&path, serde_json::to_vec(&disk).unwrap()).unwrap();
        json!({"legacy_project_id":project_id,"legacy_request_id":request_id,
            "expired_request_id":expired_id,"registry_revision":1,"url":url,
            "legacy_op":"project_create","legacy_args":args,"legacy_receipt":receipt,
            "disk_schema_seeded":5})
    }

    fn bootstrap(app: &Application, saved_only: bool) {
        let request = smith_service::app::new_id().unwrap();
        call(app, "models", json!({"request_id":request}));
        wait(app, &request);
        call(
            app,
            "configure",
            json!({"model":smith_service::catalog::CATALOG[0].name,"voice":null}),
        );
        for vendor in Vendor::ALL {
            for (action, revision) in [("save", 0), ("validate", 1)] {
                if saved_only && action == "validate" {
                    continue;
                }
                let request = smith_service::app::new_id().unwrap();
                let model = if action == "save" {
                    Value::Null
                } else {
                    json!(if vendor == Vendor::Openai {
                        "gpt-6-astra"
                    } else {
                        "claude-sonnet-5-5"
                    })
                };
                let prepared = call(
                    app,
                    "provider_prepare",
                    json!({"request_id":request,"instance_id":app.instance_id,"provider":vendor.name(),"action":action,"model":model,"expected_provider_revision":revision}),
                );
                call(
                    app,
                    "provider_apply",
                    json!({"request_id":request,"instance_id":app.instance_id,"approval_token":prepared["approval_token"],"api_key":if action == "save" { json!("synthetic-fixture-only") } else { Value::Null }}),
                );
                wait(app, &request);
            }
        }
    }
    #[test]
    fn bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts() {
        for saved_only in [false, true] {
            let fixture = Fixture::new();
            directory(&fixture.roots.data.join("smith-next"));
            let controls = Arc::new(Controls::default());
            let keys = Arc::new(Keys {
                controls: controls.clone(),
                items: Mutex::new(HashMap::new()),
            });
            let app = open(&fixture.roots, &keys, &controls);
            bootstrap(&app, saved_only);
            let before = call(&app, "route_catalog", json!({}));
            let instance = app.instance_id.clone();
            let reads = controls.key_reads.load(Ordering::SeqCst);
            let posts = controls.http_calls.load(Ordering::SeqCst);
            assert_eq!(posts, if saved_only { 0 } else { 2 });
            assert_eq!(reads, posts);
            assert_eq!(keys.items.lock().unwrap().len(), 2);
            drop(app);
            let app = open(&fixture.roots, &keys, &controls);
            assert_ne!(app.instance_id, instance);
            let after = call(&app, "route_catalog", json!({}));
            for vendor in Vendor::ALL {
                let p = &after["providers"][vendor.index()];
                assert_eq!(
                    p["credential_generation"],
                    before["providers"][vendor.index()]["credential_generation"]
                );
                assert_eq!(p["credential_state"], "not_observed");
                assert_eq!(p["credential_error"]["code"], "NOT_OBSERVED");
                assert_eq!(
                    p["validation"],
                    before["providers"][vendor.index()]["validation"]
                );
                assert_eq!(p["validation"].is_null(), saved_only);
            }
            for choice in after["models"].as_array().unwrap() {
                assert_eq!(choice["available"], true);
                assert!(choice["unavailable_reason"].is_null());
                if saved_only {
                    assert_eq!(choice["validated_model"], false);
                }
            }
            assert_eq!(controls.key_reads.load(Ordering::SeqCst), reads);
            assert_eq!(controls.http_calls.load(Ordering::SeqCst), posts);
        }
    }
    #[test]
    fn disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects() {
        let fixture = Fixture::new();
        directory(&fixture.roots.data.join("smith-next"));
        let controls = Arc::new(Controls::default());
        let keys = Arc::new(Keys {
            controls: controls.clone(),
            items: Mutex::new(HashMap::new()),
        });
        let app = open(&fixture.roots, &keys, &controls);
        bootstrap(&app, true);
        let instance = app.instance_id.clone();
        drop(app);
        // This migration-control test needs an existing empty registry, not a
        // fresh-profile starter. Preserve all unrelated current durable state.
        let path = fixture.roots.data.join("smith-next/app-state.json");
        let mut existing: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        existing["projects"] = json!({"revision":0,"projects":[],"receipts":[],"expired_ids":[]});
        fs::write(&path, serde_json::to_vec(&existing).unwrap()).unwrap();
        let metadata = seed_disk5_projects(&fixture.roots, &instance);
        let old: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(old["schema_version"], 5);
        assert_eq!(
            old["projects"]["projects"][0]["repositories"],
            json!([metadata["url"]])
        );
        let counters = (
            controls.local_calls.load(Ordering::SeqCst),
            controls.http_calls.load(Ordering::SeqCst),
            controls.key_reads.load(Ordering::SeqCst),
        );
        let app = open(&fixture.roots, &keys, &controls);
        let list = call(&app, "projects_list_v3", json!({}));
        assert_eq!(list["revision"], metadata["registry_revision"]);
        let row = &list["projects"][0]["repositories"][0];
        assert_eq!(
            row,
            &json!({"name":null,"url":metadata["url"],"read_only":true,"branch":{"base":null,"work":null,
            "types":{"feature":null,"fix":null,"release":null,"hotfix":null}}})
        );
        assert_eq!(
            call(
                &app,
                "project_receipt",
                json!({"request_id":metadata["legacy_request_id"]})
            ),
            metadata["legacy_receipt"]
        );
        for (op, args, code) in [
            (
                "project_create",
                metadata["legacy_args"].clone(),
                "UPGRADE_REQUIRED",
            ),
            (
                "project_create_v2",
                metadata["legacy_args"].clone(),
                "UPGRADE_REQUIRED",
            ),
            (
                "project_receipt",
                json!({"request_id":metadata["expired_request_id"]}),
                "REQUEST_EXPIRED",
            ),
        ] {
            let req = AppRequest {
                schema_version: 3,
                service: "smith-next".into(),
                protocol: "app".into(),
                call_id: smith_service::app::new_id().unwrap(),
                op: op.into(),
                args,
            };
            assert_eq!(app.call(req).unwrap_err().code, code);
        }
        let migrated: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(migrated["schema_version"], 10);
        assert_eq!(
            migrated["projects"]["receipts"],
            old["projects"]["receipts"]
        );
        assert_eq!(
            migrated["projects"]["expired_ids"],
            old["projects"]["expired_ids"]
        );
        for key in ["cloud", "timestamps", "conversations", "jobs"] {
            assert_eq!(migrated[key], old[key]);
        }
        // Disk7 migration deliberately advances configuration revision once.
        let mut expected_config = old["config"].clone();
        expected_config["revision"] = json!(old["config"]["revision"].as_u64().unwrap() + 1);
        assert_eq!(migrated["config"], expected_config);
        assert_eq!(migrated["instructions"], existing["instructions"]);
        assert_eq!(
            (
                controls.local_calls.load(Ordering::SeqCst),
                controls.http_calls.load(Ordering::SeqCst),
                controls.key_reads.load(Ordering::SeqCst)
            ),
            counters
        );
    }

    #[test]
    #[ignore = "launched explicitly by the Dart schema3 fixture gate"]
    fn dart_cloud_fixture() {
        let base = PathBuf::from(
            std::env::var_os("SMITH_DART_CLOUD_ROOT").expect("explicit private fixture root"),
        );
        assert!(base.is_absolute());
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("runtime"),
        };
        for path in [&roots.data, &roots.config, &roots.runtime] {
            directory(path);
        }
        directory(&roots.data.join("smith-next"));
        let controls = Arc::new(Controls::default());
        let keys = Arc::new(Keys {
            controls: controls.clone(),
            items: Mutex::new(HashMap::new()),
        });
        let mut app = open(&roots, &keys, &controls);
        let saved_only = std::env::var("SMITH_DART_CLOUD_SAVED_ONLY").is_ok_and(|v| v == "1");
        let agents_only = std::env::var("SMITH_DART_AGENTS_ONLY").is_ok_and(|v| v == "1");
        if !agents_only {
            bootstrap(&app, saved_only);
        }
        controls.local_calls.store(0, Ordering::SeqCst);
        controls.http_calls.store(0, Ordering::SeqCst);
        controls.key_reads.store(0, Ordering::SeqCst);
        controls.messages.lock().unwrap().clear();
        let listener = UnixListener::bind(base.join("app.sock")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut exchanges = 0;
        let mut catalog_exchanges = 0;
        loop {
            assert!(
                Instant::now() < deadline,
                "Dart fixture exceeded 90 seconds"
            );
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("fixture socket: {e}"),
            };
            exchanges += 1;
            assert!(exchanges <= 1024);
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut frame = String::new();
            BufReader::new((&stream).take(4 * 1024 * 1024 + 1))
                .read_line(&mut frame)
                .unwrap();
            assert!(frame.len() <= 4 * 1024 * 1024 && frame.ends_with('\n'));
            let value: Value = serde_json::from_str(&frame).unwrap();
            let mut stop = false;
            let response = if let Some(control) = value["fixture"].as_str() {
                let mut seeded = None;
                match control {
                    "hold" => controls
                        .hold
                        .store(value["value"].as_bool().unwrap(), Ordering::SeqCst),
                    "fail_http" => controls.fail_http.store(true, Ordering::SeqCst),
                    "advance" => {
                        controls
                            .offset
                            .fetch_add(value["milliseconds"].as_u64().unwrap(), Ordering::SeqCst);
                    }
                    "stats" => {}
                    "restart" => {
                        drop(app);
                        app = open(&roots, &keys, &controls);
                    }
                    "seed_disk7_agents" => {
                        assert!(agents_only);
                        assert_eq!(value, json!({"fixture":"seed_disk7_agents"}));
                        drop(app);
                        let path = roots.data.join("smith-next/app-state.json");
                        let mut disk: Value =
                            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                        assert_eq!(disk["agents"]["revision"], 1);
                        assert_eq!(disk["agents"]["receipts"], json!([]));
                        disk.as_object_mut().unwrap().remove("agents");
                        disk["schema_version"] = json!(7);
                        fs::write(&path, serde_json::to_vec(&disk).unwrap()).unwrap();
                        app = open(&roots, &keys, &controls);
                        seeded = Some(json!({"disk_schema_seeded":7}));
                    }
                    "seed_disk5_projects" => {
                        assert_eq!(value, json!({"fixture":"seed_disk5_projects"}));
                        assert!(
                            !controls.hold.load(Ordering::SeqCst),
                            "release held fixture work before seeding"
                        );
                        let instance = app.instance_id.clone();
                        drop(app);
                        seeded = Some(seed_disk5_projects(&roots, &instance));
                        app = open(&roots, &keys, &controls);
                    }
                    "shutdown" => {
                        controls.hold.store(false, Ordering::SeqCst);
                        stop = true;
                    }
                    _ => panic!("unknown test control"),
                }
                let mut response = json!({"local_calls":controls.local_calls.load(Ordering::SeqCst),"http_calls":controls.http_calls.load(Ordering::SeqCst),"key_reads":controls.key_reads.load(Ordering::SeqCst),"messages":*controls.messages.lock().unwrap()});
                if let Some(metadata) = seeded {
                    response
                        .as_object_mut()
                        .unwrap()
                        .extend(metadata.as_object().unwrap().clone());
                    response["instance_id"] = json!(app.instance_id);
                }
                response
            } else {
                let req: AppRequest = serde_json::from_value(value).unwrap();
                assert_eq!(
                    req.schema_version, 3,
                    "production Dart adapter must use schema3"
                );
                let call_id = req.call_id.clone();
                let is_catalog = req.op == "route_catalog";
                let result = app.call(req);
                if is_catalog {
                    let catalog = result.as_ref().expect("actual service catalog");
                    let cloud: Vec<_> = catalog["models"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|choice| choice["route"]["provider"] != "local")
                        .collect();
                    assert_eq!(cloud.len(), smith_service::cloud::MODELS.len());
                    for (choice, model) in cloud.iter().zip(smith_service::cloud::MODELS) {
                        assert_eq!(choice["route"]["provider"], model.vendor.name());
                        assert_eq!(choice["route"]["model"], model.id);
                        assert_eq!(choice["label"], model.label);
                    }
                    catalog_exchanges += 1;
                }
                serde_json::to_value(AppReply::from_versioned_result(
                    3,
                    &app.instance_id,
                    &call_id,
                    result,
                ))
                .unwrap()
            };
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
            if stop {
                break;
            }
        }
        assert!(
            agents_only || catalog_exchanges > 0,
            "Dart must request the actual full Rust catalog"
        );
        drop(app);
    }
}
