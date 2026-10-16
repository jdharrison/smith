//! Native replacements for the archived recipient wrapper invariants. No normal
//! user manager, provider, home publication, inference or network is exercised.
use serde_json::{json, Value};
use smith_installer::artifact::Outcome;
use smith_installer::model::{ProgressOutput, Provider, VectorProgress};
use smith_installer::setup::{self, Backend, Factories, Native, Options, Publisher};
use smith_installer::{cli, consent, Result};
use smith_service::app::{AppReply, AppRequest};
use smith_service::providers::{
    Inventory, Model, Observation as ProviderObservation, Observations,
};
use smith_service::{Action, Controller, Manager, ManagerFacts, Observation, Roots};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MODEL: &str = "tiny:exact";
const INSTANCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TOKEN: &str = "1700000000.cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn core(unit: &str, runtime: &str, bridge: &str) -> Observation {
    let enablement = if unit == "missing" {
        "missing"
    } else if runtime == "running" {
        "enabled"
    } else {
        "disabled"
    };
    serde_json::from_value(json!({
        "schema_version":2,"service":"smith-next","ok":true,"binary_state":"installed",
        "unit_state":unit,"enablement_state":enablement,"runtime_state":runtime,"bridge_state":bridge,"setup_complete":false,
        "message":"Observed","error":null,
        "effects":{"binary_path":"/managed/bin","unit_path":"/managed/unit",
            "state_path":"/managed/state","socket_path":"/managed/socket","description":"Exact isolated effects"}
    })).unwrap()
}
fn inventory(names: &[String]) -> Value {
    // Serialize the real public wire types, especially STT's required nullable
    // model field. A generic fake {ready,error} schema would miss this contract.
    serde_json::to_value(Inventory {
        models: names
            .iter()
            .map(|name| Model {
                name: name.clone(),
                digest: "b".repeat(64),
            })
            .collect(),
        voices: Vec::new(),
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

struct State {
    calls: Vec<String>,
    facts: Observation,
    published: bool,
    publisher_calls: Vec<bool>,
    backend_sources: Vec<PathBuf>,
    provider_count: usize,
    provider_calls: Vec<String>,
    inventories: VecDeque<Vec<String>>,
    current_names: Vec<String>,
    configured: Option<Value>,
    pull_error: Option<String>,
    preflight_error: Option<String>,
    mutation: Option<fn(&str, &mut Value)>,
    envelope_mutation: Option<fn(&mut AppReply)>,
    all_running: bool,
    applied: Vec<(Action, String)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            facts: core("installed", "running", "ready"),
            published: false,
            publisher_calls: Vec::new(),
            backend_sources: Vec::new(),
            provider_count: 0,
            provider_calls: Vec::new(),
            inventories: VecDeque::from([vec![MODEL.into()]]),
            current_names: Vec::new(),
            configured: None,
            pull_error: None,
            preflight_error: None,
            mutation: None,
            envelope_mutation: None,
            all_running: false,
            applied: Vec::new(),
        }
    }
}
#[derive(Clone)]
struct FakePublisher(Rc<RefCell<State>>);
impl Publisher for FakePublisher {
    fn run(
        &mut self,
        dry: bool,
        confirm: &mut dyn FnMut(&str) -> Result<String>,
    ) -> Result<Outcome> {
        self.0.borrow_mut().publisher_calls.push(dry);
        if dry {
            return Ok(Outcome::DryRun);
        }
        if !consent::matches(&confirm("Publish exact artifacts? [y/N] ")?, "y")? {
            return Ok(Outcome::Declined);
        }
        self.0.borrow_mut().published = true;
        Ok(Outcome::Installed)
    }
}
#[derive(Clone)]
struct FakeBackend(Rc<RefCell<State>>);
impl Backend for FakeBackend {
    fn inspect(&mut self) -> Result<Observation> {
        let mut state = self.0.borrow_mut();
        state.calls.push("inspect".into());
        Ok(state.facts.clone())
    }
    fn prepare(&mut self, action: Action) -> Result<Observation> {
        let mut state = self.0.borrow_mut();
        state.calls.push(format!("prepare:{action:?}"));
        let mut value = state.facts.clone();
        value.approval_token = Some(TOKEN.into());
        Ok(value)
    }
    fn apply(&mut self, action: Action, approval: &str) -> Result<Observation> {
        let mut state = self.0.borrow_mut();
        state.calls.push(format!("apply:{action:?}"));
        state.applied.push((action, approval.into()));
        state.facts = match action {
            Action::InstallUnit => core("installed", "stopped", "unavailable"),
            Action::Enable => {
                let mut facts = core("installed", "stopped", "unavailable");
                facts.enablement_state = "enabled".into();
                facts
            }
            Action::Start => core("installed", "running", "ready"),
            _ => panic!("Unexpected installer action"),
        };
        Ok(state.facts.clone())
    }
    fn app(&mut self, request: AppRequest) -> Result<AppReply> {
        assert_eq!(request.schema_version, 1);
        assert_eq!(request.service, "smith-next");
        assert_eq!(request.protocol, "app");
        assert!(smith_service::app::valid_id(&request.call_id));
        for field in ["request_id", "instance_id"] {
            if let Some(value) = request.args.get(field) {
                assert!(smith_service::app::valid_id(value.as_str().unwrap()));
            }
        }
        let mut state = self.0.borrow_mut();
        state.calls.push(request.op.clone());
        let mut result = match request.op.as_str() {
            "models" => {
                state.current_names = state
                    .inventories
                    .pop_front()
                    .unwrap_or_else(|| state.current_names.clone());
                json!({"request_id":request.args["request_id"],"status":"running","phase":"inventory","config_revision":0})
            }
            "poll" => {
                if state.all_running {
                    json!({"request_id":request.args["request_id"],"kind":"models","status":"running",
                        "phase":"inventory","config_revision":0,"result":null,"error":null})
                } else {
                    json!({"request_id":request.args["request_id"],"kind":"models","status":"succeeded",
                        "phase":"complete","config_revision":0,"result":inventory(&state.current_names),"error":null})
                }
            }
            "configure" => {
                state.configured = Some(request.args.clone());
                json!({"model":request.args["model"],"voice":null,"revision":1})
            }
            "state" => json!({}),
            _ => panic!("Unexpected app operation; installer must not submit chat or speech"),
        };
        if let Some(mutate) = state.mutation {
            mutate(&request.op, &mut result);
        }
        let mut reply = AppReply::from_result(INSTANCE, &request.call_id, Ok(result));
        if let Some(mutate) = state.envelope_mutation {
            mutate(&mut reply);
        }
        Ok(reply)
    }
}
struct FakeProvider(Rc<RefCell<State>>);
impl Provider for FakeProvider {
    fn tags(&mut self) -> Result<Vec<String>> {
        self.0.borrow_mut().provider_calls.push("tags".into());
        Ok(vec![MODEL.into()])
    }
    fn pull(&mut self, model: &str, output: &mut dyn ProgressOutput) -> Result<()> {
        let mut state = self.0.borrow_mut();
        state.provider_calls.push(format!("pull:{model}"));
        output.write_until(
            b"Fake progress, no network\n",
            Instant::now() + Duration::from_secs(1),
            consent::cancelled,
        )?;
        match &state.pull_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    fn inference(&mut self, model: &str) -> Result<String> {
        self.0
            .borrow_mut()
            .provider_calls
            .push(format!("inference:{model}"));
        Ok("OK".into())
    }
}
struct Harness {
    state: Rc<RefCell<State>>,
    answers: VecDeque<Result<String>>,
    options: Options,
    prompts: Vec<String>,
    output: VectorProgress,
}
impl Harness {
    fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(State::default())),
            answers: VecDeque::from(vec![Ok("y".into()); 12]),
            options: Options {
                bundle: PathBuf::from("/isolated/package"),
                model: Some(MODEL.into()),
                pull_model: false,
                verify_inference: false,
                dry_run: false,
            },
            prompts: Vec::new(),
            output: VectorProgress::new(),
        }
    }
    fn answers(&mut self, answers: &[&str]) {
        self.answers = answers.iter().map(|s| Ok((*s).into())).collect();
    }
    fn run(&mut self) -> Result<Outcome> {
        let mut output = std::mem::take(&mut self.output);
        let result = self.run_to(&mut output);
        self.output = output;
        result
    }
    fn run_to(&mut self, output: &mut dyn ProgressOutput) -> Result<Outcome> {
        let state = self.state.clone();
        let mut preflight = || state.borrow().preflight_error.clone().map_or(Ok(()), Err);
        let mut publisher = |client: PathBuf, service: PathBuf| {
            assert_eq!(client, Path::new("/isolated/package/payload/client"));
            assert_eq!(
                service,
                Path::new("/isolated/package/payload/service/smith-service")
            );
            Ok(Box::new(FakePublisher(state.clone())) as Box<dyn Publisher>)
        };
        let mut backend = |source| {
            state.borrow_mut().backend_sources.push(source);
            Ok(Box::new(FakeBackend(state.clone())) as Box<dyn Backend>)
        };
        let mut provider = || {
            state.borrow_mut().provider_count += 1;
            Ok(Box::new(FakeProvider(state.clone())) as Box<dyn Provider>)
        };
        let answers = &mut self.answers;
        let prompts = &mut self.prompts;
        let mut confirm = |prompt: &str| {
            prompts.push(prompt.into());
            answers.pop_front().expect("Unexpected consent prompt")
        };
        setup::run_with(
            &self.options,
            &mut Factories {
                preflight: &mut preflight,
                publisher: &mut publisher,
                backend: &mut backend,
                provider: &mut provider,
            },
            &mut confirm,
            output,
        )
    }
    fn text(&self) -> String {
        String::from_utf8(self.output.as_bytes().to_vec()).unwrap()
    }
}

