//! Tool-free local text provider backed by the private native runner.
//! No network endpoint, environment override or cloud fallback exists.
use crate::acceleration::Backend;
use crate::app::{AppError, AppResult, Message};
use crate::catalog::{CatalogEntry, CATALOG};
use crate::runner::{self, Runner, RunnerConfig};
use crate::{Result, Roots};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};

fn error(code: &str, msg: &str) -> AppError {
    AppError::new(code, msg)
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub ready: bool,
    #[serde(deserialize_with = "crate::app::required_nullable")]
    pub error: Option<AppError>,
}
impl Default for Observation {
    fn default() -> Self {
        Self {
            ready: false,
            error: Some(error(
                "NOT_OBSERVED",
                "Provider has not been explicitly refreshed",
            )),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SttObservation {
    pub ready: bool,
    #[serde(deserialize_with = "crate::app::required_nullable")]
    pub error: Option<AppError>,
    #[serde(deserialize_with = "crate::app::required_nullable")]
    pub model: Option<String>,
}
impl Default for SttObservation {
    fn default() -> Self {
        Self {
            ready: false,
            error: Some(retired_error()),
            model: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    pub llm: Observation,
    pub stt: SttObservation,
    pub tts: Observation,
}
fn retired_error() -> AppError {
    error("RETIRED", "Speech capability is retired; use text chat")
}
impl Default for Observations {
    fn default() -> Self {
        Self {
            llm: Observation::default(),
            stt: SttObservation::default(),
            tts: Observation {
                ready: false,
                error: Some(retired_error()),
            },
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub models: Vec<Model>,
    pub voices: Vec<String>,
    pub providers: Observations,
}
impl Inventory {
    pub(crate) fn retire_speech(&mut self) {
        self.voices.clear();
        self.providers.stt = SttObservation::default();
        self.providers.tts = Observations::default().tts;
    }
    pub(crate) fn valid(&self) -> bool {
        let mut models = HashSet::new();
        let mut voices = HashSet::new();
        self.models.len() <= 256
            && self.voices.len() <= 256
            && self.models.iter().all(|m| {
                valid_tag_name(&m.name) && valid_digest(&m.digest) && models.insert(&m.name)
            })
            && self
                .voices
                .iter()
                .all(|v| valid_label(v) && voices.insert(v))
            && valid_observation(self.providers.llm.ready, self.providers.llm.error.as_ref())
            && valid_observation(self.providers.stt.ready, self.providers.stt.error.as_ref())
            && valid_observation(self.providers.tts.ready, self.providers.tts.error.as_ref())
            && self
                .providers
                .stt
                .model
                .as_ref()
                .is_none_or(|m| valid_label(m))
            && (!self.providers.stt.ready || self.providers.stt.model.is_some())
            && (self.providers.llm.ready || self.models.is_empty())
            && if self.providers.tts.ready {
                !self.voices.is_empty()
            } else {
                self.voices.is_empty()
            }
            && crate::app::reply_fits(self)
    }
}
fn valid_observation(ready: bool, error: Option<&AppError>) -> bool {
    if ready {
        error.is_none()
    } else {
        error.is_some_and(AppError::valid)
    }
}
fn valid_label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn valid_tag_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name
            .rsplit_once(':')
            .is_some_and(|(model, tag)| !model.is_empty() && !tag.is_empty())
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/:_.-".contains(&c))
        && !name.to_ascii_lowercase().contains("cloud")
        && !name.starts_with("http:")
        && !name.starts_with("https:")
}
fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit())
}
/// Library injection seam; production always constructs the native-runner Providers.
/// Every method runs on the single app worker thread, which therefore owns any
/// runner child (its parent-death signal follows that thread).
pub trait Provider: Send + Sync + 'static {
    fn inventory(&self) -> AppResult<Inventory>;
    fn chat(&self, model: &str, messages: &[Message]) -> AppResult<String>;
    /// Fixtures must opt in explicitly; never pretend unsupported removal succeeded.
    fn remove_model(&self, _model: &str, _sha256: &str) -> AppResult<()> {
        Err(error(
            "UNSUPPORTED",
            "Provider does not support model removal",
        ))
    }
    /// How long the worker may wait for work before calling [`Provider::idle`].
    fn idle_timeout(&self) -> Option<Duration> {
        None
    }
    fn idle(&self) {}
    /// Called once when the worker exits.
    fn shutdown(&self) {}
    /// Must not block behind a running provider call. True requires worker join.
    fn cancel_owned(&self) -> bool {
        false
    }
}

/// Rust-only seam for isolated tests (like `Manager`); executable flags and the
/// environment cannot change these values.
#[derive(Clone, Debug)]
pub struct ProviderOptions {
    pub catalog: &'static [CatalogEntry],
    pub budget: Duration,
    pub idle: Duration,
    pub threads: u32,
}
impl Default for ProviderOptions {
    fn default() -> Self {
        Self {
            catalog: CATALOG,
            budget: runner::BUDGET,
            idle: runner::IDLE,
            threads: runner::default_threads(),
        }
    }
}

struct Active {
    runner: Option<(String, Runner)>,
    used: Instant,
    last_startup_fallback_reason: Option<AppError>,
}
pub struct Providers {
    roots: Roots,
    options: ProviderOptions,
    cancelled: Arc<AtomicBool>,
    active: Mutex<Active>,
}
fn store_error() -> AppError {
    error(
        "MODEL_STORE_UNSAFE",
        "Installed model records are unsafe, corrupt or ambiguous; preserved",
    )
}
impl Providers {
    pub fn new(roots: &Roots) -> Result<Self> {
        Ok(Self::with_options(roots, ProviderOptions::default()))
    }
    pub fn with_options(roots: &Roots, options: ProviderOptions) -> Self {
        Self {
            roots: roots.clone(),
            cancelled: Arc::new(AtomicBool::new(false)),
            options,
            active: Mutex::new(Active {
                runner: None,
                used: Instant::now(),
                last_startup_fallback_reason: None,
            }),
        }
    }
    /// Internal last-preparation history, not current execution mode or a wire API.
    /// Only fixed runtime/preparation errors are retained; native stderr is never copied.
    /// Reuse and idle unload preserve it; a fresh preparation or provider restart clears it.
    pub fn last_startup_fallback_reason(&self) -> AppResult<Option<AppError>> {
        match self.active.try_lock() {
            Ok(active) => Ok(active.last_startup_fallback_reason.clone()),
            Err(TryLockError::WouldBlock) => Err(error(
                "DIAGNOSTICS_BUSY",
                "Runner preparation or inference is in progress",
            )),
            Err(TryLockError::Poisoned(_)) => {
                Err(error("LLM_UNAVAILABLE", "Local runner state unavailable"))
            }
        }
    }
    fn records(&self) -> AppResult<Vec<runner::ModelRecord>> {
        runner::list_models_with(&runner::models_dir(&self.roots), self.options.catalog)
            .map_err(|_| store_error())
    }
    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, Active>> {
        self.active
            .lock()
            .map_err(|_| error("LLM_UNAVAILABLE", "Local runner state unavailable"))
    }
    fn stop(active: &mut Active) {
        if let Some((_, runner)) = active.runner.take() {
            let _ = runner.stop();
        }
    }
}
impl Provider for Providers {
    fn inventory(&self) -> AppResult<Inventory> {
        let mut i = Inventory::default();
        match runner::verify_runtime_bounded(
            &self.roots,
            Instant::now() + self.options.budget,
            &|| self.cancelled.load(Ordering::Acquire),
        ) {
            Ok(_) => {
                i.models = self
                    .records()?
                    .into_iter()
                    .map(|r| Model {
                        name: r.name,
                        digest: r.sha256,
                    })
                    .collect();
                i.providers.llm = Observation {
                    ready: true,
                    error: None,
                };
            }
            Err(e) => i.providers.llm.error = Some(e),
        }
        Ok(i)
    }
    fn chat(&self, model: &str, messages: &[Message]) -> AppResult<String> {
        let deadline = Instant::now() + self.options.budget;
        let mut active = self.lock()?;
        // Ollama-era or removed names are never migrated: reselect explicitly.
        let record = self
            .records()?
            .into_iter()
            .find(|r| r.name == model)
            .ok_or_else(|| {
                error(
                    "MODEL_UNAVAILABLE",
                    "Exact curated local model is not installed; select an installed model",
                )
            })?;
        let reusable = active
            .runner
            .as_ref()
            .is_some_and(|(sha, r)| *sha == record.sha256 && r.is_running());
        if !reusable {
            active.last_startup_fallback_reason = None;
            // Model switch or crash: stop first so the single-runner lock is free.
            Self::stop(&mut active);
            let cancellation = self.cancelled.clone();
            let cancelled: runner::Cancellation =
                Arc::new(move || cancellation.load(Ordering::Acquire));
            let file =
                runner::verify_model_leased(&self.roots, &record, deadline, cancelled.as_ref())
                    .map_err(|message| {
                        for code in ["RUNNER_BUSY", "RUNNER_CANCELLED", "PROVIDER_TIMEOUT"] {
                            if message.starts_with(&format!("{code}:")) {
                                return error(code, "Model verification could not acquire or retain its bounded runner lease");
                            }
                        }
                        error(
                            "MODEL_UNAVAILABLE",
                            "Selected model failed integrity or safety verification; reinstall it",
                        )
                    })?;
            remaining(deadline)?;
            let gpu = match runner::verify_runtime_backend_bounded(
                &self.roots,
                Backend::Vulkan,
                deadline,
                cancelled.as_ref(),
            ) {
                Ok(executable) => {
                    let mut config = RunnerConfig::production(&self.roots, executable);
                    config.threads = self.options.threads;
                    config.backend = Backend::Vulkan;
                    match Runner::launch_verified(&config, &file, deadline, cancelled.clone()) {
                        Ok(started) => Some(started),
                        Err(e) if e.code == "GPU_PREPARATION" => {
                            // This code is returned only after confirmed runner cleanup.
                            active.last_startup_fallback_reason = Some(e);
                            None
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err(e) if e.code == "RUNTIME_MISSING" => {
                    active.last_startup_fallback_reason = Some(e);
                    None
                }
                Err(e) => return Err(e),
            };
            let started = if let Some(started) = gpu {
                started
            } else {
                remaining(deadline)?;
                let executable = runner::verify_runtime_backend_bounded(
                    &self.roots,
                    Backend::Cpu,
                    deadline,
                    cancelled.as_ref(),
                )?;
                let mut config = RunnerConfig::production(&self.roots, executable);
                config.threads = self.options.threads;
                Runner::launch_verified(&config, &file, deadline, cancelled)?
            };
            active.runner = Some((record.sha256, started));
        }
        let (_, runner) = active
            .runner
            .as_mut()
            .ok_or_else(|| error("LLM_UNAVAILABLE", "Local runner unavailable"))?;
        let result = runner.chat(messages, runner::MAX_TOKENS, deadline);
        let alive = runner.is_running();
        active.used = Instant::now();
        if !alive {
            Self::stop(&mut active);
        }
        result
    }
    fn remove_model(&self, model: &str, sha256: &str) -> AppResult<()> {
        let _active = self.lock()?;
        runner::remove_model_with(
            &self.roots,
            model,
            sha256,
            self.options.catalog,
            Instant::now() + self.options.budget,
            &|| self.cancelled.load(Ordering::Acquire),
        )
    }
    fn idle_timeout(&self) -> Option<Duration> {
        let active = self.active.lock().ok()?;
        active.runner.as_ref()?;
        Some(
            (active.used + self.options.idle)
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1)),
        )
    }
    fn idle(&self) {
        if let Ok(mut active) = self.active.lock() {
            if active.runner.is_some() && active.used.elapsed() >= self.options.idle {
                Self::stop(&mut active);
            }
        }
    }
    fn cancel_owned(&self) -> bool {
        self.cancelled.store(true, Ordering::Release);
        true
    }
    fn shutdown(&self) {
        if let Ok(mut active) = self.active.lock() {
            Self::stop(&mut active);
        }
    }
}

fn remaining(deadline: Instant) -> AppResult<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| error("PROVIDER_TIMEOUT", "Operation budget exhausted; no retry"))
}
