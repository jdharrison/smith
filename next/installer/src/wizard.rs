//! Worker-owned paired installation and separately consented model management.
//! GTK never owns effect-capable objects or spawns a CLI.
use crate::artifact::{self, Outcome};
use crate::setup::{self, Backend, Native};
use crate::{consent, maintenance, Result};
use smith_service::{Action, Controller, Observation, Roots, SystemManager};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const POLL: Duration = Duration::from_millis(10);
const TEXT_LIMIT: usize = 64 * 1024;
const OUTPUT_LIMIT: usize = 256 * 1024;
const DETAIL_LIMIT: usize = 64 * 1024;
const DETAIL_COUNT: usize = 128;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
pub type ProviderFactory = Box<dyn FnMut() -> Result<Box<dyn crate::model::Provider>> + Send>;
pub type MaintenanceFactory =
    Box<dyn FnMut(PathBuf) -> Result<Box<dyn maintenance::Runner>> + Send>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    pub register_service: bool,
    pub enable_service: bool,
    pub start_service: bool,
}
impl Choices {
    pub const fn smith_install() -> Self {
        Self {
            register_service: true,
            enable_service: true,
            start_service: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Install {
        request_id: u64,
        choices: Choices,
    },
    Approve {
        request_id: u64,
    },
    Decline {
        request_id: u64,
    },
    RefreshReview,
    ManageModels,
    Upgrade,
    Uninstall,
    ModelAction {
        request_id: u64,
        model: String,
        remove: bool,
    },
    Cancel,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Review {
        request_id: u64,
        description: String,
    },
    Stage {
        message: String,
    },
    /// Observation only; never consent or evidence of verified installation.
    Progress {
        downloaded: u64,
        total: u64,
    },
    Models {
        request_id: u64,
        selected: Option<String>,
        installed: Vec<String>,
    },
    Confirmation {
        request_id: u64,
        title: String,
        effects: Vec<String>,
    },
    Finished {
        report: Report,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub scope: Scope,
    pub complete: bool,
    pub cancelled: bool,
    pub message: String,
    pub steps: Vec<StepResult>,
    pub details: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Installation,
    Upgrade,
    Uninstall,
    ModelCheck,
    ModelRemoval,
}
impl Scope {
    pub fn completion_heading(self) -> &'static str {
        match self {
            Self::Installation => "Installation complete",
            Self::Upgrade => "Upgrade complete",
            Self::Uninstall => "Uninstall complete",
            Self::ModelCheck => "Model check complete",
            Self::ModelRemoval => "Model removal complete",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepResult {
    pub label: String,
    pub state: StepState,
    pub detail: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Completed,
    Skipped,
    Failed,
    Uncertain,
}

/// Opaque publisher injection, not a second filesystem implementation. Objects
/// and their captured reviews are created/consumed on the one owner worker.
pub trait PreparedReview {
    fn description(&self) -> &str;
    fn review_id(&self) -> &str;
}
pub trait Publisher {
    type Prepared: PreparedReview;
    fn prepare_review(&mut self) -> Result<Self::Prepared>;
    fn install_reviewed(
        &mut self,
        prepared: Self::Prepared,
        approved_review_id: &str,
        output: &mut dyn Write,
    ) -> Result<Outcome>;
}
impl PreparedReview for artifact::PreparedArtifacts {
    fn description(&self) -> &str {
        artifact::PreparedArtifacts::description(self)
    }
    fn review_id(&self) -> &str {
        artifact::PreparedArtifacts::review_id(self)
    }
}
impl Publisher for artifact::Installer {
    type Prepared = artifact::PreparedArtifacts;
    fn prepare_review(&mut self) -> Result<Self::Prepared> {
        artifact::Installer::prepare_review(self)
    }
    fn install_reviewed(
        &mut self,
        prepared: Self::Prepared,
        approved_review_id: &str,
        output: &mut dyn Write,
    ) -> Result<Outcome> {
        artifact::Installer::install_reviewed(self, prepared, approved_review_id, output)
    }
}

/// Library-only isolated cancellation seam. Production always uses the composed
/// installer/Core handler; tests need not clear a monotonic process signal flag.
#[derive(Clone)]
pub struct Cancellation {
    observed: Arc<dyn Fn() -> bool + Send + Sync>,
    request: Arc<dyn Fn() + Send + Sync>,
}
impl Cancellation {
    pub fn from_callbacks(
        observed: impl Fn() -> bool + Send + Sync + 'static,
        request: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            observed: Arc::new(observed),
            request: Arc::new(request),
        }
    }
    fn process() -> Self {
        Self::from_callbacks(consent::cancelled, consent::request_cancel)
    }
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub pending_timeout: Duration,
    pub event_timeout: Duration,
    pub channel_capacity: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            pending_timeout: Duration::from_secs(60),
            event_timeout: Duration::from_secs(1),
            channel_capacity: 8,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<Self> {
        if self.pending_timeout.is_zero()
            || self.pending_timeout > Duration::from_secs(60)
            || self.event_timeout.is_zero()
            || self.event_timeout > Duration::from_secs(2)
            || !(1..=8).contains(&self.channel_capacity)
        {
            return Err("Wizard limits may only reduce the bounded production budgets".into());
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Review,
    Core,
    Model,
}
#[derive(Clone, Copy)]
struct Pending {
    id: u64,
    kind: Kind,
}
#[derive(Default)]
struct Gate {
    pending: Option<Pending>,
    queued: bool,
    installing: bool,
    finished: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Stop {
    None = 0,
    Cancelled = 1,
    Protocol = 2,
    Backpressure = 3,
    Disconnected = 4,
}
#[derive(Default)]
struct Numeric {
    generation: u64,
    latest: Option<(u64, u64)>,
}
impl Numeric {
    fn clear(&mut self) {
        self.latest = None;
        self.generation = self.generation.wrapping_add(1);
    }
}
struct Shared {
    latest_numeric: Mutex<Numeric>,
    gate: Mutex<Gate>,
    stop: AtomicU8,
    cancellation: Cancellation,
    report: Mutex<Option<Report>>,
}
impl Shared {
    // Each enqueue and invalidation is atomic relative to the UI's second FIFO
    // check and snapshot selection. Never hold this lock while waiting for space.
    fn emit(
        &self,
        events: &SyncSender<Event>,
        mut event: Event,
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        loop {
            self.check()?;
            if cancelled() || Instant::now() >= deadline {
                self.stop(Stop::Backpressure);
                return Err(
                    "Model progress stopped/backpressured; partial effects preserved".into(),
                );
            }
            match self.latest_numeric.try_lock() {
                Ok(mut numeric) => {
                    numeric.clear();
                    match events.try_send(event) {
                        Ok(()) => return Ok(()),
                        Err(TrySendError::Full(value)) => event = value,
                        Err(TrySendError::Disconnected(_)) => {
                            self.stop(Stop::Disconnected);
                            return self.check();
                        }
                    }
                }
                Err(std::sync::TryLockError::WouldBlock) => {}
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err("Numeric progress storage unavailable".into())
                }
            }
            thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
        }
    }
    fn stop(&self, reason: Stop) {
        let _ = self.stop.compare_exchange(
            Stop::None as u8,
            reason as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        (self.cancellation.request)();
    }
    fn reason(&self) -> Stop {
        match self.stop.load(Ordering::Acquire) {
            1 => Stop::Cancelled,
            2 => Stop::Protocol,
            3 => Stop::Backpressure,
            4 => Stop::Disconnected,
            _ if (self.cancellation.observed)() => Stop::Cancelled,
            _ => Stop::None,
        }
    }
    fn check(&self) -> Result<()> {
        match self.reason() {
            Stop::None => Ok(()),
            Stop::Cancelled => {
                Err("Installer operation cancelled; accepted effects remain; no rollback".into())
            }
            Stop::Protocol => {
                Err("Stale, duplicate or invalid UI command; further effects blocked".into())
            }
            Stop::Backpressure => Err(
                "UI channel backpressure; reporting may be incomplete; further effects blocked"
                    .into(),
            ),
            Stop::Disconnected => Err(
                "UI channel disconnected; accepted effects remain; further effects blocked".into(),
            ),
        }
    }
    fn pending(&self, pending: Option<Pending>) {
        let mut gate = self.gate.lock().unwrap_or_else(|p| p.into_inner());
        gate.pending = pending;
        gate.queued = false;
    }
}

pub struct Session {
    commands: SyncSender<Command>,
    events: Option<Receiver<Event>>,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    reported: AtomicBool,
}
impl Session {
    pub fn spawn(bundle: PathBuf) -> Result<Self> {
        // The native entrypoint installs this BEFORE GTK. Idempotence also makes
        // this guard safe when the shell has already installed the same handlers.
        consent::install_cancellation()?;
        Self::spawn_extended_with(
            bundle,
            |client, service| artifact::Installer::new(Some(client), Some(service)),
            |source| {
                Ok(Box::new(Controller {
                    roots: Roots::environment()?,
                    source,
                    manager: SystemManager,
                }) as Box<dyn Backend>)
            },
            setup::prerequisites,
            Cancellation::process(),
            Limits::default(),
            Some(Box::new(|| {
                Ok(Box::new(crate::catalog::Native::new()?) as Box<dyn crate::model::Provider>)
            })),
            Some(Box::new(|bundle| {
                Ok(Box::new(maintenance::Native::new(bundle)?) as Box<dyn maintenance::Runner>)
            })),
        )
    }
    /// Factories cross the thread boundary; publisher/Native/Roots/reviews do not.
    /// No executable flag or environment can select these library test seams.
    pub fn spawn_with<P, PF, BF, HF>(
        bundle: PathBuf,
        publisher: PF,
        backend: BF,
        preflight: HF,
        cancellation: Cancellation,
        limits: Limits,
    ) -> Result<Self>
    where
        P: Publisher + 'static,
        PF: FnMut(PathBuf, PathBuf) -> Result<P> + Send + 'static,
        BF: FnMut(PathBuf) -> Result<Box<dyn Backend>> + Send + 'static,
        HF: FnMut() -> Result<()> + Send + 'static,
    {
        Self::spawn_product_with(
            bundle,
            publisher,
            backend,
            preflight,
            cancellation,
            limits,
            None,
        )
    }
    /// Typed provider seam; None retains the explicit artifact/lifecycle-only recipe.
    pub fn spawn_product_with<P, PF, BF, HF>(
        bundle: PathBuf,
        publisher: PF,
        backend: BF,
        preflight: HF,
        cancellation: Cancellation,
        limits: Limits,
        provider: Option<ProviderFactory>,
    ) -> Result<Self>
    where
        P: Publisher + 'static,
        PF: FnMut(PathBuf, PathBuf) -> Result<P> + Send + 'static,
        BF: FnMut(PathBuf) -> Result<Box<dyn Backend>> + Send + 'static,
        HF: FnMut() -> Result<()> + Send + 'static,
    {
        Self::spawn_extended_with(
            bundle,
            publisher,
            backend,
            preflight,
            cancellation,
            limits,
            provider,
            None,
        )
    }
    /// Maintenance is constructed on the owned worker only after an explicit action.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_extended_with<P, PF, BF, HF>(
        bundle: PathBuf,
        publisher: PF,
        backend: BF,
        preflight: HF,
        cancellation: Cancellation,
        limits: Limits,
        provider: Option<ProviderFactory>,
        maintenance: Option<MaintenanceFactory>,
    ) -> Result<Self>
    where
        P: Publisher + 'static,
        PF: FnMut(PathBuf, PathBuf) -> Result<P> + Send + 'static,
        BF: FnMut(PathBuf) -> Result<Box<dyn Backend>> + Send + 'static,
        HF: FnMut() -> Result<()> + Send + 'static,
    {
        let limits = limits.validate()?;
        if !bundle.is_absolute() || bundle.as_os_str().len() > TEXT_LIMIT {
            return Err("Wizard requires an explicit bounded absolute package root".into());
        }
        let (tx, commands) = mpsc::sync_channel(limits.channel_capacity);
        let (events, rx) = mpsc::sync_channel(limits.channel_capacity);
        let shared = Arc::new(Shared {
            latest_numeric: Mutex::new(Numeric::default()),
            gate: Mutex::new(Gate::default()),
            stop: AtomicU8::new(0),
            cancellation,
            report: Mutex::new(None),
        });
        let owner = shared.clone();
        let worker = thread::Builder::new().name("smith-installer-wizard".into()).spawn(move || {
            let mut report = initial_report();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut engine = Engine { bundle, publisher_factory: publisher, backend_factory: backend,
                    preflight, commands, events: events.clone(), shared: owner.clone(), limits, provider, maintenance };
                engine.run(&mut report)
            }));
            match result {
                Ok(Ok(())) => {
                    if let Err(error) = owner.check() { report.complete = false; report.message = bounded(&error); }
                }
                Ok(Err(error)) => { report.complete = false; report.message = bounded(&error); }
                Err(_) => {
                    report.complete = false;
                    report.message = "Installer worker failed; accepted/uncertain effects preserved; no retry or rollback".into();
                    if report.steps.iter().all(|s| s.state != StepState::Uncertain) {
                        if let Some(step) = report.steps.iter_mut().find(|s| s.state != StepState::Completed) {
                            step.state = StepState::Failed;
                            step.detail = "Worker failure; this step was not verified".into();
                        }
                    }
                }
            }
            report.cancelled = owner.reason() == Stop::Cancelled;
            add_detail(&mut report, "No desktop launch, automatic retry/rollback or lingering changes. Registration, Enable and Start occur only as separately reported exact actions. Stopping observation does not stop service-accepted model work.");
            // All effect-owning objects have been dropped before publishing the
            // final bounded report. A full/lost UI cannot discard the worker.
            let mut numeric = owner.latest_numeric.lock().unwrap_or_else(|p| p.into_inner());
            numeric.clear();
            let mut gate = owner.gate.lock().unwrap_or_else(|p| p.into_inner());
            if owner.reason() != Stop::None { report.complete = false; }
            gate.pending = None; gate.finished = true;
            drop(gate);
            if let Err(error) = events.try_send(Event::Finished { report: report.clone() }) {
                owner.stop(match error { TrySendError::Full(_) => Stop::Backpressure, TrySendError::Disconnected(_) => Stop::Disconnected });
                report.complete = false;
                report.message = "Final UI report could not be delivered; verified/uncertain effects remain; no rollback inferred".into();
                report.cancelled = owner.reason() == Stop::Cancelled;
            }
            *owner.report.lock().unwrap_or_else(|p| p.into_inner()) = Some(report);
            drop(numeric);
        }).map_err(|_| "Cannot start the owned installer worker")?;
        Ok(Self {
            commands: tx,
            events: Some(rx),
            shared,
            worker: Some(worker),
            reported: AtomicBool::new(false),
        })
    }
    pub fn send(&self, command: Command) -> Result<()> {
        if matches!(command, Command::Cancel) {
            // Request the composed Core cancellation BEFORE touching a possibly
            // full command queue. Cancelling cannot depend on the worker reading it.
            self.shared.stop(Stop::Cancelled);
            let _ = self.commands.try_send(command);
            return Ok(());
        }
        let mut gate = self
            .shared
            .gate
            .try_lock()
            .map_err(|_| "Worker updating request; no command accepted")?;
        if gate.finished {
            return Err("Wizard already finished; no command accepted".into());
        }
        let valid = !gate.queued
            && self.shared.reason() == Stop::None
            && match &command {
                Command::Install {
                    request_id,
                    choices,
                } => {
                    !gate.installing
                        && (!choices.start_service || choices.register_service)
                        && gate
                            .pending
                            .is_some_and(|p| p.kind == Kind::Review && p.id == *request_id)
                }
                Command::Approve { request_id } => {
                    gate.installing
                        && gate
                            .pending
                            .is_some_and(|p| p.kind == Kind::Core && p.id == *request_id)
                }
                Command::Decline { request_id } => {
                    gate.pending.is_some_and(|p| p.id == *request_id)
                }
                Command::ManageModels | Command::Upgrade | Command::Uninstall => {
                    !gate.installing && gate.pending.is_none_or(|p| p.kind == Kind::Review)
                }
                Command::ModelAction {
                    request_id, model, ..
                } => {
                    gate.pending
                        .is_some_and(|p| p.kind == Kind::Model && p.id == *request_id)
                        && crate::catalog::entry(model).is_ok()
                }
                Command::RefreshReview => !gate.installing,
                Command::Cancel => unreachable!(),
            };
        if !valid {
            drop(gate);
            self.shared.stop(Stop::Protocol);
            return Err("Stale, duplicate, invalid recipe or out-of-phase command rejected".into());
        }
        gate.queued = true;
        if matches!(
            command,
            Command::Install { .. } | Command::ManageModels | Command::Upgrade | Command::Uninstall
        ) {
            gate.installing = true;
        }
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(error) => {
                drop(gate);
                self.shared.stop(match error {
                    TrySendError::Full(_) => Stop::Backpressure,
                    TrySendError::Disconnected(_) => Stop::Disconnected,
                });
                Err("Wizard command channel unavailable; cancellation requested; no retry".into())
            }
        }
    }
    pub fn try_event(&self) -> Result<Option<Event>> {
        if let Some(events) = &self.events {
            match events.try_recv() {
                Ok(event) => {
                    if matches!(event, Event::Finished { .. }) {
                        self.reported.store(true, Ordering::Relaxed);
                    }
                    return Ok(Some(event));
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
        }
        // Recheck critical traffic under the same lock used for boundary enqueue.
        // A racing boundary cannot leave an older numeric snapshot behind it.
        let mut numeric = match self.shared.latest_numeric.try_lock() {
            Ok(numeric) => numeric,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err("Numeric progress storage unavailable".into())
            }
        };
        if let Some(events) = &self.events {
            if let Ok(event) = events.try_recv() {
                if matches!(event, Event::Finished { .. }) {
                    self.reported.store(true, Ordering::Relaxed);
                }
                return Ok(Some(event));
            }
        }
        if !self.reported.load(Ordering::Relaxed) {
            if let Some(report) = self.terminal_report()? {
                self.reported.store(true, Ordering::Relaxed);
                return Ok(Some(Event::Finished { report }));
            }
        }
        if !self.reported.load(Ordering::Relaxed) && self.shared.reason() == Stop::None {
            if let Some((downloaded, total)) = numeric.latest.take() {
                return Ok(Some(Event::Progress { downloaded, total }));
            }
        }
        Ok(None)
    }
    pub fn terminal_report(&self) -> Result<Option<Report>> {
        match self.shared.report.try_lock() {
            Ok(report) => Ok(report.clone()),
            Err(std::sync::TryLockError::WouldBlock) => Ok(None),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                Err("Final report storage unavailable; inspect accepted effects".into())
            }
        }
    }
    /// Library-only transport-loss probe; cancellation/ownership is identical to close.
    pub fn disconnect_events(&mut self) {
        self.events.take();
        self.shared.stop(Stop::Disconnected);
    }
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn join(&mut self) -> Result<()> {
        if !self.is_finished() {
            return Err("Worker still owns work; keep polling before join".into());
        }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "Installer worker panicked")?;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.shared.stop(Stop::Cancelled);
            // Safety fallback, not a filesystem deadline. GTK should request
            // Cancel, remain responsive in Stopping, then join only when finished.
            let _ = worker.join();
        }
    }
}