#[test]
fn partial_publication_failure_reports_accepted_uncertain_effects_without_runtime_setup() {
    struct PartialPublisher;
    impl Publisher for PartialPublisher {
        fn run(
            &mut self,
            _: bool,
            confirm: &mut dyn FnMut(&str) -> Result<String>,
        ) -> Result<Outcome> {
            assert_eq!(confirm("Publish exact artifacts?")?, "y");
            Err("Publication failed after an accepted partial effect".into())
        }
    }
    let h = Harness::new();
    let mut output = VectorProgress::new();
    let mut preflight = || Ok(());
    let mut publisher =
        |_: PathBuf, _: PathBuf| Ok(Box::new(PartialPublisher) as Box<dyn Publisher>);
    let mut backend = |_: PathBuf| -> Result<Box<dyn Backend>> {
        panic!("Runtime construction after publication failure")
    };
    let mut provider = || -> Result<Box<dyn Provider>> {
        panic!("Provider construction after publication failure")
    };
    let result = setup::run_with(
        &h.options,
        &mut Factories {
            preflight: &mut preflight,
            publisher: &mut publisher,
            backend: &mut backend,
            provider: &mut provider,
        },
        &mut |_| Ok("y".into()),
        &mut output,
    );
    assert!(result.is_err());
    let text = String::from_utf8(output.into_bytes()).unwrap();
    assert!(text.contains("Artifact publication accepted; outcome may be partial"));
    assert!(!text.contains("Artifact publication completed"));
    assert!(text.contains("not rolled back"));
}
#[test]
fn initial_decline_and_dry_run_never_construct_runtime_or_provider() {
    for dry in [false, true] {
        let mut h = Harness::new();
        h.options.dry_run = dry;
        h.options.pull_model = true;
        h.options.verify_inference = true;
        h.answers(&["n"]);
        assert_eq!(
            h.run().unwrap(),
            if dry {
                Outcome::DryRun
            } else {
                Outcome::Declined
            }
        );
        let state = h.state.borrow();
        assert!(!state.published);
        assert!(state.backend_sources.is_empty());
        assert!(state.calls.is_empty());
        assert_eq!(state.provider_count, 0);
        assert_eq!(h.prompts.len(), usize::from(!dry));
        assert!(!h.text().contains("Accepted/completed"));
    }
}
#[test]
fn unsupported_prerequisite_and_semantics_stop_before_publisher() {
    let mut h = Harness::new();
    h.state.borrow_mut().preflight_error = Some("Unsupported host".into());
    assert!(h.run().is_err());
    assert!(h.state.borrow().publisher_calls.is_empty());
    assert!(h.text().contains("no sudo/OS provisioning"));
    for (model, pull, verify) in [
        (None, true, false),
        (None, false, true),
        (Some("tiny"), false, false),
        (Some("tiny:cloud"), false, false),
        (Some("http://host/model:tag"), false, false),
    ] {
        let mut h = Harness::new();
        h.options.model = model.map(str::to_owned);
        h.options.pull_model = pull;
        h.options.verify_inference = verify;
        assert!(h.run().is_err());
        assert!(h.state.borrow().publisher_calls.is_empty());
    }
}
#[test]
fn exact_registration_and_start_prepare_confirm_apply_inspect_are_separate() {
    let mut h = Harness::new();
    h.state.borrow_mut().facts = core("missing", "stopped", "unavailable");
    assert_eq!(h.run().unwrap(), Outcome::Installed);
    let state = h.state.borrow();
    assert_eq!(
        &state.calls[..10],
        [
            "inspect",
            "prepare:InstallUnit",
            "apply:InstallUnit",
            "inspect",
            "prepare:Enable",
            "apply:Enable",
            "inspect",
            "prepare:Start",
            "apply:Start",
            "inspect"
        ]
    );
    assert_eq!(
        state.applied,
        [
            (Action::InstallUnit, TOKEN.into()),
            (Action::Enable, TOKEN.into()),
            (Action::Start, TOKEN.into())
        ]
    );
    assert_eq!(
        state.backend_sources,
        [PathBuf::from(
            "/isolated/package/payload/service/smith-service"
        )]
    );
    assert_eq!(state.configured, Some(json!({"model":MODEL,"voice":null})));
    assert_eq!(h.prompts.len(), 5);
    assert_eq!(state.provider_count, 0);
    assert!(!state
        .calls
        .iter()
        .any(|s| s.contains("CompleteSetup") || s == "submit"));
}
#[test]
fn reload_only_repair_uses_existing_install_unit_without_binary_install_or_completion() {
    let mut h = Harness::new();
    h.options.model = None;
    h.state.borrow_mut().facts = core("installed", "stopped", "unavailable");
    h.state.borrow_mut().facts.message = setup::RELOAD_MESSAGE.into();
    assert_eq!(h.run().unwrap(), Outcome::Installed);
    assert_eq!(
        h.state
            .borrow()
            .applied
            .iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [Action::InstallUnit, Action::Enable, Action::Start]
    );
    assert!(h.text().contains("No default"));
}
#[test]
fn later_decline_eof_and_prepare_failure_preserve_artifacts_and_do_not_apply() {
    for answer in [
        Ok("n".into()),
        Err("EOF".into()),
        Err("Consent timed out".into()),
        Err("Interrupted".into()),
    ] {
        let mut h = Harness::new();
        h.state.borrow_mut().facts = core("missing", "stopped", "unavailable");
        h.answers = VecDeque::from([Ok("y".into()), answer]);
        assert!(h.run().is_err());
        assert!(h.state.borrow().published);
        assert!(h.state.borrow().applied.is_empty());
        assert_eq!(h.state.borrow().provider_count, 0);
        assert!(h.text().contains("artifacts remain installed"));
    }
    let mut h = Harness::new();
    h.state.borrow_mut().facts.ok = false;
    h.state.borrow_mut().facts.error = Some("Approval expired".into());
    assert!(h.run().is_err());
    assert_eq!(h.state.borrow().calls, ["inspect"]);
}
#[test]
fn unavailable_manager_conflicts_unknown_identity_and_corruption_block_runtime_effects() {
    for facts in [
        core("installed", "unavailable", "unavailable"),
        core("installed", "conflict", "conflict"),
        core("conflict", "running", "ready"),
        core("unknown", "running", "ready"),
        core("installed", "unknown", "unknown"),
    ] {
        let mut h = Harness::new();
        h.state.borrow_mut().facts = facts;
        assert!(h.run().is_err());
        assert_eq!(h.state.borrow().calls, ["inspect"]);
        assert_eq!(h.state.borrow().provider_count, 0);
    }
}
#[test]
fn missing_model_never_implicitly_pulls_and_download_flag_is_not_consent() {
    let mut h = Harness::new();
    h.state.borrow_mut().inventories = VecDeque::from([vec![]]);
    assert!(h.run().unwrap_err().contains("Selected model missing"));
    assert_eq!(h.state.borrow().provider_count, 0);
    for answer in ["y", "other:tag", "n", "", "TINY:EXACT"] {
        let mut h = Harness::new();
        h.options.pull_model = true;
        h.state.borrow_mut().inventories = VecDeque::from([vec![]]);
        h.answers(&["y", answer]);
        assert!(h.run().unwrap_err().contains("download declined"));
        assert_eq!(h.state.borrow().provider_count, 0);
        assert!(h.state.borrow().configured.is_none());
        assert!(h
            .prompts
            .last()
            .unwrap()
            .contains("revision-pinned Hugging Face"));
    }
}
#[test]
fn pull_fresh_inventory_preference_and_inference_have_exact_separate_consents() {
    let mut h = Harness::new();
    h.options.pull_model = true;
    h.options.verify_inference = true;
    h.state.borrow_mut().inventories = VecDeque::from([vec![], vec![MODEL.into()]]);
    h.answers(&["y", MODEL, "y", "y"]);
    assert_eq!(h.run().unwrap(), Outcome::Installed);
    let state = h.state.borrow();
    assert_eq!(state.provider_count, 1);
    assert_eq!(
        state.provider_calls,
        [format!("pull:{MODEL}"), format!("inference:{MODEL}")]
    );
    assert_eq!(
        state.calls,
        ["inspect", "models", "poll", "models", "poll", "configure"]
    );
    assert_eq!(h.prompts.len(), 4);
    assert!(h.text().contains("not a persisted chat turn"));
}
#[test]
fn postpull_app_inventory_must_match_before_preferences_or_inference() {
    let mut h = Harness::new();
    h.options.pull_model = true;
    h.options.verify_inference = true;
    h.state.borrow_mut().inventories = VecDeque::from([vec![], vec![]]);
    h.answers(&["y", MODEL]);
    assert!(h.run().unwrap_err().contains("Fresh app inventory"));
    assert!(h.state.borrow().configured.is_none());
    assert_eq!(h.state.borrow().provider_calls, [format!("pull:{MODEL}")]);
}
#[test]
fn failed_or_cancelled_download_reports_preserved_staging_with_no_retry() {
    for error in [
        "Interrupted",
        "Pull idle deadline exceeded",
        "Provider protocol rejected",
    ] {
        let mut h = Harness::new();
        h.options.pull_model = true;
        h.state.borrow_mut().inventories = VecDeque::from([vec![]]);
        h.state.borrow_mut().pull_error = Some(error.into());
        h.answers(&["y", MODEL]);
        assert!(h.run().is_err());
        assert_eq!(h.state.borrow().provider_calls, [format!("pull:{MODEL}")]);
        assert!(h.state.borrow().configured.is_none());
        assert!(h
            .text()
            .contains("staging or verified orphan files; no cache was deleted"));
    }
}
#[test]
fn preference_and_inference_decline_preserve_exact_prior_effects_and_lazy_provider() {
    let mut h = Harness::new();
    h.answers(&["y", "n"]);
    assert!(h.run().unwrap_err().contains("Saved preference declined"));
    assert!(h.state.borrow().configured.is_none());
    assert_eq!(h.state.borrow().provider_count, 0);
    let mut h = Harness::new();
    h.options.verify_inference = true;
    h.answers(&["y", "y", "n"]);
    assert!(h.run().unwrap_err().contains("Inference declined"));
    assert_eq!(
        h.state.borrow().configured,
        Some(json!({"model":MODEL,"voice":null}))
    );
    assert_eq!(h.state.borrow().provider_count, 0);
    assert!(h.text().contains("saved preference"));
}
#[test]
fn no_model_selection_never_invents_default_or_preference_or_inference() {
    let mut h = Harness::new();
    h.options.model = None;
    assert_eq!(h.run().unwrap(), Outcome::Installed);
    assert!(h.state.borrow().configured.is_none());
    assert_eq!(h.state.borrow().provider_count, 0);
    assert!(h.text().contains("No default was invented"));
    assert!(h.text().contains("not complete"));
}
#[test]
fn typed_inventory_matches_real_retired_speech_schema_and_rejects_wrong_shapes() {
    assert_eq!(
        setup::checked_inventory(inventory(&[MODEL.into()]))
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        [MODEL]
    );
    let mutations: &[fn(&mut Value)] = &[
        |v| {
            v["providers"]["stt"]
                .as_object_mut()
                .unwrap()
                .remove("model");
        },
        |v| v["providers"]["stt"]["model"] = json!("speech-model"),
        |v| v["providers"]["stt"]["error"] = Value::Null,
        |v| v["providers"]["stt"]["ready"] = json!(true),
        |v| v["providers"]["tts"]["model"] = Value::Null,
        |v| v["providers"]["tts"]["error"]["code"] = json!("NOT_OBSERVED"),
        |v| v["providers"]["tts"]["error"]["message"] = json!(""),
        |v| v["providers"]["stt"]["error"]["extra"] = json!(1),
        |v| v["providers"]["llm"]["model"] = Value::Null,
        |v| v["providers"]["llm"]["ready"] = json!("true"),
        |v| v["providers"]["llm"]["error"] = json!({"code":"FAILED","message":"Not ready"}),
        |v| v["providers"]["llm"]["ready"] = json!(false),
        |v| v["providers"]["stt"]["error"]["message"] = json!("x".repeat(1025)),
        |v| v["providers"]["stt"]["error"]["code"] = json!("lowercase"),
        |v| v["voices"] = json!(["voice"]),
        |v| v["models"][0]["digest"] = json!("x"),
        |v| v["models"][0]["name"] = json!("tiny:cloud"),
        |v| v["models"][0]["extra"] = json!(1),
        |v| v["models"] = Value::Array(vec![json!({"name":MODEL,"digest":"b".repeat(64)}); 257]),
    ];
    for mutate in mutations {
        let mut value = inventory(&[MODEL.into()]);
        mutate(&mut value);
        assert!(setup::checked_inventory(value).is_err());
    }
    let duplicated = inventory(&[MODEL.into(), MODEL.into()]);
    assert!(setup::checked_inventory(duplicated).is_err());
}
#[test]
fn five_operation_subset_strict_arguments_have_no_transport_side_effects_on_rejection() {
    let state = Rc::new(RefCell::new(State::default()));
    let mut native = Native::new(Box::new(FakeBackend(state.clone())));
    for (op, args) in [
        ("speak", json!({})),
        ("transcribe", json!({})),
        ("state", json!({"extra":1})),
        (
            "poll",
            json!({"request_id":"A".repeat(32),"instance_id":INSTANCE}),
        ),
        ("configure", json!({"model":MODEL,"voice":"voice"})),
        (
            "submit",
            json!({"request_id":INSTANCE,"text":"x","output":"speech"}),
        ),
        (
            "submit",
            json!({"request_id":INSTANCE,"text":"x".repeat(16385),"output":"chat"}),
        ),
        (
            "submit",
            json!({"request_id":INSTANCE,"text":"   ","output":"chat"}),
        ),
        (
            "models",
            json!({"request_id":INSTANCE,"endpoint":"http://example.test"}),
        ),
    ] {
        assert!(native.app(op, args).is_err());
    }
    assert!(state.borrow().calls.is_empty());
    assert!(native.prepare(Action::CompleteSetup).is_err());
    assert!(native.prepare(Action::InstallBinary).is_err());
}
#[test]
fn envelope_identity_shape_errors_and_changed_instance_are_not_resent() {
    for mutate in [
        (|v: &mut AppReply| v.call_id = "d".repeat(32)) as fn(&mut AppReply),
        |v| v.instance_id = "A".repeat(32),
        |v| v.schema_version = 2,
        |v| v.service = "foreign".into(),
        |v| v.protocol = "core".into(),
        |v| v.pid = 0,
        |v| v.ok = false,
    ] {
        let state = Rc::new(RefCell::new(State::default()));
        state.borrow_mut().envelope_mutation = Some(mutate);
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native.app("state", json!({})).is_err());
        assert_eq!(state.borrow().calls, ["state"]);
    }
    let state = Rc::new(RefCell::new(State::default()));
    let mut native = Native::new(Box::new(FakeBackend(state.clone())));
    native.app("state", json!({})).unwrap();
    state.borrow_mut().envelope_mutation = Some(|v| v.instance_id = "c".repeat(32));
    assert!(native
        .app("state", json!({}))
        .unwrap_err()
        .contains("instance changed"));
    assert_eq!(state.borrow().calls, ["state", "state"]);
}
#[test]
fn async_inventory_operation_ids_phases_revision_and_terminal_fields_are_bound() {
    for mutate in [
        (|op: &str, v: &mut Value| {
            if op == "poll" {
                v["request_id"] = json!("c".repeat(32));
            }
        }) as fn(&str, &mut Value),
        |op, v| {
            if op == "poll" {
                v["kind"] = json!("submit");
            }
        },
        |op, v| {
            if op == "poll" {
                v["status"] = json!("unknown");
            }
        },
        |op, v| {
            if op == "poll" {
                v["phase"] = json!("generation");
            }
        },
        |op, v| {
            if op == "poll" {
                v["config_revision"] = json!(1);
            }
        },
        |op, v| {
            if op == "poll" {
                v["error"] = json!({"code":"FAILED","message":"failure"});
            }
        },
        |op, v| {
            if op == "poll" {
                v.as_object_mut().unwrap().remove("error");
            }
        },
        |op, v| {
            if op == "poll" {
                v["extra"] = json!(true);
            }
        },
        |op, v| {
            if op == "models" {
                v["config_revision"] = json!(true);
            }
        },
        |op, v| {
            if op == "models" {
                v["status"] = json!("failed");
            }
        },
        |op, v| {
            if op == "models" {
                v["phase"] = json!("complete");
            }
        },
    ] {
        let state = Rc::new(RefCell::new(State::default()));
        state.borrow_mut().mutation = Some(mutate);
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native
            .models_with_budget(Duration::from_secs(1), Duration::ZERO)
            .is_err());
        assert_eq!(
            state
                .borrow()
                .calls
                .iter()
                .filter(|s| *s == "models")
                .count(),
            1
        );
    }
}
#[test]
fn inventory_timeout_and_poll_limit_are_finite_without_readmission() {
    for budget in [Duration::ZERO, Duration::from_secs(1)] {
        let state = Rc::new(RefCell::new(State::default()));
        state.borrow_mut().all_running = true;
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native.models_with_budget(budget, Duration::ZERO).is_err());
        let state = state.borrow();
        assert_eq!(state.calls.iter().filter(|s| *s == "models").count(), 1);
        assert!(state.calls.iter().filter(|s| *s == "poll").count() <= 180);
    }
}
#[test]
fn configure_echo_requires_exact_model_null_voice_and_bounded_unsigned_revision() {
    for mutate in [
        (|_: &str, v: &mut Value| v["model"] = json!("other:tag")) as fn(&str, &mut Value),
        |_, v| v["voice"] = json!("voice"),
        |_, v| {
            v.as_object_mut().unwrap().remove("voice");
        },
        |_, v| v["revision"] = json!(-1),
        |_, v| v["revision"] = json!(true),
        |_, v| v["extra"] = json!(1),
    ] {
        let state = Rc::new(RefCell::new(State::default()));
        state.borrow_mut().mutation = Some(mutate);
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native.configure(MODEL).is_err());
        assert_eq!(state.borrow().calls, ["configure"]);
    }
}
#[test]
fn typed_app_error_and_json_body_field_bounds_are_enforced_without_resend() {
    for mutate in [
        (|v: &mut AppReply| {
            v.ok = false;
            v.result = None;
            v.error = Some(smith_service::app::AppError::new(
                "lowercase",
                "invalid code",
            ));
        }) as fn(&mut AppReply),
        |v| {
            v.ok = false;
            v.result = None;
            v.error = Some(smith_service::app::AppError::new(
                "FAILED",
                &"x".repeat(1025),
            ));
        },
        |v| {
            v.ok = false;
            v.result = None;
            v.error = Some(smith_service::app::AppError::new("FAILED", "   "));
        },
    ] {
        let state = Rc::new(RefCell::new(State::default()));
        state.borrow_mut().envelope_mutation = Some(mutate);
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native
            .app("state", json!({}))
            .unwrap_err()
            .contains("bounded app error"));
        assert_eq!(state.borrow().calls, ["state"]);
    }
    for mutate in [
        (|_: &str, v: &mut Value| v["oversize"] = json!("x".repeat(4 * 1024 * 1024)))
            as fn(&str, &mut Value),
        |_, v| {
            let mut nested = Value::Null;
            for _ in 0..65 {
                nested = json!({"nested":nested});
            }
            v["nested"] = nested;
        },
        |_, v| *v = json!([]),
    ] {
        let state = Rc::new(RefCell::new(State::default()));
        state.borrow_mut().mutation = Some(mutate);
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native.app("state", json!({})).is_err());
        assert_eq!(state.borrow().calls, ["state"]);
    }
    let mut envelope =
        serde_json::to_value(AppReply::from_result(INSTANCE, INSTANCE, Ok(json!({})))).unwrap();
    envelope["extra"] = json!(1);
    assert!(serde_json::from_value::<AppReply>(envelope).is_err());
    // Typed transport supersedes the removed helper JSON decoder. Serde rejects
    // duplicate typed envelope fields and non-finite JSON at that boundary.
    assert!(
        serde_json::from_slice::<AppReply>(br#"{"schema_version":1,"schema_version":1}"#).is_err()
    );
    assert!(serde_json::from_slice::<AppReply>(br#"{"result":{"value":NaN}}"#).is_err());
}
#[test]
fn core_identity_states_diagnostics_and_token_format_fail_closed() {
    for mutate in [
        (|v: &mut Observation| v.service = "foreign".into()) as fn(&mut Observation),
        |v| v.schema_version = 1,
        |v| v.enablement_state = "linked-runtime".into(),
        |v| v.runtime_state = "enabled".into(),
        |v| v.approval_token = Some("not-a-binding".into()),
        |v| v.message = "x".repeat(1025),
        |v| v.ok = false,
    ] {
        let state = Rc::new(RefCell::new(State::default()));
        mutate(&mut state.borrow_mut().facts);
        let mut native = Native::new(Box::new(FakeBackend(state.clone())));
        assert!(native.inspect().is_err());
        assert_eq!(state.borrow().calls, ["inspect"]);
    }
}
#[test]
fn supported_ubuntu24_x86_nonroot_preflight_does_not_accept_ubuntu_like_or_other_releases() {
    assert!(
        setup::supported_host("linux", "x86_64", 1000, "ID=ubuntu\nVERSION_ID=\"24.04\"\n").is_ok()
    );
    for (os, arch, uid, release) in [
        ("linux", "aarch64", 1000, "ID=ubuntu\nVERSION_ID=24.04"),
        ("linux", "x86_64", 0, "ID=ubuntu\nVERSION_ID=24.04"),
        (
            "linux",
            "x86_64",
            1000,
            "ID=debian\nID_LIKE=ubuntu\nVERSION_ID=24.04",
        ),
        ("linux", "x86_64", 1000, "ID=ubuntu\nVERSION_ID=22.04"),
        ("linux", "x86_64", 1000, "ID=ubuntu\nVERSION_ID=26.04"),
        (
            "linux",
            "x86_64",
            1000,
            "ID=ubuntu\nID=ubuntu\nVERSION_ID=24.04",
        ),
        ("linux", "x86_64", 1000, "ID=ubuntu\nVERSION_ID=\"24.04"),
        ("other", "x86_64", 1000, "ID=ubuntu\nVERSION_ID=24.04"),
    ] {
        assert!(setup::supported_host(os, arch, uid, release).is_err());
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    roots: Roots,
    source: PathBuf,
}
fn directory(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "smith-installer-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        directory(&base);
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        for root in [&roots.data, &roots.config, &roots.runtime] {
            directory(root);
        }
        directory(&roots.data.join("smith-next"));
        directory(&roots.data.join("smith-next/bin"));
        let source = base.join("payload-service");
        fs::write(&source, b"private fixture never executed").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        fs::copy(&source, roots.binary()).unwrap();
        fs::set_permissions(roots.binary(), fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            base,
            roots,
            source,
        }
    }
    fn controller(&self) -> Controller<FakeManager> {
        Controller {
            roots: self.roots.clone(),
            source: self.source.clone(),
            manager: FakeManager::default(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
#[derive(Default)]
struct FakeManager {
    loaded: RefCell<bool>,
    reloads: RefCell<usize>,
    starts: RefCell<usize>,
}
impl Manager for FakeManager {
    fn inspect(&self, roots: &Roots) -> Result<ManagerFacts> {
        let loaded = *self.loaded.borrow();
        Ok(ManagerFacts {
            load: if loaded { "loaded" } else { "not-found" }.into(),
            active: "inactive".into(),
            sub: "dead".into(),
            pid: 0,
            fragment: if loaded {
                roots.unit().to_string_lossy().into()
            } else {
                String::new()
            },
            drop_ins: String::new(),
            need_reload: false,
            transient: false,
            unit_file_state: if roots.enablement_link().exists() {
                "enabled"
            } else if loaded {
                "disabled"
            } else {
                ""
            }
            .into(),
        })
    }
    fn reload(&self, _: &Roots) -> Result<()> {
        *self.loaded.borrow_mut() = true;
        *self.reloads.borrow_mut() += 1;
        Ok(())
    }
    fn start(&self, _: &Roots) -> Result<()> {
        *self.starts.borrow_mut() += 1;
        Err("Fixture does not start processes".into())
    }
    fn enable(&self, roots: &Roots) -> Result<()> {
        let link = roots.enablement_link();
        let parent = link.parent().unwrap();
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        std::os::unix::fs::symlink(roots.unit(), link).map_err(|error| error.to_string())
    }
}
#[test]
fn direct_controller_fake_manager_registration_and_exact_original_expiry_are_retained() {
    let f = Fixture::new();
    let mut native = Native::new(Box::new(f.controller()));
    assert_eq!(native.inspect().unwrap().binary_state, "installed");
    let prepared = native.prepare(Action::InstallUnit).unwrap();
    let token = prepared.approval_token.unwrap();
    native.apply(Action::InstallUnit, &token).unwrap();
    assert_eq!(
        fs::read(f.roots.unit()).unwrap(),
        f.roots.unit_text().as_bytes()
    );
    assert!(!f.roots.state().exists());
    assert!(!f.roots.app_socket().exists());
    let f = Fixture::new();
    let mut native = Native::new(Box::new(f.controller()));
    let prepared = native.prepare(Action::InstallUnit).unwrap();
    let original = prepared.approval_token.unwrap();
    let old = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 61;
    let expired = format!("{old}.{}", original.split_once('.').unwrap().1);
    assert!(native
        .apply(Action::InstallUnit, &expired)
        .unwrap_err()
        .contains("expired"));
    assert!(!f.roots.unit().exists());
}
#[test]
fn direct_controller_changed_input_is_not_reprepared_or_retried() {
    let f = Fixture::new();
    let mut native = Native::new(Box::new(f.controller()));
    let token = native
        .prepare(Action::InstallUnit)
        .unwrap()
        .approval_token
        .unwrap();
    fs::write(&f.source, b"different input").unwrap();
    assert!(native.apply(Action::InstallUnit, &token).is_err());
    assert!(!f.roots.unit().exists());
}
#[test]
fn stdin_negative_eof_overflow_utf8_and_ctrl_c_never_approve_and_flags_restore() {
    for bytes in [
        b"n\n".to_vec(),
        b"\n".to_vec(),
        b"yes\n".to_vec(),
        b"Y\n".to_vec(),
        b"y".to_vec(),
        vec![b'x'; 4096],
        b"\x03\n".to_vec(),
        b"\xff\n".to_vec(),
    ] {
        let (mut writer, reader) = UnixStream::pair().unwrap();
        writer.write_all(&bytes).unwrap();
        drop(writer);
        // SAFETY: reader owns a valid descriptor throughout this test.
        let flags = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) };
        let result = consent::read_line(reader.as_fd(), Duration::from_millis(100));
        assert!(!result
            .as_ref()
            .is_ok_and(|s| consent::matches(s, "y").unwrap()));
        assert_eq!(
            unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) },
            flags
        );
    }
}
#[test]
fn stdin_exact_bound_and_two_separate_piped_answers_are_not_read_ahead() {
    let (mut writer, reader) = UnixStream::pair().unwrap();
    writer.write_all(b"y\ny\n").unwrap();
    for _ in 0..2 {
        assert!(consent::matches(
            &consent::read_line(reader.as_fd(), Duration::from_millis(100)).unwrap(),
            "y"
        )
        .unwrap());
    }
    let mut bytes = vec![b'x'; 4095];
    bytes.push(b'\n');
    writer.write_all(&bytes).unwrap();
    assert_eq!(
        consent::read_line(reader.as_fd(), Duration::from_secs(1))
            .unwrap()
            .len(),
        4095
    );
    assert!(consent::read_line(reader.as_fd(), Duration::from_secs(61)).is_err());
}
#[test]
fn stdin_timeout_and_actual_slow_drip_are_bounded_without_reader_threads() {
    let (mut writer, reader) = UnixStream::pair().unwrap();
    let started = Instant::now();
    assert!(
        consent::read_line(reader.as_fd(), Duration::from_millis(30))
            .unwrap_err()
            .contains("timed out")
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    let drip = std::thread::spawn(move || {
        for byte in b"y     \n" {
            if writer.write_all(&[*byte]).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    });
    let started = Instant::now();
    assert!(
        consent::read_line(reader.as_fd(), Duration::from_millis(40))
            .unwrap_err()
            .contains("timed out")
    );
    assert!(started.elapsed() < Duration::from_millis(300));
    drop(reader);
    drip.join().unwrap();
}
#[test]
fn untrusted_display_removes_controls_bidi_but_does_not_execute_generated_text() {
    assert_eq!(
        consent::display("\u{1b}[2J\u{202e}http://example.test\n"),
        "[2Jhttp://example.test"
    );
}

// Signal cancellation is process-wide and monotonic. Run its test in a fresh
// compiled test process, not by resetting either production cancellation flag.
#[test]
fn signal_child_core_and_installer_are_both_cancelled() {
    if std::env::var_os("SMITH_INSTALLER_SIGNAL_CHILD").is_none() {
        return;
    }
    let f = Fixture::new();
    let controller = f.controller();
    let token = controller
        .prepare(Action::InstallUnit)
        .approval_token
        .unwrap();
    consent::install_cancellation().unwrap();
    consent::install_cancellation().unwrap();
    println!("SIGNAL-READY");
    std::io::stdout().flush().unwrap();
    let result = consent::read_line(std::io::stdin().as_fd(), Duration::from_secs(4));
    assert!(result.unwrap_err().contains("interrupted"));
    assert!(consent::cancelled());
    let result = controller.apply(Action::InstallUnit, &token);
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("cancelled"));
    assert_eq!(*controller.manager.reloads.borrow(), 0);
    assert_eq!(*controller.manager.starts.borrow(), 0);
    assert!(!f.roots.unit().exists());
}
#[test]
fn sigint_sigterm_cancel_bounded_input_and_core_before_manager_mutation() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "signal_child_core_and_installer_are_both_cancelled",
                "--nocapture",
            ])
            .env("SMITH_INSTALLER_SIGNAL_CHILD", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        // SAFETY: stdout is an owned test-child pipe, kept alive until after wait.
        let flags = unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_GETFL) };
        assert_eq!(
            unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut output = Vec::new();
        loop {
            let mut bytes = [0; 4096];
            if let Ok(n) = stdout.read(&mut bytes) {
                output.extend_from_slice(&bytes[..n]);
            }
            if String::from_utf8_lossy(&output).contains("SIGNAL-READY") {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("Signal fixture did not become ready");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // SAFETY: only the captured test child is signalled, never a manager/service.
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("Signal fixture exceeded cleanup budget");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut stderr = String::new();
        child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(status.success(), "Signal fixture failed: {stderr}");
    }
}

#[test]
fn failed_progress_sink_is_not_reentered_by_error_or_partial_effect_reporting() {
    struct FailProgress {
        failed: bool,
        calls: usize,
        memory: VectorProgress,
    }
    impl ProgressOutput for FailProgress {
        fn write_until(
            &mut self,
            bytes: &[u8],
            deadline: Instant,
            cancelled: fn() -> bool,
        ) -> Result<()> {
            assert!(
                !self.failed,
                "Setup reentered a failed stdout sink while reporting model failure"
            );
            self.calls += 1;
            if bytes.starts_with(b"Fake progress,") {
                self.failed = true;
                return Err("Model progress output deadline exceeded".into());
            }
            self.memory.write_until(bytes, deadline, cancelled)
        }
    }
    let mut h = Harness::new();
    h.options.pull_model = true;
    h.state.borrow_mut().inventories = VecDeque::from([vec![]]);
    h.answers(&["y", MODEL]);
    let mut output = FailProgress {
        failed: false,
        calls: 0,
        memory: VectorProgress::new(),
    };
    assert!(h
        .run_to(&mut output)
        .unwrap_err()
        .contains("output deadline"));
    assert!(output.failed);
    assert!(output.calls > 1);
    assert_eq!(h.state.borrow().provider_calls, [format!("pull:{MODEL}")]);
    assert!(h.state.borrow().configured.is_none());
    assert!(h.state.borrow().published);
}

#[test]
fn terminal_error_report_lines_share_one_short_deadline() {
    struct Audit {
        deadlines: Vec<Instant>,
        memory: VectorProgress,
    }
    impl ProgressOutput for Audit {
        fn write_until(
            &mut self,
            bytes: &[u8],
            deadline: Instant,
            cancelled: fn() -> bool,
        ) -> Result<()> {
            self.deadlines.push(deadline);
            self.memory.write_until(bytes, deadline, cancelled)
        }
    }
    let mut h = Harness::new();
    h.state.borrow_mut().preflight_error = Some("Fixture prerequisite failure".into());
    let mut output = Audit {
        deadlines: Vec::new(),
        memory: VectorProgress::new(),
    };
    let started = Instant::now();
    assert!(h.run_to(&mut output).is_err());
    assert!(output.deadlines.len() >= 4);
    assert!(output
        .deadlines
        .iter()
        .all(|deadline| *deadline == output.deadlines[0]));
    assert!(output.deadlines[0] <= started + consent::REPORT_BUDGET + Duration::from_millis(50));
    assert!(
        String::from_utf8_lossy(output.memory.as_bytes()).contains("No automatic retry/rollback")
    );
}

static PROGRESS_CANCELLED: AtomicBool = AtomicBool::new(false);
fn progress_cancelled() -> bool {
    PROGRESS_CANCELLED.load(Ordering::Relaxed)
}

struct BackpressuredProvider {
    mode: String,
    state: Rc<RefCell<State>>,
}
impl Provider for BackpressuredProvider {
    fn tags(&mut self) -> Result<Vec<String>> {
        panic!("Caller fixture must not contact a provider inventory")
    }
    fn inference(&mut self, _: &str) -> Result<String> {
        panic!("No inference after failed/cancelled pull")
    }
    fn pull(&mut self, model: &str, output: &mut dyn ProgressOutput) -> Result<()> {
        self.state
            .borrow_mut()
            .provider_calls
            .push(format!("pull:{model}"));
        // Fixture-only handshake: this short PIPE_BUF-sized marker is the sole
        // stderr write before the parent starts filling the initially empty pipe.
        // No flag-restoring output scope remains alive when the parent sees it.
        let marker = b"H1-PROVIDER-READY\n";
        // SAFETY: captured stderr is open and has room for this controlled marker.
        assert_eq!(
            unsafe { libc::write(libc::STDERR_FILENO, marker.as_ptr().cast(), marker.len()) },
            marker.len() as isize
        );
        // Parent fills BOTH real output pipes before releasing this library-only
        // provider seam. This is not a production endpoint/argument override.
        if consent::read_line(std::io::stdin().as_fd(), Duration::from_secs(5))? != "go" {
            return Err("Invalid controlled provider handshake".into());
        }
        for fd in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
            // SAFETY: inspect inherited fixture streams without changing flags.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            assert!(
                flags >= 0 && flags & libc::O_NONBLOCK == 0,
                "Caller must begin model output with ordinary blocking descriptors"
            );
        }
        match self.mode.as_str() {
            "deadline" => output.write_until(
                b"Blocked model progress\n",
                Instant::now() + Duration::from_millis(100),
                consent::cancelled,
            ),
            "callback" => std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(100));
                    PROGRESS_CANCELLED.store(true, Ordering::Relaxed);
                });
                output.write_until(
                    b"Blocked model progress\n",
                    Instant::now() + Duration::from_secs(3),
                    progress_cancelled,
                )
            }),
            "sigint" | "sigterm" => output.write_until(
                b"Blocked model progress\n",
                Instant::now() + Duration::from_secs(3),
                consent::cancelled,
            ),
            // Expiry can occur before a progress write. The console is not yet
            // poisoned, so this exercises the separate terminal report budget.
            "model-error" => Err("Model deadline expired before progress output".into()),
            _ => Err("Unknown controlled provider mode".into()),
        }?;
        Err("Backpressured fixture unexpectedly delivered model progress".into())
    }
}

