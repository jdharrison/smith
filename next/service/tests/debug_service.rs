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
fn app_v2_helper_correlates_preflight_and_strict_decode_failures() {
    for (malformed, suffix) in [(false, ""), (true, ""), (false, "\n"), (false, "{}\n")] {
        let mut request = serde_json::to_value(app_state_request()).unwrap();
        request["schema_version"] = serde_json::json!(2);
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
        let reply: smith_service::app::AppReply = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(reply.schema_version, 2);
        assert_eq!(reply.call_id, app_state_request().call_id);
        assert!(!reply.ok);
    }
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