struct Engine<P, PF, BF, HF>
where
    P: Publisher,
    PF: FnMut(PathBuf, PathBuf) -> Result<P>,
    BF: FnMut(PathBuf) -> Result<Box<dyn Backend>>,
    HF: FnMut() -> Result<()>,
{
    bundle: PathBuf,
    publisher_factory: PF,
    backend_factory: BF,
    preflight: HF,
    commands: Receiver<Command>,
    events: SyncSender<Event>,
    shared: Arc<Shared>,
    limits: Limits,
    provider: Option<ProviderFactory>,
    maintenance: Option<MaintenanceFactory>,
}
impl<P, PF, BF, HF> Engine<P, PF, BF, HF>
where
    P: Publisher,
    PF: FnMut(PathBuf, PathBuf) -> Result<P>,
    BF: FnMut(PathBuf) -> Result<Box<dyn Backend>>,
    HF: FnMut() -> Result<()>,
{
    fn emit(&self, event: Event) -> Result<()> {
        self.shared.emit(
            &self.events,
            event,
            Instant::now() + self.limits.event_timeout,
            || false,
        )
    }
    fn receive(&self, deadline: Option<Instant>) -> Result<Option<Command>> {
        self.receive_inner(deadline, false)
    }
    fn receive_inner(
        &self,
        deadline: Option<Instant>,
        artifact_review: bool,
    ) -> Result<Option<Command>> {
        loop {
            self.shared.check()?;
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Ok(None);
            }
            let wait = deadline.map_or(POLL, |d| {
                POLL.min(d.saturating_duration_since(Instant::now()))
            });
            match self.commands.recv_timeout(wait) {
                Ok(command) => {
                    self.shared.check()?;
                    if receipt_expired(&command, deadline, artifact_review, Instant::now()) {
                        return Err("Pending approval expired before worker receipt; no automatic reprepare or retry".into());
                    }
                    return Ok(Some(command));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.shared.stop(Stop::Disconnected);
                    return self.shared.check().map(|()| None);
                }
            }
        }
    }
    fn run(&mut self, report: &mut Report) -> Result<()> {
        let source = self.bundle.join("payload/service/smith-service");
        let client = self.bundle.join("payload/client");
        let mut publisher = None;
        let mut prepared = None;
        let mut id = 0;
        let mut deadline = None;
        loop {
            self.shared.check()?;
            if prepared.is_none() {
                self.shared.pending(None);
                let preview = (|| {
                    (self.preflight)()?;
                    if publisher.is_none() {
                        publisher = Some((self.publisher_factory)(client.clone(), source.clone())?);
                    }
                    publisher
                        .as_mut()
                        .ok_or("Publisher unavailable")?
                        .prepare_review()
                })();
                match preview {
                    Ok(review) => {
                        let description = checked_text(review.description())?;
                        if review.review_id().is_empty() || review.review_id().len() > 1024 {
                            return Err("Invalid artifact review binding".into());
                        }
                        id = next_id()?;
                        self.shared.pending(Some(Pending {
                            id,
                            kind: Kind::Review,
                        }));
                        self.emit(Event::Review {
                            request_id: id,
                            description,
                        })?;
                        deadline = Some(Instant::now() + self.limits.pending_timeout);
                        prepared = Some(review);
                    }
                    Err(error) => {
                        add_detail(
                            report,
                            &format!("Read-only preview failed: {}", bounded(&error)),
                        );
                        self.emit(Event::Stage { message: format!("Preview unavailable: {}. Explicit Refresh review is required; no effects applied.", bounded(&error)) })?;
                        publisher = None;
                    }
                }
            }
            let command = self.receive_inner(deadline, true)?;
            match command {
                Some(Command::Upgrade) => {
                    return self.maintain(maintenance::Operation::Upgrade, report)
                }
                Some(Command::Uninstall) => {
                    return self.maintain(maintenance::Operation::Uninstall, report)
                }
                Some(Command::ManageModels) if self.provider.is_some() => {
                    report.scope = Scope::ModelCheck;
                    self.shared.pending(None);
                    return self.models(source, report, false);
                }
                Some(Command::RefreshReview) => {
                    self.shared.pending(None);
                    prepared = None;
                    publisher = None;
                    deadline = None;
                }
                Some(Command::Decline { request_id }) if prepared.is_some() && request_id == id => {
                    report.message = "Artifact installation declined; no effects applied".into();
                    return Ok(());
                }
                Some(Command::Install {
                    request_id,
                    choices,
                }) if prepared.is_some() && request_id == id => {
                    if choices.enable_service && !choices.register_service {
                        return Err("Enable requires registration selected".into());
                    }
                    if choices.start_service
                        && (!choices.register_service || !choices.enable_service)
                    {
                        return Err("Start requires registration and enablement selected".into());
                    }
                    self.shared.pending(None);
                    let review = prepared.take().ok_or("Artifact review unavailable")?;
                    let approved = review.review_id().to_owned();
                    self.emit(Event::Stage {
                        message: "Installing the exact reviewed paired artifacts".into(),
                    })?;
                    self.shared.check()?;
                    step(
                        report,
                        0,
                        StepState::Uncertain,
                        "Artifact publication accepted; partial effects possible until verified",
                    );
                    let mut output = CappedOutput::default();
                    let result = publisher
                        .as_mut()
                        .ok_or("Publisher unavailable")?
                        .install_reviewed(review, &approved, &mut output);
                    add_output(report, &output.bytes);
                    if output.overflowed {
                        add_detail(report, "Publisher output exceeded bounded memory; report may be incomplete; no rollback inferred.");
                        return Err(
                            "Publisher output limit reached; artifact outcome requires inspection"
                                .into(),
                        );
                    }
                    match result? {
                        Outcome::Installed => step(report, 0, StepState::Completed, "Exact paired artifacts and publication dependencies verified; desktop not launched"),
                        _ => return Err("Unexpected reviewed-publication outcome; lifecycle effects blocked".into()),
                    }
                    self.shared.check()?;
                    self.emit(Event::Stage {
                        message: "Artifacts verified; only selected lifecycle work may follow"
                            .into(),
                    })?;
                    if choices.register_service {
                        self.lifecycle(choices, source.clone(), report)?;
                    }
                    self.shared.check()?;
                    if self.provider.is_some() {
                        let lifecycle_complete = choices == Choices::smith_install()
                            && report.steps[0..4].iter().all(|step| {
                                matches!(step.state, StepState::Completed | StepState::Skipped)
                            });
                        return self.models(source, report, lifecycle_complete);
                    }
                    if report.steps[0..4]
                        .iter()
                        .all(|step| matches!(step.state, StepState::Completed | StepState::Skipped))
                        && choices == Choices::smith_install()
                    {
                        report.complete = true;
                        report.message = "Installation complete: exact artifacts are installed and the service is enabled, running and bridge-ready. Chat/model/inference readiness was not checked; the desktop was not launched.".into();
                    } else {
                        report.message = "Setup incomplete: exact installed, enabled, running and bridge-ready state was not requested and verified. Prior effects remain.".into();
                    }
                    return Ok(());
                }
                None => {
                    self.shared.pending(None);
                    prepared = None;
                    deadline = None;
                    self.emit(Event::Stage { message: "Artifact review expired; explicit Refresh review is required. No effects applied.".into() })?;
                    // Wait without retaining an expired snapshot or re-preparing it.
                    match self.receive(None)? {
                        Some(Command::Upgrade) => {
                            return self.maintain(maintenance::Operation::Upgrade, report)
                        }
                        Some(Command::Uninstall) => {
                            return self.maintain(maintenance::Operation::Uninstall, report)
                        }
                        Some(Command::RefreshReview) => {
                            publisher = None;
                        }
                        Some(Command::ManageModels) if self.provider.is_some() => {
                            report.scope = Scope::ModelCheck;
                            self.shared.pending(None);
                            return self.models(source, report, false);
                        }
                        _ => {
                            return Err(
                                "No current artifact review; only Refresh or Cancel is allowed"
                                    .into(),
                            )
                        }
                    }
                }
                Some(Command::Cancel) => {
                    return Err("Installation cancelled; no automatic rollback".into())
                }
                _ => return Err("Unexpected command for the current artifact review".into()),
            }
            // A preview error also requires an explicit read-only refresh, never
            // a timer-driven retry of object construction or filesystem checks.
            if prepared.is_none() && publisher.is_none() && deadline.is_none() {
                // Refresh was already consumed above; the next loop prepares once.
                continue;
            }
        }
    }
    fn maintain(&mut self, operation: maintenance::Operation, report: &mut Report) -> Result<()> {
        report.scope = match operation {
            maintenance::Operation::Upgrade => Scope::Upgrade,
            maintenance::Operation::Uninstall => Scope::Uninstall,
        };
        report.steps = vec![StepResult {
            label: match operation { maintenance::Operation::Upgrade => "Upgrade", maintenance::Operation::Uninstall => "Uninstall" }.into(),
            state: StepState::Uncertain,
            detail: "Operation requested; accepted/partial/uncertain effects must be inspected if it fails. No rollback or retry.".into(),
        }];
        if matches!(operation, maintenance::Operation::Uninstall) {
            add_detail(report, "Uninstall retains ALL app/Core state, preferences, history, model data, releases and backups; no purge or recursive cleanup.");
        }
        self.shared.pending(None);
        self.shared.check()?;
        let mut runner = self
            .maintenance
            .as_mut()
            .ok_or("Maintenance is unavailable in this session")?(
            self.bundle.clone()
        )?;
        self.shared.check()?;
        let outcome = runner.execute(
            operation,
            &mut OwnedPrompt {
                engine: self,
                report,
            },
        )?;
        self.shared.check()?;
        report.complete = outcome.complete;
        report.message = bounded(&outcome.message);
        for detail in outcome.details {
            add_detail(report, &detail);
        }
        if matches!(operation, maintenance::Operation::Uninstall) {
            add_detail(report, "All app/Core state, preferences, history, model data, releases and backups are retained; disk usage is not purged.");
        } else if outcome.complete {
            add_detail(report, "No model download or model readiness check was performed. Choose Manage models separately after service upgrade.");
        }
        step(
            report,
            0,
            if outcome.complete {
                StepState::Completed
            } else {
                StepState::Uncertain
            },
            &outcome.message,
        );
        Ok(())
    }
    fn lifecycle(&mut self, choices: Choices, source: PathBuf, report: &mut Report) -> Result<()> {
        step(
            report,
            1,
            StepState::Failed,
            "Registration selected but not yet verified",
        );
        self.shared.check()?;
        let mut native = Native::new((self.backend_factory)(source)?);
        let mut facts = native.inspect()?;
        setup::usable(&facts)?;
        let mut ready_enable = None;
        let mut ready_start = None;
        if facts.unit_state == "missing" || facts.message == setup::RELOAD_MESSAGE {
            facts = self.action(
                &mut native,
                Action::InstallUnit,
                None,
                1,
                "Register / reload service",
                report,
            )?;
        } else if facts.runtime_state == "running" && facts.bridge_state == "ready" {
            step(
                report,
                1,
                StepState::Skipped,
                "Exact registration and managed runtime already verified; no reload repeated",
            );
        } else {
            // A stopped Observation alone cannot prove that the manager loaded
            // the unit. Read-only applicability of the next exact lifecycle
            // action proves registration without applying it.
            let prepared = if facts.enablement_state == "disabled" {
                native.prepare(Action::Enable).map(|value| {
                    ready_enable = Some(value);
                })
            } else {
                native.prepare(Action::Start).map(|value| {
                    ready_start = Some(value);
                })
            };
            if prepared.is_ok() {
                step(
                    report,
                    1,
                    StepState::Skipped,
                    "Exact installed unit and stopped user-manager registration verified read-only",
                );
            } else {
                facts = self.action(
                    &mut native,
                    Action::InstallUnit,
                    None,
                    1,
                    "Register / reload service",
                    report,
                )?;
            }
        }
        setup::usable(&facts)?;
        if facts.unit_state != "installed" || facts.message == setup::RELOAD_MESSAGE {
            return Err("Service registration/reload was not freshly verified".into());
        }
        if choices.enable_service {
            step(
                report,
                2,
                StepState::Failed,
                "Enable selected but not yet verified",
            );
            if facts.enablement_state == "enabled" {
                step(
                    report,
                    2,
                    StepState::Skipped,
                    "Exact persistent login enablement already verified; no Enable repeated",
                );
            } else {
                facts = self.action(
                    &mut native,
                    Action::Enable,
                    ready_enable,
                    2,
                    "Enable service at login",
                    report,
                )?;
                if facts.enablement_state != "enabled" || facts.runtime_state != "stopped" {
                    step(
                        report,
                        2,
                        StepState::Uncertain,
                        "Enable was accepted but exact enabled-without-start state was not verified",
                    );
                    return Err(
                        "Service enablement was not freshly verified; inspect separately".into(),
                    );
                }
            }
        }
        if choices.start_service {
            step(
                report,
                3,
                StepState::Failed,
                "Start selected but not yet verified",
            );
            if facts.runtime_state == "running" && facts.bridge_state == "ready" {
                step(
                    report,
                    3,
                    StepState::Skipped,
                    "Managed service already running and bridge verified; no Start repeated",
                );
            } else {
                facts = self.action(
                    &mut native,
                    Action::Start,
                    ready_start,
                    3,
                    "Start service now",
                    report,
                )?;
            }
        }
        if facts.binary_state != "installed"
            || facts.unit_state != "installed"
            || facts.enablement_state != "enabled"
            || facts.runtime_state != "running"
            || facts.bridge_state != "ready"
        {
            step(
                report,
                3,
                StepState::Uncertain,
                "Exact installed, enabled, running and bridge-ready completion was not verified",
            );
            return Err(
                "Installation completion state was not freshly verified; inspect separately".into(),
            );
        }
        Ok(())
    }
    fn model_consent(&self, title: &str, effects: Vec<String>) -> Result<()> {
        self.shared.check()?;
        let id = next_id()?;
        let deadline = Instant::now() + self.limits.pending_timeout;
        self.shared.pending(Some(Pending {
            id,
            kind: Kind::Core,
        }));
        self.emit(Event::Confirmation {
            request_id: id,
            title: title.into(),
            effects,
        })?;
        match self.receive(Some(deadline))? {
            Some(Command::Approve { request_id }) if request_id == id => {}
            _ => {
                return Err(
                    "Model confirmation declined/expired; setup incomplete; no retry".into(),
                )
            }
        }
        self.shared.pending(None);
        self.shared.check()
    }
    fn models(
        &mut self,
        source: PathBuf,
        report: &mut Report,
        lifecycle_complete: bool,
    ) -> Result<()> {
        let index = report.steps.len();
        report.steps.push(StepResult {
            label: "Model setup".into(),
            state: StepState::Failed,
            detail: "Setup incomplete; inference not verified".into(),
        });
        self.emit(Event::Stage {
            message: "Model choice — observing the managed service".into(),
        })?;
        self.shared.check()?;
        let mut native = Native::new((self.backend_factory)(source)?);
        let facts = native.inspect()?;
        setup::usable(&facts)?;
        if facts.enablement_state != "enabled"
            || facts.runtime_state != "running"
            || facts.bridge_state != "ready"
        {
            return Err("Model setup incomplete: separately register, enable and start the Smith service; no implicit lifecycle action".into());
        }
        let config = native.configuration()?;
        let installed = native.models()?;
        let id = next_id()?;
        self.shared.pending(Some(Pending {
            id,
            kind: Kind::Model,
        }));
        self.emit(Event::Models {
            request_id: id,
            selected: config.model.clone(),
            installed: installed.iter().cloned().collect(),
        })?;
        let (name, remove) = match self.receive(None)? {
            Some(Command::ModelAction {
                request_id,
                model,
                remove,
            }) if request_id == id => (model, remove),
            _ => return Err("Model choice stopped; setup incomplete".into()),
        };
        self.shared.pending(None);
        let entry = crate::catalog::entry(&name)?;
        if remove {
            if report.scope == Scope::ModelCheck {
                report.scope = Scope::ModelRemoval;
            }
            if config.model.as_deref() == Some(&name) {
                return Err(
                    "Selected model cannot be removed; select another model explicitly first"
                        .into(),
                );
            }
            if !installed.contains(&name) {
                return Err("Removal requires an observed installed catalog model".into());
            }
            self.model_consent("Remove model (destructive)", vec![format!("Permanently remove only {name}"), format!("SHA256: {}\nConfiguration revision: {}", entry.sha256, config.revision), "Service-mediated deletion. No implicit runner stop; any loaded runner may block until normal idle expiry. No history/preference changes or rollback.".into()])?;
            step(
                report,
                index,
                StepState::Uncertain,
                "Removal accepted; inspect if observation stops",
            );
            native.model_operation("remove_model", &name, config.revision, &|| {
                self.shared.check().is_err()
            })?;
            step(
                report,
                index,
                StepState::Completed,
                "Exact model removal verified by the service; no inference requested",
            );
            report.complete = report.scope == Scope::ModelRemoval || lifecycle_complete;
            report.message = if report.scope == Scope::ModelRemoval {
                "Model removal complete; no artifact publication or service registration/enable/start performed by this management session"
            } else if lifecycle_complete {
                "Installation complete: exact artifacts are installed and the service is enabled, running and bridge-ready; model removal verified; inference not checked"
            } else {
                "Setup incomplete: model removal passed, but exact installed, enabled, running and bridge-ready lifecycle completion was not verified; prior effects remain"
            }.into();
            return Ok(());
        }
        if !installed.contains(&name) {
            self.model_consent("Download exact model", vec![format!("Download {name}: {} bytes", entry.size), format!("Source: {}\nSHA256: {}\nLicense: {}", entry.url, entry.sha256, entry.license), "HTTPS network and persistent disk effects; private partial staging may remain; no automatic retry or cleanup.".into()])?;
            self.emit(Event::Stage {
                message:
                    "Downloading model — actual byte counts; hashing/publication indeterminate"
                        .into(),
            })?;
            let mut provider = self.provider.as_mut().ok_or("Model provider unavailable")?()?;
            let mut progress = WorkerProgress {
                events: self.events.clone(),
                shared: self.shared.clone(),
                timeout: self.limits.event_timeout,
                generation: self
                    .shared
                    .latest_numeric
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .generation,
                history: None,
            };
            if let Err(error) = provider.pull(&name, &mut progress) {
                step(report, index, StepState::Failed, &bounded(&error));
                return Err(error);
            }
            self.shared.check()?;
            if !native.models()?.contains(&name) {
                return Err(
                    "Downloaded model not observed by the service; setup incomplete".into(),
                );
            }
        }
        let mut revision = config.revision;
        if config.model.as_deref() != Some(&name) {
            self.model_consent("Save checked model preference", vec![format!("Select {name}; voice remains inactive"), format!("Replace saved selection {:?} only at configuration revision {revision}; stale state is rejected", config.model)])?;
            revision = native.configure_checked(&name, revision)?;
        }
        self.model_consent("Check model through Smith service", vec![format!("Verify {name}\nSHA256: {}\nConfiguration revision: {revision}", entry.sha256), "Use Smith's Vulkan-first runner with CPU fallback to generate one fixed greeting, with model-default thinking; no saved chat turn. 180s service budget / 210s observation; performance is unmeasured and timeout/truncation is failure. Stopping observation may leave accepted service work running; no resend.".into()])?;
        self.emit(Event::Stage {
            message:
                "Checking model through Smith service — hashing/loading/generation indeterminate"
                    .into(),
        })?;
        step(
            report,
            index,
            StepState::Uncertain,
            "Service check accepted; setup incomplete until verified",
        );
        native.model_operation("verify_model", &name, revision, &|| {
            self.shared.check().is_err()
        })?;
        step(
            report,
            index,
            StepState::Completed,
            "Service-backed exact model inference verified; no chat turn saved",
        );
        report.complete = report.scope != Scope::Installation || lifecycle_complete;
        report.message = match report.scope {
            Scope::ModelCheck => "Model check complete: service-backed model inference verified; no artifact publication or service registration/enable/start performed by this management session",
            Scope::Installation if lifecycle_complete => "Installation complete: exact artifacts are installed and the service is enabled, running and bridge-ready; service-backed model check verified; app not launched",
            Scope::Installation => "Setup incomplete: model check passed, but exact installed, enabled, running and bridge-ready lifecycle completion was not verified; prior effects remain",
            Scope::ModelRemoval => unreachable!("Removal completed separately"),
            Scope::Upgrade | Scope::Uninstall => unreachable!("Maintenance never runs model setup"),
        }.into();
        Ok(())
    }
    fn action(
        &self,
        native: &mut Native,
        action: Action,
        prepared: Option<Observation>,
        index: usize,
        title: &str,
        report: &mut Report,
    ) -> Result<Observation> {
        self.shared.check()?;
        let prepared = match prepared {
            Some(value) => value,
            None => native.prepare(action)?,
        };
        let token = prepared
            .approval_token
            .as_deref()
            .ok_or("Core preparation supplied no original token")?;
        let deadline = token_deadline(token)?.min(Instant::now() + self.limits.pending_timeout);
        let id = next_id()?;
        self.shared.pending(Some(Pending {
            id,
            kind: Kind::Core,
        }));
        let effects = vec![
            checked_text(&prepared.effects.description)?,
            format!("Binary: {}", prepared.effects.binary_path.display()),
            format!("Unit: {}", prepared.effects.unit_path.display()),
            format!("State: {}", prepared.effects.state_path.display()),
            format!("Socket: {}", prepared.effects.socket_path.display()),
        ];
        self.emit(Event::Confirmation {
            request_id: id,
            title: title.into(),
            effects,
        })?;
        match self.receive(Some(deadline))? {
            Some(Command::Approve { request_id }) if request_id == id => {}
            Some(Command::Decline { request_id }) if request_id == id => {
                step(report, index, StepState::Skipped, "Declined; prior artifact/lifecycle effects remain");
                return Err(format!("{title} declined; prior effects remain"));
            }
            None => return Err("Core confirmation expired; original token discarded; no automatic reprepare or retry".into()),
            _ => return Err("Wrong or stale Core confirmation; no action approved".into()),
        }
        self.shared.pending(None);
        self.shared.check()?;
        step(
            report,
            index,
            StepState::Uncertain,
            "Exact Core action accepted; outcome requires fresh inspection",
        );
        // Token remains private and byte-identical; Core rebinds current facts and
        // rechecks its original expiry before effects. Never retry/reprepare here.
        native.apply(action, token)?;
        let facts = native.inspect()?;
        setup::usable(&facts)?;
        if action == Action::InstallUnit
            && (facts.unit_state != "installed" || facts.message == setup::RELOAD_MESSAGE)
        {
            return Err("Registration/reload outcome not verified; inspect separately".into());
        }
        step(
            report,
            index,
            StepState::Completed,
            "Selected Core effect applied and freshly inspected",
        );
        self.shared.check()?;
        Ok(facts)
    }
}

