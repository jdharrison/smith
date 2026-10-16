//! Schema-3 installer client. All keyring and HTTP work belongs to the managed
//! service. These methods use the existing Controller-backed Native exchange.
use crate::{consent, secret::Secret, setup::Native, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use smith_service::{app, cloud};
use std::collections::BTreeSet;
use std::fmt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SAFE: u64 = 9_007_199_254_740_991;
pub const OBSERVATION_BUDGET: Duration = Duration::from_secs(210);
const UNKNOWN_OUTCOME: &str = "Outcome unknown. Observe the original request; do not repeat a save or validation. Earlier effects remain.";

#[derive(Debug)]
pub enum Failure {
    Service(String),
    Unknown(&'static str),
}
impl Failure {
    pub(crate) fn unknown(message: &'static str) -> Self {
        Self::Unknown(message)
    }
    pub fn guidance(&self) -> &'static str {
        match self {
            Self::Service(code) => guidance(code),
            Self::Unknown(_) => UNKNOWN_OUTCOME,
        }
    }
    fn provider_admission_rejected(&self) -> bool {
        // These categories arise only before provider admission commits. Storage,
        // wake, keyring and unknown failures can follow durable admission; even a
        // structured error must retain the original request for observation.
        matches!(self, Self::Service(code) if matches!(code.as_str(),
            "INVALID_REQUEST" | "BUSY" | "STALE_REVISION" | "CONSENT_EXPIRED"
                | "CONSENT_MISMATCH" | "INSTANCE_MISMATCH" | "CAPACITY"))
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(code) => write!(f, "App operation failed: {code}; no automatic resend"),
            Self::Unknown(message) => f.write_str(message),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Openai,
    Anthropic,
}
impl Provider {
    pub const ALL: [Self; 2] = [Self::Openai, Self::Anthropic];
    pub fn id(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI API",
            Self::Anthropic => "Anthropic API",
        }
    }
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Openai => "https://api.openai.com/v1/responses",
            Self::Anthropic => "https://api.anthropic.com/v1/messages",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Save { provider: Provider, secret: Secret },
    Validate { provider: Provider, model: String },
    Probe { provider: Provider },
    Observe,
    Refresh,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorView {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    pub request_id: String,
    pub model: String,
    pub credential_generation: String,
    pub status: String,
    pub completed_ms: u64,
    pub error: Option<ErrorView>,
    pub delivery: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderView {
    pub provider: Provider,
    pub revision: u64,
    pub credential_generation: Option<String>,
    pub credential_state: String,
    pub credential_error: Option<ErrorView>,
    pub validation: Option<Validation>,
}
impl ProviderView {
    pub fn status(&self) -> String {
        let credential = match self.credential_state.as_str() {
            "not_configured" => "Not configured",
            "available" => "Key saved; last local keyring check succeeded",
            "not_observed" => "Key saved; local keyring not yet checked in this service session",
            "locked" => "Key saved; last keyring check: locked — unlock externally",
            "missing" => {
                "Key saved; last keyring check: exact item missing — repair externally or replace"
            }
            "conflict" => {
                "Key saved; last keyring check: conflict — repair externally; no automatic cleanup"
            }
            _ => "Key saved; last keyring check: unavailable — repair the session externally",
        };
        let check = match &self.validation {
            None => "Not validated; no successful API check recorded".into(),
            Some(v) if v.status == "succeeded" => format!("Validated model: {}. Past check only, not continuing availability or billing readiness", v.model),
            Some(v) => format!("Validation {} for {}. {} Delivery: {}", v.status, v.model,
                v.error.as_ref().map_or("No successful check.", |e| guidance(&e.code)), v.delivery),
        };
        let eligibility = if self.credential_generation.is_some() {
            "\nSaved key: eligible to attempt Send or Validate. Cached checks are advisory; each attempt reads the exact saved credential and can fail."
        } else {
            ""
        };
        format!(
            "{}: {credential}\n{check}{eligibility}",
            self.provider.label()
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub provider: String,
    pub model: String,
    pub credential_generation: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelChoice {
    pub group: String,
    pub label: String,
    pub route: Route,
    pub available: bool,
    pub unavailable_reason: Option<ErrorView>,
    pub validated_model: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub config_revision: Value,
    pub inventory_observed: bool,
    pub providers: Vec<ProviderView>,
    pub models: Vec<ModelChoice>,
}
impl Catalog {
    pub fn parse(value: Value) -> Result<Self> {
        fields(
            &value,
            &[
                "config_revision",
                "inventory_observed",
                "providers",
                "models",
            ],
        )?;
        config_revision(&value["config_revision"])?;
        let providers = value["providers"]
            .as_array()
            .ok_or("Invalid provider catalog")?;
        if providers.len() != 2 {
            return Err("Expected exactly two providers".into());
        }
        for (i, provider) in providers.iter().enumerate() {
            let view = provider_view(provider.clone())?;
            if view.provider != Provider::ALL[i] {
                return Err("Provider catalog order/identity mismatch".into());
            }
        }
        let models = value["models"].as_array().ok_or("Invalid model catalog")?;
        if models.len() > 272 {
            return Err("Model catalog exceeds bound".into());
        }
        let mut seen = BTreeSet::new();
        let mut counts = [0usize; 3];
        let mut last = 0;
        for model in models {
            fields(
                model,
                &[
                    "group",
                    "label",
                    "route",
                    "available",
                    "unavailable_reason",
                    "validated_model",
                ],
            )?;
            check_route(&model["route"], true)?;
            if !plain(&model["label"], 256) {
                return Err("Invalid model label".into());
            }
            let provider = model["route"]["provider"]
                .as_str()
                .ok_or("Invalid route provider")?;
            let index = match provider {
                "local" => 0,
                "openai" => 1,
                "anthropic" => 2,
                _ => return Err("Unknown provider".into()),
            };
            if index < last || model["group"] != if index == 0 { "local" } else { "cloud" } {
                return Err("Invalid catalog grouping/order".into());
            }
            last = index;
            counts[index] += 1;
            if counts[index] > if index == 0 { 256 } else { 8 } {
                return Err("Provider model bound exceeded".into());
            }
            if !seen.insert((
                provider,
                model["route"]["model"].as_str().ok_or("Invalid model")?,
            )) {
                return Err("Duplicate model choice".into());
            }
            let available = model["available"]
                .as_bool()
                .ok_or("Invalid model availability")?;
            let validated = model["validated_model"]
                .as_bool()
                .ok_or("Invalid model verification")?;
            if available != model["unavailable_reason"].is_null() {
                return Err("Invalid availability/error facts".into());
            }
            check_error(&model["unavailable_reason"])?;
            if index == 0 {
                if validated || (available && value["inventory_observed"] != true) {
                    return Err("Local availability requires observed inventory, not inference verification".into());
                }
            } else {
                let view = provider_view(providers[index - 1].clone())?;
                if model["route"]["credential_generation"]
                    != providers[index - 1]["credential_generation"]
                {
                    return Err("Catalog credential generation mismatch".into());
                }
                let success = view
                    .validation
                    .as_ref()
                    .is_some_and(|v| v.status == "succeeded");
                if available != view.credential_generation.is_some()
                    || validated
                        != (success
                            && view
                                .validation
                                .as_ref()
                                .is_some_and(|v| model["route"]["model"] == v.model))
                {
                    return Err("Catalog attempt eligibility/validation mismatch".into());
                }
            }
        }
        serde_json::from_value(value).map_err(|_| "Invalid typed provider catalog".into())
    }
    pub fn provider(&self, provider: Provider) -> &ProviderView {
        &self.providers[if provider == Provider::Openai { 0 } else { 1 }]
    }
    pub fn choices(&self, provider: Provider) -> impl Iterator<Item = &ModelChoice> {
        self.models
            .iter()
            .filter(move |m| m.route.provider == provider.id())
    }
    pub fn summary(&self) -> String {
        self.providers
            .iter()
            .map(ProviderView::status)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

pub(crate) fn fields(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("Invalid schema-3 object")?;
    if object.len() != keys.len() || keys.iter().any(|k| !object.contains_key(*k)) {
        return Err("Unexpected schema-3 fields".into());
    }
    Ok(())
}
fn plain(v: &Value, bound: usize) -> bool {
    v.as_str().is_some_and(|s| {
        !s.trim().is_empty()
            && s.len() <= bound
            && s.chars().all(|c| {
                !c.is_control() && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
    })
}
pub(crate) fn id(value: &Value) -> bool {
    value.as_str().is_some_and(app::valid_id)
}
pub(crate) fn safe(value: &Value) -> bool {
    value.as_u64().is_some_and(|n| n <= SAFE)
}
pub(crate) fn wire_revision(revision: u64) -> Value {
    if revision <= SAFE {
        json!(revision)
    } else {
        json!(revision.to_string())
    }
}
pub(crate) fn config_revision(value: &Value) -> Result<u64> {
    if let Some(n) = value.as_u64().filter(|n| *n <= SAFE) {
        return Ok(n);
    }
    if let Some(s) = value.as_str() {
        if let Ok(n) = s.parse::<u64>() {
            if n > SAFE && n.to_string() == s {
                return Ok(n);
            }
        }
    }
    Err("Invalid lossless configuration revision".into())
}
fn check_error(value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    fields(value, &["code", "message"])?;
    let code = value["code"].as_str().ok_or("Invalid error code")?;
    if code.is_empty()
        || code.len() > 256
        || !code.as_bytes()[0].is_ascii_uppercase()
        || !code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        || !plain(&value["message"], 1024)
    {
        return Err("Invalid bounded service error".into());
    }
    Ok(())
}
fn provider_view(value: Value) -> Result<ProviderView> {
    fields(
        &value,
        &[
            "provider",
            "revision",
            "credential_generation",
            "credential_state",
            "credential_error",
            "validation",
        ],
    )?;
    if !safe(&value["revision"])
        || !(value["credential_generation"].is_null() || id(&value["credential_generation"]))
    {
        return Err("Invalid provider revision/generation".into());
    }
    let state = value["credential_state"]
        .as_str()
        .ok_or("Invalid credential state")?;
    if ![
        "not_configured",
        "not_observed",
        "available",
        "locked",
        "unavailable",
        "missing",
        "conflict",
    ]
    .contains(&state)
        || value["credential_error"].is_null() != ["not_configured", "available"].contains(&state)
        || (state == "not_configured") != value["credential_generation"].is_null()
    {
        return Err("Invalid credential observation".into());
    }
    check_error(&value["credential_error"])?;
    let v = &value["validation"];
    if !v.is_null() {
        fields(
            v,
            &[
                "request_id",
                "model",
                "credential_generation",
                "status",
                "completed_ms",
                "error",
                "delivery",
            ],
        )?;
        let success = v["status"] == "succeeded";
        if !id(&v["request_id"])
            || !reviewed_model(&value["provider"], &v["model"])
            || !id(&v["credential_generation"])
            || v["credential_generation"] != value["credential_generation"]
            || !safe(&v["completed_ms"])
            || !["succeeded", "failed", "interrupted"].contains(&v["status"].as_str().unwrap_or(""))
            || success != v["error"].is_null()
            || !["not_sent", "may_have_sent", "received"]
                .contains(&v["delivery"].as_str().unwrap_or(""))
            || (success && v["delivery"] != "received")
        {
            return Err("Invalid validation facts".into());
        }
        check_error(&v["error"])?;
    }
    serde_json::from_value(value).map_err(|_| "Invalid provider view".into())
}
fn reviewed_model(provider: &Value, model: &Value) -> bool {
    let vendor = match provider.as_str() {
        Some("openai") => cloud::Vendor::Openai,
        Some("anthropic") => cloud::Vendor::Anthropic,
        _ => return false,
    };
    model
        .as_str()
        .is_some_and(|id| cloud::model(vendor, id).is_some())
}
fn check_route(value: &Value, unconfigured: bool) -> Result<()> {
    fields(value, &["provider", "model", "credential_generation"])?;
    if !plain(&value["model"], 256) {
        return Err("Invalid route model".into());
    }
    match value["provider"].as_str() {
        Some("local") if value["credential_generation"].is_null() => Ok(()),
        Some("openai" | "anthropic")
            if reviewed_model(&value["provider"], &value["model"])
                && (id(&value["credential_generation"])
                    || (unconfigured && value["credential_generation"].is_null())) =>
        {
            Ok(())
        }
        _ => Err("Invalid route identity".into()),
    }
}
pub(crate) fn check_operation(value: &Value, terminal: bool) -> Result<()> {
    let mut keys = vec![
        "request_id",
        "conversation_id",
        "kind",
        "status",
        "phase",
        "config_revision",
    ];
    if terminal {
        keys.extend(["result", "error"]);
    }
    fields(value, &keys)?;
    config_revision(&value["config_revision"])?;
    if !id(&value["request_id"])
        || !(value["conversation_id"].is_null() || id(&value["conversation_id"]))
        || !plain(&value["phase"], 1024)
        || ![
            "models",
            "verify_model",
            "remove_model",
            "submit",
            "submit_new",
            "speak",
            "transcribe",
            "conversation_rename",
            "conversation_archive",
            "conversation_restore",
            "conversation_delete",
            "provider_probe",
            "provider_save",
            "provider_validate",
            "submit_routed",
            "submit_new_routed",
        ]
        .contains(&value["kind"].as_str().unwrap_or(""))
        || !["queued", "running", "succeeded", "failed", "interrupted"]
            .contains(&value["status"].as_str().unwrap_or(""))
    {
        return Err("Invalid schema-3 operation".into());
    }
    let active = matches!(value["status"].as_str(), Some("queued" | "running"));
    let chat = matches!(
        value["kind"].as_str(),
        Some("submit" | "submit_new" | "submit_routed" | "submit_new_routed")
    );
    let phase_valid = match value["status"].as_str() {
        Some("queued") => chat && value["phase"] == "queued",
        Some("running") => match value["kind"].as_str() {
            Some("models" | "provider_probe") => value["phase"] == "inventory",
            Some("verify_model" | "provider_validate") => value["phase"] == "verification",
            Some("remove_model") => value["phase"] == "removal",
            Some("provider_save") => value["phase"] == "credential",
            Some("transcribe") => value["phase"] == "transcription",
            Some("speak") => value["phase"] == "synthesis",
            Some("submit" | "submit_new") => {
                matches!(value["phase"].as_str(), Some("generation" | "synthesis"))
            }
            Some("submit_routed" | "submit_new_routed") => value["phase"] == "generation",
            _ => false,
        },
        _ => value["phase"] == "complete",
    };
    if !phase_valid {
        return Err("Invalid operation phase".into());
    }
    let conversation = chat
        || value["kind"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("conversation_"));
    let legacy_speech = matches!(value["kind"].as_str(), Some("speak" | "transcribe"));
    if !legacy_speech && conversation == value["conversation_id"].is_null() {
        return Err("Invalid operation conversation binding".into());
    }
    if terminal {
        check_error(&value["error"])?;
        if (active && (!value["result"].is_null() || !value["error"].is_null()))
            || (!active
                && value["status"] == "succeeded"
                && (!value["result"].is_object() || !value["error"].is_null()))
            || (!active
                && value["status"] != "succeeded"
                && (!value["result"].is_null() || value["error"].is_null()))
        {
            return Err("Invalid terminal outcome".into());
        }
    }
    Ok(())
}

pub struct Prepared {
    request: String,
    instance: String,
    token: String,
    pub provider: Provider,
    pub model: Option<String>,
    revision: u64,
    generation: Option<String>,
    pub deadline: Instant,
    pub expires_at_ms: u64,
}
impl Prepared {
    pub fn title(&self) -> String {
        if self.model.is_some() {
            format!("Validate {} model", self.provider.label())
        } else {
            format!(
                "{} {} key",
                if self.generation.is_some() {
                    "Replace"
                } else {
                    "Save"
                },
                self.provider.label()
            )
        }
    }
    pub fn effects(&self) -> Vec<String> {
        if let Some(model) = &self.model {
            vec![format!("Send one request through Smith to {} using exactly {model}.", self.provider.endpoint()),
                "Synthetic message: Say hello in one short sentence. No conversation history is sent and no chat turn is added. API charges and provider processing/retention may apply; no zero-retention or cost guarantee.".into(),
                "180-second service / 210-second observation budget. Stop only ends installer observation, not accepted service work. No retry or provider fallback. Validation applies to this model and credential generation only.".into(),
                format!("Original approval expires at {} ms since epoch; it is never renewed.", self.expires_at_ms)]
        } else {
            vec![format!("{} the {} credential in the existing unlocked Secret Service keyring, at provider revision {}. No API request or validation.", if self.generation.is_some() { "Replace" } else { "Save" }, self.provider.label(), self.revision),
                "A new opaque credential generation is created. Previous keyring items are retained, not deleted or revoked. Existing history is preserved; future cloud context sharing is confirmed separately in chat. Uninstall keeps saved keys.".into(),
                "No automatic unlock, plaintext/environment fallback, cleanup, rollback or retry. An uncertain keyring write may leave an inactive item. Credential removal is not provided in this version; manage/revoke externally.".into(),
                format!("Original approval expires at {} ms since epoch; it is never renewed.", self.expires_at_ms)]
        }
    }
}
#[derive(Clone)]
struct Pending {
    request: String,
    instance: String,
    provider: Provider,
    kind: &'static str,
    phase: &'static str,
    revision: u64,
    generation: Option<String>,
    model: Option<String>,
    config_revision: Option<Value>,
    expires_at_ms: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub success: bool,
    pub message: String,
}

pub struct Manager {
    native: Native,
    pub catalog: Catalog,
    pending: Option<Pending>,
}
impl Manager {
    pub fn open(mut native: Native) -> Result<Self> {
        let facts = native.inspect()?;
        crate::setup::usable(&facts)?;
        if facts.binary_state != "installed"
            || facts.unit_state != "installed"
            || facts.enablement_state != "enabled"
            || facts.runtime_state != "running"
            || facts.bridge_state != "ready"
        {
            return Err("Providers require the compatible installed, enabled, running Smith service. No lifecycle action is performed here.".into());
        }
        let value = native
            .app_v3("route_catalog", json!({}))
            .map_err(|e| e.guidance().to_owned())?;
        let catalog = Catalog::parse(value)?;
        Ok(Self {
            native,
            catalog,
            pending: None,
        })
    }
    pub fn pending_request(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.request.as_str())
    }
    pub fn refresh(&mut self) -> Result<()> {
        let value = self
            .native
            .app_v3("route_catalog", json!({}))
            .map_err(|e| e.guidance().to_owned())?;
        self.catalog = Catalog::parse(value)?;
        Ok(())
    }
    pub fn perform(
        &mut self,
        action: Action,
        confirm: &mut dyn FnMut(&Prepared) -> Result<bool>,
        stopped: &dyn Fn() -> bool,
    ) -> Result<Outcome> {
        if stopped() {
            return Err("Provider observation stopped; earlier effects remain".into());
        }
        if matches!(action, Action::Observe) {
            return self.observe(OBSERVATION_BUDGET, stopped);
        }
        if matches!(action, Action::Refresh) {
            self.refresh()?;
            return Ok(Outcome {
                success: true,
                message: "Cached provider status observed; no keyring or API request.".into(),
            });
        }
        if self.pending.is_some() {
            return Err("An earlier request has an unknown outcome. Observe its original identity; no new effect accepted.".into());
        }
        let (provider, model, secret, kind, phase) = match action {
            Action::Save { provider, secret } => {
                (provider, None, Some(secret), "provider_save", "credential")
            }
            Action::Validate { provider, model } => (
                provider,
                Some(model),
                None,
                "provider_validate",
                "verification",
            ),
            Action::Probe { provider } => (provider, None, None, "provider_probe", "inventory"),
            _ => return Err("No provider operation selected".into()),
        };
        let view = self.catalog.provider(provider);
        let revision = view.revision;
        let generation = view.credential_generation.clone();
        if let Some(model) = &model {
            if generation.is_none()
                || !self
                    .catalog
                    .choices(provider)
                    .any(|m| m.route.model == *model)
            {
                return Err(
                    "Validation requires a saved key and an exact model from the service catalog"
                        .into(),
                );
            }
        }
        let request = app::new_id()?;
        let instance = self.native.instance()?.to_owned();
        let prepared = if kind != "provider_probe" {
            let started = Instant::now();
            let value = self.native.app_v3("provider_prepare", json!({"request_id":request,"instance_id":instance,"provider":provider,"action":if secret.is_some(){"save"}else{"validate"},"model":model,"expected_provider_revision":revision}))
                .map_err(|e| e.guidance().to_owned())?;
            fields(
                &value,
                &[
                    "request_id",
                    "provider",
                    "action",
                    "model",
                    "expected_provider_revision",
                    "credential_generation",
                    "approval_token",
                    "expires_at_ms",
                    "endpoint",
                ],
            )?;
            if value["request_id"] != request
                || value["provider"] != provider.id()
                || value["action"] != if secret.is_some() { "save" } else { "validate" }
                || value["model"] != json!(model)
                || value["expected_provider_revision"] != revision
                || value["credential_generation"] != json!(generation)
                || !id(&value["approval_token"])
                || !safe(&value["expires_at_ms"])
                || value["endpoint"]
                    != if secret.is_some() {
                        Value::Null
                    } else {
                        json!(provider.endpoint())
                    }
            {
                return Err(
                    "Provider preparation identity/effects mismatch; no action approved".into(),
                );
            }
            let expires_at_ms = value["expires_at_ms"].as_u64().ok_or("Invalid expiry")?;
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "Cannot observe approval expiry")?
                .as_millis();
            let remaining_ms = u128::from(expires_at_ms).saturating_sub(now_ms).min(60_000) as u64;
            if remaining_ms == 0 {
                return Err("Provider preparation expired; start a new explicit attempt".into());
            }
            let prepared = Prepared {
                request: request.clone(),
                instance: instance.clone(),
                provider,
                model: model.clone(),
                revision,
                generation: generation.clone(),
                token: value["approval_token"]
                    .as_str()
                    .ok_or("Invalid approval")?
                    .into(),
                expires_at_ms,
                // Wall time may shorten, never renew the original monotonic budget.
                deadline: (started + Duration::from_secs(60))
                    .min(Instant::now() + Duration::from_millis(remaining_ms)),
            };
            if stopped() || Instant::now() >= prepared.deadline {
                return Err("Provider preparation expired; start a new explicit attempt".into());
            }
            if !confirm(&prepared)? {
                return Ok(Outcome {
                    success: true,
                    message:
                        "Provider action declined. No new effect applied; earlier effects remain."
                            .into(),
                });
            }
            if stopped() || Instant::now() >= prepared.deadline {
                return Err("Provider approval expired or stopped; no apply, no renewal".into());
            }
            Some(prepared)
        } else {
            None
        };
        self.pending = Some(Pending {
            request: request.clone(),
            instance: instance.clone(),
            provider,
            kind,
            phase,
            revision,
            generation,
            model,
            config_revision: None,
            expires_at_ms: prepared.as_ref().map(|p| p.expires_at_ms),
        });
        let response = if let Some(p) = prepared {
            // Exactly one secret-bearing apply. Never log/format this Value or
            // request; retained recovery contains public binding only.
            self.native.app_v3("provider_apply", json!({"request_id":p.request,"instance_id":p.instance,"approval_token":p.token,"api_key":secret.as_ref().map(Secret::expose)}))
        } else {
            self.native.app_v3("provider_probe", json!({"request_id":request,"instance_id":instance,"provider":provider,"expected_provider_revision":revision}))
        };
        drop(secret);
        let value = match response {
            Ok(value) => value,
            Err(error) => {
                if error.provider_admission_rejected() {
                    self.pending = None;
                    return Err(error.guidance().into());
                }
                return Err(UNKNOWN_OUTCOME.into());
            }
        };
        check_operation(&value, false)?;
        self.correlate(&value)?;
        if !matches!(value["status"].as_str(), Some("running" | "succeeded")) {
            return Err("Invalid provider admission; observe original request only".into());
        }
        self.pending
            .as_mut()
            .ok_or("Missing request")?
            .config_revision = Some(value["config_revision"].clone());
        self.observe(OBSERVATION_BUDGET, stopped)
    }
    fn correlate(&self, value: &Value) -> Result<()> {
        let p = self
            .pending
            .as_ref()
            .ok_or("No provider request to observe")?;
        if value["request_id"] != p.request
            || value["kind"] != p.kind
            || !value["conversation_id"].is_null()
            || p.config_revision
                .as_ref()
                .is_some_and(|r| *r != value["config_revision"])
            || (value["status"] == "running" && value["phase"] != p.phase)
        {
            return Err("Provider operation identity/revision mismatch; do not resend".into());
        }
        Ok(())
    }
    pub fn observe(&mut self, budget: Duration, stopped: &dyn Fn() -> bool) -> Result<Outcome> {
        if budget > OBSERVATION_BUDGET {
            return Err("Provider observation budget exceeded".into());
        }
        let deadline = Instant::now() + budget;
        loop {
            if stopped() || Instant::now() >= deadline {
                return Err("Provider observation stopped/timed out. Accepted work may continue; observe the original request, never resend.".into());
            }
            let p = self
                .pending
                .clone()
                .ok_or("No pending provider request to observe")?;
            let value = self
                .native
                .app_v3(
                    "poll",
                    json!({"request_id":p.request,"instance_id":p.instance}),
                )
                .map_err(|e| e.guidance().to_owned())?;
            if stopped() || Instant::now() >= deadline {
                return Err("Provider observation stopped/timed out. Accepted work may continue; observe the original request, never resend.".into());
            }
            check_operation(&value, true)?;
            self.correlate(&value)?;
            if value["status"] == "running" {
                std::thread::sleep(
                    Duration::from_millis(100)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
                continue;
            }
            if value["status"] == "queued" {
                return Err("Provider work must not be queued; observe original request".into());
            }
            let success = value["status"] == "succeeded";
            let message = if success {
                let result = &value["result"];
                fields(
                    result,
                    if p.kind == "provider_validate" {
                        &["provider", "route", "assistant_text"]
                    } else {
                        &["provider"]
                    },
                )?;
                let view = provider_view(result["provider"].clone())?;
                if view.provider != p.provider {
                    return Err("Provider result identity mismatch".into());
                }
                if p.kind == "provider_save" {
                    if view.revision
                        != p.revision
                            .checked_add(1)
                            .ok_or("Provider revision exhausted")?
                        || view.credential_generation.is_none()
                        || view.credential_generation == p.generation
                        || view.credential_state != "available"
                        || view.validation.is_some()
                    {
                        return Err("Saved credential result mismatch; no repeat".into());
                    }
                    "Key saved in keyring. Not validated. No API request made. Previous items are retained, not revoked.".to_owned()
                } else {
                    if view.revision != p.revision || view.credential_generation != p.generation {
                        return Err("Provider result revision/generation mismatch".into());
                    }
                    if p.kind == "provider_validate" {
                        check_route(&result["route"], false)?;
                        let v = view
                            .validation
                            .as_ref()
                            .ok_or("Validation result missing")?;
                        if result["route"]["provider"] != p.provider.id()
                            || result["route"]["model"] != json!(p.model)
                            || result["route"]["credential_generation"] != json!(p.generation)
                            || v.request_id != p.request
                            || Some(&v.model) != p.model.as_ref()
                            || v.status != "succeeded"
                            || !result["assistant_text"]
                                .as_str()
                                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 65536)
                        {
                            return Err("Validation result identity/content mismatch".into());
                        }
                        format!("{} validation succeeded for {}. One synthetic request; no chat turn added. Other models were not individually tested.", p.provider.label(), v.model)
                    } else {
                        format!(
                            "Local keyring observation complete. {} No API request or unlock.",
                            view.status()
                        )
                    }
                }
            } else {
                let code = value["error"]["code"].as_str().ok_or("Missing error")?;
                format!(
                    "{} operation {}. {} Saved keys, local setup and previous effects remain.",
                    p.provider.label(),
                    value["status"].as_str().unwrap_or("failed"),
                    guidance(code)
                )
            };
            let message = if p.kind == "provider_validate" {
                let routing = self.native.app_v3("request_routing", json!({"request_id":p.request,"instance_id":p.instance}))
                    .map_err(|_| "Validation outcome observed but delivery unconfirmed. Observe the original request; never resend.")?;
                fields(
                    &routing,
                    &[
                        "request_id",
                        "route",
                        "delivery",
                        "context_sha256",
                        "dispatch_expires_at_ms",
                    ],
                )?;
                check_route(&routing["route"], false)?;
                if routing["request_id"] != p.request
                    || routing["route"]["provider"] != p.provider.id()
                    || routing["route"]["model"] != json!(p.model)
                    || routing["route"]["credential_generation"] != json!(p.generation)
                    || !routing["context_sha256"].is_null()
                    || routing["dispatch_expires_at_ms"] != json!(p.expires_at_ms)
                    || !["not_sent", "may_have_sent", "received"]
                        .contains(&routing["delivery"].as_str().unwrap_or(""))
                    || (success && routing["delivery"] != "received")
                {
                    return Err("Validation delivery binding mismatch; observe original request, never resend".into());
                }
                let delivery = match routing["delivery"].as_str() {
                    Some("not_sent") => "Delivery: not sent.",
                    Some("received") => "Delivery: provider response received; processing/charges may apply even if validation failed.",
                    _ => "Delivery: may have been sent. Remote outcome unknown; processing/charges may apply. Do not resend automatically.",
                };
                format!("{message} {delivery}")
            } else {
                message
            };
            if stopped() || Instant::now() >= deadline {
                return Err("Provider observation stopped/timed out. Observe the original request, never resend.".into());
            }
            self.pending = None;
            return Ok(Outcome { success, message });
        }
    }
}

/// CLI management shares the same typed adapter as GTK. Confirmation and secret
/// sources are separate: an echoing callback can never receive an API key.
pub fn manage_cli(
    native: Native,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    secret_input: &mut dyn FnMut() -> Result<Secret>,
    output: &mut dyn crate::model::ProgressOutput,
) -> Result<()> {
    let mut manager = Manager::open(native)?;
    let mut console = consent::Console::new(output);
    let mut failed = false;
    console.line("Optional Providers. Local Model settings are unchanged. API credentials and billing are separate from consumer subscriptions. Saved keyring items survive replacement/uninstall; no removal, unlock or automatic API checks.")?;
    loop {
        console.line(&manager.catalog.summary())?;
        let action = if let Some(request) = manager.pending_request() {
            console.line(&format!(
                "Uncertain request: {request}. Never resend its key/apply."
            ))?;
            match confirm("Observe original request or Done? [o/d] ")?.trim() {
                "o" => Action::Observe,
                "d" => break,
                _ => {
                    console.line("Choose o or d; no effect requested.")?;
                    continue;
                }
            }
        } else {
            let provider = match confirm("Provider: OpenAI, Anthropic or Done? [o/a/d] ")?.trim() {
                "o" => Provider::Openai,
                "a" => Provider::Anthropic,
                "d" => break,
                _ => {
                    console.line("Choose o, a or d; no effect requested.")?;
                    continue;
                }
            };
            match confirm("Save/replace key, Validate model, Probe local keyring, Refresh cached status or Done? [s/v/p/r/d] ")?.trim() {
                "s" => Action::Save { provider, secret: secret_input()? },
                "v" => {
                    for (i, choice) in manager.catalog.choices(provider).enumerate() {
                        console.line(&format!("{}: {} ({})", i + 1, choice.label, choice.route.model))?;
                    }
                    let answer = confirm("Choose exact model number for validation (selection is not consent): ")?;
                    let model = answer.trim().parse::<usize>().ok().and_then(|i| i.checked_sub(1))
                        .and_then(|i| manager.catalog.choices(provider).nth(i)).map(|m| m.route.model.clone());
                    let Some(model) = model else { console.line("Invalid model selection; no request prepared.")?; continue; };
                    Action::Validate { provider, model }
                }
                "p" => Action::Probe { provider },
                "r" => Action::Refresh,
                "d" => break,
                _ => { console.line("Unrecognized action; no effect requested.")?; continue; }
            }
        };
        let result = manager.perform(
            action,
            &mut |prepared| {
                let prompt = format!(
                    "{}\n{}\nApprove this exact effect only? [y/N] ",
                    prepared.title(),
                    prepared.effects().join("\n")
                );
                consent::matches(&confirm(&prompt)?, "y")
            },
            &consent::cancelled,
        );
        match result {
            Ok(outcome) => {
                failed |= !outcome.success;
                console.line(&outcome.message)?;
            }
            Err(error) => {
                failed = true;
                console.line(&error)?;
            }
        }
        consent::check()?;
        // Cache-only observation, never a keyring probe or API validation.
        if let Err(error) = manager.refresh() {
            console.line(&error)?;
            failed = true;
        }
    }
    console.line("Provider management ended. Saved keys and local setup remain; skipping is not validation. No automatic app launch.")?;
    if failed || manager.pending_request().is_some() {
        Err("Provider management had failed/uncertain effects; inspect the reported outcomes. Local installation was not changed.".into())
    } else {
        Ok(())
    }
}

pub fn run_cli(
    bundle: std::path::PathBuf,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut dyn crate::model::ProgressOutput,
) -> Result<()> {
    crate::setup::prerequisites()?;
    let native = Native::new_v3(Box::new(smith_service::Controller {
        roots: smith_service::Roots::environment()?,
        source: bundle.join("payload/service/smith-service"),
        manager: smith_service::SystemManager,
    }));
    manage_cli(native, confirm, &mut crate::secret::prompt, output)
}

fn guidance(code: &str) -> &'static str {
    match code {
        "BUSY" => "Smith has pending work. No new provider operation admitted; wait and explicitly try again.",
        "KEYRING_LOCKED" | "KEYRING_PROMPT_REQUIRED" => "Keyring locked or requires a prompt. Unlock externally before a new explicit attempt; Smith never unlocks it. No preliminary probe is required.",
        "KEYRING_UNAVAILABLE" | "KEYRING_TIMEOUT" => "The exact saved key could not be read. Repair the session externally before a new explicit attempt; no preliminary probe or fallback storage.",
        "NOT_OBSERVED" => "Local keyring availability has not been checked in this service instance. A saved key permits an explicit Send or Validate attempt, which reads it afresh.",
        "KEYRING_MISSING" => "The exact saved credential could not be found. Repair externally or explicitly replace it; no other credential was used.",
        "PROVIDER_NOT_CONFIGURED" => "No active saved credential is configured. Save a key explicitly in the installer.",
        "KEYRING_CONFLICT" | "CREDENTIAL_CHANGED" => "Credential identity changed or conflicts. Inspect the keyring externally; no automatic cleanup or substitution.",
        "CREDENTIAL_OUTCOME_UNKNOWN" => "Credential write outcome unknown. A retained inactive keyring item may exist. Never automatically repeat the save.",
        "REMOTE_OUTCOME_UNKNOWN" => "Remote outcome unknown. The provider may have received the request and charged for it; never automatically resend.",
        "CLOUD_AUTH" => "Provider rejected authentication or access. Check the API key and account/model access separately.",
        "CLOUD_QUOTA" => "Provider quota or billing rejected the request. Check the account separately; no retry.",
        "CLOUD_RATE_LIMIT" => "Provider rate limit reached. Try only through a new explicit validation later; no automatic retry.",
        "CLOUD_UNAVAILABLE" => "Provider unavailable. No fallback or retry was performed.",
        "CONSENT_EXPIRED" | "PERMISSION_EXPIRED" => "Original approval expired. No renewal; inspect the outcome before a new explicit attempt.",
        "STALE_REVISION" | "INSTANCE_MISMATCH" | "CONSENT_MISMATCH" => "Service/configuration changed. Observe current state and prepare a new explicit action; do not reuse consent.",
        "UPGRADE_REQUIRED" => "A compatible provider-aware Smith service is required. Upgrade separately; no protocol fallback.",
        "REQUEST_EXPIRED" => "Original request receipt unavailable. Outcome is not proven absent; inspect before making another request.",
        _ => "Provider operation failed or was interrupted. No automatic retry; inspect saved status before another explicit action.",
    }
}