#[test]
fn backpressured_setup_child() {
    let Some(mode) = std::env::var_os("SMITH_INSTALLER_BACKPRESSURE_CHILD") else {
        return;
    };
    let mode = mode.into_string().unwrap();
    consent::install_cancellation().unwrap();
    let h = Harness::new();
    let state = h.state.clone();
    state.borrow_mut().inventories = VecDeque::from([vec![]]);
    let mut preflight = || Ok(());
    let mut publisher =
        |_: PathBuf, _: PathBuf| Ok(Box::new(FakePublisher(state.clone())) as Box<dyn Publisher>);
    let mut backend = |_: PathBuf| Ok(Box::new(FakeBackend(state.clone())) as Box<dyn Backend>);
    let mut provider = || {
        Ok(Box::new(BackpressuredProvider {
            mode: mode.clone(),
            state: state.clone(),
        }) as Box<dyn Provider>)
    };
    let mut answers = VecDeque::from(["y".to_owned(), MODEL.to_owned()]);
    let mut confirm = |_: &str| {
        Ok(answers
            .pop_front()
            .expect("No consent or retry after failed model stage"))
    };
    let mut options = h.options;
    options.pull_model = true;
    let result = setup::run_with(
        &options,
        &mut Factories {
            preflight: &mut preflight,
            publisher: &mut publisher,
            backend: &mut backend,
            provider: &mut provider,
        },
        &mut confirm,
        &mut consent::StdoutConsole,
    );
    assert!(result.is_err());
    assert_eq!(state.borrow().calls, ["inspect", "models", "poll"]);
    assert_eq!(state.borrow().provider_calls, [format!("pull:{MODEL}")]);
    assert!(state.borrow().published);
    assert!(state.borrow().configured.is_none());
    let code = cli::exit_status(result);
    assert_eq!(code, 1);
    // Nonzero exit is required even if BOTH terminal/error streams are full.
    std::process::exit(code);
}

