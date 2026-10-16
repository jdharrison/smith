//! Recipient orchestration through existing typed Core/app APIs, never a helper
//! subprocess or an Application::production owned by the installer.
use crate::{artifact, consent, model, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use smith_service::app::{self, AppError, AppReply, AppRequest, Config, Operation};
use smith_service::providers::Inventory;
use smith_service::{Action, Controller, Manager, Observation, Roots, SystemManager};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const RELOAD_MESSAGE: &str =
    "Exact installed unit requires a separately approved reload-only repair before Start";
const CORE_LIMIT: usize = 16 * 1024;
const POLL_BUDGET: Duration = Duration::from_secs(45);
const POLL_LIMIT: usize = 180;

#[derive(Clone, Debug)]
pub struct Options {
    pub bundle: PathBuf,
    pub model: Option<String>,
    pub pull_model: bool,
    pub verify_inference: bool,
    pub dry_run: bool,
}
impl Options {
    pub fn validate(&self) -> Result<()> {
        if self
            .model
            .as_deref()
            .is_some_and(|m| !model::valid_model(m))
        {
            return Err(
                "Use --model with an exact eligible local name:tag; no default is selected".into(),
            );
        }
        if (self.pull_model || self.verify_inference) && self.model.is_none() {
            return Err("--pull-model/--verify-inference require --model EXACT".into());
        }
        Ok(())
    }
}

/// Library-only injection seam. The executable cannot select a backend or
/// manager command through arguments, PATH, or environment overrides.
pub trait Backend {
    fn inspect(&mut self) -> Result<Observation>;
    fn prepare(&mut self, action: Action) -> Result<Observation>;
    fn apply(&mut self, action: Action, approval: &str) -> Result<Observation>;
    fn app(&mut self, request: AppRequest) -> Result<AppReply>;
}
impl<M: Manager> Backend for Controller<M> {
    fn inspect(&mut self) -> Result<Observation> {
        Ok(Controller::inspect(self))
    }
    fn prepare(&mut self, action: Action) -> Result<Observation> {
        Ok(Controller::prepare(self, action))
    }
    fn apply(&mut self, action: Action, approval: &str) -> Result<Observation> {
        Ok(Controller::apply(self, action, approval))
    }
    fn app(&mut self, request: AppRequest) -> Result<AppReply> {
        Controller::app(self, request)
    }
}

pub trait Publisher {
    fn run(
        &mut self,
        dry_run: bool,
        confirm: &mut dyn FnMut(&str) -> Result<String>,
    ) -> Result<artifact::Outcome>;
}
impl Publisher for artifact::Installer {
    fn run(
        &mut self,
        dry_run: bool,
        confirm: &mut dyn FnMut(&str) -> Result<String>,
    ) -> Result<artifact::Outcome> {
        artifact::Installer::run(self, dry_run, false, confirm)
    }
}

/// All runtime objects remain lazy, including Roots::environment. Test factories
/// can fail if dry-run/initial decline accidentally crosses an authority boundary.
pub struct Factories<'a> {
    pub preflight: &'a mut dyn FnMut() -> Result<()>,
    pub publisher: &'a mut dyn FnMut(PathBuf, PathBuf) -> Result<Box<dyn Publisher>>,
    pub backend: &'a mut dyn FnMut(PathBuf) -> Result<Box<dyn Backend>>,
    pub provider: &'a mut dyn FnMut() -> Result<Box<dyn model::Provider>>,
}

pub fn run(
    options: &Options,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut dyn model::ProgressOutput,
) -> Result<artifact::Outcome> {
    let mut preflight = prerequisites;
    let mut publisher = |client, service| {
        artifact::Installer::new(Some(client), Some(service))
            .map(|p| Box::new(p) as Box<dyn Publisher>)
    };
    let mut backend = |source| {
        let roots = Roots::environment().map_err(|_| {
            "Existing safe XDG data/config/runtime roots are required; prepare the user session separately".to_owned()
        })?;
        Ok(Box::new(Controller {
            roots,
            source,
            manager: SystemManager,
        }) as Box<dyn Backend>)
    };
    let mut provider =
        || crate::catalog::Native::new().map(|p| Box::new(p) as Box<dyn model::Provider>);
    run_with_schema(
        options,
        &mut Factories {
            preflight: &mut preflight,
            publisher: &mut publisher,
            backend: &mut backend,
            provider: &mut provider,
        },
        confirm,
        output,
        true,
    )
}

pub fn run_with(
    options: &Options,
    factories: &mut Factories<'_>,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut dyn model::ProgressOutput,
) -> Result<artifact::Outcome> {
    run_with_schema(options, factories, confirm, output, false)
}