struct OwnedPrompt<'a, P, PF, BF, HF>
where
    P: Publisher,
    PF: FnMut(PathBuf, PathBuf) -> Result<P>,
    BF: FnMut(PathBuf) -> Result<Box<dyn Backend>>,
    HF: FnMut() -> Result<()>,
{
    engine: &'a Engine<P, PF, BF, HF>,
    report: &'a mut Report,
}
impl<P, PF, BF, HF> maintenance::Prompt for OwnedPrompt<'_, P, PF, BF, HF>
where
    P: Publisher,
    PF: FnMut(PathBuf, PathBuf) -> Result<P>,
    BF: FnMut(PathBuf) -> Result<Box<dyn Backend>>,
    HF: FnMut() -> Result<()>,
{
    fn confirm(&mut self, title: &str, effects: &[String], deadline: Instant) -> Result<bool> {
        self.engine.shared.check()?;
        // Exact consent is refused, never truncated or sanitized into a different review.
        if title.trim().is_empty()
            || title.len() > 256
            || title.contains(['\n', '\t'])
            || plain(title) != title
            || effects.is_empty()
            || effects.len() > 32
            || effects
                .iter()
                .any(|s| s.trim().is_empty() || plain(s) != *s)
            || effects
                .iter()
                .map(|s| s.len().saturating_add(2))
                .sum::<usize>()
                > TEXT_LIMIT
        {
            return Err("Exact maintenance effects cannot be safely displayed".into());
        }
        if Instant::now() >= deadline {
            return Err("Original maintenance consent expired; no reprepare".into());
        }
        let id = next_id()?;
        self.engine.shared.pending(Some(Pending {
            id,
            kind: Kind::Core,
        }));
        self.engine.emit(Event::Confirmation {
            request_id: id,
            title: title.into(),
            effects: effects.to_vec(),
        })?;
        let response = self.engine.receive(Some(deadline))?;
        self.engine.shared.pending(None);
        match response {
            Some(Command::Approve { request_id }) if request_id == id => {
                self.engine.shared.check()?;
                if Instant::now() >= deadline {
                    return Err("Original maintenance consent expired; no reprepare".into());
                }
                add_detail(
                    self.report,
                    &format!("Confirmed {title}; effects accepted, outcome pending"),
                );
                Ok(true)
            }
            Some(Command::Decline { request_id }) if request_id == id => Ok(false),
            None => Err("Original maintenance consent expired; no reprepare".into()),
            _ => Err("Mismatched maintenance response ID or phase".into()),
        }
    }
    fn progress(&mut self, message: &str) -> Result<()> {
        self.engine.shared.check()?;
        let message = checked_text(message)?;
        add_detail(self.report, &message);
        self.engine.emit(Event::Stage { message })
    }
}