fn pipe() -> (OwnedFd, OwnedFd) {
    let mut descriptors = [-1; 2];
    // SAFETY: pipe2 initializes two descriptors with CLOEXEC on success.
    assert_eq!(
        unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) },
        0
    );
    unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    }
}
fn nonblocking(fd: &OwnedFd) {
    // SAFETY: this is an owned test descriptor kept open throughout the fixture.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
}
fn blocking(fd: &OwnedFd) {
    // SAFETY: no child output scope is active during the fixture handshake.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK) },
        0
    );
}
fn fill_pipe(writer: &OwnedFd) {
    let byte = b'x';
    let mut count = 0;
    loop {
        // One-byte writes fill the last non-PIPE_BUF-aligned free space too.
        // SAFETY: writer is nonblocking/owned; byte points to one initialized byte.
        let n = unsafe { libc::write(writer.as_raw_fd(), std::ptr::addr_of!(byte).cast(), 1) };
        if n < 0 {
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::WouldBlock
            );
            break;
        }
        assert_eq!(n, 1);
        count += 1;
        assert!(count <= 1024 * 1024, "Unexpected pipe capacity");
    }
    assert!(count > 0);
}
fn drain_pipe(reader: &OwnedFd) -> Vec<u8> {
    let mut result = Vec::new();
    loop {
        let mut bytes = [0u8; 4096];
        // SAFETY: reader is nonblocking/owned; bytes provides valid output storage.
        let n = unsafe { libc::read(reader.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
        if n <= 0 {
            break;
        }
        result.extend_from_slice(&bytes[..n as usize]);
        assert!(result.len() <= 1024 * 1024);
    }
    result
}

fn caller_with_full_stdout_and_stderr_exits_without_kill(mode: &str, signal: Option<i32>) {
    let (out_read, out_write) = pipe();
    let (err_read, err_write) = pipe();
    for fd in [&out_read, &err_read] {
        nonblocking(fd);
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "backpressured_setup_child", "--nocapture"])
        .env("SMITH_INSTALLER_BACKPRESSURE_CHILD", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(out_write.try_clone().unwrap()))
        .stderr(Stdio::from(err_write.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut ready = Vec::new();
    loop {
        ready.extend_from_slice(&drain_pipe(&err_read));
        if String::from_utf8_lossy(&ready).contains("H1-PROVIDER-READY") {
            break;
        }
        if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "Controlled provider did not reach model stage: {}",
                String::from_utf8_lossy(&ready)
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    for fd in [&out_write, &err_write] {
        nonblocking(fd);
    }
    fill_pipe(&out_write);
    fill_pipe(&err_write);
    // Restore ordinary blocking output before GO. The production console must
    // itself establish nonblocking writes, not inherit protection from the test.
    for fd in [&out_write, &err_write] {
        blocking(fd);
    }
    let started = Instant::now();
    child.stdin.take().unwrap().write_all(b"go\n").unwrap();
    if let Some(signal) = signal {
        std::thread::sleep(Duration::from_millis(50));
        // SAFETY: signal only the captured fixture child, never a manager/provider.
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Backpressured setup required KILL after {mode}; stdout/stderr remained full");
        }
        // Deliberately do NOT drain either output pipe while waiting for exit.
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(
        status.code(),
        Some(1),
        "Unexpected fixture outcome for {mode}: {}",
        String::from_utf8_lossy(&drain_pipe(&err_read))
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn model_deadline_and_callback_cancellation_with_full_output_pipes_exit_nonzero_without_kill() {
    for mode in ["deadline", "callback", "model-error"] {
        caller_with_full_stdout_and_stderr_exits_without_kill(mode, None);
    }
}
#[test]
fn sigint_and_sigterm_with_full_model_output_pipes_exit_nonzero_without_kill() {
    for (mode, signal) in [("sigint", libc::SIGINT), ("sigterm", libc::SIGTERM)] {
        caller_with_full_stdout_and_stderr_exits_without_kill(mode, Some(signal));
    }
}