fn run_with_schema(
    options: &Options,
    factories: &mut Factories<'_>,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut dyn model::ProgressOutput,
    provider_aware: bool,
) -> Result<artifact::Outcome> {
    let mut effects = Vec::new();
    let mut console = consent::Console::new(output);
    let result = run_inner(
        options,
        factories,
        confirm,
        &mut console,
        &mut effects,
        provider_aware,
    );
    if consent::cancelled() {
        // A signal cannot require another write to the same backpressured pipe.
        // Accepted effects remain; incomplete reporting never implies rollback.
        return result.and(Err(
            "Installer interrupted; prior effects remain; report may be incomplete".into(),
        ));
    }
    // All terminal/error/partial-effect lines share ONE short output budget.
    // Console remembers a failed model-progress sink and refuses further writes.
    // There is no blocking Write/flush fallback after a failed model stage.
    let deadline = Instant::now() + consent::REPORT_BUDGET;
    let report = (|| {
        if let Err(error) = &result {
            console.line_until(
                &format!("Installer stopped: {}", consent::display(error)),
                deadline,
            )?;
            console.line_until("No automatic retry/rollback. The bundled runtime and user manager must be available; no sudo/OS provisioning is performed.", deadline)?;
        }
        if !effects.is_empty() {
            console.line_until(
                "Accepted/completed or uncertain effects (not rolled back):",
                deadline,
            )?;
            for effect in effects {
                console.line_until(&format!("  {effect}"), deadline)?;
            }
            console.line_until("Interrupted downloads may leave staging or verified orphan files; no cache was deleted.", deadline)?;
        }
        Ok(())
    })();
    match result {
        Err(error) => Err(error),
        Ok(outcome) => report.map(|()| outcome),
    }
}

