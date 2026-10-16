//! Explicit artifact/lifecycle-only library recipes under isolated factories/private roots.
//! Product model recipes are exercised separately in model_wizard.rs; no live effects.
use smith_installer::artifact::Outcome;
use smith_installer::setup::Backend;
use smith_installer::wizard::{
    Cancellation, Choices, Command, Event, Limits, PreparedReview, Publisher, Report, Session,
    StepState,
};
use smith_installer::Result;
use smith_service::app::{AppReply, AppRequest};
use smith_service::{Action, Observation};
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn facts(unit: &str, runtime: &str, bridge: &str) -> Observation {
    let enablement = if unit == "missing" {
        "missing"
    } else if runtime == "running" {
        "enabled"
    } else {
        "disabled"
    };
    serde_json::from_value(serde_json::json!({"schema_version":2,"service":"smith-next","ok":true,
        "binary_state":"installed","unit_state":unit,"enablement_state":enablement,"runtime_state":runtime,"bridge_state":bridge,
        "setup_complete":false,"message":"Observed","error":null,
        "effects":{"binary_path":"/isolated/bin/smith-service","unit_path":"/isolated/config/smith-next.service",
            "state_path":"/isolated/data/state.json","socket_path":"/isolated/run/service.sock","description":"Exact current Core effects"}})).unwrap()
}
struct State {
    calls: Vec<String>,
    owners: Vec<ThreadId>,
    pub_count: usize,
    preview_count: usize,
    previews_fail: usize,
    installed: bool,
    attempts: usize,
    generation: usize,
    backend_sources: Vec<PathBuf>,
    facts: Observation,
    tokens: Vec<(Action, String)>,
    applied: Vec<(Action, String)>,
    expired: bool,
    fail_action: Option<Action>,
    fail_install: bool,
    hold_install: bool,
    large_output: bool,
    publisher_dropped: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            owners: Vec::new(),
            pub_count: 0,
            preview_count: 0,
            previews_fail: 0,
            installed: false,
            attempts: 0,
            generation: 1,
            backend_sources: Vec::new(),
            facts: facts("missing", "stopped", "unavailable"),
            tokens: Vec::new(),
            applied: Vec::new(),
            expired: false,
            fail_action: None,
            fail_install: false,
            hold_install: false,
            large_output: false,
            publisher_dropped: false,
        }
    }
}
struct Review {
    generation: usize,
    id: String,
    description: String,
}
impl PreparedReview for Review {
    fn review_id(&self) -> &str {
        &self.id
    }
    fn description(&self) -> &str {
        &self.description
    }
}
struct FakePublisher {
    state: Arc<Mutex<State>>,
    cancelled: Arc<AtomicBool>,
    owner: ThreadId,
    _not_send: Rc<()>,
}
impl Publisher for FakePublisher {
    type Prepared = Review;
    fn prepare_review(&mut self) -> Result<Review> {
        assert_eq!(thread::current().id(), self.owner);
        let mut s = self.state.lock().unwrap();
        s.preview_count += 1;
        if s.previews_fail > 0 {
            s.previews_fail -= 1;
            return Err("Missing read-only preview prerequisite".into());
        }
        Ok(Review {
            generation: s.generation,
            id: format!("artifact-review-{}", s.generation),
            description:
                "Exact paired desktop/service artifact plan\nNo unit/start/provider effects".into(),
        })
    }
    fn install_reviewed(
        &mut self,
        review: Review,
        approved: &str,
        output: &mut dyn Write,
    ) -> Result<Outcome> {
        assert_eq!(thread::current().id(), self.owner);
        assert_eq!(review.id, approved);
        let (hold, fail, large) = {
            let mut s = self.state.lock().unwrap();
            s.attempts += 1;
            if s.generation != review.generation {
                return Err("Artifacts changed after displayed review".into());
            }
            (s.hold_install, s.fail_install, s.large_output)
        };
        if large {
            let _ = output.write_all(&vec![b'x'; 256 * 1024 + 1]);
            return Ok(Outcome::Installed);
        }
        writeln!(output, "Captured publisher effects; no stdout scraping")
            .map_err(|e| e.to_string())?;
        if hold {
            while !self.cancelled.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(2));
            }
            return Err("Publication cancelled after acceptance; partial effects preserved".into());
        }
        if fail {
            return Err("Publication failed after an accepted partial effect".into());
        }
        self.state.lock().unwrap().installed = true;
        Ok(Outcome::Installed)
    }
}
impl Drop for FakePublisher {
    fn drop(&mut self) {
        assert_eq!(thread::current().id(), self.owner);
        self.state.lock().unwrap().publisher_dropped = true;
    }
}
struct FakeBackend {
    state: Arc<Mutex<State>>,
    owner: ThreadId,
}
impl Backend for FakeBackend {
    fn inspect(&mut self) -> Result<Observation> {
        assert_eq!(thread::current().id(), self.owner);
        let mut s = self.state.lock().unwrap();
        s.calls.push("inspect".into());
        Ok(s.facts.clone())
    }
    fn prepare(&mut self, action: Action) -> Result<Observation> {
        let mut s = self.state.lock().unwrap();
        s.calls.push(format!("prepare:{action:?}"));
        let issued = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - if s.expired { 61 } else { 0 };
        let token = format!("{issued}.{}", "a".repeat(64));
        s.tokens.push((action, token.clone()));
        let mut result = s.facts.clone();
        result.approval_token = Some(token);
        Ok(result)
    }
    fn apply(&mut self, action: Action, token: &str) -> Result<Observation> {
        let mut s = self.state.lock().unwrap();
        s.calls.push(format!("apply:{action:?}"));
        assert!(s
            .tokens
            .iter()
            .any(|(a, original)| *a == action && original == token));
        s.applied.push((action, token.into()));
        if s.fail_action == Some(action) {
            return Err("Fake manager failed after accepting the exact action".into());
        }
        s.facts = match action {
            Action::InstallUnit => facts("installed", "stopped", "unavailable"),
            Action::Enable => {
                let mut facts = facts("installed", "stopped", "unavailable");
                facts.enablement_state = "enabled".into();
                facts
            }
            Action::Start => facts("installed", "running", "ready"),
            _ => panic!("No binary/setup-complete actions in GUI"),
        };
        Ok(s.facts.clone())
    }
    fn app(&mut self, _: AppRequest) -> Result<AppReply> {
        panic!("Explicit artifact/lifecycle-only recipe must not use model/provider operations")
    }
}
struct Harness {
    state: Arc<Mutex<State>>,
    cancelled: Arc<AtomicBool>,
    session: Session,
}
impl Harness {
    fn new(configure: impl FnOnce(&mut State), limits: Limits) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        configure(&mut state.lock().unwrap());
        let cancelled = Arc::new(AtomicBool::new(false));
        let observed = cancelled.clone();
        let requested = cancelled.clone();
        let cancellation = Cancellation::from_callbacks(
            move || observed.load(Ordering::Acquire),
            move || {
                requested.store(true, Ordering::Release);
            },
        );
        let pub_state = state.clone();
        let pub_cancel = cancelled.clone();
        let back_state = state.clone();
        let parent = thread::current().id();
        let session = Session::spawn_with(
            PathBuf::from("/isolated/package"),
            move |client, service| {
                assert_eq!(client, PathBuf::from("/isolated/package/payload/client"));
                assert_eq!(
                    service,
                    PathBuf::from("/isolated/package/payload/service/smith-service")
                );
                let owner = thread::current().id();
                assert_ne!(owner, parent);
                let mut s = pub_state.lock().unwrap();
                s.pub_count += 1;
                s.owners.push(owner);
                drop(s);
                Ok(FakePublisher {
                    state: pub_state.clone(),
                    cancelled: pub_cancel.clone(),
                    owner,
                    _not_send: Rc::new(()),
                })
            },
            move |source| {
                let owner = thread::current().id();
                assert_ne!(owner, parent);
                let mut s = back_state.lock().unwrap();
                assert!(s.installed);
                s.backend_sources.push(source);
                s.owners.push(owner);
                drop(s);
                Ok(Box::new(FakeBackend {
                    state: back_state.clone(),
                    owner,
                }) as Box<dyn Backend>)
            },
            || Ok(()),
            cancellation,
            limits,
        )
        .unwrap();
        Self {
            state,
            cancelled,
            session,
        }
    }
    fn review(&self) -> u64 {
        loop {
            if let Event::Review {
                request_id,
                description,
            } = event(&self.session)
            {
                assert!(description.contains("Exact paired"));
                return request_id;
            }
        }
    }
    fn install(&self, id: u64, choices: Choices) {
        self.session
            .send(Command::Install {
                request_id: id,
                choices,
            })
            .unwrap();
    }
    fn confirmation(&self) -> (u64, String) {
        loop {
            match event(&self.session) {
                Event::Confirmation {
                    request_id,
                    title,
                    effects,
                } => {
                    assert!(!effects.is_empty());
                    assert!(effects.iter().all(|s| !s.contains(&"a".repeat(64))));
                    return (request_id, title);
                }
                Event::Finished { report } => panic!("Expected confirmation: {}", report.message),
                _ => {}
            }
        }
    }
    fn finish(&mut self) -> Report {
        finish(&mut self.session)
    }
}
fn event(session: &Session) -> Event {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(event) = session.try_event().unwrap() {
            return event;
        }
        assert!(Instant::now() < deadline, "Wizard event budget");
        thread::sleep(Duration::from_millis(2));
    }
}
fn finish(session: &mut Session) -> Report {
    let report = loop {
        if let Event::Finished { report } = event(session) {
            break report;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(4);
    while !session.is_finished() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    session.join().unwrap();
    report
}

struct FakeMaintenance {
    calls: Arc<Mutex<Vec<String>>>,
    owner: ThreadId,
    consent_budget: Duration,
    fail_after_consent: bool,
}
impl smith_installer::maintenance::Runner for FakeMaintenance {
    fn execute(
        &mut self,
        operation: smith_installer::maintenance::Operation,
        prompt: &mut dyn smith_installer::maintenance::Prompt,
    ) -> Result<smith_installer::maintenance::Outcome> {
        assert_eq!(thread::current().id(), self.owner);
        self.calls
            .lock()
            .unwrap()
            .push(format!("execute:{operation:?}"));
        prompt.progress("Captured old installation; no host calls")?;
        let deadline = Instant::now() + self.consent_budget;
        let accepted = prompt.confirm(
            "Exact maintenance action",
            &["Exact original effects — preserve all state/models/releases/backups".into()],
            deadline,
        )?;
        if accepted {
            self.calls.lock().unwrap().push("accepted".into());
            prompt.progress("Accepted stage preserved")?;
            if self.fail_after_consent {
                return Err("Failure after accepted stage; partial effects retained".into());
            }
        }
        Ok(smith_installer::maintenance::Outcome {
            complete: accepted,
            message: "Fake maintenance result".into(),
            details: vec!["Synthetic effects only".into()],
        })
    }
}
fn maintenance_session(
    preview_fails: bool,
    timeout: Duration,
    consent_budget: Duration,
    fail_after_consent: bool,
) -> (Session, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let constructed = calls.clone();
    let state = Arc::new(Mutex::new(State::default()));
    state.lock().unwrap().previews_fail = usize::from(preview_fails);
    let cancelled = Arc::new(AtomicBool::new(false));
    let observed = cancelled.clone();
    let requested = cancelled.clone();
    let parent = thread::current().id();
    let session = Session::spawn_extended_with(
        PathBuf::from("/isolated/package"),
        move |_, _| {
            Ok(FakePublisher {
                state: state.clone(),
                cancelled: cancelled.clone(),
                owner: thread::current().id(),
                _not_send: Rc::new(()),
            })
        },
        |_| panic!("Maintenance must not construct model/lifecycle backend"),
        || Ok(()),
        Cancellation::from_callbacks(
            move || observed.load(Ordering::Acquire),
            move || {
                requested.store(true, Ordering::Release);
            },
        ),
        Limits {
            pending_timeout: timeout,
            ..Limits::default()
        },
        None,
        Some(Box::new(move |bundle| {
            assert_eq!(bundle, PathBuf::from("/isolated/package"));
            assert_ne!(thread::current().id(), parent);
            constructed.lock().unwrap().push("constructed".into());
            Ok(Box::new(FakeMaintenance {
                calls: constructed.clone(),
                owner: thread::current().id(),
                consent_budget,
                fail_after_consent,
            }))
        })),
    )
    .unwrap();
    (session, calls)
}
fn maintenance_confirmation(session: &Session) -> u64 {
    loop {
        match event(session) {
            Event::Confirmation {
                request_id,
                effects,
                ..
            } => {
                assert_eq!(
                    effects,
                    ["Exact original effects — preserve all state/models/releases/backups"]
                );
                return request_id;
            }
            Event::Finished { report } => panic!("Expected original consent: {}", report.message),
            _ => {}
        }
    }
}
#[test]
fn maintenance_actions_route_after_conflicted_and_expired_previews_without_publication() {
    use smith_installer::wizard::Scope;
    for preview_fails in [false, true] {
        for (command, scope) in [
            (Command::Upgrade, Scope::Upgrade),
            (Command::Uninstall, Scope::Uninstall),
        ] {
            let (mut session, calls) = maintenance_session(
                preview_fails,
                Duration::from_millis(20),
                Duration::from_secs(1),
                false,
            );
            let first = event(&session);
            assert!(matches!(first, Event::Review { .. } | Event::Stage { .. }));
            assert!(
                calls.lock().unwrap().is_empty(),
                "Opening constructed maintenance"
            );
            if !preview_fails {
                assert!(
                    matches!(event(&session), Event::Stage { message } if message.contains("expired"))
                );
            }
            session.send(command).unwrap();
            let id = maintenance_confirmation(&session);
            session.send(Command::Approve { request_id: id }).unwrap();
            let report = finish(&mut session);
            assert_eq!(report.scope, scope);
            assert!(report.complete, "{}", report.message);
            assert_eq!(calls.lock().unwrap().len(), 3);
        }
    }
}
#[test]
fn maintenance_original_deadline_cancel_and_wrong_ids_never_approve() {
    for mode in [0, 1, 2, 3] {
        let (mut session, calls) = maintenance_session(
            true,
            Duration::from_secs(1),
            Duration::from_millis(40),
            false,
        );
        event(&session);
        session.send(Command::Uninstall).unwrap();
        let id = maintenance_confirmation(&session);
        match mode {
            0 => thread::sleep(Duration::from_millis(70)),
            1 => session.send(Command::Cancel).unwrap(),
            3 => session.send(Command::Decline { request_id: id }).unwrap(),
            _ => assert!(session
                .send(Command::Approve { request_id: id + 1 })
                .is_err()),
        }
        let report = finish(&mut session);
        assert!(!report.complete);
        assert_eq!(report.scope, smith_installer::wizard::Scope::Uninstall);
        assert!(!calls.lock().unwrap().iter().any(|s| s == "accepted"));
        assert_eq!(report.cancelled, mode == 1);
    }
}
#[test]
fn maintenance_error_preserves_scope_and_accepted_uncertain_evidence() {
    let (mut session, _) =
        maintenance_session(true, Duration::from_secs(1), Duration::from_secs(1), true);
    event(&session);
    session.send(Command::Upgrade).unwrap();
    let id = maintenance_confirmation(&session);
    session.send(Command::Approve { request_id: id }).unwrap();
    let report = finish(&mut session);
    assert_eq!(report.scope, smith_installer::wizard::Scope::Upgrade);
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Uncertain);
    assert!(report
        .details
        .iter()
        .any(|s| s == "Accepted stage preserved"));
    assert!(report.message.contains("partial effects"));
}

#[test]
fn shell_contract_defaults_and_read_only_preview_have_no_lifecycle_or_effects() {
    assert_eq!(
        Choices::default(),
        Choices {
            register_service: false,
            enable_service: false,
            start_service: false,
        }
    );
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    assert_ne!(id, 0);
    assert!(!h.session.is_finished());
    assert!(h.session.join().is_err());
    let s = h.state.lock().unwrap();
    assert_eq!(s.attempts, 0);
    assert!(s.backend_sources.is_empty());
    assert!(s.calls.is_empty());
    drop(s);
    h.session.send(Command::Decline { request_id: id }).unwrap();
    let report = h.finish();
    assert!(!report.complete);
    assert!(!report.cancelled);
    assert!(report.message.contains("declined"));
    assert!(report.steps.iter().all(|s| s.state == StepState::Skipped));
}
#[test]
fn artifact_only_is_incomplete_without_constructing_runtime_roots_or_backend() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    h.install(id, Choices::default());
    let report = h.finish();
    assert!(!report.complete);
    assert!(!report.cancelled);
    assert_eq!(report.steps[0].state, StepState::Completed);
    assert!(report.steps[1..]
        .iter()
        .all(|s| s.state == StepState::Skipped));
    let s = h.state.lock().unwrap();
    assert_eq!(s.attempts, 1);
    assert!(s.backend_sources.is_empty());
    assert!(s.publisher_dropped);
    assert!(report.message.contains("Setup incomplete"));
}
#[test]
fn selected_existing_stopped_registration_is_verified_read_only_without_start() {
    let mut h = Harness::new(
        |s| s.facts = facts("installed", "stopped", "unavailable"),
        Limits::default(),
    );
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: false,
            start_service: false,
        },
    );
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[1].state, StepState::Skipped);
    assert_eq!(report.steps[2].state, StepState::Skipped);
    let s = h.state.lock().unwrap();
    assert!(s.applied.is_empty());
    assert_eq!(s.tokens.len(), 1);
    assert_eq!(s.tokens[0].0, Action::Enable);
}
#[test]
fn declining_start_keeps_verified_registration_and_never_starts() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: true,
        },
    );
    let (id, _) = h.confirmation();
    h.session.send(Command::Approve { request_id: id }).unwrap();
    let (id, title) = h.confirmation();
    assert!(title.contains("Enable"));
    h.session.send(Command::Approve { request_id: id }).unwrap();
    let (id, title) = h.confirmation();
    assert!(title.contains("Start"));
    h.session.send(Command::Decline { request_id: id }).unwrap();
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Completed);
    assert_eq!(report.steps[1].state, StepState::Completed);
    assert_eq!(report.steps[2].state, StepState::Completed);
    assert_eq!(report.steps[3].state, StepState::Skipped);
    assert_eq!(h.state.lock().unwrap().applied.len(), 2);
}
#[test]
fn final_event_backpressure_cannot_be_reported_as_complete() {
    let mut h = Harness::new(
        |_| {},
        Limits {
            channel_capacity: 2,
            ..Limits::default()
        },
    );
    let id = h.review();
    h.install(id, Choices::default());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !h.session.is_finished() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Completed);
    assert!(report.message.contains("Final UI report"));
}
#[test]
fn start_requires_registration_and_invalid_recipe_blocks_even_artifact_publication() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    assert!(h
        .session
        .send(Command::Install {
            request_id: id,
            choices: Choices {
                register_service: false,
                enable_service: false,
                start_service: true
            }
        })
        .is_err());
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(h.state.lock().unwrap().attempts, 0);
}
#[test]
fn register_only_is_incomplete_with_one_fresh_confirmation_and_no_enable_or_start() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: false,
            start_service: false,
        },
    );
    let (id, title) = h.confirmation();
    assert!(title.contains("Register"));
    assert!(h.state.lock().unwrap().applied.is_empty());
    h.session.send(Command::Approve { request_id: id }).unwrap();
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[1].state, StepState::Completed);
    assert_eq!(report.steps[2].state, StepState::Skipped);
    let s = h.state.lock().unwrap();
    assert_eq!(s.applied.len(), 1);
    assert_eq!(s.applied[0].0, Action::InstallUnit);
    assert_eq!(
        s.backend_sources,
        [PathBuf::from(
            "/isolated/package/payload/service/smith-service"
        )]
    );
    assert!(s.owners.iter().all(|owner| *owner == s.owners[0]));
}
#[test]
fn registration_and_start_are_separate_original_token_confirmations() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: true,
        },
    );
    let (register, _) = h.confirmation();
    h.session
        .send(Command::Approve {
            request_id: register,
        })
        .unwrap();
    let (enable, title) = h.confirmation();
    assert_ne!(register, enable);
    assert!(title.contains("Enable"));
    assert_eq!(h.state.lock().unwrap().applied.len(), 1);
    h.session
        .send(Command::Approve { request_id: enable })
        .unwrap();
    let (start, title) = h.confirmation();
    assert_ne!(enable, start);
    assert!(title.contains("Start"));
    assert_eq!(h.state.lock().unwrap().applied.len(), 2);
    h.session
        .send(Command::Approve { request_id: start })
        .unwrap();
    let report = h.finish();
    assert!(report.complete);
    assert!(report.steps.iter().all(|s| s.state == StepState::Completed));
    let s = h.state.lock().unwrap();
    assert_eq!(s.applied, s.tokens);
    assert_eq!(s.tokens.len(), 3);
}
#[test]
fn reload_only_and_already_running_recipes_do_not_repeat_unselected_work() {
    let mut h = Harness::new(
        |s| {
            s.facts = facts("installed", "stopped", "unavailable");
            s.facts.message = smith_installer::setup::RELOAD_MESSAGE.into();
        },
        Limits::default(),
    );
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: false,
            start_service: false,
        },
    );
    let (id, _) = h.confirmation();
    h.session.send(Command::Approve { request_id: id }).unwrap();
    assert!(!h.finish().complete);
    assert_eq!(h.state.lock().unwrap().applied[0].0, Action::InstallUnit);
    let mut h = Harness::new(
        |s| s.facts = facts("installed", "running", "ready"),
        Limits::default(),
    );
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: true,
        },
    );
    let report = h.finish();
    assert!(report.complete);
    assert!(h.state.lock().unwrap().applied.is_empty());
    assert!(report.steps[1..]
        .iter()
        .all(|s| s.state == StepState::Skipped));
}
#[test]
fn later_decline_preserves_artifacts_and_never_applies_that_core_action() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: true,
        },
    );
    let (id, _) = h.confirmation();
    h.session.send(Command::Decline { request_id: id }).unwrap();
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Completed);
    assert_eq!(report.steps[1].state, StepState::Skipped);
    assert!(h.state.lock().unwrap().installed);
    assert!(h.state.lock().unwrap().applied.is_empty());
}
#[test]
fn wrong_stale_and_duplicate_commands_are_terminal_and_never_approve_a_pending_core_action() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let old = h.review();
    h.session.send(Command::RefreshReview).unwrap();
    let new = h.review();
    assert_ne!(old, new);
    assert!(h
        .session
        .send(Command::Install {
            request_id: old,
            choices: Choices::default()
        })
        .is_err());
    assert!(!h.finish().complete);
    assert_eq!(h.state.lock().unwrap().attempts, 0);
    let mut h = Harness::new(|_| {}, Limits::default());
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: false,
        },
    );
    let (confirm, _) = h.confirmation();
    assert!(h
        .session
        .send(Command::Approve {
            request_id: confirm + 1
        })
        .is_err());
    assert!(!h.finish().complete);
    assert!(h.state.lock().unwrap().applied.is_empty());
    let mut h = Harness::new(|s| s.hold_install = true, Limits::default());
    let id = h.review();
    h.install(id, Choices::default());
    assert!(h
        .session
        .send(Command::Install {
            request_id: id,
            choices: Choices::default()
        })
        .is_err());
    assert!(!h.finish().complete);
    assert!(h.state.lock().unwrap().attempts <= 1);
}
#[test]
fn expired_original_core_token_is_never_reprepared_or_applied() {
    let mut h = Harness::new(|s| s.expired = true, Limits::default());
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: false,
        },
    );
    let report = h.finish();
    assert!(!report.complete);
    assert!(report.message.contains("expired"));
    let s = h.state.lock().unwrap();
    assert_eq!(s.tokens.len(), 1);
    assert!(s.applied.is_empty());
}
#[test]
fn pending_core_input_timeout_does_not_renew_the_original_token() {
    let mut h = Harness::new(
        |_| {},
        Limits {
            pending_timeout: Duration::from_millis(150),
            ..Limits::default()
        },
    );
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: false,
        },
    );
    let _ = h.confirmation();
    let report = h.finish();
    assert!(!report.complete);
    assert!(report.message.contains("expired"));
    assert_eq!(h.state.lock().unwrap().tokens.len(), 1);
    assert!(h.state.lock().unwrap().applied.is_empty());
}
#[test]
fn explicit_refresh_recovers_preview_error_only_and_changed_review_never_installs() {
    let mut h = Harness::new(|s| s.previews_fail = 1, Limits::default());
    match event(&h.session) {
        Event::Stage { message } => assert!(message.contains("Preview unavailable")),
        _ => panic!(),
    }
    thread::sleep(Duration::from_millis(30));
    assert_eq!(h.state.lock().unwrap().preview_count, 1);
    h.session.send(Command::RefreshReview).unwrap();
    let id = h.review();
    h.state.lock().unwrap().generation += 1;
    h.install(id, Choices::default());
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Uncertain);
    assert!(!h.state.lock().unwrap().installed);
    assert_eq!(h.state.lock().unwrap().preview_count, 2);
}
#[test]
fn output_cap_and_partial_failures_do_not_claim_complete_or_construct_lifecycle_backend() {
    for large in [false, true] {
        let mut h = Harness::new(
            |s| {
                s.fail_install = !large;
                s.large_output = large;
            },
            Limits::default(),
        );
        let id = h.review();
        h.install(
            id,
            Choices {
                register_service: true,
                enable_service: true,
                start_service: true,
            },
        );
        let report = h.finish();
        assert!(!report.complete);
        assert_eq!(report.steps[0].state, StepState::Uncertain);
        assert!(report.details.iter().map(String::len).sum::<usize>() <= 64 * 1024);
        assert!(h.state.lock().unwrap().backend_sources.is_empty());
    }
}
#[test]
fn manager_failure_preserves_completed_artifacts_and_uncertain_accepted_action() {
    let mut h = Harness::new(
        |s| s.fail_action = Some(Action::InstallUnit),
        Limits::default(),
    );
    let id = h.review();
    h.install(
        id,
        Choices {
            register_service: true,
            enable_service: true,
            start_service: true,
        },
    );
    let (id, _) = h.confirmation();
    h.session.send(Command::Approve { request_id: id }).unwrap();
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Completed);
    assert_eq!(report.steps[1].state, StepState::Uncertain);
    assert_eq!(h.state.lock().unwrap().applied.len(), 1);
}
#[test]
fn event_backpressure_retains_a_failed_terminal_report_and_owned_worker() {
    let mut h = Harness::new(
        |_| {},
        Limits {
            channel_capacity: 1,
            event_timeout: Duration::from_millis(40),
            ..Limits::default()
        },
    );
    let id = h.review();
    h.install(id, Choices::default());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !h.session.is_finished() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    let report = h.finish();
    assert!(!report.complete);
    assert!(h.cancelled.load(Ordering::Acquire));
    assert!(h.state.lock().unwrap().publisher_dropped);
}
#[test]
fn disconnect_and_cancel_before_install_have_no_publication_or_backend_effects() {
    let mut h = Harness::new(|_| {}, Limits::default());
    let _ = h.review();
    h.session.disconnect_events();
    let report = h.finish();
    assert!(!report.complete);
    assert_eq!(h.state.lock().unwrap().attempts, 0);
    let mut h = Harness::new(|_| {}, Limits::default());
    let _ = h.review();
    h.session.send(Command::Cancel).unwrap();
    let report = h.finish();
    assert!(report.cancelled);
    assert!(!report.complete);
    assert_eq!(h.state.lock().unwrap().attempts, 0);
}
#[test]
fn cancel_during_publication_reaches_owned_worker_before_cancel_message_and_drop_joins() {
    let mut h = Harness::new(|s| s.hold_install = true, Limits::default());
    let id = h.review();
    h.install(id, Choices::default());
    let deadline = Instant::now() + Duration::from_secs(2);
    while h.state.lock().unwrap().attempts == 0 {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    h.session.send(Command::Cancel).unwrap();
    assert!(h.cancelled.load(Ordering::Acquire));
    let report = h.finish();
    assert!(report.cancelled);
    assert_eq!(report.steps[0].state, StepState::Uncertain);
    let h = Harness::new(|s| s.hold_install = true, Limits::default());
    let id = h.review();
    h.install(id, Choices::default());
    let state = h.state.clone();
    let cancelled = h.cancelled.clone();
    drop(h);
    assert!(cancelled.load(Ordering::Acquire));
    assert!(state.lock().unwrap().publisher_dropped);
}

// Preallocate the two real filesystem fixture leaves once. Their shared ancestor
// nlinks remain stable while these tests run in parallel; no lifetime mutex or
// production publisher check is changed to obtain a passing snapshot test.
static PRIVATE_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
fn private_root() -> &'static PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    PRIVATE_ROOT.get_or_init(|| {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("smith-wizard-{}-{stamp}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        for leaf in ["publisher", "cancel"] {
            directory(&root.join(leaf));
        }
        // SAFETY: process exit follows joined test workers; only this fresh
        // private namespace is removed, never normal HOME/XDG paths.
        assert_eq!(unsafe { libc::atexit(clean_private_root) }, 0);
        root
    })
}
extern "C" fn clean_private_root() {
    if let Some(root) = PRIVATE_ROOT.get() {
        if std::fs::remove_dir_all(root).is_err() {
            // SAFETY: a failed fixture cleanup must fail the test process.
            unsafe {
                libc::_exit(1);
            }
        }
    }
}
fn directory(path: &std::path::Path) {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}
fn file(path: &std::path::Path, bytes: &[u8], mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}
fn snapshot(base: &std::path::Path) -> std::collections::BTreeMap<PathBuf, (u32, u64, Vec<u8>)> {
    fn visit(
        path: &std::path::Path,
        entries: &mut std::collections::BTreeMap<PathBuf, (u32, u64, Vec<u8>)>,
    ) {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path).unwrap();
        let contents = if m.is_file() {
            std::fs::read(path).unwrap()
        } else if m.file_type().is_symlink() {
            std::fs::read_link(path)
                .unwrap()
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else {
            Vec::new()
        };
        entries.insert(path.into(), (m.mode() & 0o7777, m.ino(), contents));
        if m.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                visit(&entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = std::collections::BTreeMap::new();
    visit(base, &mut entries);
    entries
}
fn local_cancellation() -> Cancellation {
    let flag = Arc::new(AtomicBool::new(false));
    let observed = flag.clone();
    Cancellation::from_callbacks(
        move || observed.load(Ordering::Acquire),
        move || {
            flag.store(true, Ordering::Release);
        },
    )
}
fn real_publisher_session(bundle: PathBuf, home: PathBuf, data: PathBuf) -> Session {
    Session::spawn_with(
        bundle,
        move |client, service| {
            smith_installer::artifact::Installer::with_roots(
                Some(client),
                Some(service),
                Some(home.clone()),
                data.clone(),
            )
        },
        |_| -> Result<Box<dyn Backend>> {
            panic!("Artifact-only GUI must not construct runtime roots or a manager")
        },
        || Ok(()),
        local_cancellation(),
        Limits::default(),
    )
    .unwrap()
}
#[test]
fn real_private_publisher_preview_decline_changed_review_and_artifact_only_incomplete() {
    let base = private_root().join("publisher");
    let home = base.join("home");
    let data = base.join("data");
    let bundle = base.join("package");
    for root in [&home, &data, &bundle] {
        directory(root);
    }
    for rel in [
        "payload/service",
        "payload/client/smith_backend",
        "payload/client/lib",
        "payload/client/data/flutter_assets",
        "payload/client/share/pixmaps",
        "payload/client/share/applications",
    ] {
        directory(&bundle.join(rel));
    }
    for (name, bytes, mode) in [
        (
            "payload/service/smith-service",
            b"service-v1".as_slice(),
            0o755,
        ),
        (
            "payload/client/smith_backend/smith-service",
            b"service-v1",
            0o755,
        ),
        (
            "payload/client/smith_client",
            b"synthetic client, never executed",
            0o755,
        ),
        (
            "payload/client/lib/libflutter_linux_gtk.so",
            b"synthetic flutter",
            0o644,
        ),
        ("payload/client/lib/libapp.so", b"synthetic app", 0o644),
        ("payload/client/data/icudtl.dat", b"synthetic icu", 0o644),
        (
            "payload/client/data/flutter_assets/asset",
            b"synthetic asset",
            0o644,
        ),
        (
            "payload/client/share/pixmaps/dev.smith.smith_client.png",
            b"synthetic icon",
            0o644,
        ),
        (
            "payload/client/share/applications/dev.smith.smith_client.desktop",
            b"synthetic launcher",
            0o644,
        ),
    ] {
        file(&bundle.join(name), bytes, mode);
    }
    let before = snapshot(&base);
    let mut session = real_publisher_session(bundle.clone(), home.clone(), data.clone());
    let id = match event(&session) {
        Event::Review { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    assert_eq!(snapshot(&base), before);
    session.send(Command::Decline { request_id: id }).unwrap();
    assert!(!finish(&mut session).complete);
    assert_eq!(snapshot(&base), before);
    let mut session = real_publisher_session(bundle.clone(), home.clone(), data.clone());
    let id = match event(&session) {
        Event::Review { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    for name in [
        "payload/service/smith-service",
        "payload/client/smith_backend/smith-service",
    ] {
        file(&bundle.join(name), b"service-v2", 0o755);
    }
    let changed = snapshot(&base);
    session
        .send(Command::Install {
            request_id: id,
            choices: Choices::default(),
        })
        .unwrap();
    let report = finish(&mut session);
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Uncertain);
    assert_eq!(snapshot(&base), changed);
    let mut session = real_publisher_session(bundle, home, data.clone());
    let id = match event(&session) {
        Event::Review { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    session
        .send(Command::Install {
            request_id: id,
            choices: Choices::default(),
        })
        .unwrap();
    let report = finish(&mut session);
    assert!(!report.complete);
    assert_eq!(report.steps[0].state, StepState::Completed);
    assert_eq!(
        std::fs::read(data.join("smith-next/bin/smith-service")).unwrap(),
        b"service-v2"
    );
    assert!(!data.join("smith-next/state.json").exists());
    assert!(!data.join("systemd").exists());
}

struct PrivateManager {
    reloads: Arc<std::sync::atomic::AtomicU64>,
}
impl smith_service::Manager for PrivateManager {
    fn inspect(&self, _: &smith_service::Roots) -> Result<smith_service::ManagerFacts> {
        Ok(smith_service::ManagerFacts {
            load: "not-found".into(),
            active: "inactive".into(),
            sub: "dead".into(),
            pid: 0,
            fragment: String::new(),
            drop_ins: String::new(),
            need_reload: false,
            transient: false,
            unit_file_state: String::new(),
        })
    }
    fn reload(&self, _: &smith_service::Roots) -> Result<()> {
        self.reloads.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn start(&self, _: &smith_service::Roots) -> Result<()> {
        panic!("No service start in cancellation fixture")
    }
}
#[test]
fn composed_cancel_child() {
    let Some(base) = std::env::var_os("SMITH_WIZARD_CANCEL_ROOT") else {
        return;
    };
    let base = PathBuf::from(base);
    smith_installer::consent::install_cancellation().unwrap();
    let roots = smith_service::Roots {
        data: base.join("data"),
        config: base.join("config"),
        runtime: base.join("run"),
    };
    for root in [&roots.data, &roots.config, &roots.runtime] {
        directory(root);
    }
    directory(&roots.data.join("smith-next/bin"));
    let bundle = base.join("package");
    directory(&bundle.join("payload/service"));
    let source = bundle.join("payload/service/smith-service");
    file(&source, b"private control fixture, never executed", 0o755);
    file(
        &roots.binary(),
        b"private control fixture, never executed",
        0o755,
    );
    let state = Arc::new(Mutex::new(State::default()));
    let pub_state = state.clone();
    let reloads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let reload_worker = reloads.clone();
    let worker_roots = roots.clone();
    let mut session = Session::spawn_with(
        bundle,
        move |_, _| {
            Ok(FakePublisher {
                state: pub_state.clone(),
                cancelled: Arc::new(AtomicBool::new(false)),
                owner: thread::current().id(),
                _not_send: Rc::new(()),
            })
        },
        move |source| {
            Ok(Box::new(smith_service::Controller {
                roots: worker_roots.clone(),
                source,
                manager: PrivateManager {
                    reloads: reload_worker.clone(),
                },
            }) as Box<dyn Backend>)
        },
        || Ok(()),
        Cancellation::from_callbacks(
            smith_installer::consent::cancelled,
            smith_installer::consent::request_cancel,
        ),
        Limits::default(),
    )
    .unwrap();
    let id = match event(&session) {
        Event::Review { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    session
        .send(Command::Install {
            request_id: id,
            choices: Choices {
                register_service: true,
                enable_service: true,
                start_service: false,
            },
        })
        .unwrap();
    loop {
        if matches!(event(&session), Event::Confirmation { .. }) {
            break;
        }
    }
    session.send(Command::Cancel).unwrap();
    let report = finish(&mut session);
    assert!(report.cancelled);
    assert!(!report.complete);
    assert!(smith_installer::consent::cancelled());
    let controller = smith_service::Controller {
        roots: roots.clone(),
        source,
        manager: PrivateManager {
            reloads: reloads.clone(),
        },
    };
    let probe = controller
        .prepare(Action::InstallUnit)
        .approval_token
        .unwrap();
    let refused = controller.apply(Action::InstallUnit, &probe);
    assert!(!refused.ok);
    assert!(refused.error.unwrap().contains("cancelled"));
    assert_eq!(reloads.load(Ordering::Relaxed), 0);
    assert!(!roots.unit().exists());
}
#[test]
fn production_cancel_request_reaches_core_and_joins_without_applying_the_pending_action() {
    use std::process::{Command as Process, Stdio};
    let mut child = Process::new(std::env::current_exe().unwrap())
        .args(["--exact", "composed_cancel_child", "--nocapture"])
        .env("SMITH_WIZARD_CANCEL_ROOT", private_root().join("cancel"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Composed cancellation fixture exceeded budget");
        }
        thread::sleep(Duration::from_millis(2));
    };
    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut stderr).unwrap();
    assert!(status.success(), "{stderr}");
}