fn receipt_expired(
    command: &Command,
    deadline: Option<Instant>,
    artifact_review: bool,
    now: Instant,
) -> bool {
    deadline.is_some_and(|d| now >= d)
        && !(artifact_review
            && matches!(
                command,
                Command::ManageModels | Command::Upgrade | Command::Uninstall
            ))
}

#[cfg(test)]
#[test]
fn management_receipt_does_not_inherit_artifact_consent_expiry() {
    let expired = Instant::now();
    let received = expired + Duration::from_millis(1);
    assert!(!receipt_expired(
        &Command::ManageModels,
        Some(expired),
        true,
        received
    ));
    for command in [Command::Upgrade, Command::Uninstall] {
        assert!(!receipt_expired(&command, Some(expired), true, received));
        assert!(receipt_expired(&command, Some(expired), false, received));
    }
    assert!(receipt_expired(
        &Command::ManageModels,
        Some(expired),
        false,
        received
    ));
    assert!(receipt_expired(
        &Command::Install {
            request_id: 1,
            choices: Choices::default()
        },
        Some(expired),
        true,
        received
    ));
    assert!(receipt_expired(
        &Command::Approve { request_id: 1 },
        Some(expired),
        true,
        received
    ));
}

fn next_id() -> Result<u64> {
    NEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| "Wizard request ID space exhausted".into())
}
fn token_deadline(token: &str) -> Result<Instant> {
    let issued: u64 = token
        .split_once('.')
        .ok_or("Invalid original Core token")?
        .0
        .parse()
        .map_err(|_| "Invalid original Core time")?;
    let now = SystemTime::now();
    let issued = UNIX_EPOCH
        .checked_add(Duration::from_secs(issued))
        .ok_or("Invalid Core time")?;
    if issued > now {
        return Err("Core token is from the future".into());
    }
    let remaining = issued
        .checked_add(Duration::from_secs(60))
        .ok_or("Invalid Core expiry")?
        .duration_since(now)
        .map_err(|_| "Original Core token expired; no automatic reprepare")?;
    if remaining.is_zero() {
        return Err("Original Core token expired".into());
    }
    Ok(Instant::now() + remaining)
}
fn initial_report() -> Report {
    Report {
        scope: Scope::Installation,
        complete: false,
        cancelled: false,
        message: "No installation effects verified".into(),
        details: Vec::new(),
        steps: [
            "Artifacts",
            "Service registration",
            "Service enablement",
            "Service start",
        ]
        .iter()
        .map(|label| StepResult {
            label: (*label).into(),
            state: StepState::Skipped,
            detail: "Not selected or not yet accepted; no effect applied by this step".into(),
        })
        .collect(),
    }
}
fn step(report: &mut Report, index: usize, state: StepState, detail: &str) {
    report.steps[index].state = state;
    report.steps[index].detail = bounded(detail);
}
fn checked_text(text: &str) -> Result<String> {
    if text.trim().is_empty() || text.len() > TEXT_LIMIT {
        return Err("Wizard event text is empty or exceeds bound".into());
    }
    Ok(plain(text))
}
fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| {
            (!c.is_control() || matches!(c, '\n' | '\t'))
                && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect()
}
fn bounded(text: &str) -> String {
    let mut end = text.len().min(4096);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = plain(&text[..end]);
    if end < text.len() {
        result.push_str(" [details truncated]");
    }
    result
}
fn add_detail(report: &mut Report, detail: &str) {
    let detail = bounded(detail);
    if report.details.len() < DETAIL_COUNT
        && report.details.iter().map(String::len).sum::<usize>() + detail.len() <= DETAIL_LIMIT
    {
        report.details.push(detail);
    }
}
fn add_output(report: &mut Report, bytes: &[u8]) {
    for line in String::from_utf8_lossy(bytes).lines() {
        add_detail(report, line);
    }
    if bytes.len() == OUTPUT_LIMIT {
        add_detail(report, "Publisher output reached the bounded memory limit; details may be incomplete; no rollback inferred.");
    }
}
struct WorkerProgress {
    events: SyncSender<Event>,
    shared: Arc<Shared>,
    timeout: Duration,
    generation: u64,
    history: Option<(u64, u64)>,
}
impl crate::model::ProgressOutput for WorkerProgress {
    fn numeric_until(
        &mut self,
        downloaded: u64,
        total: u64,
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        if total == 0
            || downloaded > total
            || self
                .history
                .is_some_and(|(previous, expected)| total != expected || downloaded < previous)
        {
            return Err("Invalid numeric model progress".into());
        }
        self.shared.check()?;
        if cancelled() || Instant::now() >= deadline {
            return Err(
                "Numeric model progress cancelled or expired; partial effects preserved".into(),
            );
        }
        match self.shared.latest_numeric.try_lock() {
            Ok(mut numeric) => {
                self.shared.check()?;
                if cancelled() || Instant::now() >= deadline {
                    return Err(
                        "Numeric model progress cancelled or expired; partial effects preserved"
                            .into(),
                    );
                }
                if numeric.generation != self.generation {
                    return Err("Numeric model progress belongs to an inactive download".into());
                }
                numeric.latest = Some((downloaded, total));
                self.history = Some((downloaded, total));
                Ok(())
            }
            // Numeric observations are supersedable. The UI may briefly own the
            // one-slot snapshot while draining it; never wait behind that lock.
            Err(std::sync::TryLockError::WouldBlock) => {
                self.history = Some((downloaded, total));
                Ok(())
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                Err("Numeric progress storage unavailable".into())
            }
        }
    }
    fn write_until(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        if bytes.len() > 4096 {
            return Err("Model progress bound exceeded".into());
        }
        let event = Event::Stage {
            message: std::str::from_utf8(bytes)
                .map_err(|_| "Invalid progress text")?
                .trim()
                .into(),
        };
        let deadline = deadline.min(Instant::now() + self.timeout);
        self.shared.emit(&self.events, event, deadline, cancelled)
    }
}
#[derive(Default)]
struct CappedOutput {
    bytes: Vec<u8>,
    overflowed: bool,
}
impl Write for CappedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_LIMIT.saturating_sub(self.bytes.len()) {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "Wizard publisher output cap reached; partial effects preserved",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::model::ProgressOutput;