fn run_inner(
    options: &Options,
    factories: &mut Factories<'_>,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut consent::Console<'_>,
    effects: &mut Vec<String>,
    provider_aware: bool,
) -> Result<artifact::Outcome> {
    consent::check()?;
    options.validate()?;
    (factories.preflight)()?;
    let client = options.bundle.join("payload/client");
    // This source is deliberately NOT current_exe (the installer is a different ELF).
    let source = options.bundle.join("payload/service/smith-service");
    let mut publisher = (factories.publisher)(client, source.clone())?;
    if !options.dry_run {
        say(output, "Smith recipient installation: artifacts, native user-unit registration, login enablement, Start and saved model preference are separate confirmations. Enable never starts the service, Start never enables it, and lingering is unchanged. CPU/Vulkan llama.cpp runtimes and CPU fallback are bundled. No upgrades or OS/GPU provisioning. Ubuntu runtime libraries and a usable user manager must already exist. Model weights require a separately confirmed curated download.")?;
    }
    let mut accepted = false;
    let publication = publisher.run(options.dry_run, &mut |prompt| {
        if options.dry_run {
            return Err("Dry-run must not request consent".into());
        }
        let answer = confirm(prompt)?;
        accepted = consent::matches(&answer, "y")?;
        // Normalize the full recipient's exact consent before handing it to
        // the developer publisher, whose legacy prompt is case-insensitive.
        Ok(if accepted { "y" } else { "n" }.into())
    });
    if accepted {
        effects.push("Artifact publication accepted; outcome may be partial until verified".into());
    }
    let outcome = publication?;
    match outcome {
        artifact::Outcome::Declined if !accepted => return Ok(artifact::Outcome::Declined),
        artifact::Outcome::DryRun if options.dry_run => return Ok(artifact::Outcome::DryRun),
        artifact::Outcome::Installed if !options.dry_run && accepted => {}
        _ => return Err("Unexpected artifact outcome; runtime setup blocked".into()),
    }
    effects.push("Artifact publication completed; artifacts remain installed".into());
    consent::check()?;
    let backend = (factories.backend)(source)?;
    let mut native = if provider_aware {
        Native::new_v3(backend)
    } else {
        Native::new(backend)
    };
    let mut facts = native.inspect()?;
    usable(&facts)?;
    if facts.unit_state == "missing" || facts.message == RELOAD_MESSAGE {
        facts = action(
            &mut native,
            Action::InstallUnit,
            "install-unit",
            confirm,
            output,
            effects,
        )?;
        usable(&facts)?;
    }
    if facts.enablement_state == "disabled" {
        facts = action(
            &mut native,
            Action::Enable,
            "enable-at-login",
            confirm,
            output,
            effects,
        )?;
        usable(&facts)?;
    }
    if facts.runtime_state == "stopped" {
        facts = action(
            &mut native,
            Action::Start,
            "start",
            confirm,
            output,
            effects,
        )?;
        usable(&facts)?;
    }
    if facts.binary_state != "installed"
        || facts.unit_state != "installed"
        || facts.enablement_state != "enabled"
        || facts.runtime_state != "running"
        || facts.bridge_state != "ready"
    {
        return Err("Verified installation requires exact installed, enabled, running and bridge-ready state; inspect/repair separately".into());
    }
    let mut names = native.models()?;
    say(
        output,
        &format!(
            "Observed installed local tags (inventory, not inference): {}",
            names.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    )?;
    say(output, "Optional cloud Providers are separate from the local Model step. After local setup, run smith-installer --providers with this package to save/replace and explicitly validate OpenAI and/or Anthropic. Skipping keeps local-only use; no cloud request is made here.")?;
    let Some(selected) = options.model.as_deref() else {
        say(output, "No model selected. Rerun with --model EXACT or explicitly Save an observed model in Smith. No default was invented; model/inference setup is not complete.")?;
        return Ok(artifact::Outcome::Installed);
    };
    let mut provider: Option<Box<dyn model::Provider>> = None;
    if !names.contains(selected) {
        if !options.pull_model {
            return Err(
                "Selected model missing; --pull-model can offer a separate download confirmation"
                    .into(),
            );
        }
        let prompt = format!("Download exactly {selected} into Smith's private model store from the revision-pinned Hugging Face catalog? This uses internet and persistent disk; failed transfers may leave staging files. No retry or automatic cleanup. See --models for exact size/license. Type the exact model to consent: ");
        if !consent::matches(&confirm(&prompt)?, selected)? {
            return Err("Model download declined; prior artifacts/lifecycle effects remain".into());
        }
        consent::check()?;
        provider = Some((factories.provider)()?);
        effects.push(format!(
            "Download of {selected} accepted; daemon/cache outcome may be unknown"
        ));
        provider
            .as_mut()
            .ok_or("Provider unavailable")?
            .pull(selected, output)?;
        consent::check()?;
        // Even a successful provider pull must be freshly observed by the paired service.
        names = native.models()?;
        if !names.contains(selected) {
            return Err("Fresh app inventory did not observe selected model after pull".into());
        }
    }
    let prompt = format!("Save exact model {selected} with voice=null as a persistent preference (not inference verification)? [y/N] ");
    if !consent::matches(&confirm(&prompt)?, "y")? {
        return Err("Saved preference declined; prior effects remain".into());
    }
    effects.push("Saved model preference accepted; outcome requires synchronization".into());
    native.configure(selected)?;
    say(
        output,
        "Exact observed model preference saved; inference is not yet verified.",
    )?;
    if options.verify_inference {
        let prompt = format!("Run a direct minimal inference with {selected}? Uses provider compute, no tools; not a saved chat turn. [y/N] ");
        if !consent::matches(&confirm(&prompt)?, "y")? {
            return Err("Inference declined; saved preference and prior effects remain".into());
        }
        consent::check()?;
        if provider.is_none() {
            provider = Some((factories.provider)()?);
        }
        effects.push("Direct minimal inference accepted; not a persisted chat turn".into());
        let answer = provider
            .as_mut()
            .ok_or("Provider unavailable")?
            .inference(selected)?;
        consent::check()?;
        if answer.trim().is_empty() || answer.len() > 4096 {
            return Err("Direct inference returned invalid/oversized text; no retry".into());
        }
        say(
            output,
            &format!("Direct inference verified: {}", consent::display(&answer)),
        )?;
    }
    Ok(artifact::Outcome::Installed)
}

fn say(output: &mut consent::Console<'_>, message: &str) -> Result<()> {
    output.line(message)
}

pub(crate) fn usable(facts: &Observation) -> Result<()> {
    if facts.schema_version != 2 || facts.service != "smith-next" || !facts.ok {
        return Err("Strict schema-2 Smith service observation required".into());
    }
    if facts.binary_state != "installed" {
        return Err("Installed binary mismatch; no implicit upgrade".into());
    }
    if facts.runtime_state == "unavailable" {
        return Err("User manager unavailable; establish a user session/manager separately".into());
    }
    if !["missing", "installed"].contains(&facts.unit_state.as_str())
        || !["missing", "disabled", "enabled"].contains(&facts.enablement_state.as_str())
        || !["stopped", "running"].contains(&facts.runtime_state.as_str())
        || ["conflict", "unknown"].contains(&facts.bridge_state.as_str())
    {
        return Err(
            "Unsafe, conflicting or uncertain managed identity; inspect/repair separately".into(),
        );
    }
    Ok(())
}

fn action(
    native: &mut Native,
    action: Action,
    label: &str,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut consent::Console<'_>,
    effects: &mut Vec<String>,
) -> Result<Observation> {
    let prepared = native.prepare(action)?;
    let token = prepared
        .approval_token
        .as_deref()
        .ok_or("Native preparation supplied no approval")?;
    let description =
        serde_json::to_string(&prepared.effects).map_err(|_| "Cannot display prepared effects")?;
    say(
        output,
        &format!(
            "Prepared native effects: {}",
            consent::display(&description)
        ),
    )?;
    if !consent::matches(&confirm(&format!("Confirm {label} only? [y/N] "))?, "y")? {
        return Err(format!(
            "Declined {label}; prior artifact/lifecycle effects remain"
        ));
    }
    effects.push(format!(
        "{label} accepted; outcome requires fresh inspection"
    ));
    // Preserve the EXACT original token and its Core-owned 60s expiry. There is
    // no automatic reprepare, retry, helper process, or wrapper filesystem deadline.
    native.apply(action, token)?;
    native.inspect()
}

pub fn prerequisites() -> Result<()> {
    // Bound the OS metadata read; this neither provisions libraries nor discovers
    // HOME/XDG roots, a manager, or an application/provider runtime.
    let mut bytes = Vec::new();
    std::fs::File::open("/etc/os-release")
        .and_then(|f| f.take(CORE_LIMIT as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| "Cannot identify Ubuntu; provision the supported host separately")?;
    if bytes.len() > CORE_LIMIT {
        return Err("OS identification exceeds bound".into());
    }
    let release = std::str::from_utf8(&bytes).map_err(|_| "Invalid Ubuntu identification")?;
    // SAFETY: geteuid has no arguments or memory access.
    supported_host(
        std::env::consts::OS,
        std::env::consts::ARCH,
        unsafe { libc::geteuid() },
        release,
    )
}

pub fn supported_host(os: &str, arch: &str, uid: u32, release: &str) -> Result<()> {
    if os != "linux" || arch != "x86_64" {
        return Err("Supported target is Ubuntu 24.04 LTS x86_64 only".into());
    }
    if uid == 0 {
        return Err("Run as the trusted unprivileged desktop user, not sudo/root".into());
    }
    let field = |name: &str| -> Option<&str> {
        let mut values = release
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| {
                value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                    .unwrap_or(value)
            });
        let value = values.next()?;
        if values.next().is_some() {
            None
        } else {
            Some(value)
        }
    };
    if field("ID") != Some("ubuntu") || field("VERSION_ID") != Some("24.04") {
        return Err("Ubuntu 24.04 LTS required; no OS/runtime installation is performed".into());
    }
    Ok(())
}

/// Typed transport correlation and bounded model inventory. Only read-only
/// models/poll and separately approved configure are used by orchestration.
pub struct Native {
    backend: Box<dyn Backend>,
    instance: Option<String>,
    pid: Option<u32>,
    schema: u32,
}
impl Native {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend,
            instance: None,
            pid: None,
            schema: 1,
        }
    }
    /// Explicit production/provider-aware adapter; never a version fallback.
    pub fn new_v3(backend: Box<dyn Backend>) -> Self {
        Self {
            schema: 3,
            ..Self::new(backend)
        }
    }
    pub(crate) fn instance(&self) -> Result<&str> {
        self.instance
            .as_deref()
            .ok_or_else(|| "Service identity has not been observed".into())
    }
    pub fn inspect(&mut self) -> Result<Observation> {
        consent::check()?;
        checked_core(self.backend.inspect()?)
    }
    pub fn prepare(&mut self, action: Action) -> Result<Observation> {
        consent::check()?;
        if ![Action::InstallUnit, Action::Enable, Action::Start].contains(&action) {
            return Err("Installer does not install through Core or write setup completion".into());
        }
        checked_core(self.backend.prepare(action)?)
    }
    pub fn apply(&mut self, action: Action, approval: &str) -> Result<Observation> {
        consent::check()?;
        if ![Action::InstallUnit, Action::Enable, Action::Start].contains(&action)
            || !valid_token(approval)
        {
            return Err("Unsupported action or malformed approval binding".into());
        }
        checked_core(self.backend.apply(action, approval)?)
    }
    pub fn app(&mut self, op: &str, mut args: Value) -> Result<Value> {
        consent::check()?;
        validate_args(op, &args)?;
        if self.schema == 3 {
            if let Some(revision) = args.get_mut("expected_revision") {
                if let Some(number) = revision.as_u64() {
                    *revision = crate::providers::wire_revision(number);
                }
            }
        }
        self.exchange(self.schema, op, args)
            .map_err(|e| e.to_string())
    }
    pub(crate) fn app_v3(
        &mut self,
        op: &str,
        args: Value,
    ) -> std::result::Result<Value, crate::providers::Failure> {
        self.exchange(3, op, args)
    }
    fn exchange(
        &mut self,
        schema: u32,
        op: &str,
        args: Value,
    ) -> std::result::Result<Value, crate::providers::Failure> {
        use crate::providers::Failure;
        consent::check()
            .map_err(|_| Failure::unknown("Installer interrupted; no automatic resend"))?;
        let request = AppRequest {
            schema_version: schema,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: app::new_id()
                .map_err(|_| Failure::unknown("Cannot allocate app call identity"))?,
            op: op.into(),
            args,
        };
        let call = request.call_id.clone();
        let reply = self.backend.app(request).map_err(|_| {
            Failure::unknown("App transport unavailable; outcome unknown; no automatic resend")
        })?;
        if reply.schema_version != schema
            || reply.service != "smith-next"
            || reply.protocol != "app"
            || reply.call_id != call
            || !app::valid_id(&reply.instance_id)
            || reply.pid == 0
            || reply.ok != reply.error.is_none()
            || reply.ok != reply.result.is_some()
        {
            return Err(Failure::unknown(
                "App envelope identity/schema mismatch; no automatic resend",
            ));
        }
        if self
            .instance
            .as_ref()
            .is_some_and(|i| i != &reply.instance_id)
            || self.pid.is_some_and(|pid| pid != reply.pid)
        {
            return Err(Failure::unknown(
                "Service instance changed; synchronize without resending",
            ));
        }
        self.instance = Some(reply.instance_id);
        self.pid = Some(reply.pid);
        if let Some(error) = reply.error {
            if !valid_error(&error) {
                return Err(Failure::unknown("Invalid bounded app error"));
            }
            return Err(Failure::Service(error.code));
        }
        let value = reply
            .result
            .ok_or_else(|| Failure::unknown("Missing app result"))?;
        bounded_value(&value).map_err(|_| Failure::unknown("Invalid bounded app JSON"))?;
        if !value.is_object() {
            return Err(Failure::unknown("Invalid app result shape"));
        }
        Ok(value)
    }
    pub fn models(&mut self) -> Result<BTreeSet<String>> {
        self.models_with_budget(POLL_BUDGET, Duration::from_millis(250))
    }
    /// Smaller budgets/intervals are a library test seam. A call in flight retains
    /// Core's manager/socket budgets: 45s bounds observation scheduling, not total
    /// filesystem/manager duration or cancellation of service/provider work.
    pub fn models_with_budget(
        &mut self,
        budget: Duration,
        interval: Duration,
    ) -> Result<BTreeSet<String>> {
        if budget > POLL_BUDGET || interval > Duration::from_millis(250) {
            return Err("Inventory polling cannot exceed its observation budget".into());
        }
        let id = app::new_id()?;
        let value = self.app("models", json!({"request_id":id}))?;
        let admission = self.admission(value, "models")?;
        if admission.request_id != id
            || !["running", "succeeded"].contains(&admission.status.as_str())
            || (admission.status == "running" && admission.phase != "inventory")
            || (admission.status == "succeeded" && admission.phase != "complete")
        {
            return Err("Invalid inventory admission; do not resend".into());
        }
        let instance = self.instance.clone().ok_or("Missing service instance")?;
        let deadline = Instant::now() + budget;
        for attempt in 0..POLL_LIMIT {
            consent::check()?;
            if Instant::now() >= deadline {
                return Err(
                    "Inventory observation timed out; operation may continue; no resend".into(),
                );
            }
            let value = self.app("poll", json!({"request_id":id,"instance_id":instance}))?;
            let operation = self.local_operation(value)?;
            consent::check()?;
            if Instant::now() >= deadline {
                return Err(
                    "Inventory observation timed out; operation may continue; no resend".into(),
                );
            }
            if operation.request_id != id
                || operation.kind != "models"
                || operation.config_revision != admission.config_revision
                || !["running", "succeeded", "failed", "interrupted"]
                    .contains(&operation.status.as_str())
            {
                return Err("Inventory operation mismatch; no resend".into());
            }
            if operation.status == "succeeded" {
                if operation.phase != "complete" || operation.error.is_some() {
                    return Err("Invalid inventory completion".into());
                }
                return checked_inventory(operation.result.ok_or("Missing inventory result")?);
            }
            if operation.status != "running"
                || operation.phase != "inventory"
                || operation.result.is_some()
                || operation.error.is_some()
            {
                return Err("Inventory failed/interrupted; no automatic retry".into());
            }
            if attempt + 1 == POLL_LIMIT {
                break;
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("Inventory observation timed out; operation may continue; no resend")?;
            std::thread::sleep(interval.min(remaining));
        }
        Err("Inventory poll limit exceeded; no resend".into())
    }
    pub fn configuration(&mut self) -> Result<Config> {
        let value = self.app("state", json!({}))?;
        fields(
            &value,
            if self.schema == 3 {
                &[
                    "config",
                    "inventory_observed",
                    "inventory",
                    "conversations",
                    "operations",
                    "retention",
                    "revision",
                ]
            } else {
                &["config", "history", "active", "providers", "retention"]
            },
        )?;
        let config = self.local_config(value["config"].clone())?;
        if self.schema == 3 {
            checked_state_v3(&value, config.revision)?;
        }
        if config
            .model
            .as_deref()
            .is_some_and(|m| !model::valid_model(m))
        {
            return Err("Invalid saved model identity".into());
        }
        Ok(config)
    }
    pub fn configure_checked(&mut self, selected: &str, revision: u64) -> Result<u64> {
        let value = self.app(
            "configure_checked",
            json!({"model":selected,"voice":null,"expected_revision":revision}),
        )?;
        let config = self.local_config(value)?;
        if config.model.as_deref() != Some(selected)
            || config.voice.is_some()
            || config.revision != revision.checked_add(1).ok_or("Revision exhausted")?
        {
            return Err("Checked preference response mismatch; synchronize before retry".into());
        }
        Ok(config.revision)
    }
    pub fn model_operation(
        &mut self,
        op: &str,
        selected: &str,
        revision: u64,
        stopped: &dyn Fn() -> bool,
    ) -> Result<()> {
        self.model_operation_with_budget(
            op,
            selected,
            revision,
            stopped,
            Duration::from_secs(210),
            Duration::from_millis(250),
        )
    }
    /// Observation only after admission: timeout/cancel never resends or stops accepted service work.
    pub fn model_operation_with_budget(
        &mut self,
        op: &str,
        selected: &str,
        revision: u64,
        stopped: &dyn Fn() -> bool,
        budget: Duration,
        interval: Duration,
    ) -> Result<()> {
        if !["verify_model", "remove_model"].contains(&op)
            || budget > Duration::from_secs(210)
            || interval > Duration::from_millis(250)
        {
            return Err("Invalid bounded model operation".into());
        }
        let entry = crate::catalog::entry(selected)?;
        let id = app::new_id()?;
        let phase = if op == "verify_model" {
            "verification"
        } else {
            "removal"
        };
        if stopped() {
            return Err("Observation stopped before admission".into());
        }
        let value = self.app(op, json!({"request_id":id,"model":selected,"sha256":entry.sha256,"expected_revision":revision}))?;
        let admission = self.admission(value, op)?;
        if admission.request_id != id
            || admission.config_revision != revision
            || !["running", "succeeded"].contains(&admission.status.as_str())
            || admission.phase
                != if admission.status == "running" {
                    phase
                } else {
                    "complete"
                }
        {
            return Err("Model admission mismatch; no resend".into());
        }
        let instance = self.instance.clone().ok_or("Missing service instance")?;
        let deadline = Instant::now() + budget;
        for _ in 0..4096 {
            if stopped() || Instant::now() >= deadline {
                return Err(
                    "Model observation stopped; accepted work may continue; no resend".into(),
                );
            }
            let value = self.app("poll", json!({"request_id":id,"instance_id":instance}))?;
            let operation = self.local_operation(value)?;
            if stopped() || Instant::now() >= deadline {
                return Err(
                    "Model observation stopped; accepted work may continue; no resend".into(),
                );
            }
            if operation.request_id != id
                || operation.kind != op
                || operation.config_revision != revision
                || operation
                    .error
                    .as_ref()
                    .is_some_and(|error| !valid_error(error))
            {
                return Err("Model operation correlation mismatch; no resend".into());
            }
            if operation.status == "succeeded" {
                if operation.phase != "complete" || operation.error.is_some() {
                    return Err("Invalid model completion".into());
                }
                let result = operation.result.ok_or("Missing model result")?;
                fields(
                    &result,
                    if op == "verify_model" {
                        &["model", "sha256", "verified", "assistant_text"]
                    } else {
                        &["model", "sha256", "removed"]
                    },
                )?;
                if result["model"].as_str() != Some(selected)
                    || result["sha256"].as_str() != Some(entry.sha256)
                    || (op == "verify_model"
                        && (result["verified"].as_bool() != Some(true)
                            || !result["assistant_text"].as_str().is_some_and(|s| {
                                !s.trim().is_empty() && s.len() <= app::MAX_CONTEXT
                            })))
                    || (op == "remove_model" && result["removed"].as_bool() != Some(true))
                {
                    return Err("Model result identity/content mismatch".into());
                }
                return Ok(());
            }
            if operation.status != "running"
                || operation.phase != phase
                || operation.result.is_some()
                || operation.error.is_some()
            {
                return Err(format!(
                    "Model operation failed/interrupted: {}; setup incomplete; no retry",
                    operation
                        .error
                        .as_ref()
                        .map(|e| e.code.as_str())
                        .unwrap_or("INVALID_RESULT")
                ));
            }
            std::thread::sleep(interval.min(deadline.saturating_duration_since(Instant::now())));
        }
        Err("Model poll limit exceeded; accepted work may continue; no resend".into())
    }
    fn local_config(&self, mut value: Value) -> Result<Config> {
        fields(&value, &["model", "voice", "revision"])?;
        if self.schema == 3 {
            value["revision"] = json!(crate::providers::config_revision(&value["revision"])?);
        }
        decode(value)
    }
    fn admission(&self, mut value: Value, kind: &str) -> Result<Admission> {
        if self.schema == 3 {
            crate::providers::check_operation(&value, false)?;
            if value["kind"] != kind || !value["conversation_id"].is_null() {
                return Err("Local operation admission mismatch".into());
            }
            let map = value.as_object_mut().ok_or("Invalid admission")?;
            map.remove("kind");
            map.remove("conversation_id");
            value["config_revision"] = json!(crate::providers::config_revision(
                &value["config_revision"]
            )?);
        }
        decode(value)
    }
    fn local_operation(&self, mut value: Value) -> Result<Operation> {
        if self.schema == 3 {
            crate::providers::check_operation(&value, true)?;
            if !value["conversation_id"].is_null() {
                return Err("Unexpected conversation in local installer work".into());
            }
            value
                .as_object_mut()
                .ok_or("Invalid operation")?
                .remove("conversation_id");
            value["config_revision"] = json!(crate::providers::config_revision(
                &value["config_revision"]
            )?);
        }
        decode(value)
    }
    pub fn configure(&mut self, selected: &str) -> Result<()> {
        let value = self.app("configure", json!({"model":selected,"voice":null}))?;
        let config = self.local_config(value)?;
        if config.model.as_deref() != Some(selected) || config.voice.is_some() {
            return Err("Saved preference response mismatch; synchronize before retry".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    request_id: String,
    status: String,
    phase: String,
    config_revision: u64,
}

fn fields(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or("Unexpected response/argument shape")?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("Unexpected response/argument fields".into());
    }
    Ok(())
}
fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    bounded_value(&value)?;
    serde_json::from_value(value).map_err(|_| "Invalid typed application response".into())
}
fn bounded_value(value: &Value) -> Result<()> {
    let mut pending = vec![(value, 0)];
    while let Some((value, depth)) = pending.pop() {
        if depth > 64 {
            return Err("Application JSON nesting bound exceeded".into());
        }
        match value {
            Value::Array(items) => pending.extend(items.iter().map(|v| (v, depth + 1))),
            Value::Object(items) => pending.extend(items.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    if serde_json::to_vec(value)
        .map_err(|_| "Invalid app JSON")?
        .len()
        > app::FRAME_LIMIT
    {
        return Err("Application response exceeds frame bound".into());
    }
    Ok(())
}
fn valid_error(error: &AppError) -> bool {
    error
        .code
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_uppercase)
        && error.code.len() <= 256
        && error
            .code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        && !error.message.trim().is_empty()
        && error.message.len() <= 1024
}
fn valid_token(token: &str) -> bool {
    token.split_once('.').is_some_and(|(time, hash)| {
        !time.is_empty()
            && time.len() <= 20
            && time.bytes().all(|b| b.is_ascii_digit())
            && hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn checked_core(observation: Observation) -> Result<Observation> {
    if observation.schema_version != 2
        || observation.service != "smith-next"
        || !["installed", "missing", "conflict", "unknown"]
            .contains(&observation.binary_state.as_str())
        || !["installed", "missing", "conflict", "unknown"]
            .contains(&observation.unit_state.as_str())
        || !["enabled", "disabled", "missing", "conflict", "unknown"]
            .contains(&observation.enablement_state.as_str())
        || !["running", "stopped", "conflict", "unknown", "unavailable"]
            .contains(&observation.runtime_state.as_str())
        || !["ready", "conflict", "unknown", "unavailable"]
            .contains(&observation.bridge_state.as_str())
        || observation.message.len() > 1024
        || observation.error.as_ref().is_some_and(|e| e.len() > 1024)
        || observation
            .approval_token
            .as_deref()
            .is_some_and(|t| !valid_token(t))
        || serde_json::to_vec(&observation)
            .map_err(|_| "Invalid Core response")?
            .len()
            > CORE_LIMIT
    {
        return Err("Core identity/schema/bounds mismatch".into());
    }
    if !observation.ok || observation.error.is_some() {
        return Err(format!(
            "Native preflight/action failed: {}; inspect prerequisites and current facts",
            consent::display(observation.error.as_deref().unwrap_or("unknown"))
        ));
    }
    Ok(observation)
}

pub fn validate_args(op: &str, args: &Value) -> Result<()> {
    let expected: &[&str] = match op {
        "state" => &[],
        "models" => &["request_id"],
        "poll" => &["request_id", "instance_id"],
        "configure" => &["model", "voice"],
        "configure_checked" => &["model", "voice", "expected_revision"],
        "verify_model" | "remove_model" => &["request_id", "model", "sha256", "expected_revision"],
        "submit" => &["request_id", "text", "output"],
        _ => return Err("Unsupported installer app operation".into()),
    };
    fields(args, expected)?;
    for field in ["request_id", "instance_id"] {
        if let Some(value) = args.get(field) {
            if !value.as_str().is_some_and(app::valid_id) {
                return Err("Invalid operation identity".into());
            }
        }
    }
    if ["configure", "configure_checked"].contains(&op)
        && (!args["model"].as_str().is_some_and(model::valid_model) || !args["voice"].is_null())
    {
        return Err("Invalid exact text preference".into());
    }
    if ["configure_checked", "verify_model", "remove_model"].contains(&op)
        && args["expected_revision"].as_u64().is_none()
    {
        return Err("Invalid expected revision".into());
    }
    if ["verify_model", "remove_model"].contains(&op) {
        let entry = crate::catalog::entry(args["model"].as_str().ok_or("Invalid model")?)?;
        if args["sha256"].as_str() != Some(entry.sha256) {
            return Err("Catalog SHA mismatch".into());
        }
    }
    if op == "submit"
        && (!args["text"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty() && s.len() <= app::MAX_TEXT)
            || args["output"].as_str() != Some("chat"))
    {
        return Err("Invalid text submission".into());
    }
    Ok(())
}

pub fn checked_inventory(value: Value) -> Result<BTreeSet<String>> {
    let inventory: Inventory = decode(value)?;
    let names = inventory_names(&inventory)?;
    if !inventory.providers.llm.ready {
        return Err("Native runtime unavailable; install the complete paired Smith package".into());
    }
    Ok(names)
}

// Cached state may legitimately have no observed/available local runtime. Share
// strict shape/identity validation without turning that fact into a setup error.
fn inventory_names(inventory: &Inventory) -> Result<BTreeSet<String>> {
    if !inventory.voices.is_empty() || inventory.models.len() > 256 {
        return Err("Unexpected active voice or oversized model inventory".into());
    }
    let observations = &inventory.providers;
    for observation in [&observations.llm, &observations.tts] {
        if observation.ready != observation.error.is_none()
            || observation.error.as_ref().is_some_and(|e| !valid_error(e))
        {
            return Err("Invalid provider readiness/error facts".into());
        }
    }
    if observations.stt.ready != observations.stt.error.is_none()
        || observations
            .stt
            .error
            .as_ref()
            .is_some_and(|e| !valid_error(e))
    {
        return Err("Invalid STT readiness/error facts".into());
    }
    if observations.stt.ready
        || observations.stt.model.is_some()
        || observations.tts.ready
        || observations.stt.error.as_ref().map(|e| e.code.as_str()) != Some("RETIRED")
        || observations.tts.error.as_ref().map(|e| e.code.as_str()) != Some("RETIRED")
    {
        return Err("Unexpected active speech provider/model".into());
    }
    if !observations.llm.ready && !inventory.models.is_empty() {
        return Err("Unavailable local provider has model inventory".into());
    }
    let mut names = BTreeSet::new();
    for entry in &inventory.models {
        if !model::valid_model(&entry.name)
            || entry.digest.len() != 64
            || !entry.digest.bytes().all(|b| b.is_ascii_hexdigit())
            || !names.insert(entry.name.clone())
        {
            return Err("Invalid/ambiguous eligible model inventory".into());
        }
    }
    Ok(names)
}

fn checked_state_v3(value: &Value, revision: u64) -> Result<()> {
    use crate::providers::{check_operation, config_revision, id, safe};
    let observed = value["inventory_observed"]
        .as_bool()
        .ok_or("Invalid local inventory observation flag")?;
    let inventory: Inventory = decode(value["inventory"].clone())?;
    inventory_names(&inventory)?;
    if !observed
        && (inventory.providers.llm.ready
            || inventory
                .providers
                .llm
                .error
                .as_ref()
                .map(|e| e.code.as_str())
                != Some("NOT_OBSERVED"))
    {
        return Err("Unobserved local inventory has observed facts".into());
    }
    if !safe(&value["revision"]) {
        return Err("Invalid state revision".into());
    }
    let expected_retention = json!({
        "max_conversations":32,"max_turn_pairs":16,"max_context_bytes":65536,
        "max_terminal_jobs":32,"max_expired_ids":1024,"max_pending_jobs":4,
        "queue_timeout_ms":600000,"capacity_policy":"reject_when_expiry_metadata_full",
        "missing_request_code":"REQUEST_EXPIRED"
    });
    if value["retention"] != expected_retention {
        return Err("Unexpected state retention contract".into());
    }
    let conversations = value["conversations"]
        .as_array()
        .filter(|items| items.len() <= 32)
        .ok_or("Invalid bounded conversation summaries")?;
    let mut conversations_seen = BTreeSet::new();
    let mut origins_seen = BTreeSet::new();
    let mut previous_sequence = None;
    for conversation in conversations {
        fields(
            conversation,
            &[
                "conversation_id",
                "title",
                "archived",
                "metadata_revision",
                "history_revision",
                "sequence",
                "origin_request_id",
            ],
        )?;
        // Historical titles keep the wire's 256-byte limit, not the newer
        // 40-scalar limit on writes. Reading must not trap existing chats.
        let title_valid = conversation["title"].as_str().is_some_and(|title| {
            !title.is_empty() && title.trim() == title && title.len() <= 256
                && !title.chars().any(|c| c.is_control()
                    || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        });
        if !id(&conversation["conversation_id"])
            || !conversations_seen.insert(conversation["conversation_id"].as_str().unwrap_or(""))
            || !title_valid
            || !conversation["archived"].is_boolean()
            || !safe(&conversation["metadata_revision"])
            || !safe(&conversation["history_revision"])
            || !safe(&conversation["sequence"])
            || (!conversation["origin_request_id"].is_null()
                && (!id(&conversation["origin_request_id"])
                    || !origins_seen
                        .insert(conversation["origin_request_id"].as_str().unwrap_or(""))))
        {
            return Err("Invalid conversation summary".into());
        }
        let sequence = conversation["sequence"]
            .as_u64()
            .ok_or("Invalid sequence")?;
        if previous_sequence.is_some_and(|previous| sequence >= previous) {
            return Err("Invalid conversation ordering/sequence".into());
        }
        previous_sequence = Some(sequence);
    }
    let operations = value["operations"]
        .as_array()
        .filter(|items| items.len() <= 32)
        .ok_or("Invalid bounded operation summaries")?;
    let mut requests = BTreeSet::new();
    let mut pending_conversations = BTreeSet::new();
    let mut pending = 0;
    let mut running = 0;
    let mut global_pending = 0;
    for operation in operations {
        check_operation(operation, false)?;
        if !requests.insert(operation["request_id"].as_str().ok_or("Invalid request")?)
            || config_revision(&operation["config_revision"])? > revision
        {
            return Err("Invalid operation identity/configuration revision".into());
        }
        if let Some(conversation) = operation["conversation_id"].as_str() {
            if conversations_seen.contains(conversation)
                == (operation["kind"] == "conversation_delete")
            {
                return Err("Invalid operation conversation reference".into());
            }
        }
        if matches!(operation["status"].as_str(), Some("queued" | "running")) {
            pending += 1;
            running += usize::from(operation["status"] == "running");
            if let Some(conversation) = operation["conversation_id"].as_str() {
                if !pending_conversations.insert(conversation) {
                    return Err("Multiple pending operations in one conversation".into());
                }
            } else {
                global_pending += 1;
            }
        }
    }
    if pending > 4 || running > 1 || (global_pending != 0 && pending != 1) {
        return Err("Invalid pending operation bounds".into());
    }
    Ok(())
}
