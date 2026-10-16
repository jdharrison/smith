//! Provider-aware installer contract tests. Typed fake Controller seam only:
//! no D-Bus/keyring, vendor HTTP, manager, installed files, weights or credentials.
use serde_json::{json, Value};
use smith_installer::{
    artifact, cli,
    model::{ProgressOutput, Provider as LocalProvider, VectorProgress},
    providers::{self, Action, Catalog, Manager, Provider},
    secret::Secret,
    setup::{Backend, Native},
    wizard::{
        Cancellation, Choices, Command, Event, Limits, PreparedReview, Publisher, Scope, Session,
    },
    Result,
};
use smith_service::{
    app::{AppError, AppReply, AppRequest},
    cloud,
    providers::{Inventory, Model, Observation as ProviderObservation, Observations},
    runner, Action as CoreAction, Observation,
};
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::OsString,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const INSTANCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SECRET: &str = "synthetic-INSTALLER-secret-canary-NOT-A-CREDENTIAL";
fn default_model(provider: Provider) -> &'static str {
    cloud::MODELS
        .iter()
        .find(|model| model.vendor.name() == provider.id())
        .unwrap()
        .id
}
fn catalog_choice(value: &mut Value, provider: Provider) -> &mut Value {
    value["models"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|choice| {
            choice["route"]["provider"] == provider.id()
                && choice["route"]["model"] == default_model(provider)
        })
        .unwrap()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn revision(n: u64) -> Value {
    if n > 9_007_199_254_740_991 {
        json!(n.to_string())
    } else {
        json!(n)
    }
}
fn error(code: &str) -> Value {
    json!({"code":code,"message":"Fixed synthetic failure"})
}
fn view(provider: Provider) -> Value {
    json!({"provider":provider,"revision":0,"credential_generation":null,"credential_state":"not_configured","credential_error":null,"validation":null})
}
fn inventory() -> Value {
    serde_json::to_value(Inventory {
        models: vec![Model {
            name: runner::DEFAULT_MODEL.into(),
            digest: runner::CATALOG
                .iter()
                .find(|m| m.name == runner::DEFAULT_MODEL)
                .unwrap()
                .sha256
                .into(),
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
    .unwrap()
}
struct State {
    calls: Vec<String>,
    public_args: Vec<(String, Value)>,
    views: [Value; 2],
    prepared: BTreeMap<String, Value>,
    operations: BTreeMap<String, Value>,
    routing: BTreeMap<String, Value>,
    config_revision: u64,
    selected: Option<String>,
    mutation: Option<fn(&str, &mut Value)>,
    envelope: Option<fn(&mut AppReply)>,
    lost_apply: bool,
    post_commit_error: Option<&'static str>,
    state_reply: Option<Value>,
    lost_poll: bool,
    poll_delay: Duration,
    poll_running: bool,
    rejection: Option<&'static str>,
    validation_error: Option<&'static str>,
    probe_locked: bool,
    publications: usize,
    preview_error: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            public_args: vec![],
            views: Provider::ALL.map(view),
            prepared: BTreeMap::new(),
            operations: BTreeMap::new(),
            routing: BTreeMap::new(),
            config_revision: 0,
            selected: None,
            mutation: None,
            envelope: None,
            lost_apply: false,
            post_commit_error: None,
            state_reply: None,
            lost_poll: false,
            poll_delay: Duration::ZERO,
            poll_running: false,
            rejection: None,
            validation_error: None,
            probe_locked: false,
            publications: 0,
            preview_error: false,
        }
    }
}
impl State {
    fn catalog(&self) -> Value {
        let models: Vec<Value> = cloud::MODELS.iter().map(|model| {
            let v = &self.views[model.vendor.index()];
            let success = v["validation"]["status"] == "succeeded";
            let available = !v["credential_generation"].is_null();
            json!({"group":"cloud","label":model.label,
                "route":{"provider":v["provider"],"model":model.id,"credential_generation":v["credential_generation"]},
                "available":available,"unavailable_reason":if available { Value::Null } else { error("PROVIDER_NOT_CONFIGURED") },
                "validated_model":success && v["validation"]["model"] == model.id})
        }).collect();
        json!({"config_revision":revision(self.config_revision),"inventory_observed":false,"providers":self.views,"models":models})
    }
    fn state(&self) -> Value {
        json!({"config":{"model":self.selected,"voice":null,"revision":revision(self.config_revision)},
            "inventory_observed":true,"inventory":inventory(),"conversations":[],"operations":[],
            "retention":{"max_conversations":32,"max_turn_pairs":16,"max_context_bytes":65536,
                "max_terminal_jobs":32,"max_expired_ids":1024,"max_pending_jobs":4,"queue_timeout_ms":600000,
                "capacity_policy":"reject_when_expiry_metadata_full","missing_request_code":"REQUEST_EXPIRED"},"revision":0})
    }
    fn count(&self, op: &str) -> usize {
        self.calls.iter().filter(|c| c.as_str() == op).count()
    }
}
#[derive(Clone)]
struct Fake(Arc<Mutex<State>>);
fn facts() -> Observation {
    serde_json::from_value(json!({"schema_version":2,"service":"smith-next","ok":true,
        "binary_state":"installed","unit_state":"installed","enablement_state":"enabled",
        "runtime_state":"running","bridge_state":"ready","setup_complete":false,"message":"Synthetic ready","error":null,
        "effects":{"binary_path":"/fixture/bin","unit_path":"/fixture/unit","state_path":"/fixture/state","socket_path":"/fixture/socket","description":"Fixture only"}})).unwrap()
}
fn operation(
    request: &str,
    kind: &str,
    config: u64,
    result: Value,
    failure: Option<&str>,
) -> Value {
    json!({"request_id":request,"conversation_id":null,"kind":kind,"status":if failure.is_some(){"failed"}else{"succeeded"},
        "phase":"complete","config_revision":revision(config),"result":if failure.is_some(){Value::Null}else{result},"error":failure.map(error)})
}
fn admission(terminal: &Value, phase: &str) -> Value {
    let mut v = terminal.clone();
    let object = v.as_object_mut().unwrap();
    object.remove("result");
    object.remove("error");
    v["status"] = json!("running");
    v["phase"] = json!(phase);
    v
}
impl Backend for Fake {
    fn inspect(&mut self) -> Result<Observation> {
        self.0.lock().unwrap().calls.push("inspect".into());
        Ok(facts())
    }
    fn prepare(&mut self, _: CoreAction) -> Result<Observation> {
        panic!("No lifecycle preparation")
    }
    fn apply(&mut self, _: CoreAction, _: &str) -> Result<Observation> {
        panic!("No lifecycle mutation")
    }
    fn app(&mut self, req: AppRequest) -> Result<AppReply> {
        assert_eq!(req.schema_version, 3);
        assert_eq!(req.service, "smith-next");
        assert_eq!(req.protocol, "app");
        let mut s = self.0.lock().unwrap();
        s.calls.push(req.op.clone());
        let mut public = req.args.clone();
        if let Some(object) = public.as_object_mut() {
            object.remove("api_key");
        }
        s.public_args.push((req.op.clone(), public));
        let mut rejected = None;
        let request = req.args["request_id"].as_str().unwrap_or("").to_owned();
        let mut result = match req.op.as_str() {
            "route_catalog" => {
                assert_eq!(req.args, json!({}));
                s.catalog()
            }
            "provider_prepare" => {
                assert_eq!(req.args["instance_id"], INSTANCE);
                let i = usize::from(req.args["provider"] == "anthropic");
                assert_eq!(
                    req.args["expected_provider_revision"],
                    s.views[i]["revision"]
                );
                if req.args["action"] == "save" {
                    assert!(req.args["model"].is_null());
                } else {
                    assert!(cloud::model(
                        cloud::Vendor::ALL[i],
                        req.args["model"].as_str().unwrap()
                    )
                    .is_some());
                }
                let p = json!({"request_id":request,"provider":req.args["provider"],"action":req.args["action"],"model":req.args["model"],
                    "expected_provider_revision":s.views[i]["revision"],"credential_generation":s.views[i]["credential_generation"],
                    "approval_token":"b".repeat(32),"expires_at_ms":now()+60_000,
                    "endpoint":if req.args["action"]=="save" {Value::Null} else {json!(Provider::ALL[i].endpoint())}});
                s.prepared.insert(request.clone(), p.clone());
                p
            }
            "provider_apply" | "provider_probe" => {
                assert_eq!(req.args["instance_id"], INSTANCE);
                let p = if req.op == "provider_apply" {
                    s.prepared[&request].clone()
                } else {
                    req.args.clone()
                };
                if req.op == "provider_apply" {
                    assert_eq!(req.args["approval_token"], p["approval_token"]);
                }
                let i = usize::from(p["provider"] == "anthropic");
                let (kind, phase) = if req.op == "provider_probe" {
                    ("provider_probe", "inventory")
                } else if p["action"] == "save" {
                    ("provider_save", "credential")
                } else {
                    ("provider_validate", "verification")
                };
                if let Some(code) = s.rejection {
                    rejected = Some(code);
                    Value::Null
                } else {
                    let mut failure = None;
                    let result = if kind == "provider_save" {
                        assert_eq!(req.args["api_key"], SECRET);
                        let rev = s.views[i]["revision"].as_u64().unwrap() + 1;
                        s.views[i]["revision"] = json!(rev);
                        s.views[i]["credential_generation"] =
                            json!(format!("{:032x}", rev * 10 + i as u64));
                        s.views[i]["credential_state"] = json!("available");
                        s.views[i]["credential_error"] = Value::Null;
                        s.views[i]["validation"] = Value::Null;
                        json!({"provider":s.views[i]})
                    } else if kind == "provider_validate" {
                        assert!(req.args["api_key"].is_null());
                        failure = s.validation_error;
                        let delivery = if failure.is_some() {
                            "may_have_sent"
                        } else {
                            "received"
                        };
                        s.views[i]["validation"] = json!({"request_id":request,"model":p["model"],"credential_generation":p["credential_generation"],
                            "status":if failure.is_some(){"failed"}else{"succeeded"},"completed_ms":now(),"error":failure.map(error),"delivery":delivery});
                        let route = json!({"provider":p["provider"],"model":p["model"],"credential_generation":p["credential_generation"]});
                        s.routing.insert(request.clone(),json!({"request_id":request,"route":route,"delivery":delivery,"context_sha256":null,"dispatch_expires_at_ms":p["expires_at_ms"]}));
                        json!({"provider":s.views[i],"route":route,"assistant_text":"Synthetic hello."})
                    } else {
                        if s.probe_locked && !s.views[i]["credential_generation"].is_null() {
                            s.views[i]["credential_state"] = json!("locked");
                            s.views[i]["credential_error"] = error("KEYRING_LOCKED");
                        }
                        json!({"provider":s.views[i]})
                    };
                    let terminal = operation(&request, kind, s.config_revision, result, failure);
                    s.operations.insert(request.clone(), terminal.clone());
                    if s.lost_apply && req.op == "provider_apply" {
                        s.lost_apply = false;
                        return Err(SECRET.into());
                    }
                    if req.op == "provider_apply" {
                        rejected = s.post_commit_error.take();
                    }
                    admission(&terminal, phase)
                }
            }
            "poll" => {
                assert_eq!(req.args["instance_id"], INSTANCE);
                if s.lost_poll {
                    s.lost_poll = false;
                    return Err(SECRET.into());
                }
                std::thread::sleep(s.poll_delay);
                let mut terminal = s.operations[&request].clone();
                if s.poll_running {
                    terminal["status"] = json!("running");
                    terminal["phase"] = json!("credential");
                    terminal["result"] = Value::Null;
                    terminal["error"] = Value::Null;
                }
                terminal
            }
            "request_routing" => s.routing[&request].clone(),
            "state" => s.state_reply.clone().unwrap_or_else(|| s.state()),
            "configure_checked" => {
                assert_eq!(req.args["expected_revision"], revision(s.config_revision));
                s.config_revision += 1;
                s.selected = req.args["model"].as_str().map(str::to_owned);
                json!({"model":s.selected,"voice":null,"revision":revision(s.config_revision)})
            }
            "models" | "verify_model" => {
                let result = if req.op == "models" {
                    inventory()
                } else {
                    json!({"model":req.args["model"],"sha256":req.args["sha256"],"verified":true,"assistant_text":"Synthetic local hello."})
                };
                let terminal = operation(&request, &req.op, s.config_revision, result, None);
                s.operations.insert(request.clone(), terminal.clone());
                admission(
                    &terminal,
                    if req.op == "models" {
                        "inventory"
                    } else {
                        "verification"
                    },
                )
            }
            _ => panic!("Unexpected operation {}", req.op),
        };
        if let Some(mutate) = s.mutation {
            mutate(&req.op, &mut result);
        }
        let mut reply = AppReply {
            schema_version: 3,
            service: "smith-next".into(),
            protocol: "app".into(),
            pid: 123,
            instance_id: INSTANCE.into(),
            call_id: req.call_id,
            ok: rejected.is_none(),
            result: if rejected.is_none() {
                Some(result)
            } else {
                None
            },
            error: rejected.map(|code| AppError {
                code: code.into(),
                message: "Fixed structured service error".into(),
            }),
        };
        if let Some(mutate) = s.envelope {
            mutate(&mut reply);
        }
        Ok(reply)
    }
}
fn native(s: &Arc<Mutex<State>>) -> Native {
    Native::new_v3(Box::new(Fake(s.clone())))
}
fn manager(s: &Arc<Mutex<State>>) -> Manager {
    Manager::open(native(s)).unwrap()
}
fn save(provider: Provider) -> Action {
    Action::Save {
        provider,
        secret: Secret::new(SECRET.into()).unwrap(),
    }
}
fn approve(_: &providers::Prepared) -> Result<bool> {
    Ok(true)
}
fn saved(m: &mut Manager, provider: Provider) {
    assert!(
        m.perform(save(provider), &mut approve, &|| false)
            .unwrap()
            .success
    );
    m.refresh().unwrap();
}

#[test]
fn both_providers_save_independently_separate_validation_and_replacement() {
    let s = Arc::new(Mutex::new(State::default()));
    let mut m = manager(&s);
    assert_eq!(s.lock().unwrap().calls, ["inspect", "route_catalog"]);
    for provider in Provider::ALL {
        let mut effects = vec![];
        let outcome = m
            .perform(
                save(provider),
                &mut |p| {
                    effects = p.effects();
                    assert!(p.title().starts_with("Save"));
                    Ok(true)
                },
                &|| false,
            )
            .unwrap();
        assert!(outcome.success && outcome.message.contains("Not validated"));
        assert!(effects.join(" ").contains("No API request"));
        assert!(!format!("{effects:?} {outcome:?}").contains(SECRET));
        m.refresh().unwrap();
        assert!(m.catalog.provider(provider).validation.is_none());
    }
    assert_eq!(s.lock().unwrap().count("provider_apply"), 2);
    let old = m.catalog.provider(Provider::Anthropic).clone();
    let validation = m
        .perform(
            Action::Validate {
                provider: Provider::Openai,
                model: default_model(Provider::Openai).into(),
            },
            &mut |p| {
                let effects = p.effects().join(" ");
                assert!(effects.contains("Say hello in one short sentence."));
                assert!(effects.contains("charges") && effects.contains("No conversation history"));
                Ok(true)
            },
            &|| false,
        )
        .unwrap();
    assert!(validation.success && validation.message.contains("provider response received"));
    m.refresh().unwrap();
    for choice in m.catalog.choices(Provider::Openai) {
        assert!(choice.available);
        assert_eq!(
            choice.validated_model,
            choice.route.model == default_model(Provider::Openai)
        );
    }
    let generation = m
        .catalog
        .provider(Provider::Openai)
        .credential_generation
        .clone();
    m.perform(
        save(Provider::Openai),
        &mut |p| {
            assert!(p.title().starts_with("Replace"));
            assert!(p.effects().join(" ").contains("retained"));
            Ok(true)
        },
        &|| false,
    )
    .unwrap();
    m.refresh().unwrap();
    assert_ne!(
        m.catalog.provider(Provider::Openai).credential_generation,
        generation
    );
    assert!(m.catalog.provider(Provider::Openai).validation.is_none());
    assert_eq!(*m.catalog.provider(Provider::Anthropic), old);
    assert!(!format!("{:?}", s.lock().unwrap().public_args).contains(SECRET));
}

#[test]
fn decline_and_cached_refresh_do_not_apply_probe_or_validate() {
    let s = Arc::new(Mutex::new(State::default()));
    let mut m = manager(&s);
    assert!(m
        .perform(save(Provider::Openai), &mut |_| Ok(false), &|| false)
        .unwrap()
        .message
        .contains("declined"));
    m.perform(
        Action::Refresh,
        &mut |_| panic!("No consent for cached read"),
        &|| false,
    )
    .unwrap();
    assert!(m.pending_request().is_none());
    assert_eq!(
        s.lock().unwrap().calls,
        [
            "inspect",
            "route_catalog",
            "provider_prepare",
            "route_catalog"
        ]
    );
}

#[test]
fn lost_apply_and_poll_results_recover_only_by_original_id() {
    for lost_apply in [true, false] {
        let s = Arc::new(Mutex::new(State {
            lost_apply,
            lost_poll: !lost_apply,
            ..State::default()
        }));
        let mut m = manager(&s);
        let err = m
            .perform(save(Provider::Openai), &mut approve, &|| false)
            .unwrap_err();
        assert!(!err.contains(SECRET));
        let original = m.pending_request().unwrap().to_owned();
        assert!(m
            .perform(save(Provider::Anthropic), &mut approve, &|| false)
            .is_err());
        assert_eq!(s.lock().unwrap().count("provider_apply"), 1);
        assert!(
            m.perform(
                Action::Observe,
                &mut |_| panic!("Do not prepare again"),
                &|| false
            )
            .unwrap()
            .success
        );
        assert!(m.pending_request().is_none());
        let state = s.lock().unwrap();
        assert_eq!(state.count("provider_prepare"), 1);
        assert_eq!(state.count("provider_apply"), 1);
        for (_, args) in state.public_args.iter().filter(|(op, _)| op == "poll") {
            assert_eq!(args["request_id"], original);
        }
    }
}

#[test]
fn post_commit_structured_errors_recover_save_and_validation_by_original_id_only() {
    for provider in Provider::ALL {
        for validate in [false, true] {
            for code in [
                "STATE_UNAVAILABLE",
                "KEYRING_LOCKED",
                "ID_CONFLICT",
                "REQUEST_EXPIRED",
                "FUTURE_FAILURE",
            ] {
                let s = Arc::new(Mutex::new(State::default()));
                let mut m = manager(&s);
                if validate {
                    saved(&mut m, provider);
                }
                let (before_prepare, before_apply, before_args) = {
                    let mut state = s.lock().unwrap();
                    state.post_commit_error = Some(code);
                    (
                        state.count("provider_prepare"),
                        state.count("provider_apply"),
                        state.public_args.len(),
                    )
                };
                let action = if validate {
                    Action::Validate {
                        provider,
                        model: default_model(provider).into(),
                    }
                } else {
                    save(provider)
                };
                let error = m.perform(action, &mut approve, &|| false).unwrap_err();
                assert!(
                    error.contains("Outcome unknown") && error.contains("original request"),
                    "{code}: {error}"
                );
                assert!(!error.contains(SECRET));
                let original = m.pending_request().unwrap().to_owned();
                assert!(
                    s.lock().unwrap().operations.contains_key(&original),
                    "The fake must commit before returning {code}"
                );
                m.refresh().unwrap();
                assert_eq!(m.pending_request(), Some(original.as_str()));
                for action in [
                    save(provider),
                    Action::Validate {
                        provider,
                        model: default_model(provider).into(),
                    },
                    Action::Probe { provider },
                ] {
                    assert!(m
                        .perform(
                            action,
                            &mut |_| panic!("Pending work must block new consent"),
                            &|| false
                        )
                        .is_err());
                }
                assert!(
                    m.perform(
                        Action::Observe,
                        &mut |_| panic!("Recovery must not reprepare"),
                        &|| false
                    )
                    .unwrap()
                    .success
                );
                assert!(m.pending_request().is_none());
                let state = s.lock().unwrap();
                assert_eq!(state.count("provider_prepare"), before_prepare + 1);
                assert_eq!(state.count("provider_apply"), before_apply + 1);
                let calls = &state.public_args[before_args..];
                assert_eq!(calls.iter().filter(|(op, _)| op == "poll").count(), 1);
                for (op, args) in calls.iter().filter(|(op, _)| {
                    matches!(
                        op.as_str(),
                        "provider_prepare" | "provider_apply" | "poll" | "request_routing"
                    )
                }) {
                    assert_eq!(
                        args["request_id"], original,
                        "{op} must use the original identity"
                    );
                    assert_eq!(args["instance_id"], INSTANCE);
                }
                if validate {
                    assert_eq!(
                        state.routing[&original]["dispatch_expires_at_ms"],
                        state.prepared[&original]["expires_at_ms"]
                    );
                }
                assert!(!format!("{calls:?}").contains(SECRET));
            }
        }
    }
}

#[test]
fn cancellation_and_late_poll_never_accept_or_replay_a_late_result() {
    let s = Arc::new(Mutex::new(State::default()));
    let mut m = manager(&s);
    let stop = || s.lock().unwrap().count("poll") > 0;
    assert!(m
        .perform(save(Provider::Openai), &mut approve, &stop)
        .unwrap_err()
        .contains("stopped"));
    assert!(m.pending_request().is_some());
    s.lock().unwrap().poll_delay = Duration::from_millis(15);
    assert!(m.observe(Duration::from_millis(1), &|| false).is_err());
    assert!(m.pending_request().is_some());
    assert_eq!(s.lock().unwrap().count("provider_apply"), 1);
    s.lock().unwrap().poll_delay = Duration::ZERO;
    assert!(
        m.observe(Duration::from_secs(1), &|| false)
            .unwrap()
            .success
    );
}

#[test]
fn prepare_binding_expiry_and_original_deadline_are_fail_closed() {
    for mutation in [
        (|op: &str, v: &mut Value| {
            if op == "provider_prepare" {
                v["provider"] = json!("anthropic");
            }
        }) as fn(&str, &mut Value),
        |op, v| {
            if op == "provider_prepare" {
                v["expected_provider_revision"] = json!(9);
            }
        },
        |op, v| {
            if op == "provider_prepare" {
                v["credential_generation"] = json!("d".repeat(32));
            }
        },
        |op, v| {
            if op == "provider_prepare" {
                v["expires_at_ms"] = json!(now() - 1);
            }
        },
        |op, v| {
            if op == "provider_prepare" {
                v["endpoint"] = json!("https://unexpected.invalid");
            }
        },
        |op, v| {
            if op == "provider_prepare" {
                v.as_object_mut().unwrap().remove("model");
            }
        },
        |op, v| {
            if op == "provider_prepare" {
                v["approval_token"] = json!("invalid");
            }
        },
    ] {
        let s = Arc::new(Mutex::new(State {
            mutation: Some(mutation),
            ..State::default()
        }));
        let mut m = manager(&s);
        assert!(m
            .perform(
                save(Provider::Openai),
                &mut |_| panic!("Do not offer invalid preparation"),
                &|| false
            )
            .is_err());
        assert_eq!(s.lock().unwrap().count("provider_apply"), 0);
    }
    let s = Arc::new(Mutex::new(State {
        mutation: Some(|op, v| {
            if op == "provider_prepare" {
                v["expires_at_ms"] = json!(now() + 20);
            }
        }),
        ..State::default()
    }));
    let mut m = manager(&s);
    assert!(m
        .perform(
            save(Provider::Openai),
            &mut |_| {
                std::thread::sleep(Duration::from_millis(30));
                Ok(true)
            },
            &|| false
        )
        .is_err());
    assert_eq!(s.lock().unwrap().count("provider_prepare"), 1);
    assert_eq!(s.lock().unwrap().count("provider_apply"), 0);
}

#[test]
fn saved_generation_catalog_accepts_advisory_observations_and_validation_facts() {
    for (observation, code) in [
        ("not_observed", Some("NOT_OBSERVED")),
        ("available", None),
        ("locked", Some("KEYRING_LOCKED")),
        ("missing", Some("KEYRING_MISSING")),
        ("conflict", Some("KEYRING_CONFLICT")),
        ("unavailable", Some("KEYRING_UNAVAILABLE")),
    ] {
        for status in [None, Some("succeeded"), Some("failed"), Some("interrupted")] {
            let state = State {
                views: Provider::ALL.map(|provider| {
                    let generation = if provider == Provider::Openai { "c" } else { "d" }.repeat(32);
                    json!({"provider":provider,"revision":1,"credential_generation":generation,
                        "credential_state":observation,"credential_error":code.map(error),
                        "validation":status.map(|status| json!({"request_id":"e".repeat(32),
                            "model":default_model(provider),"credential_generation":generation,
                            "status":status,"completed_ms":now(),
                            "error":if status == "succeeded" {Value::Null} else {error("CLOUD_AUTH")},
                            "delivery":if status == "interrupted" {"may_have_sent"} else {"received"}}))})
                }),
                ..State::default()
            };
            let value = state.catalog();
            let catalog = Catalog::parse(value.clone()).unwrap();
            for provider in Provider::ALL {
                let p = catalog.provider(provider);
                assert_eq!(p.credential_state, observation);
                assert_eq!(p.credential_error.as_ref().map(|e| e.code.as_str()), code);
                assert_eq!(p.validation.as_ref().map(|v| v.status.as_str()), status);
                assert!(p.status().contains("eligible to attempt"));
                assert!(!p.status().contains("then explicitly observe"));
                for choice in catalog.choices(provider) {
                    assert!(choice.available && choice.unavailable_reason.is_none());
                    assert_eq!(choice.route.credential_generation, p.credential_generation);
                    assert_eq!(
                        choice.validated_model,
                        status == Some("succeeded")
                            && choice.route.model == default_model(provider)
                    );
                }
            }
            let mut stale = value.clone();
            catalog_choice(&mut stale, Provider::Openai)["route"]["credential_generation"] =
                json!("f".repeat(32));
            assert!(Catalog::parse(stale).is_err());
            let mut disabled = value.clone();
            catalog_choice(&mut disabled, Provider::Openai)["available"] = json!(false);
            catalog_choice(&mut disabled, Provider::Openai)["unavailable_reason"] =
                error("NOT_OBSERVED");
            assert!(Catalog::parse(disabled).is_err());
            let mut forged = value;
            catalog_choice(&mut forged, Provider::Openai)["validated_model"] =
                json!(status != Some("succeeded"));
            assert!(Catalog::parse(forged).is_err());
        }
    }
    let mut missing = State::default().catalog();
    catalog_choice(&mut missing, Provider::Openai)["available"] = json!(true);
    catalog_choice(&mut missing, Provider::Openai)["unavailable_reason"] = Value::Null;
    assert!(Catalog::parse(missing).is_err());
}
#[test]
fn strict_catalog_nullable_fields_bounds_and_full_u64_revisions() {
    let state = State {
        config_revision: u64::MAX,
        ..State::default()
    };
    assert_eq!(
        Catalog::parse(state.catalog()).unwrap().config_revision,
        json!(u64::MAX.to_string())
    );
    for mutation in [
        (|v: &mut Value| {
            v.as_object_mut().unwrap().remove("inventory_observed");
        }) as fn(&mut Value),
        |v| {
            v["providers"][0]
                .as_object_mut()
                .unwrap()
                .remove("validation");
        },
        |v| {
            v["providers"][0]["revision"] = json!(9_007_199_254_740_992u64);
        },
        |v| {
            v["providers"][0]["unexpected"] = json!(true);
        },
        |v| {
            catalog_choice(v, Provider::Openai)["route"]
                .as_object_mut()
                .unwrap()
                .remove("credential_generation");
        },
        |v| {
            catalog_choice(v, Provider::Openai)["available"] = json!(true);
        },
        |v| {
            catalog_choice(v, Provider::Openai)["validated_model"] = json!(true);
        },
        |v| {
            catalog_choice(v, Provider::Openai)["route"]["provider"] = json!("other");
        },
        |v| {
            catalog_choice(v, Provider::Openai)["label"] = json!("line\nbreak");
        },
        |v| {
            v["config_revision"] = json!(u64::MAX);
        },
        |v| {
            v["config_revision"] = json!("01");
        },
    ] {
        let mut v = state.catalog();
        mutation(&mut v);
        assert!(Catalog::parse(v).is_err());
    }
    let s = Arc::new(Mutex::new(State {
        config_revision: u64::MAX - 1,
        ..State::default()
    }));
    let mut n = native(&s);
    assert_eq!(n.configuration().unwrap().revision, u64::MAX - 1);
    assert_eq!(
        n.configure_checked(runner::DEFAULT_MODEL, u64::MAX - 1)
            .unwrap(),
        u64::MAX
    );
    n.model_operation_with_budget(
        "verify_model",
        runner::DEFAULT_MODEL,
        u64::MAX,
        &|| false,
        Duration::from_secs(1),
        Duration::ZERO,
    )
    .unwrap();
}

fn populated_state() -> Value {
    let state = State {
        config_revision: u64::MAX,
        ..State::default()
    };
    let mut value = state.state();
    value["config"]["voice"] = json!("retained-inert-legacy-voice");
    value["conversations"] = json!([
        {"conversation_id":"c".repeat(32),"title":"Historical title longer than forty Unicode scalars remains readable",
            "archived":false,"metadata_revision":0,"history_revision":9_007_199_254_740_991u64,
            "sequence":7,"origin_request_id":null},
        {"conversation_id":"d".repeat(32),"title":"Archived chat","archived":true,
            "metadata_revision":2,"history_revision":0,"sequence":3,"origin_request_id":"9".repeat(32)}
    ]);
    value["operations"] = json!([
        {"request_id":"1".repeat(32),"conversation_id":"c".repeat(32),"kind":"submit_routed",
            "status":"succeeded","phase":"complete","config_revision":revision(u64::MAX)},
        {"request_id":"2".repeat(32),"conversation_id":null,"kind":"provider_validate",
            "status":"failed","phase":"complete","config_revision":0},
        {"request_id":"3".repeat(32),"conversation_id":"c".repeat(32),"kind":"speak",
            "status":"succeeded","phase":"complete","config_revision":0}
    ]);
    value
}

fn reject_state(value: Value) {
    let s = Arc::new(Mutex::new(State {
        state_reply: Some(value),
        ..State::default()
    }));
    assert!(native(&s).configuration().is_err());
    assert_eq!(
        s.lock().unwrap().calls,
        ["state"],
        "Malformed state permits no follow-up effect"
    );
}

#[test]
fn schema3_state_rejects_unknown_nested_and_missing_nullable_fields() {
    for path in [
        "",
        "/config",
        "/inventory",
        "/inventory/models/0",
        "/inventory/providers",
        "/inventory/providers/llm",
        "/inventory/providers/stt",
        "/inventory/providers/stt/error",
        "/inventory/providers/tts",
        "/inventory/providers/tts/error",
        "/conversations/0",
        "/operations/0",
        "/retention",
    ] {
        let mut value = populated_state();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        reject_state(value);
    }
    for (path, key) in [
        ("/config", "model"),
        ("/config", "voice"),
        ("/inventory/providers/llm", "error"),
        ("/inventory/providers/stt", "error"),
        ("/inventory/providers/stt", "model"),
        ("/inventory/providers/tts", "error"),
        ("/conversations/0", "origin_request_id"),
        ("/operations/0", "conversation_id"),
        ("/retention", "max_pending_jobs"),
    ] {
        let mut value = populated_state();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        reject_state(value);
    }
}

#[test]
fn schema3_state_rejects_wrong_types_identities_revisions_and_bounds() {
    for (path, replacement) in [
        ("/config/model", json!(7)),
        ("/config/voice", json!(false)),
        ("/config/revision", json!(u64::MAX)),
        ("/config/revision", json!("01")),
        ("/inventory_observed", json!("true")),
        ("/inventory", Value::Null),
        ("/inventory/models", json!({})),
        ("/inventory/voices", json!(null)),
        (
            "/inventory/models/0/name",
            json!("not-an-exact-local-model"),
        ),
        ("/inventory/models/0/digest", json!("bad")),
        ("/inventory/providers/llm/ready", json!(1)),
        ("/inventory/providers/llm/error", error("BAD")),
        ("/inventory/providers/stt/model", json!(true)),
        ("/inventory/providers/stt/error/code", json!("lowercase")),
        ("/inventory/providers/tts/error/message", json!([])),
        ("/conversations", json!({})),
        ("/conversations/0/title", json!(42)),
        ("/conversations/0/title", json!(" padded ")),
        ("/conversations/0/title", json!("x".repeat(257))),
        ("/conversations/0/title", json!("hidden\u{200e}title")),
        ("/conversations/0/conversation_id", json!("C".repeat(32))),
        ("/conversations/0/archived", json!("false")),
        ("/conversations/0/metadata_revision", json!(-1)),
        (
            "/conversations/0/history_revision",
            json!(9_007_199_254_740_992u64),
        ),
        ("/conversations/0/sequence", json!("7")),
        ("/conversations/1/sequence", json!(7)),
        ("/conversations/1/conversation_id", json!("c".repeat(32))),
        ("/conversations/0/origin_request_id", json!(false)),
        ("/operations", json!(null)),
        ("/operations/0/kind", json!("future_kind")),
        ("/operations/0/status", json!("accepted")),
        ("/operations/0/phase", json!(null)),
        (
            "/operations/0/config_revision",
            json!("18446744073709551616"),
        ),
        ("/operations/0/request_id", json!("invalid")),
        ("/operations/0/conversation_id", json!("f".repeat(32))),
        ("/operations/1/conversation_id", json!("c".repeat(32))),
        ("/operations/1/request_id", json!("1".repeat(32))),
        ("/retention", json!({})),
        ("/retention/max_conversations", json!("32")),
        ("/retention/max_pending_jobs", json!(5)),
        ("/retention/queue_timeout_ms", json!(null)),
        ("/retention/capacity_policy", json!("evict")),
        ("/revision", json!("0")),
        ("/revision", json!(9_007_199_254_740_992u64)),
    ] {
        let mut value = populated_state();
        *value.pointer_mut(path).unwrap() = replacement;
        reject_state(value);
    }
    for path in ["/conversations", "/operations", "/inventory/models"] {
        let mut value = populated_state();
        let items = value.pointer_mut(path).unwrap().as_array_mut().unwrap();
        items.resize(
            if path == "/inventory/models" { 257 } else { 33 },
            items[0].clone(),
        );
        reject_state(value);
    }
    let mut value = populated_state();
    value["operations"][0]["status"] = json!("running");
    value["operations"][0]["phase"] = json!("inventory");
    reject_state(value);
}

#[test]
fn schema3_state_accepts_cached_unobserved_unavailable_and_legacy_facts() {
    for unavailable in [false, true] {
        let mut value = populated_state();
        value["inventory"] = serde_json::to_value(Inventory::default()).unwrap();
        value["inventory_observed"] = json!(unavailable);
        if unavailable {
            value["inventory"]["providers"]["llm"]["error"] = error("RUNTIME_UNAVAILABLE");
        }
        let s = Arc::new(Mutex::new(State {
            state_reply: Some(value),
            ..State::default()
        }));
        let config = native(&s).configuration().unwrap();
        assert_eq!(config.revision, u64::MAX);
        assert_eq!(config.voice.as_deref(), Some("retained-inert-legacy-voice"));
        assert_eq!(s.lock().unwrap().calls, ["state"]);
    }
    let mut value = populated_state();
    value["inventory_observed"] = json!(false);
    reject_state(value);
    let mut value = populated_state();
    value["inventory"]["providers"]["llm"] =
        json!({"ready":false,"error":error("RUNTIME_UNAVAILABLE")});
    reject_state(value);
}

#[test]
fn cloud_models_must_belong_to_the_reviewed_provider_catalog() {
    let parsed = Catalog::parse(State::default().catalog()).unwrap();
    assert_eq!(parsed.models.len(), cloud::MODELS.len());
    for (actual, reviewed) in parsed.models.iter().zip(cloud::MODELS) {
        assert_eq!(actual.route.provider, reviewed.vendor.name());
        assert_eq!(actual.route.model, reviewed.id);
    }
    for (provider, count, first) in [
        (Provider::Openai, 7, "gpt-6-astra"),
        (Provider::Anthropic, 8, "claude-sonnet-5-5"),
    ] {
        assert_eq!(parsed.choices(provider).count(), count);
        assert_eq!(parsed.choices(provider).next().unwrap().route.model, first);
    }
    for (provider, wrong) in [
        (Provider::Openai, "gpt6.0"),
        (Provider::Openai, "gpt6.1.sol"),
        (Provider::Openai, "fixture-text-model"),
        (Provider::Openai, default_model(Provider::Anthropic)),
        (Provider::Anthropic, "claude5.5"),
        (Provider::Anthropic, default_model(Provider::Openai)),
        (Provider::Anthropic, "claude-sonnet-5-5-renamed"),
    ] {
        let mut value = State::default().catalog();
        catalog_choice(&mut value, provider)["route"]["model"] = json!(wrong);
        assert!(
            Catalog::parse(value).is_err(),
            "Unreviewed/wrong-provider route: {wrong}"
        );
    }
    for model in cloud::MODELS {
        let s = Arc::new(Mutex::new(State::default()));
        let provider = Provider::ALL[model.vendor.index()];
        let mut m = manager(&s);
        saved(&mut m, provider);
        m.perform(
            Action::Validate {
                provider,
                model: model.id.into(),
            },
            &mut approve,
            &|| false,
        )
        .unwrap();
        m.refresh().unwrap();
        assert_eq!(
            m.catalog
                .provider(provider)
                .validation
                .as_ref()
                .unwrap()
                .model,
            model.id
        );
        let mut value = s.lock().unwrap().catalog();
        value["providers"][model.vendor.index()]["validation"]["model"] = json!("unreviewed-model");
        assert!(Catalog::parse(value).is_err());
        let before = s.lock().unwrap().count("provider_prepare");
        assert!(m
            .perform(
                Action::Validate {
                    provider,
                    model: "unreviewed-model".into()
                },
                &mut |_| panic!("No consent for unknown models"),
                &|| false
            )
            .is_err());
        assert_eq!(s.lock().unwrap().count("provider_prepare"), before);
    }
}

#[test]
fn rejected_admission_wrong_envelope_and_terminal_binding_do_not_replay() {
    for code in [
        "BUSY",
        "STALE_REVISION",
        "CONSENT_EXPIRED",
        "CONSENT_MISMATCH",
        "INSTANCE_MISMATCH",
        "INVALID_REQUEST",
        "CAPACITY",
    ] {
        let s = Arc::new(Mutex::new(State {
            rejection: Some(code),
            ..State::default()
        }));
        let mut m = manager(&s);
        assert!(m
            .perform(save(Provider::Openai), &mut approve, &|| false)
            .is_err());
        assert!(m.pending_request().is_none());
        assert_eq!(s.lock().unwrap().count("provider_apply"), 1);
    }
    for mutate in [
        (|r: &mut AppReply| r.schema_version = 1) as fn(&mut AppReply),
        |r| r.instance_id = "f".repeat(32),
        |r| r.pid = 999,
        |r| r.call_id = "f".repeat(32),
    ] {
        let s = Arc::new(Mutex::new(State::default()));
        let mut m = manager(&s);
        s.lock().unwrap().envelope = Some(mutate);
        assert!(m
            .perform(save(Provider::Openai), &mut approve, &|| false)
            .is_err());
        assert_eq!(s.lock().unwrap().count("provider_apply"), 0);
    }
    for mutate in [
        (|op: &str, v: &mut Value| {
            if op == "poll" {
                v["kind"] = json!("provider_validate");
            }
        }) as fn(&str, &mut Value),
        |op, v| {
            if op == "poll" {
                v["conversation_id"] = json!("e".repeat(32));
            }
        },
        |op, v| {
            if op == "poll" {
                v["config_revision"] = json!(4);
            }
        },
        |op, v| {
            if op == "poll" {
                v["result"]["provider"]["revision"] = json!(9);
            }
        },
        |op, v| {
            if op == "poll" {
                v.as_object_mut().unwrap().remove("error");
            }
        },
    ] {
        let s = Arc::new(Mutex::new(State {
            mutation: Some(mutate),
            ..State::default()
        }));
        let mut m = manager(&s);
        assert!(m
            .perform(save(Provider::Openai), &mut approve, &|| false)
            .is_err());
        assert!(m.pending_request().is_some());
        assert_eq!(s.lock().unwrap().count("provider_apply"), 1);
    }
}

#[test]
fn locked_probe_stays_advisory_and_failed_validation_exposes_delivery() {
    let s = Arc::new(Mutex::new(State::default()));
    let mut m = manager(&s);
    saved(&mut m, Provider::Openai);
    s.lock().unwrap().probe_locked = true;
    let out = m
        .perform(
            Action::Probe {
                provider: Provider::Openai,
            },
            &mut |_| panic!("No API confirmation for local observation"),
            &|| false,
        )
        .unwrap();
    assert!(out.message.contains("locked"));
    m.refresh().unwrap();
    assert!(m
        .catalog
        .choices(Provider::Openai)
        .all(|choice| choice.available && !choice.validated_model));
    assert_eq!(
        m.catalog.provider(Provider::Openai).credential_state,
        "locked"
    );
    s.lock().unwrap().validation_error = Some("REMOTE_OUTCOME_UNKNOWN");
    let out = m
        .perform(
            Action::Validate {
                provider: Provider::Openai,
                model: default_model(Provider::Openai).into(),
            },
            &mut approve,
            &|| false,
        )
        .unwrap();
    assert!(
        !out.success
            && out.message.contains("may have been sent")
            && out.message.contains("charges")
    );
    assert_eq!(s.lock().unwrap().count("request_routing"), 1);
}

#[test]
fn cli_mode_is_narrow_and_secret_source_is_not_confirmation_input() {
    let parse = |args: &[&str]| cli::parse(args.iter().map(OsString::from));
    let cli::Command::Run(options) =
        parse(&["--providers", "--bundle", "/fixture/package"]).unwrap()
    else {
        panic!()
    };
    assert!(matches!(options.mode, cli::Mode::Providers { .. }));
    for args in [
        vec!["--yes"],
        vec!["--dry-run"],
        vec!["--models"],
        vec!["--artifacts-only"],
        vec!["--model", default_model(Provider::Openai)],
        vec!["--pull-model"],
        vec!["--verify-inference"],
        vec!["--client", "/fixture/client"],
        vec!["--service", "/fixture/service"],
    ] {
        let mut all = vec!["--providers"];
        all.extend(args);
        assert!(parse(&all).is_err());
    }
    for text in [
        SECRET,
        &format!("{SECRET}=value"),
        &format!("--api-key={SECRET}"),
    ] {
        assert!(!parse(&["--providers", text]).unwrap_err().contains(SECRET));
    }
    let s = Arc::new(Mutex::new(State::default()));
    let mut answers = VecDeque::from(["o", "s", "y", "a", "s", "y", "o", "v", "1", "y", "d"]);
    let mut prompts = String::new();
    let mut secrets = 0;
    let mut output = VectorProgress::new();
    providers::manage_cli(
        native(&s),
        &mut |prompt| {
            prompts.push_str(prompt);
            Ok(answers.pop_front().unwrap().into())
        },
        &mut || {
            secrets += 1;
            Secret::new(SECRET.into())
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(secrets, 2);
    assert!(answers.is_empty());
    assert_eq!(s.lock().unwrap().count("provider_apply"), 3);
    assert!(!prompts.contains(SECRET));
    assert!(!String::from_utf8(output.into_bytes())
        .unwrap()
        .contains(SECRET));
    assert!(!format!(
        "{:?}",
        Command::ProviderAction {
            request_id: 42,
            action: save(Provider::Openai)
        }
    )
    .contains(SECRET));
}

struct Review;
impl PreparedReview for Review {
    fn description(&self) -> &str {
        "Synthetic artifact review"
    }
    fn review_id(&self) -> &str {
        "fixture"
    }
}
impl Publisher for Fake {
    type Prepared = Review;
    fn prepare_review(&mut self) -> Result<Review> {
        if self.0.lock().unwrap().preview_error {
            Err("Synthetic preview unavailable".into())
        } else {
            Ok(Review)
        }
    }
    fn install_reviewed(
        &mut self,
        _: Review,
        _: &str,
        _: &mut dyn std::io::Write,
    ) -> Result<artifact::Outcome> {
        self.0.lock().unwrap().publications += 1;
        Ok(artifact::Outcome::Installed)
    }
}
impl LocalProvider for Fake {
    fn tags(&mut self) -> Result<Vec<String>> {
        panic!("Use service inventory")
    }
    fn pull(&mut self, _: &str, _: &mut dyn ProgressOutput) -> Result<()> {
        panic!("No weights in tests")
    }
    fn inference(&mut self, _: &str) -> Result<String> {
        panic!("Use fake service check")
    }
}
fn session(s: &Arc<Mutex<State>>) -> Session {
    let p = s.clone();
    let b = s.clone();
    let l = s.clone();
    Session::spawn_provider_aware_with(
        "/fixture/package".into(),
        move |_, _| Ok(Fake(p.clone())),
        move |_| Ok(Box::new(Fake(b.clone())) as Box<dyn Backend>),
        || Ok(()),
        Cancellation::from_callbacks(|| false, || {}),
        Limits::default(),
        Some(Box::new(move || {
            Ok(Box::new(Fake(l.clone())) as Box<dyn LocalProvider>)
        })),
        None,
        true,
    )
    .unwrap()
}
fn event(s: &Session) -> Event {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(e) = s.try_event().unwrap() {
            return e;
        }
        assert!(Instant::now() < deadline, "Worker event deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn join(s: &mut Session) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !s.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    s.join().unwrap();
}
#[test]
fn production_v3_local_model_then_optional_skip_keeps_verified_install() {
    for provider_failure in [false, true] {
        let s = Arc::new(Mutex::new(State::default()));
        let mut wizard = session(&s);
        let Event::Review { request_id, .. } = event(&wizard) else {
            panic!()
        };
        wizard
            .send(Command::Install {
                request_id,
                choices: Choices::smith_install(),
            })
            .unwrap();
        let mut local_checked = false;
        loop {
            match event(&wizard) {
                Event::Models { request_id, .. } => wizard
                    .send(Command::ModelAction {
                        request_id,
                        model: runner::DEFAULT_MODEL.into(),
                        remove: false,
                    })
                    .unwrap(),
                Event::Confirmation {
                    request_id, title, ..
                } => {
                    if title.contains("Check") {
                        local_checked = true;
                    }
                    wizard.send(Command::Approve { request_id }).unwrap();
                }
                Event::Providers { request_id, .. } => {
                    assert!(local_checked);
                    assert!(s.lock().unwrap().count("verify_model") > 0);
                    if provider_failure && s.lock().unwrap().count("provider_prepare") == 0 {
                        s.lock().unwrap().rejection = Some("KEYRING_LOCKED");
                        wizard
                            .send(Command::ProviderAction {
                                request_id,
                                action: save(Provider::Openai),
                            })
                            .unwrap();
                    } else {
                        wizard
                            .send(Command::ProviderAction {
                                request_id,
                                action: Action::Done,
                            })
                            .unwrap();
                    }
                }
                Event::ProviderConfirmation { request_id, .. } => {
                    wizard.send(Command::Approve { request_id }).unwrap()
                }
                Event::Finished { report } => {
                    assert!(report.complete);
                    assert_eq!(report.scope, Scope::Installation);
                    assert_eq!(
                        report.providers.as_ref().unwrap().success,
                        !provider_failure
                    );
                    assert!(!format!("{report:?}").contains(SECRET));
                    break;
                }
                Event::Stage { .. } | Event::Progress { .. } => {}
                _ => panic!("Unexpected event"),
            }
        }
        join(&mut wizard);
        assert_eq!(s.lock().unwrap().publications, 1);
    }
}

#[test]
fn management_after_failed_preview_and_decline_stays_in_optional_loop() {
    let s = Arc::new(Mutex::new(State {
        preview_error: true,
        ..State::default()
    }));
    let mut wizard = session(&s);
    let mut selections = 0;
    wizard.send(Command::ManageProviders).unwrap();
    loop {
        match event(&wizard) {
            Event::Providers { request_id, .. } => {
                selections += 1;
                wizard
                    .send(Command::ProviderAction {
                        request_id,
                        action: if selections == 1 {
                            save(Provider::Anthropic)
                        } else {
                            Action::Done
                        },
                    })
                    .unwrap();
            }
            Event::ProviderConfirmation { request_id, .. } => {
                wizard.send(Command::Decline { request_id }).unwrap()
            }
            Event::Finished { report } => {
                assert!(report.complete && !report.cancelled);
                assert_eq!(report.scope, Scope::Providers);
                break;
            }
            Event::Stage { .. } => {}
            _ => panic!("Unexpected management event"),
        }
    }
    join(&mut wizard);
    let state = s.lock().unwrap();
    assert_eq!(state.publications, 0);
    assert_eq!(state.count("provider_apply"), 0);
    assert_eq!(state.count("models"), 0);
}

#[test]
fn cancelling_accepted_provider_work_joins_and_keeps_public_recovery_identity() {
    let state = Arc::new(Mutex::new(State {
        poll_running: true,
        ..State::default()
    }));
    let mut wizard = session(&state);
    wizard.send(Command::ManageProviders).unwrap();
    loop {
        match event(&wizard) {
            Event::Providers { request_id, .. } => wizard
                .send(Command::ProviderAction {
                    request_id,
                    action: save(Provider::Openai),
                })
                .unwrap(),
            Event::ProviderConfirmation { request_id, .. } => {
                wizard.send(Command::Approve { request_id }).unwrap();
                break;
            }
            Event::Stage { .. } | Event::Review { .. } => {}
            _ => panic!("Unexpected event before provider apply"),
        }
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while state.lock().unwrap().count("provider_apply") == 0 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    wizard.send(Command::Cancel).unwrap();
    loop {
        if let Event::Finished { report } = event(&wizard) {
            assert!(report.cancelled && !report.complete);
            let state = state.lock().unwrap();
            let original = state
                .public_args
                .iter()
                .find(|(op, _)| op == "provider_apply")
                .unwrap()
                .1["request_id"]
                .as_str()
                .unwrap();
            assert!(report
                .providers
                .as_ref()
                .unwrap()
                .message
                .contains(original));
            assert!(!format!("{report:?}").contains(SECRET));
            assert_eq!(state.count("provider_apply"), 1);
            break;
        }
    }
    join(&mut wizard);
}

mod real_domain {
    use super::*;
    use smith_service::{
        app::{AppResult, Application, Message},
        cloud, Roots,
    };
    use std::{
        fs,
        os::unix::fs::DirBuilderExt,
        path::PathBuf,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    struct Fixture {
        base: PathBuf,
        roots: Roots,
    }
    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "smith-installer-cloud-{}",
                smith_service::app::new_id().unwrap()
            ));
            let roots = Roots {
                data: base.join("data"),
                config: base.join("config"),
                runtime: base.join("run"),
            };
            for path in [
                &base,
                &roots.data,
                &roots.config,
                &roots.runtime,
                &roots.data.join("smith-next"),
            ] {
                fs::DirBuilder::new().mode(0o700).create(path).unwrap();
            }
            Self { base, roots }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.base).unwrap();
        }
    }
    #[derive(Default)]
    struct Capabilities {
        items: Mutex<Vec<cloud::CredentialRef>>,
        reads: AtomicUsize,
        posts: AtomicUsize,
        read_failure: Mutex<Option<&'static str>>,
        status: Mutex<Option<u16>>,
    }
    impl cloud::CredentialStore for Capabilities {
        fn save(
            &self,
            identity: &cloud::CredentialRef,
            secret: &cloud::Secret,
            _: Instant,
            _: &AtomicBool,
        ) -> AppResult<()> {
            assert_eq!(secret.expose(), SECRET.as_bytes());
            self.items.lock().unwrap().push(identity.clone());
            Ok(())
        }
        fn read(
            &self,
            identity: &cloud::CredentialRef,
            _: Instant,
            _: &AtomicBool,
        ) -> AppResult<cloud::Secret> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if let Some(code) = *self.read_failure.lock().unwrap() {
                return Err(AppError::new(code, "Synthetic read-error canary"));
            }
            if !self.items.lock().unwrap().contains(identity) {
                return Err(AppError::new("KEYRING_MISSING", "Synthetic missing item"));
            }
            cloud::Secret::new(SECRET.as_bytes().to_vec())
        }
    }
    impl cloud::HttpTransport for Capabilities {
        fn post(
            &self,
            vendor: cloud::Vendor,
            body: Vec<u8>,
            secret: &cloud::Secret,
            _: Instant,
            _: Instant,
            _: &AtomicBool,
        ) -> std::result::Result<cloud::HttpResponse, cloud::HttpFailure> {
            self.posts.fetch_add(1, Ordering::SeqCst);
            assert_eq!(secret.expose(), SECRET.as_bytes());
            let input: Value = serde_json::from_slice(&body).unwrap();
            assert!(String::from_utf8(body)
                .unwrap()
                .contains(cloud::VALIDATION_TEXT));
            let response = match vendor {
                cloud::Vendor::Openai => {
                    json!({"object":"response","model":input["model"],"status":"completed","error":null,"incomplete_details":null,
                    "output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Synthetic hello."}]}]})
                }
                cloud::Vendor::Anthropic => {
                    json!({"type":"message","role":"assistant","model":input["model"],"content":[{"type":"text","text":"Synthetic hello."}],"stop_reason":"end_turn","stop_sequence":null})
                }
            };
            Ok(cloud::HttpResponse {
                status: self.status.lock().unwrap().unwrap_or(200),
                header_bytes: 0,
                body: serde_json::to_vec(&response).unwrap(),
            })
        }
    }
    struct Local;
    impl smith_service::providers::Provider for Local {
        fn inventory(&self) -> AppResult<Inventory> {
            Ok(serde_json::from_value(inventory()).unwrap())
        }
        fn chat(&self, model: &str, _: &[Message]) -> AppResult<String> {
            assert_eq!(model, runner::DEFAULT_MODEL);
            Ok("Synthetic local answer.".into())
        }
    }
    struct Domain(Arc<Application>);
    impl Backend for Domain {
        fn inspect(&mut self) -> Result<Observation> {
            Ok(facts())
        }
        fn prepare(&mut self, _: CoreAction) -> Result<Observation> {
            panic!("No live manager")
        }
        fn apply(&mut self, _: CoreAction, _: &str) -> Result<Observation> {
            panic!("No live manager")
        }
        fn app(&mut self, request: AppRequest) -> Result<AppReply> {
            let version = request.schema_version;
            let call = request.call_id.clone();
            Ok(AppReply::from_versioned_result(
                version,
                &self.0.instance_id,
                &call,
                self.0.call(request),
            ))
        }
    }
    fn open(fixture: &Fixture, capabilities: &Arc<Capabilities>) -> Arc<Application> {
        Arc::new(
            Application::open_with_cloud(
                &fixture.roots,
                Arc::new(Local),
                cloud::Dependencies::new(capabilities.clone(), capabilities.clone()),
            )
            .unwrap(),
        )
    }
    #[test]
    fn actual_v3_domain_roundtrip_with_fake_capabilities_then_local_model_management() {
        let fixture = Fixture::new();
        let capabilities = Arc::new(Capabilities::default());
        let app = open(&fixture, &capabilities);
        let mut manager = Manager::open(Native::new_v3(Box::new(Domain(app.clone())))).unwrap();
        assert_eq!(capabilities.reads.load(Ordering::SeqCst), 0);
        assert_eq!(capabilities.posts.load(Ordering::SeqCst), 0);
        assert_eq!(manager.catalog.models.len(), cloud::MODELS.len());
        for (choice, model) in manager.catalog.models.iter().zip(cloud::MODELS) {
            assert_eq!(choice.route.provider, model.vendor.name());
            assert_eq!(choice.route.model, model.id);
            assert_eq!(choice.label, model.label);
            assert!(!choice.available && !choice.validated_model);
        }
        for provider in Provider::ALL {
            saved(&mut manager, provider);
            assert_eq!(capabilities.posts.load(Ordering::SeqCst), 0);
            assert!(manager.catalog.provider(provider).validation.is_none());
            assert!(manager
                .catalog
                .choices(provider)
                .all(|c| c.available && !c.validated_model));
        }
        drop(manager);
        drop(app);
        let app = open(&fixture, &capabilities);
        let mut manager = Manager::open(Native::new_v3(Box::new(Domain(app.clone())))).unwrap();
        for provider in Provider::ALL {
            assert_eq!(
                manager.catalog.provider(provider).credential_state,
                "not_observed"
            );
            assert!(manager.catalog.provider(provider).validation.is_none());
            assert!(manager
                .catalog
                .choices(provider)
                .all(|c| c.available && !c.validated_model));
        }
        assert_eq!(capabilities.reads.load(Ordering::SeqCst), 0);
        assert_eq!(capabilities.posts.load(Ordering::SeqCst), 0);
        for (i, provider) in Provider::ALL.into_iter().enumerate() {
            let model = manager
                .catalog
                .choices(provider)
                .next()
                .unwrap()
                .route
                .model
                .clone();
            let outcome = manager
                .perform(Action::Validate { provider, model }, &mut approve, &|| {
                    false
                })
                .unwrap();
            assert!(outcome.success, "{}", outcome.message);
            let reads = capabilities.reads.load(Ordering::SeqCst);
            manager.refresh().unwrap();
            assert_eq!(capabilities.reads.load(Ordering::SeqCst), reads);
            assert_eq!(capabilities.posts.load(Ordering::SeqCst), i + 1);
            for choice in manager.catalog.choices(provider) {
                assert!(choice.available);
                assert_eq!(
                    choice.validated_model,
                    choice.route.model == default_model(provider)
                );
            }
        }
        assert_eq!(capabilities.posts.load(Ordering::SeqCst), 2);
        let mut legacy = Native::new(Box::new(Domain(app.clone())));
        assert!(legacy
            .configuration()
            .unwrap_err()
            .contains("UPGRADE_REQUIRED"));
        let mut local = Native::new_v3(Box::new(Domain(app.clone())));
        let config = local.configuration().unwrap();
        assert!(config.model.is_none());
        assert!(local.models().unwrap().contains(runner::DEFAULT_MODEL));
        let revision = local
            .configure_checked(runner::DEFAULT_MODEL, config.revision)
            .unwrap();
        local
            .model_operation("verify_model", runner::DEFAULT_MODEL, revision, &|| false)
            .unwrap();
        assert_eq!(
            capabilities.posts.load(Ordering::SeqCst),
            2,
            "Local model work stays local after v3 jobs"
        );
        let disk =
            fs::read_to_string(fixture.roots.data.join("smith-next/app-state.json")).unwrap();
        assert!(!disk.contains(SECRET));
    }
    #[test]
    fn cold_validation_read_failure_repairs_without_probe_and_auth_fails_once() {
        for provider in Provider::ALL {
            let fixture = Fixture::new();
            let capabilities = Arc::new(Capabilities::default());
            let app = open(&fixture, &capabilities);
            let mut manager = Manager::open(Native::new_v3(Box::new(Domain(app.clone())))).unwrap();
            saved(&mut manager, provider);
            let generation = manager
                .catalog
                .provider(provider)
                .credential_generation
                .clone();
            drop(manager);
            drop(app);
            let app = open(&fixture, &capabilities);
            let mut manager = Manager::open(Native::new_v3(Box::new(Domain(app.clone())))).unwrap();
            assert_eq!(
                manager.catalog.provider(provider).credential_state,
                "not_observed"
            );
            assert_eq!(capabilities.reads.load(Ordering::SeqCst), 0);
            assert_eq!(capabilities.posts.load(Ordering::SeqCst), 0);
            *capabilities.read_failure.lock().unwrap() = Some("KEYRING_LOCKED");
            let failed = manager
                .perform(
                    Action::Validate {
                        provider,
                        model: default_model(provider).into(),
                    },
                    &mut approve,
                    &|| false,
                )
                .unwrap();
            assert!(!failed.success && failed.message.contains("not sent"));
            assert!(!failed.message.contains("read-error canary"));
            assert!(manager.pending_request().is_none());
            assert_eq!(capabilities.reads.load(Ordering::SeqCst), 1);
            assert_eq!(capabilities.posts.load(Ordering::SeqCst), 0);
            manager.refresh().unwrap();
            let observed = manager.catalog.provider(provider);
            assert_eq!(observed.credential_generation, generation);
            assert_eq!(observed.credential_state, "locked");
            assert_eq!(observed.validation.as_ref().unwrap().status, "failed");
            assert!(manager
                .catalog
                .choices(provider)
                .all(|c| c.available && !c.validated_model));
            *capabilities.read_failure.lock().unwrap() = None;
            let repaired = manager
                .perform(
                    Action::Validate {
                        provider,
                        model: default_model(provider).into(),
                    },
                    &mut approve,
                    &|| false,
                )
                .unwrap();
            assert!(repaired.success, "{}", repaired.message);
            assert_eq!(capabilities.reads.load(Ordering::SeqCst), 2);
            assert_eq!(capabilities.posts.load(Ordering::SeqCst), 1);
            *capabilities.status.lock().unwrap() = Some(401);
            let failed = manager
                .perform(
                    Action::Validate {
                        provider,
                        model: default_model(provider).into(),
                    },
                    &mut approve,
                    &|| false,
                )
                .unwrap();
            assert!(
                !failed.success
                    && failed.message.contains("authentication")
                    && failed.message.contains("response received")
            );
            assert!(manager.pending_request().is_none());
            manager.refresh().unwrap();
            assert!(manager
                .catalog
                .choices(provider)
                .all(|c| c.available && !c.validated_model));
            assert_eq!(capabilities.reads.load(Ordering::SeqCst), 3);
            assert_eq!(capabilities.posts.load(Ordering::SeqCst), 2);
            assert_eq!(capabilities.items.lock().unwrap().len(), 1);
        }
    }
}