    fn shared() -> Arc<Shared> {
        Arc::new(Shared {
            latest_numeric: Mutex::new(Numeric::default()),
            gate: Mutex::new(Gate::default()),
            stop: AtomicU8::new(0),
            cancellation: Cancellation::from_callbacks(|| false, || {}),
            report: Mutex::new(None),
        })
    }

    fn output(shared: Arc<Shared>, events: SyncSender<Event>) -> WorkerProgress {
        WorkerProgress {
            events,
            shared,
            timeout: Duration::from_millis(20),
            generation: 0,
            history: None,
        }
    }

    #[test]
    fn numeric_burst_does_not_use_critical_queue_and_keeps_only_latest() {
        let (events, receiver) = mpsc::sync_channel(1);
        let shared = shared();
        let mut output = output(shared.clone(), events);
        let deadline = Instant::now() + Duration::from_secs(4);
        for downloaded in 0..10_000 {
            output
                .numeric_until(downloaded, 10_000, deadline, || false)
                .expect("numeric telemetry must not fill the critical FIFO");
        }
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
        assert_eq!(
            shared.latest_numeric.lock().unwrap().latest,
            Some((9_999, 10_000))
        );
    }

    #[test]
    fn numeric_validation_is_monotonic_stable_and_bounded() {
        let (events, _) = mpsc::sync_channel(1);
        let mut output = output(shared(), events);
        let deadline = Instant::now() + Duration::from_secs(1);
        output.numeric_until(4, 10, deadline, || false).unwrap();
        assert!(output.numeric_until(3, 10, deadline, || false).is_err());
        assert!(output.numeric_until(5, 11, deadline, || false).is_err());
        assert!(output.numeric_until(11, 10, deadline, || false).is_err());
        assert!(output.numeric_until(0, 0, deadline, || false).is_err());
    }

