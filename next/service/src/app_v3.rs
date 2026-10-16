//! Schema 3 routing, explicit permissions and key-free immutable accepted work.
use super::*;
use crate::cloud::{self, CredentialRef, Secret, Vendor};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use v2::{blocked, capacity, missing, stale, MAX_REVISION};
#[cfg(test)]
#[path = "app_v3_tests.rs"]
mod tests;
#[path = "app_v3_worker.rs"]
mod worker;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Route {
    provider: String,
    model: String,
    #[serde(deserialize_with = "required_nullable")]
    credential_generation: Option<String>,
}
impl Route {
    fn vendor(&self) -> Option<Vendor> {
        Vendor::ALL.into_iter().find(|v| v.name() == self.provider)
    }
    fn valid_history(&self) -> bool {
        !self.model.is_empty()
            && self.model.len() <= 256
            && !self.model.chars().any(char::is_control)
            && if self.provider == "local" {
                self.credential_generation.is_none()
            } else {
                self.vendor().is_some()
                    && self
                        .credential_generation
                        .as_ref()
                        .is_some_and(|s| valid_id(s))
            }
    }
    fn destination(&self) -> Option<Destination> {
        Some(Destination {
            provider: self.vendor()?,
            credential_generation: self.credential_generation.clone()?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Destination {
    provider: Vendor,
    credential_generation: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Validation {
    request_id: String,
    model: String,
    credential_generation: String,
    status: String,
    completed_ms: u64,
    #[serde(deserialize_with = "required_nullable")]
    error: Option<AppError>,
    delivery: Delivery,
}
impl Validation {
    fn valid(&self) -> bool {
        valid_id(&self.request_id)
            && valid_id(&self.credential_generation)
            && !self.model.is_empty()
            && self.model.len() <= 256
            && !self.model.chars().any(char::is_control)
            && self.completed_ms <= MAX_REVISION
            && match self.status.as_str() {
                "succeeded" => self.error.is_none() && self.delivery == Delivery::Received,
                "failed" | "interrupted" => {
                    self.error.as_ref().is_some_and(AppError::valid)
                        && self.delivery != Delivery::NotApplicable
                }
                _ => false,
            }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderSlot {
    provider: Vendor,
    revision: u64,
    #[serde(deserialize_with = "required_nullable")]
    credential: Option<CredentialRef>,
    #[serde(deserialize_with = "required_nullable")]
    validation: Option<Validation>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderView {
    provider: Vendor,
    revision: u64,
    #[serde(deserialize_with = "required_nullable")]
    credential_generation: Option<String>,
    credential_state: String,
    #[serde(deserialize_with = "required_nullable")]
    credential_error: Option<AppError>,
    #[serde(deserialize_with = "required_nullable")]
    validation: Option<Validation>,
}
impl ProviderView {
    fn valid(&self) -> bool {
        self.revision <= MAX_REVISION
            && self
                .credential_generation
                .as_ref()
                .is_none_or(|s| valid_id(s))
            && self.validation.as_ref().is_none_or(|v| {
                v.valid() && Some(&v.credential_generation) == self.credential_generation.as_ref()
            })
            && match self.credential_state.as_str() {
                "not_configured" => {
                    self.credential_generation.is_none()
                        && self.credential_error.is_none()
                        && self.validation.is_none()
                }
                "available" => {
                    self.credential_generation.is_some() && self.credential_error.is_none()
                }
                "not_observed" | "locked" | "unavailable" | "missing" | "conflict" => {
                    self.credential_generation.is_some()
                        && self.credential_error.as_ref().is_some_and(AppError::valid)
                }
                _ => false,
            }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    #[serde(deserialize_with = "required_nullable")]
    request_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    route: Option<Route>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Routing {
    #[serde(deserialize_with = "required_nullable")]
    last_route: Option<Route>,
    approved_destinations: Vec<Destination>,
    turn_routes: Vec<Provenance>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CloudDurable {
    providers: [ProviderSlot; 2],
    pub(super) conversations: BTreeMap<String, Routing>,
}
impl Default for CloudDurable {
    fn default() -> Self {
        Self {
            providers: Vendor::ALL.map(|provider| ProviderSlot {
                provider,
                revision: 0,
                credential: None,
                validation: None,
            }),
            conversations: BTreeMap::new(),
        }
    }
}
impl CloudDurable {
    pub(super) fn validate(&self, d: &Durable) -> Result<()> {
        let fail = || "Invalid cloud state; preserved".to_string();
        for (i, p) in self.providers.iter().enumerate() {
            if p.provider.index() != i
                || p.revision > MAX_REVISION
                || (p.credential.is_none() && (p.revision != 0 || p.validation.is_some()))
                || p.credential
                    .as_ref()
                    .is_some_and(|c| c.provider != p.provider || !valid_ref(c) || p.revision == 0)
                || p.validation.as_ref().is_some_and(|v| {
                    !v.valid()
                        || p.credential
                            .as_ref()
                            .is_none_or(|c| v.credential_generation != c.generation)
                })
            {
                return Err(fail());
            }
        }
        for (id, r) in &self.conversations {
            let c = d
                .conversations
                .iter()
                .find(|c| &c.conversation_id == id)
                .ok_or_else(fail)?;
            if r.turn_routes.len() != c.history.len() / 2
                || r.approved_destinations.len() > 16
                || r.last_route.as_ref().is_some_and(|r| !r.valid_history())
            {
                return Err(fail());
            }
            let mut destinations = HashSet::new();
            for a in &r.approved_destinations {
                if !valid_id(&a.credential_generation)
                    || !destinations.insert((a.provider, &a.credential_generation))
                {
                    return Err(fail());
                }
            }
            if r.last_route
                .as_ref()
                .and_then(Route::destination)
                .is_some_and(|d| !r.approved_destinations.contains(&d))
            {
                return Err(fail());
            }
            let mut provenance_ids = HashSet::new();
            for p in &r.turn_routes {
                if p.request_id
                    .as_ref()
                    .is_some_and(|id| !provenance_ids.insert(id))
                    || p.route
                        .as_ref()
                        .and_then(Route::destination)
                        .is_some_and(|d| !r.approved_destinations.contains(&d))
                {
                    return Err(fail());
                }
                if let Some(job) = p
                    .request_id
                    .as_ref()
                    .and_then(|id| d.jobs.iter().find(|j| &j.operation.request_id == id))
                {
                    if job.conversation_id.as_ref() != Some(id)
                        || !matches!(&job.task, Some(Task::Routed(t)) if Some(&t.route) == p.route.as_ref())
                    {
                        return Err(fail());
                    }
                }
                if p.request_id.is_some() != p.route.is_some()
                    || p.request_id.as_ref().is_some_and(|id| !valid_id(id))
                    || p.route.as_ref().is_some_and(|r| !r.valid_history())
                {
                    return Err(fail());
                }
            }
        }
        for j in d.jobs.iter().filter(|j| v2::pending(&j.operation)) {
            match &j.task {
                Some(Task::Routed(t)) => {
                    let id = j.conversation_id.as_ref().ok_or_else(fail)?;
                    let c = d
                        .conversations
                        .iter()
                        .find(|c| &c.conversation_id == id)
                        .ok_or_else(fail)?;
                    let r = self.conversations.get(id).ok_or_else(fail)?;
                    if r.last_route.as_ref() != Some(&t.route)
                        || t.history_revision != c.history_revision
                        || t.config_revision != j.operation.config_revision
                        || !snapshot_context_matches(t, &c.history)
                    {
                        return Err(fail());
                    }
                    if let Some(dest) = t.route.destination() {
                        let slot = &self.providers[dest.provider.index()];
                        if !r.approved_destinations.contains(&dest)
                            || slot.credential != t.credential
                            || Some(slot.revision) != t.provider_revision
                        {
                            return Err(fail());
                        }
                    }
                }
                Some(Task::Provider(t)) => {
                    let p = &self.providers[t.provider.index()];
                    if p.revision != t.provider_revision
                        || (t.action != Action::Save && p.credential != t.credential)
                    {
                        return Err(fail());
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
fn valid_ref(c: &CredentialRef) -> bool {
    valid_id(&c.generation) && valid_id(&c.save_request_id)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Delivery {
    NotApplicable,
    NotSent,
    MayHaveSent,
    Received,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoutedTask {
    route: Route,
    #[serde(deserialize_with = "required_nullable")]
    credential: Option<CredentialRef>,
    #[serde(deserialize_with = "required_nullable")]
    provider_revision: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    local_digest: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    endpoint: Option<String>,
    policy: String,
    config_revision: u64,
    history_revision: u64,
    messages: Vec<Message>,
    context_sha256: String,
    output_tokens: u32,
    budget_ms: u64,
    #[serde(deserialize_with = "required_nullable")]
    dispatch_expires_at_ms: Option<u64>,
    delivery: Delivery,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Probe,
    Save,
    Validate,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProviderTask {
    provider: Vendor,
    action: Action,
    provider_revision: u64,
    #[serde(deserialize_with = "required_nullable")]
    credential: Option<CredentialRef>,
    #[serde(deserialize_with = "required_nullable")]
    model: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    endpoint: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    dispatch_expires_at_ms: Option<u64>,
    policy: String,
    delivery: Delivery,
}
fn digest_messages(messages: &[Message]) -> AppResult<String> {
    Ok(filesystem::digest(
        &serde_json::to_vec(messages).map_err(|_| invalid())?,
    ))
}
fn snapshot_context_matches(t: &RoutedTask, history: &[Message]) -> bool {
    let Some(input) = t.messages.last() else {
        return false;
    };
    let mut expected = history.to_vec();
    while expected.iter().map(|m| m.content.len()).sum::<usize>() + input.content.len()
        > MAX_CONTEXT
    {
        if expected.len() < 2 {
            return false;
        }
        expected.drain(..2);
    }
    expected.push(input.clone());
    digest_messages(&expected).is_ok_and(|h| h == t.context_sha256)
}
fn valid_messages(t: &RoutedTask) -> bool {
    !t.messages.is_empty()
        && t.messages.len() <= 33
        && t.messages.len() % 2 == 1
        && t.messages.iter().map(|m| m.content.len()).sum::<usize>() <= MAX_CONTEXT
        && t.messages.iter().enumerate().all(|(i, m)| {
            m.role == if i % 2 == 0 { "user" } else { "assistant" }
                && m.content.len() <= if i % 2 == 0 { MAX_TEXT } else { MAX_CONTEXT }
        })
        && t.messages
            .last()
            .is_some_and(|m| text(&m.content, MAX_TEXT).is_ok())
        && digest_messages(&t.messages).is_ok_and(|h| h == t.context_sha256)
}
pub(super) fn valid_job(j: &Job) -> bool {
    let o = &j.operation;
    if j.version != 3 || !reply_fits(&j.poll()) {
        return false;
    }
    let valid = match &j.task {
        Some(Task::Routed(t)) => {
            matches!(o.kind.as_str(), "submit_routed" | "submit_new_routed")
                && j.conversation_id.is_some()
                && Some(t.history_revision) == j.history_revision
                && t.config_revision == o.config_revision
                && t.history_revision <= MAX_REVISION
                && t.route.valid_history()
                && valid_messages(t)
                && t.policy == cloud::POLICY
                && t.budget_ms == 180_000
                && if let Some(v) = t.route.vendor() {
                    t.credential.as_ref().is_some_and(|c| {
                        valid_ref(c)
                            && c.provider == v
                            && Some(&c.generation) == t.route.credential_generation.as_ref()
                    }) && t.provider_revision.is_some_and(|r| r <= MAX_REVISION)
                        && t.local_digest.is_none()
                        && t.endpoint.as_deref() == Some(v.endpoint())
                        && t.output_tokens == cloud::OUTPUT_TOKENS
                        && t.dispatch_expires_at_ms.is_some_and(|n| n <= MAX_REVISION)
                        && t.delivery != Delivery::NotApplicable
                } else {
                    t.credential.is_none()
                        && t.provider_revision.is_none()
                        && t.local_digest
                            .as_ref()
                            .is_some_and(|s| model_identity(&t.route.model, s))
                        && t.endpoint.is_none()
                        && t.output_tokens == 1024
                        && t.dispatch_expires_at_ms.is_none()
                        && t.delivery == Delivery::NotApplicable
                }
        }
        Some(Task::Provider(t)) => {
            j.conversation_id.is_none()
                && t.provider_revision <= MAX_REVISION
                && t.policy == cloud::POLICY
                && t.credential
                    .as_ref()
                    .is_none_or(|c| valid_ref(c) && c.provider == t.provider)
                && match t.action {
                    Action::Probe => {
                        o.kind == "provider_probe"
                            && t.model.is_none()
                            && t.endpoint.is_none()
                            && t.dispatch_expires_at_ms.is_none()
                            && t.delivery == Delivery::NotApplicable
                    }
                    Action::Save => {
                        o.kind == "provider_save"
                            && t.credential
                                .as_ref()
                                .is_some_and(|c| c.save_request_id == o.request_id)
                            && t.model.is_none()
                            && t.endpoint.is_none()
                            && t.dispatch_expires_at_ms.is_some_and(|n| n <= MAX_REVISION)
                            && t.delivery == Delivery::NotApplicable
                    }
                    Action::Validate => {
                        o.kind == "provider_validate"
                            && t.credential.is_some()
                            && t.model
                                .as_ref()
                                .is_some_and(|m| !m.is_empty() && m.len() <= 256)
                            && t.endpoint.as_deref() == Some(t.provider.endpoint())
                            && t.dispatch_expires_at_ms.is_some_and(|n| n <= MAX_REVISION)
                            && t.delivery != Delivery::NotApplicable
                    }
                }
        }
        _ => false,
    };
    if !valid {
        return false;
    }
    if v2::pending(o) {
        return o.result.is_none()
            && o.error.is_none()
            && match o.kind.as_str() {
                "submit_routed" | "submit_new_routed" => matches!(
                    (o.status.as_str(), o.phase.as_str()),
                    ("queued", "queued") | ("running", "generation")
                ),
                "provider_probe" => o.status == "running" && o.phase == "inventory",
                "provider_save" => o.status == "running" && o.phase == "credential",
                "provider_validate" => o.status == "running" && o.phase == "verification",
                _ => false,
            };
    }
    if o.phase != "complete" {
        return false;
    }
    if o.status != "succeeded" {
        return o.result.is_none() && o.error.as_ref().is_some_and(AppError::valid);
    }
    if o.error.is_some() {
        return false;
    }
    let Some(result) = o.result.clone() else {
        return false;
    };
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ProviderResult {
        provider: ProviderView,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ValidationResult {
        provider: ProviderView,
        route: Route,
        assistant_text: String,
    }
    match &j.task {
        Some(Task::Routed(t)) => serde_json::from_value::<TurnResult>(result).is_ok_and(|r| {
            !r.assistant_text.trim().is_empty()
                && r.assistant_text.len() <= MAX_CONTEXT
                && r.audio.is_none()
                && r.speech_error.is_none()
                && matches!(t.delivery, Delivery::Received | Delivery::NotApplicable)
        }),
        Some(Task::Provider(t)) if t.action == Action::Validate => {
            serde_json::from_value::<ValidationResult>(result).is_ok_and(|r| {
                r.provider.valid()
                    && r.provider.provider == t.provider
                    && r.route.valid_history()
                    && r.route.provider == t.provider.name()
                    && Some(&r.route.model) == t.model.as_ref()
                    && r.route.credential_generation
                        == t.credential.as_ref().map(|c| c.generation.clone())
                    && !r.assistant_text.trim().is_empty()
                    && r.assistant_text.len() <= MAX_CONTEXT
                    && t.delivery == Delivery::Received
            })
        }
        Some(Task::Provider(t)) => {
            serde_json::from_value::<ProviderResult>(result).is_ok_and(|r| {
                r.provider.valid()
                    && r.provider.provider == t.provider
                    && (t.action != Action::Save
                        || r.provider.credential_generation
                            == t.credential.as_ref().map(|c| c.generation.clone()))
            })
        }
        _ => false,
    }
}
pub(super) fn reserve_delivery(j: &mut Job) {
    match &mut j.task {
        Some(Task::Routed(t)) if t.route.vendor().is_some() => t.delivery = Delivery::MayHaveSent,
        Some(Task::Provider(t)) if t.action == Action::Validate => {
            t.delivery = Delivery::MayHaveSent
        }
        _ => {}
    }
}
pub(super) fn transitions(j: &Job) -> u64 {
    let base = match j.operation.status.as_str() {
        "queued" => 2,
        "running" => 1,
        _ => return 0,
    };
    let needs_marker = match &j.task {
        Some(Task::Routed(t)) => t.route.vendor().is_some() && t.delivery == Delivery::NotSent,
        Some(Task::Provider(t)) => t.action == Action::Validate && t.delivery == Delivery::NotSent,
        _ => false,
    };
    base + u64::from(needs_marker)
}
pub(super) fn restart_error(j: &Job) -> AppError {
    match &j.task {
        Some(Task::Routed(t)) if t.delivery == Delivery::MayHaveSent => {
            cloud::error("REMOTE_OUTCOME_UNKNOWN")
        }
        Some(Task::Provider(t)) if t.action == Action::Save => {
            cloud::error("CREDENTIAL_OUTCOME_UNKNOWN")
        }
        Some(Task::Provider(t)) if t.delivery == Delivery::MayHaveSent => {
            cloud::error("REMOTE_OUTCOME_UNKNOWN")
        }
        _ => cloud::error("INTERRUPTED"),
    }
}
// Called only for a pending job interrupted by this open, never for retained
// receipts: an older interrupted validation must not replace a later result.
pub(super) fn reconcile_validation(cloud: &mut CloudDurable, j: &Job, now: u64) {
    if let Some(Task::Provider(t)) = &j.task {
        if t.action == Action::Validate && j.operation.status == "interrupted" {
            let p = &mut cloud.providers[t.provider.index()];
            if p.credential == t.credential {
                p.validation = Some(validation(j, t, now));
            }
        }
    }
}
fn validation(j: &Job, t: &ProviderTask, now: u64) -> Validation {
    Validation {
        request_id: j.operation.request_id.clone(),
        model: t.model.clone().unwrap_or_default(),
        credential_generation: t
            .credential
            .as_ref()
            .map(|c| c.generation.clone())
            .unwrap_or_default(),
        status: j.operation.status.clone(),
        completed_ms: now,
        error: j.operation.error.clone(),
        delivery: t.delivery,
    }
}

#[derive(Default)]
pub(super) struct Volatile {
    observations: [Option<AppResult<()>>; 2],
    preparations: HashMap<String, Preparation>,
    expired: HashSet<String>,
    jobs: HashMap<String, Live>,
}
struct Live {
    deadline: Instant,
    secret: Option<Secret>,
}
#[derive(Clone)]
struct Preparation {
    fingerprint: String,
    token: String,
    expires: Instant,
    dispatch: Instant,
    result: Value,
    kind: Prepared,
}
#[derive(Clone)]
enum Prepared {
    Provider(ProviderTask),
    Route {
        context: String,
        requires_consent: bool,
    },
}
impl Volatile {
    pub(super) fn complete(&mut self, id: &str) {
        self.jobs.remove(id);
    }
    fn expire(&mut self, now: Instant) {
        self.preparations.retain(|id, p| {
            if now >= p.expires {
                self.expired.insert(id.clone());
                false
            } else {
                true
            }
        });
    }
    fn prepare(&mut self, id: &str, fp: &str, now: Instant) -> AppResult<Option<Value>> {
        self.expire(now);
        if self.expired.contains(id) {
            return Err(cloud::error("CONSENT_EXPIRED"));
        }
        if let Some(p) = self.preparations.get(id) {
            if p.fingerprint != fp {
                return Err(AppError::new(
                    "ID_CONFLICT",
                    "Request identifier is bound to different preparation",
                ));
            }
            return Ok(Some(p.result.clone()));
        }
        // Reserve expiry slots before allocating tokens; never evict/renew an ID.
        if self.preparations.len() >= 32 || self.expired.len() + self.preparations.len() >= 1024 {
            return Err(capacity());
        }
        Ok(None)
    }
}
fn view(slot: &ProviderSlot, observation: Option<&AppResult<()>>) -> ProviderView {
    let (state, error) = if slot.credential.is_none() {
        ("not_configured", None)
    } else {
        match observation {
            None => ("not_observed", Some(cloud::error("NOT_OBSERVED"))),
            Some(Ok(())) => ("available", None),
            Some(Err(e)) => (
                match e.code.as_str() {
                    "KEYRING_LOCKED" => "locked",
                    "KEYRING_MISSING" => "missing",
                    "KEYRING_CONFLICT" => "conflict",
                    _ => "unavailable",
                },
                Some(e.clone()),
            ),
        }
    };
    ProviderView {
        provider: slot.provider,
        revision: slot.revision,
        credential_generation: slot.credential.as_ref().map(|c| c.generation.clone()),
        credential_state: state.into(),
        credential_error: error,
        validation: slot.validation.clone(),
    }
}
fn ready(
    inner: &Inner,
    route: &Route,
) -> AppResult<(Option<CredentialRef>, Option<u64>, Option<String>)> {
    if !route.valid_history() {
        return Err(invalid());
    }
    if let Some(v) = route.vendor() {
        if cloud::model(v, &route.model).is_none() {
            return Err(cloud::error("ROUTE_UNAVAILABLE"));
        }
        let p = &inner.durable.cloud.providers[v.index()];
        let credential = p
            .credential
            .as_ref()
            .ok_or_else(|| cloud::error("PROVIDER_NOT_CONFIGURED"))?;
        if route.credential_generation.as_ref() != Some(&credential.generation) {
            return Err(cloud::error("CREDENTIAL_CHANGED"));
        }
        // Cached keyring and validation facts are advisory; execution must freshly
        // retrieve this exact credential before any HTTP attempt.
        Ok((Some(credential.clone()), Some(p.revision), None))
    } else {
        let m = inner
            .inventory
            .models
            .iter()
            .find(|m| m.name == route.model)
            .filter(|_| inner.inventory_observed && inner.inventory.providers.llm.ready)
            .ok_or_else(|| cloud::error("ROUTE_UNAVAILABLE"))?;
        if !crate::catalog::CATALOG
            .iter()
            .any(|e| e.name == m.name && e.sha256 == m.digest)
        {
            return Err(cloud::error("ROUTE_UNAVAILABLE"));
        }
        Ok((None, None, Some(m.digest.clone())))
    }
}
fn require_instance(got: &str, expected: &str) -> AppResult<()> {
    if !valid_id(got) {
        Err(invalid())
    } else if got != expected {
        Err(AppError::new(
            "INSTANCE_MISMATCH",
            "Synchronize without resending",
        ))
    } else {
        Ok(())
    }
}
fn fresh(inner: &Inner, id: &str) -> AppResult<()> {
    if !valid_id(id) {
        return Err(invalid());
    }
    if inner.fault || !inner.owned {
        return Err(storage());
    }
    if inner
        .durable
        .jobs
        .iter()
        .any(|j| j.operation.request_id == id)
    {
        return Err(AppError::new(
            "ID_CONFLICT",
            "Request is already admitted; poll without resending",
        ));
    }
    if inner.durable.expired_ids.iter().any(|i| i == id) {
        return Err(AppError::new(
            "REQUEST_EXPIRED",
            "Expired identifier cannot be reused",
        ));
    }
    if inner.cloud.expired.contains(id) {
        return Err(cloud::error("CONSENT_EXPIRED"));
    }
    Ok(())
}
fn fp(op: &str, args: &Value) -> AppResult<String> {
    fingerprint(&format!("v3:{op}"), args)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeArgs {
    request_id: String,
    instance_id: String,
    provider: Vendor,
    expected_provider_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareProvider {
    request_id: String,
    instance_id: String,
    provider: Vendor,
    action: Action,
    #[serde(deserialize_with = "required_nullable")]
    model: Option<String>,
    expected_provider_revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteArgs {
    request_id: String,
    instance_id: String,
    #[serde(deserialize_with = "required_nullable")]
    conversation_id: Option<String>,
    text: String,
    route: Route,
    #[serde(deserialize_with = "v2::config_revision")]
    expected_config_revision: u64,
    #[serde(deserialize_with = "required_nullable")]
    expected_history_revision: Option<u64>,
}
fn route_args(
    mut args: Value,
    submit: bool,
) -> AppResult<(RouteArgs, Option<String>, bool, Value)> {
    let (token, confirmed) = if submit {
        let o = args.as_object_mut().ok_or_else(invalid)?;
        let token = o.remove("approval_token").ok_or_else(invalid)?;
        let token = match token {
            Value::Null => None,
            Value::String(s) if valid_id(&s) => Some(s),
            _ => return Err(invalid()),
        };
        let confirmed = o
            .remove("confirm_transfer")
            .and_then(|v| v.as_bool())
            .ok_or_else(invalid)?;
        (token, confirmed)
    } else {
        (None, false)
    };
    Ok((decode(args.clone())?, token, confirmed, args))
}
struct RouteCandidate {
    task: RoutedTask,
    requires_consent: bool,
}
fn route_candidate(inner: &Inner, a: &RouteArgs) -> AppResult<RouteCandidate> {
    text(&a.text, MAX_TEXT)?;
    if a.expected_config_revision != inner.durable.config.revision {
        return Err(stale());
    }
    if inner
        .durable
        .jobs
        .iter()
        .filter(|j| v2::pending(&j.operation))
        .count()
        >= v2::MAX_PENDING
        || inner
            .durable
            .jobs
            .iter()
            .any(|j| v2::pending(&j.operation) && j.conversation_id.is_none())
    {
        return Err(blocked());
    }
    let history = if let Some(id) = &a.conversation_id {
        if !valid_id(id) {
            return Err(invalid());
        }
        let c = inner
            .durable
            .conversations
            .iter()
            .find(|c| &c.conversation_id == id)
            .ok_or_else(missing)?;
        if c.archived {
            return Err(AppError::new(
                "CONVERSATION_ARCHIVED",
                "Restore before sending",
            ));
        }
        if Some(c.history_revision) != a.expected_history_revision {
            return Err(stale());
        }
        if c.history_revision >= MAX_REVISION {
            return Err(capacity());
        }
        if inner
            .durable
            .jobs
            .iter()
            .any(|j| v2::pending(&j.operation) && j.conversation_id.as_ref() == Some(id))
        {
            return Err(blocked());
        }
        c.history.clone()
    } else {
        if a.expected_history_revision.is_some() {
            return Err(invalid());
        }
        if inner.durable.conversations.len() >= v2::MAX_CONVERSATIONS {
            return Err(capacity());
        }
        vec![]
    };
    let (credential, provider_revision, local_digest) = ready(inner, &a.route)?;
    let messages = if let Some(v) = a.route.vendor() {
        cloud::context(v, &a.route.model, &history, &a.text)?
    } else {
        let mut h = history;
        while h.iter().map(|m| m.content.len()).sum::<usize>() + a.text.len() > MAX_CONTEXT {
            h.drain(..2);
        }
        h.push(Message {
            role: "user".into(),
            content: a.text.clone(),
        });
        h
    };
    let requires_consent = a.route.destination().is_some_and(|d| {
        a.conversation_id
            .as_ref()
            .and_then(|id| inner.durable.cloud.conversations.get(id))
            .is_none_or(|r| !r.approved_destinations.contains(&d))
    });
    if requires_consent
        && a.conversation_id
            .as_ref()
            .and_then(|id| inner.durable.cloud.conversations.get(id))
            .is_some_and(|r| r.approved_destinations.len() >= 16)
    {
        return Err(capacity());
    }
    let task = RoutedTask {
        route: a.route.clone(),
        credential,
        provider_revision,
        local_digest,
        endpoint: a.route.vendor().map(|v| v.endpoint().into()),
        policy: cloud::POLICY.into(),
        config_revision: a.expected_config_revision,
        history_revision: a.expected_history_revision.unwrap_or(0),
        context_sha256: digest_messages(&messages)?,
        messages,
        output_tokens: if a.route.vendor().is_some() {
            cloud::OUTPUT_TOKENS
        } else {
            1024
        },
        budget_ms: 180_000,
        dispatch_expires_at_ms: None,
        delivery: if a.route.vendor().is_some() {
            Delivery::NotSent
        } else {
            Delivery::NotApplicable
        },
    };
    Ok(RouteCandidate {
        task,
        requires_consent,
    })
}
fn route_result(
    a: &RouteArgs,
    t: &RoutedTask,
    requires: bool,
    token: Option<&str>,
    expires: Option<u64>,
) -> Value {
    json!({"request_id":a.request_id,"conversation_id":a.conversation_id,"route":a.route,"config_revision":v2::wire_config_revision(a.expected_config_revision),"history_revision":a.expected_history_revision,"context_sha256":t.context_sha256,"context_messages":t.messages.len(),"context_bytes":t.messages.iter().map(|m|m.content.len()).sum::<usize>(),"requires_consent":requires,"approval_token":token,"expires_at_ms":expires,"dispatch_expires_at_ms":t.dispatch_expires_at_ms})
}

impl Application {
    pub(super) fn legacy_guard(&self, req: &AppRequest) -> AppResult<()> {
        let inner = self.inner.lock().map_err(|_| storage())?;
        let routed = |id: &str| {
            inner
                .durable
                .cloud
                .conversations
                .get(id)
                .is_some_and(|r| r.last_route.is_some() || !r.approved_destinations.is_empty())
        };
        let guard = match req.op.as_str() {
            "state" => {
                inner.durable.jobs.iter().any(|j| j.version == 3)
                    || inner
                        .durable
                        .cloud
                        .conversations
                        .keys()
                        .any(|id| routed(id))
            }
            "poll" => req
                .args
                .get("request_id")
                .and_then(Value::as_str)
                .is_some_and(|id| {
                    inner
                        .durable
                        .jobs
                        .iter()
                        .any(|j| j.operation.request_id == id && j.version == 3)
                }),
            "submit" if req.schema_version == 1 => inner
                .durable
                .legacy_conversation_id
                .as_ref()
                .is_some_and(|id| routed(id)),
            "conversation_get"
            | "submit"
            | "conversation_rename"
            | "conversation_archive"
            | "conversation_restore"
            | "conversation_delete" => req
                .args
                .get("conversation_id")
                .and_then(Value::as_str)
                .is_some_and(routed),
            _ => false,
        };
        if guard {
            Err(cloud::error("UPGRADE_REQUIRED"))
        } else {
            Ok(())
        }
    }
    pub(super) fn call_v3(&self, mut req: AppRequest) -> AppResult<Value> {
        self.inner
            .lock()
            .map_err(|_| storage())?
            .cloud
            .expire(self.cloud.clock.now());
        match req.op.as_str() {
            "route_catalog" => {
                let _: Empty = decode(req.args)?;
                self.route_catalog()
            }
            "provider_probe" => self.provider_probe(req.args),
            "provider_prepare" => self.provider_prepare(req.args),
            "provider_apply" => self.provider_apply(req.args),
            "conversation_routing" => self.conversation_routing(req.args),
            "conversation_timestamps" => self.conversation_timestamps(req.args),
            "route_prepare" => self.route_prepare(req.args),
            "submit_routed" => self.submit_routed(req.args),
            "request_routing" => self.request_routing(req.args),
            _ => {
                req.schema_version = 2;
                self.call_v2(req)
            }
        }
    }
    fn route_catalog(&self) -> AppResult<Value> {
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault {
            return Err(storage());
        }
        let providers: Vec<_> = inner
            .durable
            .cloud
            .providers
            .iter()
            .map(|p| view(p, inner.cloud.observations[p.provider.index()].as_ref()))
            .collect();
        let mut models = Vec::new();
        for m in crate::catalog::CATALOG {
            if inner.inventory.models.iter().any(|i| i.name == m.name) {
                let route = Route {
                    provider: "local".into(),
                    model: m.name.into(),
                    credential_generation: None,
                };
                let reason = ready(&inner, &route).err();
                models.push(json!({"group":"local","label":m.display_name,"route":route,"available":reason.is_none(),"unavailable_reason":reason,"validated_model":false}));
            }
        }
        for m in cloud::MODELS {
            let p = &inner.durable.cloud.providers[m.vendor.index()];
            let route = Route {
                provider: m.vendor.name().into(),
                model: m.id.into(),
                credential_generation: p.credential.as_ref().map(|c| c.generation.clone()),
            };
            let reason = if p.credential.is_none() {
                Some(cloud::error("PROVIDER_NOT_CONFIGURED"))
            } else {
                ready(&inner, &route).err()
            };
            models.push(json!({"group":"cloud","label":m.label,"route":route,"available":reason.is_none(),"unavailable_reason":reason,"validated_model":p.validation.as_ref().is_some_and(|v|v.status=="succeeded"&&v.model==m.id)}));
        }
        Ok(
            json!({"config_revision":v2::wire_config_revision(inner.durable.config.revision),"inventory_observed":inner.inventory_observed,"providers":providers,"models":models}),
        )
    }
    fn provider_probe(&self, args: Value) -> AppResult<Value> {
        let a: ProbeArgs = decode(args.clone())?;
        require_instance(&a.instance_id, &self.instance_id)?;
        let fingerprint = fp("provider_probe", &args)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if let Some(v) = self.retained_v3(&inner, &a.request_id, &fingerprint)? {
            return Ok(v);
        }
        fresh(&inner, &a.request_id)?;
        if inner.cloud.preparations.contains_key(&a.request_id) {
            return Err(AppError::new(
                "ID_CONFLICT",
                "Request identifier belongs to a preparation",
            ));
        }
        if busy(&inner) {
            return Err(blocked());
        }
        let p = &inner.durable.cloud.providers[a.provider.index()];
        if p.revision != a.expected_provider_revision {
            return Err(stale());
        }
        let task = ProviderTask {
            provider: a.provider,
            action: Action::Probe,
            provider_revision: p.revision,
            credential: p.credential.clone(),
            model: None,
            endpoint: None,
            dispatch_expires_at_ms: None,
            policy: cloud::POLICY.into(),
            delivery: Delivery::NotApplicable,
        };
        self.admit_provider(&mut inner, a.request_id, fingerprint, task, None, None)
    }
    fn provider_prepare(&self, args: Value) -> AppResult<Value> {
        let a: PrepareProvider = decode(args.clone())?;
        require_instance(&a.instance_id, &self.instance_id)?;
        if a.action == Action::Probe || a.expected_provider_revision > MAX_REVISION {
            return Err(invalid());
        }
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        fresh(&inner, &a.request_id)?;
        let fingerprint = fp("provider_prepare", &args)?;
        let now = self.cloud.clock.now();
        if let Some(result) = inner.cloud.prepare(&a.request_id, &fingerprint, now)? {
            return Ok(result);
        }
        if busy(&inner) {
            return Err(blocked());
        }
        let p = &inner.durable.cloud.providers[a.provider.index()];
        if p.revision != a.expected_provider_revision {
            return Err(stale());
        }
        if a.action == Action::Save {
            if a.model.is_some() {
                return Err(invalid());
            }
            if p.revision >= MAX_REVISION {
                return Err(capacity());
            }
        } else {
            if a.model
                .as_ref()
                .is_none_or(|m| cloud::model(a.provider, m).is_none())
            {
                return Err(invalid());
            }
            if p.credential.is_none() {
                return Err(cloud::error("PROVIDER_NOT_CONFIGURED"));
            }
        }
        let expires = self
            .cloud
            .clock
            .wall_ms()
            .checked_add(60_000)
            .filter(|t| *t <= MAX_REVISION)
            .ok_or_else(capacity)?;
        let token = new_id().map_err(|_| storage())?;
        let endpoint = (a.action == Action::Validate).then(|| a.provider.endpoint().to_string());
        let result = json!({"request_id":a.request_id,"provider":a.provider,"action":a.action,"model":a.model,"expected_provider_revision":p.revision,"credential_generation":p.credential.as_ref().map(|c|&c.generation),"approval_token":token,"expires_at_ms":expires,"endpoint":endpoint});
        let task = ProviderTask {
            provider: a.provider,
            action: a.action,
            provider_revision: p.revision,
            credential: p.credential.clone(),
            model: a.model,
            endpoint,
            dispatch_expires_at_ms: Some(expires),
            policy: cloud::POLICY.into(),
            delivery: if a.action == Action::Validate {
                Delivery::NotSent
            } else {
                Delivery::NotApplicable
            },
        };
        inner.cloud.preparations.insert(
            a.request_id,
            Preparation {
                fingerprint,
                token,
                expires: now + Duration::from_secs(60),
                dispatch: now + Duration::from_secs(60),
                result: result.clone(),
                kind: Prepared::Provider(task),
            },
        );
        Ok(result)
    }
    fn provider_apply(&self, mut args: Value) -> AppResult<Value> {
        // Remove the secret before fingerprinting. A retained public binding never
        // validates, compares or stores a supplied replacement secret.
        let obj = args.as_object_mut().ok_or_else(invalid)?;
        struct SecretArgument(Value);
        impl Drop for SecretArgument {
            fn drop(&mut self) {
                if let Value::String(s) = &mut self.0 {
                    zeroize::Zeroize::zeroize(s);
                }
            }
        }
        let mut secret_arg = SecretArgument(obj.remove("api_key").ok_or_else(invalid)?);
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Apply {
            request_id: String,
            instance_id: String,
            approval_token: String,
        }
        let a: Apply = decode(args.clone())?;
        let fingerprint = fp("provider_apply", &args)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if let Some(result) = self.retained_v3(&inner, &a.request_id, &fingerprint)? {
            return Ok(result);
        }
        require_instance(&a.instance_id, &self.instance_id)?;
        fresh(&inner, &a.request_id)?;
        inner.cloud.expire(self.cloud.clock.now());
        let p = inner
            .cloud
            .preparations
            .get(&a.request_id)
            .cloned()
            .ok_or_else(|| {
                cloud::error(if inner.cloud.expired.contains(&a.request_id) {
                    "CONSENT_EXPIRED"
                } else {
                    "CONSENT_MISMATCH"
                })
            })?;
        if p.token != a.approval_token {
            return Err(cloud::error("CONSENT_MISMATCH"));
        }
        let Prepared::Provider(mut task) = p.kind else {
            return Err(cloud::error("CONSENT_MISMATCH"));
        };
        if busy(&inner) {
            return Err(blocked());
        }
        let current = &inner.durable.cloud.providers[task.provider.index()];
        if current.revision != task.provider_revision || current.credential != task.credential {
            return Err(stale());
        }
        let secret = match secret_arg.0.take() {
            Value::String(s) => Some(zeroize::Zeroizing::new(s)),
            Value::Null => None,
            _ => return Err(invalid()),
        };
        let secret = if task.action == Action::Save {
            let secret = Secret::new(secret.ok_or_else(invalid)?.as_bytes().to_vec())?;
            task.credential = Some(CredentialRef {
                provider: task.provider,
                generation: new_id().map_err(|_| storage())?,
                save_request_id: a.request_id.clone(),
            });
            Some(secret)
        } else {
            if secret.is_some() {
                return Err(invalid());
            }
            None
        };
        let result = self.admit_provider(
            &mut inner,
            a.request_id.clone(),
            fingerprint,
            task,
            Some(p.dispatch),
            secret,
        )?;
        inner.cloud.preparations.remove(&a.request_id);
        Ok(result)
    }
    fn retained_v3(&self, inner: &Inner, id: &str, fingerprint: &str) -> AppResult<Option<Value>> {
        if !valid_id(id) {
            return Err(invalid());
        }
        if retained(inner, id, fingerprint)?.is_some() {
            return Ok(inner
                .durable
                .jobs
                .iter()
                .find(|j| j.operation.request_id == id)
                .map(Job::summary));
        }
        Ok(None)
    }
    fn admit_provider(
        &self,
        inner: &mut Inner,
        id: String,
        fingerprint: String,
        task: ProviderTask,
        deadline: Option<Instant>,
        secret: Option<Secret>,
    ) -> AppResult<Value> {
        let (kind, phase) = match task.action {
            Action::Probe => ("provider_probe", "inventory"),
            Action::Save => ("provider_save", "credential"),
            Action::Validate => ("provider_validate", "verification"),
        };
        let mut next = inner.durable.clone();
        next.make_room()?;
        let job = Job {
            operation: Operation {
                request_id: id.clone(),
                kind: kind.into(),
                status: "running".into(),
                phase: phase.into(),
                config_revision: next.config.revision,
                result: None,
                error: None,
            },
            fingerprint,
            version: 3,
            conversation_id: None,
            history_revision: None,
            task: Some(Task::Provider(task)),
            accepted_ms: self.cloud.clock.wall_ms(),
        };
        let receipt = job.summary();
        next.jobs.push(job);
        self.commit(inner, next)?;
        inner.cloud.jobs.insert(
            id,
            Live {
                deadline: deadline
                    .unwrap_or_else(|| self.cloud.clock.now() + cloud::KEYRING_BUDGET),
                secret,
            },
        );
        self.wake()?;
        Ok(receipt)
    }
    fn conversation_routing(&self, args: Value) -> AppResult<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Get {
            conversation_id: String,
        }
        let a: Get = decode(args)?;
        if !valid_id(&a.conversation_id) {
            return Err(invalid());
        }
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault {
            return Err(storage());
        }
        let c = inner
            .durable
            .conversations
            .iter()
            .find(|c| c.conversation_id == a.conversation_id)
            .ok_or_else(missing)?;
        let unknown = Routing {
            turn_routes: (0..c.history.len() / 2)
                .map(|_| Provenance {
                    request_id: None,
                    route: None,
                })
                .collect(),
            ..Routing::default()
        };
        let r = inner
            .durable
            .cloud
            .conversations
            .get(&a.conversation_id)
            .unwrap_or(&unknown);
        Ok(
            json!({"conversation_id":a.conversation_id,"history_revision":c.history_revision,"last_route":r.last_route,"approved_destinations":r.approved_destinations,"turn_routes":r.turn_routes}),
        )
    }
    fn route_prepare(&self, args: Value) -> AppResult<Value> {
        let (a, _, _, public) = route_args(args, false)?;
        require_instance(&a.instance_id, &self.instance_id)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        fresh(&inner, &a.request_id)?;
        let fingerprint = fp("route_prepare", &public)?;
        if inner
            .cloud
            .preparations
            .get(&a.request_id)
            .is_some_and(|p| p.fingerprint != fingerprint)
        {
            return Err(AppError::new(
                "ID_CONFLICT",
                "Request identifier is bound to different preparation",
            ));
        }
        let now = self.cloud.clock.now();
        if a.route.vendor().is_some() {
            if let Some(result) = inner.cloud.prepare(&a.request_id, &fingerprint, now)? {
                return Ok(result);
            }
        }
        let mut candidate = route_candidate(&inner, &a)?;
        if a.route.vendor().is_none() {
            return Ok(route_result(&a, &candidate.task, false, None, None));
        }
        let wall = self.cloud.clock.wall_ms();
        let dispatch_wall = wall
            .checked_add(600_000)
            .filter(|t| *t <= MAX_REVISION)
            .ok_or_else(capacity)?;
        candidate.task.dispatch_expires_at_ms = Some(dispatch_wall);
        let token = new_id().map_err(|_| storage())?;
        let result = route_result(
            &a,
            &candidate.task,
            candidate.requires_consent,
            Some(&token),
            Some(wall + 60_000),
        );
        inner.cloud.preparations.insert(
            a.request_id,
            Preparation {
                fingerprint,
                token,
                expires: now + Duration::from_secs(60),
                dispatch: now + Duration::from_secs(600),
                result: result.clone(),
                kind: Prepared::Route {
                    context: candidate.task.context_sha256,
                    requires_consent: candidate.requires_consent,
                },
            },
        );
        Ok(result)
    }
    fn submit_routed(&self, args: Value) -> AppResult<Value> {
        let fingerprint = fp("submit_routed", &args)?;
        let (a, token, confirmed, public) = route_args(args, true)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if let Some(result) = self.retained_v3(&inner, &a.request_id, &fingerprint)? {
            return Ok(result);
        }
        require_instance(&a.instance_id, &self.instance_id)?;
        fresh(&inner, &a.request_id)?;
        if a.route.vendor().is_none() && inner.cloud.preparations.contains_key(&a.request_id) {
            return Err(AppError::new(
                "ID_CONFLICT",
                "Request identifier belongs to a cloud preparation",
            ));
        }
        let mut candidate = route_candidate(&inner, &a)?;
        let mut live = None;
        if a.route.vendor().is_some() {
            inner.cloud.expire(self.cloud.clock.now());
            let prep = inner.cloud.preparations.get(&a.request_id).ok_or_else(|| {
                cloud::error(if inner.cloud.expired.contains(&a.request_id) {
                    "CONSENT_EXPIRED"
                } else {
                    "CONSENT_REQUIRED"
                })
            })?;
            let Prepared::Route {
                context,
                requires_consent,
            } = &prep.kind
            else {
                return Err(cloud::error("CONSENT_MISMATCH"));
            };
            if token.as_ref() != Some(&prep.token)
                || prep.fingerprint != fp("route_prepare", &public)?
                || context != &candidate.task.context_sha256
                || *requires_consent != candidate.requires_consent
            {
                return Err(cloud::error("CONSENT_MISMATCH"));
            }
            if confirmed != *requires_consent {
                return Err(cloud::error(if *requires_consent {
                    "CONSENT_REQUIRED"
                } else {
                    "CONSENT_MISMATCH"
                }));
            }
            candidate.task.dispatch_expires_at_ms = prep
                .result
                .get("dispatch_expires_at_ms")
                .and_then(Value::as_u64);
            live = Some(Live {
                deadline: prep.dispatch,
                secret: None,
            });
        } else if token.is_some() || confirmed {
            return Err(invalid());
        }
        let mut next = inner.durable.clone();
        let target = match &a.conversation_id {
            Some(id) => id.clone(),
            None => next.allocate(Some(a.request_id.clone()), v2::initial_title(&a.text))?,
        };
        let pairs = next
            .conversations
            .iter()
            .find(|c| c.conversation_id == target)
            .ok_or_else(missing)?
            .history
            .len()
            / 2;
        let routing = next
            .cloud
            .conversations
            .entry(target.clone())
            .or_insert_with(|| Routing {
                turn_routes: (0..pairs)
                    .map(|_| Provenance {
                        request_id: None,
                        route: None,
                    })
                    .collect(),
                ..Routing::default()
            });
        if candidate.requires_consent {
            routing
                .approved_destinations
                .push(a.route.destination().ok_or_else(invalid)?);
        }
        routing.last_route = Some(a.route.clone());
        next.make_room()?;
        let job = Job {
            operation: Operation {
                request_id: a.request_id.clone(),
                kind: if a.conversation_id.is_some() {
                    "submit_routed"
                } else {
                    "submit_new_routed"
                }
                .into(),
                status: "queued".into(),
                phase: "queued".into(),
                config_revision: a.expected_config_revision,
                result: None,
                error: None,
            },
            fingerprint,
            version: 3,
            conversation_id: Some(target),
            history_revision: Some(a.expected_history_revision.unwrap_or(0)),
            task: Some(Task::Routed(candidate.task)),
            accepted_ms: self.cloud.clock.wall_ms(),
        };
        let receipt = job.summary();
        next.jobs.push(job);
        self.commit(&mut inner, next)?;
        inner.cloud.preparations.remove(&a.request_id);
        if let Some(live) = live {
            inner.cloud.jobs.insert(a.request_id, live);
        }
        self.wake()?;
        Ok(receipt)
    }
    fn request_routing(&self, args: Value) -> AppResult<Value> {
        let a: PollArgs = decode(args)?;
        require_instance(&a.instance_id, &self.instance_id)?;
        if !valid_id(&a.request_id) {
            return Err(invalid());
        }
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault {
            return Err(storage());
        }
        let j = inner
            .durable
            .jobs
            .iter()
            .find(|j| j.operation.request_id == a.request_id)
            .ok_or_else(|| {
                AppError::new(
                    "REQUEST_EXPIRED",
                    "Request is absent or expired; do not resend",
                )
            })?;
        let (route, delivery, hash, expires) = match &j.task {
            Some(Task::Routed(t)) => (
                Some(t.route.clone()),
                t.delivery,
                Some(t.context_sha256.clone()),
                t.dispatch_expires_at_ms,
            ),
            Some(Task::Provider(t)) if t.action == Action::Validate => (
                Some(provider_route(t)?),
                t.delivery,
                None,
                t.dispatch_expires_at_ms,
            ),
            _ => (None, Delivery::NotApplicable, None, None),
        };
        Ok(
            json!({"request_id":a.request_id,"route":route,"delivery":delivery,"context_sha256":hash,"dispatch_expires_at_ms":expires}),
        )
    }
}
fn provider_route(t: &ProviderTask) -> AppResult<Route> {
    Ok(Route {
        provider: t.provider.name().into(),
        model: t.model.clone().ok_or_else(invalid)?,
        credential_generation: Some(
            t.credential
                .as_ref()
                .ok_or_else(invalid)?
                .generation
                .clone(),
        ),
    })
}
pub(super) fn append_provenance(
    d: &mut CloudDurable,
    id: &str,
    previous: usize,
    request: Option<String>,
    route: Option<Route>,
) {
    if let Some(r) = d.conversations.get_mut(id) {
        while r.turn_routes.len() < previous {
            r.turn_routes.push(Provenance {
                request_id: None,
                route: None,
            });
        }
        r.turn_routes.push(Provenance {
            request_id: request,
            route,
        });
    }
}
pub(super) fn trim_provenance(d: &mut CloudDurable, id: &str, pairs: usize) {
    if let Some(r) = d.conversations.get_mut(id) {
        let remove = r.turn_routes.len().saturating_sub(pairs);
        r.turn_routes.drain(..remove);
    }
}
