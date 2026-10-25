use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::{atomic::AtomicUsize, OnceLock};

fn fixture_executable() -> Vec<u8> {
    static BINARY: OnceLock<Vec<u8>> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let root = std::env::temp_dir().join(format!(
                "smith-retained-native-{}",
                crate::app::new_id().unwrap()
            ));
            filesystem::mkdir(&root).unwrap();
            let binary = root.join("supervisor");
            let output = Process::new("rustc")
                .args(["--edition=2021", "-Dwarnings"])
                .arg(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("src/retained_supervisor_fixture.rs"),
                )
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(output.status.success(), "native fixture compilation failed");
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
            let bytes = filesystem::read_file(&binary, 0o700, 128 * 1024 * 1024)
                .unwrap()
                .unwrap();
            fs::remove_dir_all(&root).unwrap();
            bytes
        })
        .clone()
}
struct Resolver {
    calls: AtomicUsize,
    binding: SourceBinding,
}
impl SourceResolver for Resolver {
    fn resolve(
        &self,
        p: &FrozenProject,
        mode: JobType,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<SourceBinding> {
        check(deadline, cancelled).ok()?;
        assert_eq!(p.context, "Frozen context");
        assert_eq!(mode, JobType::Read);
        self.calls.fetch_add(1, Ordering::SeqCst);
        Some(self.binding.clone())
    }
}
struct Verifier {
    accepted: AtomicBool,
    calls: Mutex<Vec<CleanupTarget>>,
}
impl CleanupVerifier for Verifier {
    fn verify(&self, target: &CleanupTarget, deadline: Instant, cancelled: &AtomicBool) -> bool {
        if check(deadline, cancelled).is_err() {
            return false;
        }
        self.calls.lock().unwrap().push(CleanupTarget {
            job_id: target.job_id.clone(),
            source_id: target.source_id.clone(),
            source_sha256: target.source_sha256.clone(),
            finish_operation_id: target.finish_operation_id.clone(),
            supervisor_pid: target.supervisor_pid,
        });
        self.accepted.load(Ordering::SeqCst)
    }
}
struct Fixture {
    root: PathBuf,
    config: PreparedConfig,
    resolver: Arc<Resolver>,
    verifier: Arc<Verifier>,
}
impl Fixture {
    fn new(behavior: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "smith-retained-transport-{}",
            crate::app::new_id().unwrap()
        ));
        filesystem::mkdir(&root).unwrap();
        let module = root.join("source-module");
        filesystem::mkdir(&module).unwrap();
        filesystem::publish(&module.join("behavior"), behavior.as_bytes(), 0o600, false).unwrap();
        let workspace = root.join("jobs");
        filesystem::mkdir(&workspace).unwrap();
        let executable = root.join("native-launcher");
        let bytes = fixture_executable();
        filesystem::publish(&executable, &bytes, 0o700, false).unwrap();
        let config = PreparedConfig {
            executable,
            executable_sha256: filesystem::digest(&bytes),
            module,
            manifest: BTreeMap::from([(
                PathBuf::from("behavior"),
                filesystem::digest(behavior.as_bytes()),
            )]),
            workspace,
            operation_timeout: Duration::from_secs(2),
            cleanup_timeout: Duration::from_millis(100),
            poll_interval: POLL,
            max_polls: 16,
        };
        Self {
            root,
            config,
            resolver: Arc::new(Resolver {
                calls: AtomicUsize::new(0),
                binding: SourceBinding {
                    source_id: crate::app::new_id().unwrap(),
                    manifest_sha256: "a".repeat(64),
                },
            }),
            verifier: Arc::new(Verifier {
                accepted: AtomicBool::new(true),
                calls: Mutex::new(Vec::new()),
            }),
        }
    }
    fn adapter(&self) -> Prepared {
        Prepared::new(
            self.config.clone(),
            self.resolver.clone(),
            self.verifier.clone(),
        )
        .unwrap()
    }
    fn command(&self) -> Command {
        Command {
            session_id: crate::app::new_id().unwrap(),
            operation_id: crate::app::new_id().unwrap(),
            action: Action::Create,
            job_type: JobType::Read,
            project: FrozenProject {
                project_id: crate::app::new_id().unwrap(),
                project_key: "fixture".into(),
                context: "Frozen context".into(),
                repositories: vec![],
            },
            prompt: Some("PROMPT-SECRET-never-in-argv".into()),
        }
    }
    fn events(&self, c: &Command) -> String {
        fs::read_to_string(self.config.workspace.join(&c.session_id).join("events"))
            .unwrap_or_default()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn next(c: &Command, action: Action, prompt: Option<&str>) -> Command {
    let mut c = c.clone();
    c.operation_id = crate::app::new_id().unwrap();
    c.action = action;
    c.prompt = prompt.map(str::to_owned);
    c
}
fn pid(events: &str) -> u32 {
    events
        .split_whitespace()
        .next()
        .unwrap()
        .strip_prefix("pid:")
        .unwrap()
        .parse()
        .unwrap()
}
fn unknown(outcome: Outcome) {
    assert!(matches!(outcome, Outcome::Unknown));
}

#[test]
fn unknown_environments_keep_transport_capacity_reserved() {
    let f = Fixture::new("malformed");
    let adapter = f.adapter();
    for _ in 0..3 {
        let c = f.command();
        unknown(adapter.execute(&c).outcome);
        assert!(!f.events(&c).is_empty());
    }
    let fourth = f.command();
    unknown(adapter.execute(&fourth).outcome);
    assert!(f.events(&fourth).is_empty());
    assert_eq!(f.resolver.calls.load(Ordering::SeqCst), 3);
}

#[test]
fn executable_digest_mismatch_refuses_before_spawn() {
    let f = Fixture::new("normal");
    let adapter = f.adapter();
    filesystem::publish(
        &f.config.executable,
        b"not the approved launcher",
        0o700,
        true,
    )
    .unwrap();
    let c = f.command();
    unknown(adapter.execute(&c).outcome);
    assert!(f.events(&c).is_empty());
}

#[test]
fn contradictory_success_code_is_not_accepted() {
    let request = json!({"operation_id":"a".repeat(32),"job_id":"b".repeat(32)});
    let response = json!({"v":1,"operation_id":"a".repeat(32),"job_id":"b".repeat(32),"ok":true,"code":null,
        "job":{"state":"active","mode":"read","retained":true,"conversation":true,"ready":false,"blockers":[]},
        "operation":{"operation_id":"c".repeat(32),"op":"create","state":"succeeded","code":"runtime_unknown","text":""}});
    assert!(Response::decode(&serde_json::to_vec(&response).unwrap(), &request).is_err());
}

#[test]
fn same_process_pending_polls_failure_followup_and_verified_finish() {
    let f = Fixture::new("normal");
    let adapter = f.adapter();
    assert!(!adapter.admission_ready());
    let create = f.command();
    assert!(matches!(adapter.execute(&create).outcome, Outcome::Ready));
    let initial_events = f.events(&create);
    let original_pid = pid(&initial_events);
    assert_eq!(initial_events.matches("op:observe").count(), 2);
    assert!(initial_events.contains(&format!("id:{}", wire_id(&create))));
    assert!(!initial_events.contains("PROMPT-SECRET"));
    let marker = fs::read_to_string(
        f.config
            .workspace
            .join(&create.session_id)
            .join("consumed.json"),
    )
    .unwrap();
    assert!(marker.contains(&f.resolver.binding.source_id));
    assert!(!marker.contains("PROMPT-SECRET"));
    // Same domain identity for create and initial prompt must NOT collide in Python.
    let mut prompt = create.clone();
    prompt.action = Action::Prompt;
    prompt.prompt = Some("known-failure".into());
    assert_ne!(wire_id(&create), wire_id(&prompt));
    assert!(matches!(adapter.execute(&prompt).outcome, Outcome::Failed));
    let followup = next(
        &create,
        Action::Prompt,
        Some("Follow up after known failure"),
    );
    match adapter.execute(&followup).outcome {
        Outcome::Reply(text) => assert_eq!(text, format!("pid:{original_pid}")),
        other => panic!("unexpected {other:?}"),
    }
    let count = f.events(&create).lines().count();
    assert!(matches!(
        adapter.execute(&followup).outcome,
        Outcome::Reply(_)
    ));
    assert_eq!(f.events(&create).lines().count(), count);
    let mut conflict = followup.clone();
    conflict.prompt = Some("Conflicting text".into());
    unknown(adapter.execute(&conflict).outcome);
    assert_eq!(f.events(&create).lines().count(), count);
    assert_eq!(f.resolver.calls.load(Ordering::SeqCst), 1);
    let finish = next(&create, Action::Finish, None);
    assert!(matches!(
        adapter.execute(&finish).outcome,
        Outcome::Finished
    ));
    let targets = f.verifier.calls.lock().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].source_id, f.resolver.binding.source_id);
    assert_eq!(targets[0].supervisor_pid, original_pid);
    assert_eq!(targets[0].finish_operation_id, wire_id(&finish));
    assert_eq!(unsafe { libc::kill(original_pid as i32, 0) }, -1);
    assert!(f
        .events(&create)
        .lines()
        .all(|line| line.starts_with(&format!("pid:{original_pid} "))));
}
#[test]
fn missing_independent_cleanup_proof_never_finishes() {
    let f = Fixture::new("normal");
    f.verifier.accepted.store(false, Ordering::SeqCst);
    let adapter = f.adapter();
    let create = f.command();
    assert!(matches!(adapter.execute(&create).outcome, Outcome::Ready));
    unknown(
        adapter
            .execute(&next(&create, Action::Finish, None))
            .outcome,
    );
    assert_eq!(f.verifier.calls.lock().unwrap().len(), 1);
}
#[test]
fn python_cleanup_unknown_does_not_invoke_verifier_or_release() {
    let f = Fixture::new("finish_unknown");
    let adapter = f.adapter();
    let create = f.command();
    assert!(matches!(adapter.execute(&create).outcome, Outcome::Ready));
    unknown(
        adapter
            .execute(&next(&create, Action::Finish, None))
            .outcome,
    );
    assert!(f.verifier.calls.lock().unwrap().is_empty());
}
#[test]
fn malformed_wrong_identity_duplicate_and_output_limits_are_unknown() {
    for behavior in [
        "malformed",
        "wrong_identity",
        "duplicate_member",
        "stdout_flood",
        "stderr_flood",
        "extra",
        "unknown",
        "exit",
    ] {
        let f = Fixture::new(behavior);
        let adapter = f.adapter();
        let c = f.command();
        unknown(adapter.execute(&c).outcome);
        let before = f.events(&c);
        unknown(adapter.execute(&c).outcome);
        assert_eq!(before, f.events(&c));
        unknown(
            adapter
                .execute(&next(&c, Action::Prompt, Some("Never retry")))
                .outcome,
        );
        assert_eq!(before, f.events(&c));
    }
}
#[test]
fn timeout_and_poll_limit_fence_original_process_without_replacement() {
    for behavior in ["slow", "pending_forever"] {
        let mut f = Fixture::new(behavior);
        f.config.operation_timeout = Duration::from_secs(2);
        f.config.max_polls = 2;
        let adapter = f.adapter();
        let c = f.command();
        let start = Instant::now();
        unknown(adapter.execute(&c).outcome);
        assert!(start.elapsed() < Duration::from_secs(3));
        let before = f.events(&c);
        assert!(!before.is_empty(), "supervisor must record its start");
        let child = pid(&before);
        assert_eq!(unsafe { libc::kill(child as i32, 0) }, -1);
        drop(adapter);
        let replacement = f.adapter();
        unknown(replacement.execute(&c).outcome);
        assert_eq!(before, f.events(&c));
    }
}
#[test]
fn cancellation_interrupts_blocked_exchange_and_kills_owned_process() {
    let mut f = Fixture::new("slow");
    f.config.operation_timeout = Duration::from_secs(10);
    let adapter = Arc::new(f.adapter());
    let c = f.command();
    let request = c.clone();
    let worker = adapter.clone();
    let handle = std::thread::spawn(move || worker.execute(&request));
    let deadline = Instant::now() + Duration::from_secs(2);
    while f.events(&c).is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(POLL);
    }
    let child = pid(&f.events(&c));
    let start = Instant::now();
    adapter.cancel();
    unknown(handle.join().unwrap().outcome);
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(unsafe { libc::kill(child as i32, 0) }, -1);
    unknown(
        adapter
            .execute(&next(&c, Action::Prompt, Some("No replacement")))
            .outcome,
    );
}
#[test]
fn process_loss_is_not_reconnected_and_idle_cancel_reaps() {
    let f = Fixture::new("normal");
    let adapter = f.adapter();
    let c = f.command();
    assert!(matches!(adapter.execute(&c).outcome, Outcome::Ready));
    let child = pid(&f.events(&c));
    unsafe {
        libc::kill(child as i32, libc::SIGKILL);
    }
    unknown(
        adapter
            .execute(&next(&c, Action::Prompt, Some("No replay")))
            .outcome,
    );
    assert_eq!(f.events(&c).matches("argv:").count(), 1);
    let c2 = f.command();
    assert!(matches!(adapter.execute(&c2).outcome, Outcome::Ready));
    let child2 = pid(&f.events(&c2));
    adapter.cancel();
    assert_eq!(unsafe { libc::kill(child2 as i32, 0) }, -1);
}
#[test]
fn immutable_snapshots_reject_drift_and_never_restart_consumed_job() {
    let f = Fixture::new("normal");
    let adapter = f.adapter();
    let c = f.command();
    filesystem::publish(&f.config.module.join("behavior"), b"changed", 0o600, true).unwrap();
    unknown(adapter.execute(&c).outcome);
    assert!(f.events(&c).is_empty());
    filesystem::publish(&f.config.module.join("behavior"), b"normal", 0o600, true).unwrap();
    unknown(adapter.execute(&c).outcome);
    assert!(f.events(&c).is_empty());
    let fresh = f.command();
    assert!(matches!(adapter.execute(&fresh).outcome, Outcome::Ready));
    filesystem::publish(&f.config.module.join("behavior"), b"changed", 0o600, true).unwrap();
    assert!(matches!(
        adapter
            .execute(&next(&fresh, Action::Prompt, Some("Uses immutable copy")))
            .outcome,
        Outcome::Reply(_)
    ));
}
#[test]
fn request_prompt_and_frame_bounds_do_not_dispatch_oversized_input() {
    let f = Fixture::new("normal");
    let adapter = f.adapter();
    let c = f.command();
    assert!(matches!(adapter.execute(&c).outcome, Outcome::Ready));
    let before = f.events(&c);
    let large = next(&c, Action::Prompt, Some(&"x".repeat(16384)));
    assert!(matches!(adapter.execute(&large).outcome, Outcome::Failed));
    assert_eq!(before, f.events(&c));
    let escaped = next(&c, Action::Prompt, Some(&"\u{1}".repeat(6000)));
    assert!(matches!(adapter.execute(&escaped).outcome, Outcome::Failed));
    assert_eq!(before, f.events(&c));
}
#[test]
fn no_constructor_effects_and_invalid_configuration_rejected() {
    let mut f = Fixture::new("normal");
    let adapter = f.adapter();
    assert_eq!(fs::read_dir(&f.config.workspace).unwrap().count(), 0);
    drop(adapter);
    f.config
        .manifest
        .insert(PathBuf::from("../escape"), "a".repeat(64));
    assert!(Prepared::new(f.config.clone(), f.resolver.clone(), f.verifier.clone()).is_err());
}
