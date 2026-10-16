//! Approved model-wizard slice: typed fakes only, no host manager or weights.
use serde_json::{json, Value};
use smith_installer::{
    model::{ProgressOutput, Provider},
    setup::{Backend, Native},
    wizard::{Cancellation, Choices, Command, Event, Limits, PreparedReview, Publisher, Session},
    Result,
};
use smith_service::{
    app::{AppReply, AppRequest},
    providers::{Inventory, Model, Observation as ProviderObservation, Observations},
    runner, Action, Observation,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default)]
struct State {
    calls: Vec<String>,
    selected: Option<String>,
    installed: Vec<String>,
    revision: u64,
    operation: Option<(String, Value)>,
    mutation: Option<fn(&str, &mut Value)>,
    pulls: usize,
    publications: usize,
    preview_error: bool,
}
struct Fake(Arc<Mutex<State>>);
fn inventory(names: &[String]) -> Value {
    serde_json::to_value(Inventory {
        models: names
            .iter()
            .map(|name| Model {
                name: name.clone(),
                digest: runner::CATALOG
                    .iter()
                    .find(|e| e.name == name)
                    .unwrap()
                    .sha256
                    .into(),
            })
            .collect(),
        voices: vec![],
        providers: Observations {
            llm: ProviderObservation {
                ready: true,
                error: None,
            },
            ..Observations::default()
        },
    })
    .unwrap()
}
impl Backend for Fake {
    fn inspect(&mut self) -> Result<Observation> {
        Ok(serde_json::from_value(json!({"schema_version":2,"service":"smith-next","ok":true,"binary_state":"installed","unit_state":"installed","enablement_state":"enabled","runtime_state":"running","bridge_state":"ready","setup_complete":false,"message":"Observed","error":null,"effects":{"binary_path":"/fixture/bin","unit_path":"/fixture/unit","state_path":"/fixture/state","socket_path":"/fixture/socket","description":"Fixture only"}})).unwrap())
    }
    fn prepare(&mut self, _: Action) -> Result<Observation> {
        panic!("No live lifecycle")
    }
    fn apply(&mut self, _: Action, _: &str) -> Result<Observation> {
        panic!("No live lifecycle")
    }
    fn app(&mut self, req: AppRequest) -> Result<AppReply> {
        let mut s = self.0.lock().unwrap();
        s.calls.push(req.op.clone());
        let mut result = match req.op.as_str() {
            "state" => {
                json!({"config":{"model":s.selected,"voice":null,"revision":s.revision},"history":[],"active":null,"providers":{},"retention":{}})
            }
            "models" | "verify_model" | "remove_model" => {
                s.operation = Some((req.op.clone(), req.args.clone()));
                json!({"request_id":req.args["request_id"],"status":"running","phase":match req.op.as_str() { "models" => "inventory", "verify_model" => "verification", _ => "removal" },"config_revision":s.revision})
            }
            "configure_checked" => {
                assert_eq!(req.args["expected_revision"], s.revision);
                s.revision += 1;
                s.selected = Some(req.args["model"].as_str().unwrap().into());
                json!({"model":s.selected,"voice":null,"revision":s.revision})
            }
            "poll" => {
                let (kind, args) = s.operation.clone().unwrap();
                assert_eq!(args["request_id"], req.args["request_id"]);
                let result = match kind.as_str() {
                    "models" => inventory(&s.installed),
                    "verify_model" => {
                        json!({"model":args["model"],"sha256":args["sha256"],"verified":true,"assistant_text":"Hello."})
                    }
                    "remove_model" => {
                        s.installed
                            .retain(|m| Some(m.as_str()) != args["model"].as_str());
                        json!({"model":args["model"],"sha256":args["sha256"],"removed":true})
                    }
                    _ => panic!(),
                };
                json!({"request_id":req.args["request_id"],"kind":kind,"config_revision":s.revision,"status":"succeeded","phase":"complete","result":result,"error":null})
            }
            _ => panic!("Unexpected op {}", req.op),
        };
        if let Some(mutate) = s.mutation {
            mutate(&req.op, &mut result);
        }
        Ok(AppReply {
            schema_version: 1,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: req.call_id,
            instance_id: "a".repeat(32),
            pid: 123,
            ok: true,
            result: Some(result),
            error: None,
        })
    }
}
impl Provider for Fake {
    fn tags(&mut self) -> Result<Vec<String>> {
        panic!("Use service inventory")
    }
    fn pull(&mut self, name: &str, output: &mut dyn ProgressOutput) -> Result<()> {
        output.write_until(
            b"Downloaded 123/123 bytes\n",
            Instant::now() + Duration::from_secs(1),
            || false,
        )?;
        let mut s = self.0.lock().unwrap();
        s.pulls += 1;
        s.installed.push(name.into());
        Ok(())
    }
    fn inference(&mut self, _: &str) -> Result<String> {
        panic!("Check must use service, not direct inference")
    }
}
struct Review;
impl PreparedReview for Review {
    fn description(&self) -> &str {
        "Exact fixture artifacts"
    }
    fn review_id(&self) -> &str {
        "fixture-review"
    }
}
impl Publisher for Fake {
    type Prepared = Review;
    fn prepare_review(&mut self) -> Result<Review> {
        if self.0.lock().unwrap().preview_error {
            return Err("Existing service differs; upgrades require separate approval".into());
        }
        Ok(Review)
    }
    fn install_reviewed(
        &mut self,
        _: Review,
        id: &str,
        _: &mut dyn std::io::Write,
    ) -> Result<smith_installer::artifact::Outcome> {
        assert_eq!(id, "fixture-review");
        self.0.lock().unwrap().publications += 1;
        Ok(smith_installer::artifact::Outcome::Installed)
    }
}
fn session(s: Arc<Mutex<State>>) -> Session {
    session_with_cancellation(s, Cancellation::from_callbacks(|| false, || {}))
}
fn session_with_cancellation(s: Arc<Mutex<State>>, cancellation: Cancellation) -> Session {
    let p = s.clone();
    let b = s.clone();
    Session::spawn_product_with(
        "/fixture/package".into(),
        move |_, _| Ok(Fake(p.clone())),
        move |_| Ok(Box::new(Fake(b.clone())) as Box<dyn Backend>),
        || Ok(()),
        cancellation,
        Limits::default(),
        Some(Box::new(move || {
            Ok(Box::new(Fake(s.clone())) as Box<dyn Provider>)
        })),
    )
    .unwrap()
}
fn joined(session: &mut Session) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !session.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    session.join().unwrap();
}
fn event(session: &Session) -> Event {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(e) = session.try_event().unwrap() {
            return e;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn service_check_strict_identity_revision_result_and_no_resend() {
    for mutation in [
        None,
        Some(
            (|op: &str, v: &mut Value| {
                if op == "poll" {
                    v["config_revision"] = json!(99);
                }
            }) as fn(&str, &mut Value),
        ),
        Some(|op: &str, v: &mut Value| {
            if op == "poll" {
                v["result"]["sha256"] = json!("0".repeat(64));
            }
        }),
        Some(|op: &str, v: &mut Value| {
            if op == "poll" {
                v["result"]["assistant_text"] = json!(" ");
            }
        }),
        Some(|op: &str, v: &mut Value| {
            if op == "poll" {
                v["kind"] = json!("submit");
            }
        }),
    ] {
        let s = Arc::new(Mutex::new(State {
            mutation,
            ..State::default()
        }));
        let mut native = Native::new(Box::new(Fake(s.clone())));
        assert_eq!(
            native
                .model_operation_with_budget(
                    "verify_model",
                    runner::DEFAULT_MODEL,
                    0,
                    &|| false,
                    Duration::from_secs(1),
                    Duration::ZERO
                )
                .is_ok(),
            mutation.is_none()
        );
        let calls = &s.lock().unwrap().calls;
        assert_eq!(calls.iter().filter(|c| *c == "verify_model").count(), 1);
        assert!(!calls.iter().any(|c| c == "submit"));
    }
}
#[test]
fn observation_stop_never_readmits_accepted_work() {
    let s = Arc::new(Mutex::new(State::default()));
    let mut native = Native::new(Box::new(Fake(s.clone())));
    let error = native
        .model_operation_with_budget(
            "verify_model",
            runner::DEFAULT_MODEL,
            0,
            &|| s.lock().unwrap().calls.iter().any(|c| c == "verify_model"),
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .unwrap_err();
    assert!(error.contains("may continue"));
    assert_eq!(s.lock().unwrap().calls, vec!["verify_model"]);
}
#[test]
fn install_download_preference_check_and_repeated_manage_bypass_publication() {
    let s = Arc::new(Mutex::new(State::default()));
    for pass in 0..3 {
        let mut session = session(s.clone());
        let Event::Review { request_id, .. } = event(&session) else {
            panic!()
        };
        assert_eq!(s.lock().unwrap().publications, usize::from(pass > 0));
        if pass == 0 {
            session
                .send(Command::Install {
                    request_id,
                    choices: Choices::smith_install(),
                })
                .unwrap();
        } else {
            session.send(Command::ManageModels).unwrap();
        }
        let mut confirmations = vec![];
        loop {
            match event(&session) {
                Event::Models {
                    request_id,
                    selected,
                    ..
                } => {
                    assert_eq!(
                        selected,
                        if pass == 0 {
                            None
                        } else {
                            Some(runner::DEFAULT_MODEL.into())
                        }
                    );
                    session
                        .send(Command::ModelAction {
                            request_id,
                            model: runner::DEFAULT_MODEL.into(),
                            remove: false,
                        })
                        .unwrap();
                }
                Event::Confirmation {
                    request_id, title, ..
                } => {
                    confirmations.push(title);
                    session.send(Command::Approve { request_id }).unwrap();
                }
                Event::Finished { report } => {
                    assert!(report.complete, "{}", report.message);
                    if pass == 0 {
                        assert_eq!(report.scope, smith_installer::wizard::Scope::Installation);
                        assert!(report.message.starts_with("Installation complete:"));
                    } else {
                        assert_eq!(report.scope, smith_installer::wizard::Scope::ModelCheck);
                        assert!(report.message.starts_with("Model check complete:"));
                        assert!(!report.message.contains("paired"));
                        assert!(report.steps[..3]
                            .iter()
                            .all(|s| s.state == smith_installer::wizard::StepState::Skipped));
                    }
                    break;
                }
                _ => {}
            }
        }
        joined(&mut session);
        assert_eq!(confirmations.len(), if pass == 0 { 3 } else { 1 });
    }
    let s = s.lock().unwrap();
    assert_eq!(s.publications, 1);
    assert_eq!(s.pulls, 1);
    assert_eq!(
        s.calls.iter().filter(|c| *c == "configure_checked").count(),
        1
    );
    assert_eq!(s.calls.iter().filter(|c| *c == "verify_model").count(), 3);
}
#[test]
fn management_removal_is_separate_destructive_service_action() {
    let remove = runner::CATALOG[1].name;
    let s = Arc::new(Mutex::new(State {
        selected: Some(runner::DEFAULT_MODEL.into()),
        installed: vec![runner::DEFAULT_MODEL.into(), remove.into()],
        ..State::default()
    }));
    let mut session = session(s.clone());
    assert!(matches!(event(&session), Event::Review { .. }));
    session.send(Command::ManageModels).unwrap();
    loop {
        match event(&session) {
            Event::Models { request_id, .. } => session
                .send(Command::ModelAction {
                    request_id,
                    model: remove.into(),
                    remove: true,
                })
                .unwrap(),
            Event::Confirmation {
                request_id,
                title,
                effects,
            } => {
                assert!(title.contains("destructive"));
                assert!(effects.join(" ").contains(remove));
                session.send(Command::Approve { request_id }).unwrap();
            }
            Event::Finished { report } => {
                assert!(report.complete, "{}", report.message);
                assert_eq!(report.scope, smith_installer::wizard::Scope::ModelRemoval);
                assert_eq!(report.scope.completion_heading(), "Model removal complete");
                assert!(report.message.starts_with("Model removal complete;"));
                assert!(report.steps[..3]
                    .iter()
                    .all(|s| s.state == smith_installer::wizard::StepState::Skipped));
                break;
            }
            _ => {}
        }
    }
    joined(&mut session);
    let s = s.lock().unwrap();
    assert_eq!(s.publications, 0);
    assert_eq!(s.pulls, 0);
    assert_eq!(s.selected.as_deref(), Some(runner::DEFAULT_MODEL));
    assert!(s.calls.iter().any(|c| c == "remove_model"));
    assert!(!s
        .calls
        .iter()
        .any(|c| c == "verify_model" || c == "configure_checked"));
}
#[test]
fn catalog_bound_arguments_reject_stale_shapes_and_wrong_sha() {
    let e = &runner::CATALOG[0];
    let valid =
        json!({"request_id":"b".repeat(32),"model":e.name,"sha256":e.sha256,"expected_revision":0});
    smith_installer::setup::validate_args("verify_model", &valid).unwrap();
    for key in ["request_id", "model", "sha256", "expected_revision"] {
        let mut v = valid.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(smith_installer::setup::validate_args("remove_model", &v).is_err());
    }
    assert_eq!(e.name, runner::DEFAULT_MODEL);
    assert_eq!(e.size, 9001752960);
}

#[test]
fn opening_and_declining_download_have_no_model_effects() {
    let s = Arc::new(Mutex::new(State::default()));
    let mut first = session(s.clone());
    assert!(matches!(event(&first), Event::Review { .. }));
    first.send(Command::Cancel).unwrap();
    joined(&mut first);
    assert!(s.lock().unwrap().calls.is_empty());
    let mut second = session(s.clone());
    assert!(matches!(event(&second), Event::Review { .. }));
    second.send(Command::ManageModels).unwrap();
    loop {
        match event(&second) {
            Event::Models { request_id, .. } => second
                .send(Command::ModelAction {
                    request_id,
                    model: runner::DEFAULT_MODEL.into(),
                    remove: false,
                })
                .unwrap(),
            Event::Confirmation {
                request_id, title, ..
            } => {
                assert!(title.contains("Download"));
                second.send(Command::Decline { request_id }).unwrap();
            }
            Event::Finished { report } => {
                assert!(!report.complete);
                assert!(report.message.contains("incomplete"));
                break;
            }
            _ => {}
        }
    }
    joined(&mut second);
    let s = s.lock().unwrap();
    assert_eq!(s.pulls, 0);
    assert_eq!(s.publications, 0);
    assert!(!s
        .calls
        .iter()
        .any(|c| c == "configure_checked" || c == "verify_model"));
}

#[test]
fn selected_removal_refused_without_service_mutation_or_stops() {
    let s = Arc::new(Mutex::new(State {
        selected: Some(runner::DEFAULT_MODEL.into()),
        installed: vec![runner::DEFAULT_MODEL.into()],
        ..State::default()
    }));
    let mut session = session(s.clone());
    assert!(matches!(event(&session), Event::Review { .. }));
    session.send(Command::ManageModels).unwrap();
    loop {
        match event(&session) {
            Event::Models { request_id, .. } => session
                .send(Command::ModelAction {
                    request_id,
                    model: runner::DEFAULT_MODEL.into(),
                    remove: true,
                })
                .unwrap(),
            Event::Confirmation { .. } => panic!("Selected model removal must be refused"),
            Event::Finished { report } => {
                assert!(!report.complete);
                assert!(report.message.contains("Selected model"));
                break;
            }
            _ => {}
        }
    }
    joined(&mut session);
    assert!(!s.lock().unwrap().calls.iter().any(|c| c == "remove_model"));
}

#[test]
fn repeated_model_only_checks_report_no_artifact_or_lifecycle_verification() {
    let s = Arc::new(Mutex::new(State {
        selected: Some(runner::DEFAULT_MODEL.into()),
        installed: vec![runner::DEFAULT_MODEL.into()],
        ..State::default()
    }));
    for _ in 0..3 {
        let mut session = session(s.clone());
        assert!(matches!(event(&session), Event::Review { .. }));
        session.send(Command::ManageModels).unwrap();
        loop {
            match event(&session) {
                Event::Models { request_id, .. } => session
                    .send(Command::ModelAction {
                        request_id,
                        model: runner::DEFAULT_MODEL.into(),
                        remove: false,
                    })
                    .unwrap(),
                Event::Confirmation {
                    request_id, title, ..
                } => {
                    assert!(title.starts_with("Check model"));
                    session.send(Command::Approve { request_id }).unwrap();
                }
                Event::Finished { report } => {
                    assert!(report.complete);
                    assert_eq!(report.scope, smith_installer::wizard::Scope::ModelCheck);
                    assert_eq!(report.scope.completion_heading(), "Model check complete");
                    assert_eq!(report.message, "Model check complete: service-backed model inference verified; no artifact publication or service registration/enable/start performed by this management session");
                    assert!(report.steps[..3]
                        .iter()
                        .all(|s| s.state == smith_installer::wizard::StepState::Skipped));
                    break;
                }
                _ => {}
            }
        }
        joined(&mut session);
        assert_eq!(s.lock().unwrap().publications, 0);
    }
    assert_eq!(
        s.lock()
            .unwrap()
            .calls
            .iter()
            .filter(|c| *c == "verify_model")
            .count(),
        3
    );
}

#[test]
fn shared_sticky_cancellation_is_never_cleared_for_a_new_session() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let sticky = Arc::new(AtomicBool::new(false));
    let observed = sticky.clone();
    let requested = sticky.clone();
    let cancellation = Cancellation::from_callbacks(
        move || observed.load(Ordering::Acquire),
        move || {
            requested.store(true, Ordering::Release);
        },
    );
    let s = Arc::new(Mutex::new(State::default()));
    let mut first = session_with_cancellation(s.clone(), cancellation.clone());
    assert!(matches!(event(&first), Event::Review { .. }));
    first.send(Command::Cancel).unwrap();
    joined(&mut first);
    assert!(sticky.load(Ordering::Acquire));
    assert!(first.terminal_report().unwrap().unwrap().cancelled);
    let mut next = session_with_cancellation(s.clone(), cancellation);
    let Event::Finished { report } = event(&next) else {
        panic!("Sticky cancellation must block a new review")
    };
    assert!(report.cancelled);
    assert!(!report.complete);
    joined(&mut next);
    assert!(sticky.load(Ordering::Acquire));
    assert_eq!(s.lock().unwrap().publications, 0);
    assert!(s.lock().unwrap().calls.is_empty());
}

#[test]
fn management_is_independent_of_a_blocked_artifact_review() {
    let state = Arc::new(Mutex::new(State {
        preview_error: true,
        selected: Some(runner::DEFAULT_MODEL.into()),
        installed: vec![runner::DEFAULT_MODEL.into()],
        ..State::default()
    }));
    let mut session = session(state.clone());
    assert!(
        matches!(event(&session), Event::Stage { message } if message.contains("Existing service differs"))
    );
    session.send(Command::ManageModels).unwrap();
    let mut saw_models = false;
    loop {
        match event(&session) {
            Event::Models {
                request_id,
                selected,
                ..
            } => {
                saw_models = true;
                assert_eq!(selected.as_deref(), Some(runner::DEFAULT_MODEL));
                session
                    .send(Command::ModelAction {
                        request_id,
                        model: runner::DEFAULT_MODEL.into(),
                        remove: false,
                    })
                    .unwrap();
            }
            Event::Confirmation {
                request_id, title, ..
            } => {
                assert_eq!(title, "Check model through Smith service");
                session.send(Command::Approve { request_id }).unwrap();
            }
            Event::Finished { report } => {
                assert!(report.complete, "{}", report.message);
                assert_eq!(report.scope, smith_installer::wizard::Scope::ModelCheck);
                break;
            }
            _ => {}
        }
    }
    joined(&mut session);
    assert!(saw_models);
    let state = state.lock().unwrap();
    assert_eq!(state.publications, 0);
    assert_eq!(state.pulls, 0);
    assert!(state.calls.iter().any(|op| op == "verify_model"));
}

#[test]
fn management_remains_available_after_artifact_review_expires() {
    let state = Arc::new(Mutex::new(State {
        selected: Some(runner::DEFAULT_MODEL.into()),
        installed: vec![runner::DEFAULT_MODEL.into()],
        ..State::default()
    }));
    let p = state.clone();
    let b = state.clone();
    let provider = state.clone();
    let mut session = Session::spawn_product_with(
        "/fixture/package".into(),
        move |_, _| Ok(Fake(p.clone())),
        move |_| Ok(Box::new(Fake(b.clone())) as Box<dyn Backend>),
        || Ok(()),
        Cancellation::from_callbacks(|| false, || {}),
        Limits {
            pending_timeout: Duration::from_millis(20),
            ..Limits::default()
        },
        Some(Box::new(move || {
            Ok(Box::new(Fake(provider.clone())) as Box<dyn Provider>)
        })),
    )
    .unwrap();
    assert!(matches!(event(&session), Event::Review { .. }));
    assert!(matches!(event(&session), Event::Stage { message } if message.contains("expired")));
    session.send(Command::ManageModels).unwrap();
    loop {
        match event(&session) {
            Event::Models { selected, .. } => {
                assert_eq!(selected.as_deref(), Some(runner::DEFAULT_MODEL));
                break;
            }
            Event::Stage { .. } => {}
            other => panic!("Unexpected event {other:?}"),
        }
    }
    session.send(Command::Cancel).unwrap();
    joined(&mut session);
    let state = state.lock().unwrap();
    assert_eq!(state.publications, 0);
    assert_eq!(state.pulls, 0);
    assert!(state
        .calls
        .iter()
        .all(|op| op == "state" || op == "models" || op == "poll"));
}