    #[test]
    fn numeric_lock_contention_drops_without_waiting_but_retains_history() {
        let (events, _) = mpsc::sync_channel(1);
        let shared = shared();
        let held = shared.latest_numeric.lock().unwrap();
        let mut output = output(shared.clone(), events);
        let start = Instant::now();
        output
            .numeric_until(4, 10, start + Duration::from_secs(1), || false)
            .unwrap();
        assert!(start.elapsed() < Duration::from_millis(100));
        drop(held);
        assert!(output
            .numeric_until(3, 10, Instant::now() + Duration::from_secs(1), || false)
            .is_err());
        assert_eq!(shared.latest_numeric.lock().unwrap().latest, None);
    }

    #[test]
    fn critical_boundary_fences_stale_numeric_and_remains_strict() {
        let (events, receiver) = mpsc::sync_channel(1);
        let shared = shared();
        shared.latest_numeric.lock().unwrap().latest = Some((4, 10));
        shared
            .emit(
                &events,
                Event::Stage {
                    message: "boundary".into(),
                },
                Instant::now() + Duration::from_secs(1),
                || false,
            )
            .unwrap();
        assert_eq!(shared.latest_numeric.lock().unwrap().latest, None);
        assert!(matches!(receiver.try_recv(), Ok(Event::Stage { .. })));

        events
            .try_send(Event::Stage {
                message: "full".into(),
            })
            .unwrap();
        let error = shared
            .emit(
                &events,
                Event::Models {
                    request_id: 1,
                    selected: None,
                    installed: Vec::new(),
                },
                Instant::now() + Duration::from_millis(20),
                || false,
            )
            .unwrap_err();
        assert!(error.contains("backpressured"));
        assert!(shared.reason() == Stop::Backpressure);
    }

    #[test]
    fn numeric_preserves_cancellation_deadline_generation_and_disconnect() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let (events, _) = mpsc::sync_channel(1);
        let mut cancelled = output(shared(), events);
        assert!(cancelled.numeric_until(1, 2, deadline, || true).is_err());

        let (events, _) = mpsc::sync_channel(1);
        let mut expired = output(shared(), events);
        assert!(expired
            .numeric_until(1, 2, Instant::now(), || false)
            .is_err());

        let (events, _) = mpsc::sync_channel(1);
        let stale_shared = shared();
        stale_shared.latest_numeric.lock().unwrap().clear();
        let mut stale = output(stale_shared, events);
        assert!(stale.numeric_until(1, 2, deadline, || false).is_err());

        let (events, receiver) = mpsc::sync_channel(1);
        let disconnected_shared = shared();
        drop(receiver);
        assert!(disconnected_shared
            .emit(
                &events,
                Event::Stage {
                    message: "lost".into()
                },
                deadline,
                || false,
            )
            .is_err());
        assert!(disconnected_shared.reason() == Stop::Disconnected);
    }
}
