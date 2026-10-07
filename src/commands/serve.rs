//! Loopback chat bridge for native clients (Flutter later).
//!
//! Smith owns the turn (same session as `smith chat`). The client owns UI / STT / TTS.
//! Bind is localhost only — not a public API.

use crate::brain::pipeline_jobs;
use crate::brain::reset_chat_session;
use crate::brain::task_board::{self, TaskRecord};
use crate::commands::chat::{ensure_ready, run_chat_turn, ChatMode, ChatTurn};
use crate::commands::tts::TtsGuard;
use crate::commands::whisper::WhisperGuard;
use crate::service::{self, ServiceMode};
use crate::{current_network, current_runtime, RuntimeMode};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::Notify;

/// Default bridge port (avoid Ollama 11434 / OpenCode 4096).
pub const BRIDGE_DEFAULT_PORT: u16 = 4783;
/// `sysexits.h` EX_CONFIG: deterministic operator-action-required startup failure.
pub const CONFIGURATION_EXIT_CODE: i32 = 78;

const BRIDGE_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const BRIDGE_IDENTITY: &str = "smith-bridge";
const DEFAULT_SESSION: &str = "default";

type ChatFn = Arc<dyn Fn(&str, ChatMode) -> Result<ChatTurn, String> + Send + Sync>;

struct SidecarHub {
    whisper: WhisperGuard,
    tts: TtsGuard,
}

#[derive(Clone)]
struct BridgeState {
    chat: ChatFn,
    echo: bool,
    sidecars: Arc<Mutex<SidecarHub>>,
    shutdown: Arc<Notify>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub v: Option<u32>,
    pub text: String,
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub new: bool,
    /// Deprecated; ignored (kept for older clients).
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ChatAttachmentDto {
    pub kind: String,
    pub path: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PipelineStatusDto {
    pub job_id: String,
    pub kind: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskDto {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub stage: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ack_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expected_result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(default)]
    pub session: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<&TaskRecord> for TaskDto {
    fn from(t: &TaskRecord) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            kind: t.kind.as_str().to_string(),
            stage: t.stage.clone(),
            status: t.status_str().to_string(),
            user_text: t.user_text.clone(),
            ack_text: t.ack_text.clone(),
            result_text: t.result_text.clone(),
            expected_result: t.expected_result.clone(),
            job_id: t.job_id.clone(),
            session: t.session.clone(),
            created_at: t.created_at,
            updated_at: t.updated_at,
            error: t.error.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatResponse {
    pub v: u32,
    pub text: String,
    pub session: String,
    #[serde(default)]
    pub isolation: String,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub network: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachmentDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<PipelineStatusDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskDto>,
}

#[derive(Debug, Deserialize)]
struct TasksQuery {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    since: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TasksListResponse {
    pub v: u32,
    pub tasks: Vec<TaskDto>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    pub echo: bool,
    #[serde(default)]
    pub isolation: String,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub network: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}

fn validate_chat_request(req: &ChatRequest) -> Result<(), String> {
    if req.v.is_some_and(|v| v != 1) {
        return Err("unsupported protocol version (expected v=1)".to_string());
    }
    if req.text.trim().is_empty() {
        return Err("text is required".to_string());
    }
    if let Some(ref session) = req.session {
        if session != DEFAULT_SESSION {
            return Err(format!(
                "unsupported session '{}'; v1 only supports '{}'",
                session, DEFAULT_SESSION
            ));
        }
    }
    Ok(())
}

fn echo_chat(text: &str, _mode: ChatMode) -> Result<ChatTurn, String> {
    Ok(ChatTurn {
        text: text.to_string(),
        attachments: Vec::new(),
        pipeline: None,
        task: None,
    })
}

fn pipeline_dto(meta: &pipeline_jobs::PipelineJobMeta) -> PipelineStatusDto {
    PipelineStatusDto {
        job_id: meta.id.clone(),
        kind: meta.kind.clone(),
        state: match meta.state {
            pipeline_jobs::JobState::Queued => "queued".into(),
            pipeline_jobs::JobState::Running => "running".into(),
            pipeline_jobs::JobState::Succeeded => "succeeded".into(),
            pipeline_jobs::JobState::Failed => "failed".into(),
            pipeline_jobs::JobState::AwaitingSignoff => "awaiting_signoff".into(),
            pipeline_jobs::JobState::Interrupted => "interrupted".into(),
        },
        summary: meta.snapshot_summary(),
        plan: meta.plan.clone(),
        project: meta.project.clone(),
        branch: meta.branch.clone(),
    }
}

fn runtime_label() -> String {
    current_runtime().as_str().to_string()
}

fn network_label() -> String {
    current_network().as_str().to_string()
}

/// Deprecated compat field derived from runtime.
fn isolation_label() -> String {
    match current_runtime() {
        RuntimeMode::Host => "host".to_string(),
        RuntimeMode::Container => "sealed".to_string(),
    }
}

fn bridge_router(state: BridgeState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/chat", post(chat))
        .route("/v1/tasks", get(list_tasks))
        .route("/v1/tasks/{id}", get(get_task))
        .route("/v1/services", get(list_services).patch(patch_services))
        .route("/v1/services/apply", post(apply_services))
        .route("/v1/sidecars/ensure", post(ensure_sidecars))
        .route("/v1/shutdown", post(shutdown_bridge))
        .route("/v1/open", post(open_file))
        .with_state(state)
}

fn stop_sidecars(hub: &Arc<Mutex<SidecarHub>>) {
    let mut guard = match hub.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.whisper.stop();
    guard.tts.stop();
}

fn ensure_voice_sidecars_locked(hub: &Arc<Mutex<SidecarHub>>) {
    let need_whisper = !port_listening(4790);
    let need_tts = !port_listening(4791);
    let whisper = if need_whisper {
        Some(crate::commands::whisper::ensure_whisper_sidecar())
    } else {
        None
    };
    let tts = if need_tts {
        Some(crate::commands::tts::ensure_tts_sidecar())
    } else {
        None
    };
    let mut guard = match hub.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(w) = whisper {
        guard.whisper = w;
    }
    if let Some(t) = tts {
        guard.tts = t;
    }
}

async fn health(State(state): State<BridgeState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: BRIDGE_IDENTITY.to_string(),
        echo: state.echo,
        isolation: isolation_label(),
        runtime: runtime_label(),
        network: network_label(),
    })
}

async fn chat(
    State(state): State<BridgeState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<ErrorBody>)> {
    if let Err(e) = validate_chat_request(&req) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorBody { error: e })));
    }
    if req.new && !state.echo {
        if let Err(e) = reset_chat_session() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: format!("failed to reset session: {e}"),
                }),
            ));
        }
    }
    let text = req.text.trim().to_string();
    let mode = ChatMode::parse(req.mode.as_deref().unwrap_or("chat"));
    let chat = state.chat.clone();
    let turn = tokio::task::spawn_blocking(move || chat(&text, mode))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: format!("chat worker failed: {e}"),
                }),
            )
        })?
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(ErrorBody { error: e })))?;

    Ok(Json(ChatResponse {
        v: 1,
        text: turn.text,
        session: DEFAULT_SESSION.to_string(),
        isolation: isolation_label(),
        runtime: runtime_label(),
        network: network_label(),
        mode: mode.as_str().to_string(),
        attachments: turn
            .attachments
            .into_iter()
            .map(|a| ChatAttachmentDto {
                kind: a.kind,
                path: a.path,
                name: a.name,
            })
            .collect(),
        pipeline: turn.pipeline.as_ref().map(pipeline_dto),
        task: turn.task.as_ref().map(TaskDto::from),
    }))
}

async fn list_tasks(
    Query(q): Query<TasksQuery>,
) -> Result<Json<TasksListResponse>, (StatusCode, Json<ErrorBody>)> {
    let active_only = matches!(
        q.active.as_deref().map(str::trim),
        Some("1") | Some("true") | Some("yes")
    );
    let tasks = tokio::task::spawn_blocking(move || task_board::list_tasks(active_only, q.since))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: format!("tasks worker failed: {e}"),
                }),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody { error: e }),
            )
        })?;
    Ok(Json(TasksListResponse {
        v: 1,
        tasks: tasks.iter().map(TaskDto::from).collect(),
    }))
}

async fn get_task(Path(id): Path<String>) -> Result<Json<TaskDto>, (StatusCode, Json<ErrorBody>)> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "task id required".into(),
            }),
        ));
    }
    let task = tokio::task::spawn_blocking(move || task_board::get_task(&id))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: format!("task worker failed: {e}"),
                }),
            )
        })?
        .map_err(|e| (StatusCode::NOT_FOUND, Json(ErrorBody { error: e })))?;
    Ok(Json(TaskDto::from(&task)))
}

#[derive(Debug, Deserialize)]
struct ServicesPatchRequest {
    #[serde(default)]
    autostart: Option<bool>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    mode: Option<String>,
}

async fn list_services() -> Result<Json<service::ServicesSnapshot>, (StatusCode, Json<ErrorBody>)> {
    tokio::task::spawn_blocking(service::collect_status)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: format!("services worker failed: {e}"),
                }),
            )
        })?
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody { error: e }),
            )
        })
}

async fn patch_services(
    State(state): State<BridgeState>,
    Json(req): Json<ServicesPatchRequest>,
) -> Result<Json<service::ServicesSnapshot>, (StatusCode, Json<ErrorBody>)> {
    let sidecars = state.sidecars.clone();
    tokio::task::spawn_blocking(move || {
        if req.id.is_some() != req.mode.is_some() {
            return Err(
                "PATCH /v1/services requires both `id` and `mode` when changing a service".into(),
            );
        }
        if let Some(on) = req.autostart {
            service::set_autostart(on)?;
        }
        let mut voice_warm = false;
        if let (Some(id), Some(mode)) = (req.id.as_deref(), req.mode.as_deref()) {
            let mode = ServiceMode::parse(mode)?;
            service::set_service_mode(id, mode)?;
            let canonical = service::resolve_service_id(id)?;
            if mode == ServiceMode::Warm
                && (canonical == service::ID_SERVICES_WHISPER
                    || canonical == service::ID_SERVICES_TTS)
            {
                voice_warm = true;
            }
        }
        if voice_warm {
            ensure_voice_sidecars_locked(&sidecars);
        }
        service::collect_status()
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: format!("services worker failed: {e}"),
            }),
        )
    })?
    .map(Json)
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorBody { error: e })))
}

async fn apply_services(
    State(state): State<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let sidecars = state.sidecars.clone();
    let notes = tokio::task::spawn_blocking(move || {
        let mut notes = service::apply_warm_set()?;
        let svc = service::load_service_config().unwrap_or_default();
        if svc.is_warm(service::ID_SERVICES_WHISPER) || svc.is_warm(service::ID_SERVICES_TTS) {
            ensure_voice_sidecars_locked(&sidecars);
            if svc.is_warm(service::ID_SERVICES_WHISPER) && port_listening(4790) {
                notes.push("services.whisper: ensured".into());
            }
            if svc.is_warm(service::ID_SERVICES_TTS) && port_listening(4791) {
                notes.push("services.tts: ensured".into());
            }
        }
        Ok::<_, String>(notes)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: format!("apply worker failed: {e}"),
            }),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { error: e }),
        )
    })?;
    Ok(Json(serde_json::json!({ "ok": true, "notes": notes })))
}

async fn ensure_sidecars(
    State(state): State<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let sidecars = state.sidecars.clone();
    tokio::task::spawn_blocking(move || {
        ensure_voice_sidecars_locked(&sidecars);
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: format!("sidecar worker failed: {e}"),
            }),
        )
    })?
    .map_err(|e| (StatusCode::BAD_GATEWAY, Json(ErrorBody { error: e })))?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "whisper": port_listening(4790),
        "tts": port_listening(4791),
    })))
}

async fn shutdown_bridge(State(state): State<BridgeState>) -> Json<serde_json::Value> {
    state.shutdown.notify_one();
    Json(serde_json::json!({ "ok": true }))
}

fn port_listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(300),
    )
    .is_ok()
}

enum BridgePortClaim {
    Reserved(TcpListener),
    ExistingSmith,
}

async fn verify_smith_identity(addr: SocketAddr) -> Result<(), String> {
    let health_url = format!("http://{addr}/v1/health");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| format!("build Smith identity client: {e}"))?;
    let response = client
        .get(&health_url)
        .send()
        .await
        .map_err(|e| format!("GET {health_url} failed: {e}"))?;
    if response.status() != StatusCode::OK {
        return Err(format!(
            "GET {health_url} returned HTTP {} instead of 200",
            response.status()
        ));
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GET {health_url} returned an invalid Smith health body: {e}"))?;
    if body
        .get("profile")
        .is_some_and(|profile| profile != "orchestration")
    {
        return Err("existing Smith bridge has a different process profile".into());
    }
    let health: HealthResponse = serde_json::from_value(body)
        .map_err(|e| format!("GET {health_url} returned an invalid Smith health body: {e}"))?;
    if !health.ok || health.service != BRIDGE_IDENTITY {
        return Err(format!(
            "GET {health_url} did not return the healthy `{BRIDGE_IDENTITY}` identity"
        ));
    }
    Ok(())
}

async fn claim_bridge_port(addr: SocketAddr) -> Result<BridgePortClaim, String> {
    match TcpListener::bind(addr).await {
        Ok(listener) => Ok(BridgePortClaim::Reserved(listener)),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            verify_smith_identity(addr).await.map_err(|identity_error| {
                format!(
                    "cannot start Smith: {addr} is occupied by a non-Smith listener ({identity_error}). Stop that listener or choose another loopback port"
                )
            })?;
            Ok(BridgePortClaim::ExistingSmith)
        }
        Err(e) => Err(format!("failed to reserve Smith bridge on {addr}: {e}")),
    }
}

pub(super) async fn wait_for_shutdown(shutdown: Arc<Notify>) {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                    _ = shutdown.notified() => {}
                }
            }
            Err(e) => {
                eprintln!("Warning: failed to register SIGTERM handler: {e}");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = shutdown.notified() => {}
                }
            }
        }
    }

    #[cfg(not(unix))]
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = shutdown.notified() => {}
    }
}

#[derive(Debug, Deserialize)]
struct OpenRequest {
    path: String,
}

async fn open_file(
    Json(req): Json<OpenRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    if current_runtime() != RuntimeMode::Host {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorBody {
                error: "open is host-runtime only (`smith settings runtime set host`)".into(),
            }),
        ));
    }
    let path = std::path::PathBuf::from(req.path.trim());
    tokio::task::spawn_blocking(move || crate::brain::files::open_path(&path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: format!("open worker failed: {e}"),
                }),
            )
        })?
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorBody { error: e })))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupStep {
    ReconcileTasks,
    ReconcileJobs,
    HostPreflight,
    WarmServices,
    VoiceSidecars,
    BrainReadiness,
    BridgeReadiness,
}

impl StartupStep {
    fn label(self) -> &'static str {
        match self {
            Self::ReconcileTasks => "task reconciliation",
            Self::ReconcileJobs => "pipeline-job reconciliation",
            Self::HostPreflight => "host-runtime preflight",
            Self::WarmServices => "warm-service application",
            Self::VoiceSidecars => "voice-sidecar startup",
            Self::BrainReadiness => "brain readiness",
            Self::BridgeReadiness => "bridge readiness",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartupStepError {
    PermanentConfiguration(String),
    Retryable(String),
}

impl From<crate::brain::host_runtime::HostPreflightError> for StartupStepError {
    fn from(error: crate::brain::host_runtime::HostPreflightError) -> Self {
        match error {
            crate::brain::host_runtime::HostPreflightError::PermanentConfiguration(message) => {
                Self::PermanentConfiguration(message)
            }
            crate::brain::host_runtime::HostPreflightError::Transient(message) => {
                Self::Retryable(message)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartupFailure {
    step: StartupStep,
    message: String,
    exit_code: i32,
}

fn run_startup_sequence(
    echo: bool,
    runtime: RuntimeMode,
    mut run: impl FnMut(StartupStep) -> Result<(), StartupStepError>,
) -> Result<(), StartupFailure> {
    for step in [
        StartupStep::ReconcileTasks,
        StartupStep::ReconcileJobs,
        StartupStep::HostPreflight,
        StartupStep::WarmServices,
        StartupStep::VoiceSidecars,
        StartupStep::BrainReadiness,
        StartupStep::BridgeReadiness,
    ] {
        let should_run = match step {
            StartupStep::HostPreflight => !echo && runtime.is_host(),
            StartupStep::WarmServices
            | StartupStep::VoiceSidecars
            | StartupStep::BrainReadiness => !echo,
            StartupStep::ReconcileTasks
            | StartupStep::ReconcileJobs
            | StartupStep::BridgeReadiness => true,
        };
        if !should_run {
            continue;
        }
        if let Err(error) = run(step) {
            let (message, exit_code) = match error {
                StartupStepError::PermanentConfiguration(message) => {
                    (message, CONFIGURATION_EXIT_CODE)
                }
                StartupStepError::Retryable(message) => (message, 1),
            };
            return Err(StartupFailure {
                step,
                message,
                exit_code,
            });
        }
    }
    Ok(())
}

pub async fn handle_serve(port: u16, echo: bool) {
    let addr = SocketAddr::new(BRIDGE_HOST, port);
    let listener = match claim_bridge_port(addr).await {
        Ok(BridgePortClaim::ExistingSmith) => {
            println!("smith bridge is already running on http://{addr}");
            println!("  supervisor is active in the background.");
            println!("  run `smith settings service status` to view services, or `smith settings service stop` to stop.");
            return;
        }
        Ok(BridgePortClaim::Reserved(listener)) => listener,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let runtime = current_runtime();
    let mut reconciled_tasks = 0;
    let mut reconciled_jobs = 0;
    let mut started_sidecars = None;
    let mut ready_model = None;
    let startup = run_startup_sequence(echo, runtime, |step| match step {
        StartupStep::ReconcileTasks => {
            reconciled_tasks = task_board::reconcile_interrupted().map_err(|error| {
                StartupStepError::Retryable(format!(
                    "failed to reconcile persisted tasks before readiness: {error}"
                ))
            })?;
            Ok(())
        }
        StartupStep::ReconcileJobs => {
            reconciled_jobs = pipeline_jobs::reconcile_interrupted().map_err(|error| {
                StartupStepError::Retryable(format!(
                    "failed to reconcile persisted pipeline jobs before readiness: {error}"
                ))
            })?;
            Ok(())
        }
        StartupStep::HostPreflight => {
            crate::brain::host_runtime::preflight_host_brain().map_err(StartupStepError::from)
        }
        StartupStep::WarmServices => {
            match crate::service::apply_warm_set() {
                Ok(notes) => {
                    for note in notes {
                        println!("  {note}");
                    }
                }
                Err(error) => eprintln!("  Warning: warm apply: {error}"),
            }
            Ok(())
        }
        StartupStep::VoiceSidecars => {
            let whisper = if crate::service::whisper_is_warm() {
                crate::commands::whisper::ensure_whisper_sidecar()
            } else {
                println!("  services.whisper: on_demand (POST /v1/sidecars/ensure)");
                WhisperGuard::none()
            };
            let tts = if crate::service::tts_is_warm() {
                crate::commands::tts::ensure_tts_sidecar()
            } else {
                println!("  services.tts: on_demand (POST /v1/sidecars/ensure)");
                TtsGuard::none()
            };
            started_sidecars = Some(SidecarHub { whisper, tts });
            Ok(())
        }
        StartupStep::BrainReadiness => {
            let (_memory, model) = ensure_ready().map_err(StartupStepError::Retryable)?;
            ready_model = Some(model);
            Ok(())
        }
        StartupStep::BridgeReadiness => Ok(()),
    });

    if let Err(failure) = startup {
        if let Some(mut sidecars) = started_sidecars {
            sidecars.whisper.stop();
            sidecars.tts.stop();
        }
        if failure.exit_code == CONFIGURATION_EXIT_CODE {
            eprintln!(
                "Error: Smith host-runtime configuration is not ready: {}",
                failure.message
            );
            eprintln!(
                "  automatic service restart is suppressed for this configuration failure; correct it, then explicitly start Smith"
            );
        } else {
            eprintln!(
                "Error: {} failed: {}",
                failure.step.label(),
                failure.message
            );
        }
        std::process::exit(failure.exit_code);
    }

    if reconciled_tasks > 0 || reconciled_jobs > 0 {
        println!(
            "  recovery: marked {reconciled_tasks} task(s) and {reconciled_jobs} pipeline job(s) interrupted"
        );
    }

    // Echo mode is a lightweight client/bridge integration check. It must not
    // provision the default model or start configured warm sidecars.
    let sidecar_hub = started_sidecars.unwrap_or_else(|| {
        println!("  services.whisper: on_demand (POST /v1/sidecars/ensure)");
        println!("  services.tts: on_demand (POST /v1/sidecars/ensure)");
        SidecarHub {
            whisper: WhisperGuard::none(),
            tts: TtsGuard::none(),
        }
    });
    let sidecars = Arc::new(Mutex::new(sidecar_hub));
    let shutdown = Arc::new(Notify::new());

    let chat: ChatFn = if echo {
        Arc::new(echo_chat)
    } else {
        let model = ready_model.expect("startup sequence must establish brain readiness");
        Arc::new(move |text: &str, mode: ChatMode| run_chat_turn(text, &model, mode))
    };

    println!("smith bridge listening on http://{addr}");
    println!(
        "  runtime: {}  |  network: {}",
        runtime_label(),
        network_label()
    );
    if echo {
        println!("  mode: echo (no brain or warm services — client integration)");
    } else {
        println!("  mode: chat (same session as `smith chat`)");
        if current_runtime().is_host() {
            println!("  control plane: native  |  coding/pipeline/search: Docker");
        }
    }
    println!("  GET  /v1/health");
    println!(
        "  POST /v1/chat   {{\"v\":1,\"text\":\"...\",\"session\":\"default\",\"new\":false}}"
    );
    println!("  GET  /v1/tasks  ?active=1&since=<unix>");
    println!("  GET  /v1/tasks/:id");
    println!("  GET  /v1/services");
    println!("  PATCH /v1/services");
    println!("  POST /v1/services/apply");
    println!("  POST /v1/sidecars/ensure");
    println!("  POST /v1/shutdown");
    println!("  POST /v1/open   {{\"path\":\"...\"}}  (host runtime)");
    println!(
        "  loopback only. Ctrl-C, SIGTERM, or POST /v1/shutdown to stop (also stops Whisper/TTS if smith started them)."
    );

    let app = bridge_router(BridgeState {
        chat,
        echo,
        sidecars: sidecars.clone(),
        shutdown: shutdown.clone(),
    });
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(wait_for_shutdown(shutdown))
        .await;

    stop_sidecars(&sidecars);
    if let Err(e) = result {
        eprintln!("Error: bridge server failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode as HttpStatus;

    fn echo_state() -> BridgeState {
        BridgeState {
            chat: Arc::new(echo_chat),
            echo: true,
            sidecars: Arc::new(Mutex::new(SidecarHub {
                whisper: WhisperGuard::none(),
                tts: TtsGuard::none(),
            })),
            shutdown: Arc::new(Notify::new()),
        }
    }

    async fn spawn_bridge(state: BridgeState) -> SocketAddr {
        let listener = TcpListener::bind(SocketAddr::new(BRIDGE_HOST, 0))
            .await
            .expect("bind test port");
        let addr = listener.local_addr().expect("test address");
        let app = bridge_router(state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        addr
    }

    #[test]
    fn startup_sequence_stops_after_classified_host_preflight_failure() {
        assert_eq!(CONFIGURATION_EXIT_CODE, 78);
        for (probe_error, expected_exit) in [
            (
                StartupStepError::PermanentConfiguration("invalid OpenCode identity".into()),
                CONFIGURATION_EXIT_CODE,
            ),
            (
                StartupStepError::Retryable("OpenCode probe timed out".into()),
                1,
            ),
        ] {
            let mut effects = Vec::new();
            let error = run_startup_sequence(false, RuntimeMode::Host, |step| {
                effects.push(step);
                if step == StartupStep::HostPreflight {
                    Err(probe_error.clone())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(
                effects,
                vec![
                    StartupStep::ReconcileTasks,
                    StartupStep::ReconcileJobs,
                    StartupStep::HostPreflight
                ]
            );
            assert_eq!(error.step, StartupStep::HostPreflight);
            assert_eq!(error.exit_code, expected_exit);
            assert!(!effects.contains(&StartupStep::WarmServices));
            assert!(!effects.contains(&StartupStep::VoiceSidecars));
            assert!(!effects.contains(&StartupStep::BrainReadiness));
            assert!(!effects.contains(&StartupStep::BridgeReadiness));
        }
    }

    #[test]
    fn startup_sequence_skips_host_and_warm_effects_for_echo() {
        let effects = std::cell::RefCell::new(Vec::new());
        run_startup_sequence(true, RuntimeMode::Host, |step| {
            effects.borrow_mut().push(step);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            effects.into_inner(),
            vec![
                StartupStep::ReconcileTasks,
                StartupStep::ReconcileJobs,
                StartupStep::BridgeReadiness
            ]
        );
    }

    #[test]
    fn validate_rejects_empty_text_and_bad_session() {
        assert!(validate_chat_request(&ChatRequest {
            v: Some(1),
            text: "  ".into(),
            session: None,
            new: false,
            mode: None,
        })
        .is_err());
        assert!(validate_chat_request(&ChatRequest {
            v: Some(1),
            text: "hi".into(),
            session: Some("other".into()),
            new: false,
            mode: None,
        })
        .is_err());
        assert!(validate_chat_request(&ChatRequest {
            v: Some(2),
            text: "hi".into(),
            session: None,
            new: false,
            mode: None,
        })
        .is_err());
        assert!(validate_chat_request(&ChatRequest {
            v: Some(1),
            text: "hi".into(),
            session: Some(DEFAULT_SESSION.into()),
            new: false,
            mode: None,
        })
        .is_ok());
    }

    #[tokio::test]
    async fn legacy_router_rejects_chat_only_route_before_chat_handler() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let capture = calls.clone();
        // Echo state keeps this negative-route fixture safe even if routing regresses:
        // it cannot reset real legacy memory or dispatch orchestration work.
        let mut state = echo_state();
        state.chat = Arc::new(move |text, mode| {
            capture.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            echo_chat(text, mode)
        });
        let app = bridge_router(state);
        let listener = TcpListener::bind(SocketAddr::new(BRIDGE_HOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for new in [false, true] {
            let response = client
                .post(format!("http://{addr}/v1/chat-only"))
                .json(&serde_json::json!({
                    "v":1, "text":"yes, run it", "session":"default",
                    "new":new, "mode":"chat-only"
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), HttpStatus::NOT_FOUND);
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        // The callback is wired: only the legacy shared route invokes it.
        let response = client
            .post(format!("http://{addr}/v1/chat"))
            .json(&serde_json::json!({"text":"hello"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), HttpStatus::OK);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        task.abort();
    }

    #[tokio::test]
    async fn health_and_echo_chat_roundtrip() {
        let addr = spawn_bridge(echo_state()).await;
        let base = format!("http://{addr}");
        let client = reqwest::Client::new();

        let health: HealthResponse = client
            .get(format!("{base}/v1/health"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(health.ok);
        assert_eq!(health.service, BRIDGE_IDENTITY);
        assert!(health.echo);

        let chat: ChatResponse = client
            .post(format!("{base}/v1/chat"))
            .json(&serde_json::json!({
                "v": 1,
                "text": "hello from client",
                "session": "default"
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(chat.v, 1);
        assert_eq!(chat.text, "hello from client");
        assert_eq!(chat.session, DEFAULT_SESSION);
    }

    #[test]
    fn task_dto_exposes_interrupted_status() {
        let task = TaskRecord {
            id: "task-test".into(),
            title: "Interrupted task".into(),
            kind: task_board::TaskKind::Pipeline,
            stage: "production".into(),
            status: task_board::TaskStatus::Interrupted,
            user_text: "do work".into(),
            ack_text: "working".into(),
            result_text: "recovery required".into(),
            expected_result: "result".into(),
            job_id: Some("job-test".into()),
            session: "default".into(),
            created_at: 1,
            updated_at: 2,
            error: Some("retry".into()),
            extra: Default::default(),
        };

        assert_eq!(TaskDto::from(&task).status, "interrupted");
    }

    #[test]
    fn pipeline_dto_exposes_interrupted_state() {
        let meta = pipeline_jobs::PipelineJobMeta {
            id: "job-test".into(),
            kind: "develop".into(),
            state: pipeline_jobs::JobState::Interrupted,
            project: None,
            branch: None,
            base: None,
            plan: None,
            task: None,
            pr: false,
            summary: "recovery required".into(),
            error: Some("retry".into()),
            created_at: 1,
            updated_at: 2,
            extra: Default::default(),
        };

        assert_eq!(pipeline_dto(&meta).state, "interrupted");
    }

    #[tokio::test]
    async fn free_port_is_reserved_for_the_new_supervisor() {
        let available = TcpListener::bind(SocketAddr::new(BRIDGE_HOST, 0))
            .await
            .expect("find test port");
        let addr = available.local_addr().expect("test address");
        drop(available);

        match claim_bridge_port(addr).await.expect("claim free port") {
            BridgePortClaim::Reserved(listener) => {
                assert_eq!(listener.local_addr().unwrap(), addr);
            }
            BridgePortClaim::ExistingSmith => panic!("unexpected existing Smith"),
        }
    }

    #[tokio::test]
    async fn valid_smith_health_identity_is_the_only_existing_bridge_accepted() {
        let addr = spawn_bridge(echo_state()).await;

        assert!(matches!(
            claim_bridge_port(addr).await.expect("recognize Smith"),
            BridgePortClaim::ExistingSmith
        ));
    }

    #[tokio::test]
    async fn offline_chat_profile_is_not_accepted_as_legacy_serve() {
        let listener = TcpListener::bind(SocketAddr::new(BRIDGE_HOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/v1/health", get(|| async {
            Json(serde_json::json!({"ok": true, "service": "smith-bridge", "profile": "chat-only"}))
        }));
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        assert!(verify_smith_identity(addr)
            .await
            .unwrap_err()
            .contains("different process profile"));
        task.abort();
    }

    #[tokio::test]
    async fn non_smith_listener_is_rejected() {
        let listener = TcpListener::bind(SocketAddr::new(BRIDGE_HOST, 0))
            .await
            .expect("bind non-Smith listener");
        let addr = listener.local_addr().expect("non-Smith address");
        let app = Router::new().route(
            "/v1/health",
            get(|| async {
                Json(serde_json::json!({
                    "ok": true,
                    "service": "not-smith",
                    "echo": false
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve non-Smith");
        });

        let error = match claim_bridge_port(addr).await {
            Err(error) => error,
            Ok(_) => panic!("non-Smith listener must be rejected"),
        };
        assert!(error.contains("non-Smith listener"));
        assert!(error.contains("/v1/health"));
    }

    #[tokio::test]
    async fn chat_rejects_empty_and_unknown_session() {
        let addr = spawn_bridge(echo_state()).await;
        let base = format!("http://{addr}");
        let client = reqwest::Client::new();

        let empty = client
            .post(format!("{base}/v1/chat"))
            .json(&serde_json::json!({"text": "  "}))
            .send()
            .await
            .unwrap();
        assert_eq!(empty.status(), HttpStatus::BAD_REQUEST);

        let bad_session = client
            .post(format!("{base}/v1/chat"))
            .json(&serde_json::json!({"text": "hi", "session": "mobile"}))
            .send()
            .await
            .unwrap();
        assert_eq!(bad_session.status(), HttpStatus::BAD_REQUEST);
    }

    #[tokio::test]
    async fn chat_brain_error_is_bad_gateway() {
        let state = BridgeState {
            chat: Arc::new(|_, _| Err("brain down".into())),
            echo: false,
            sidecars: Arc::new(Mutex::new(SidecarHub {
                whisper: WhisperGuard::none(),
                tts: TtsGuard::none(),
            })),
            shutdown: Arc::new(Notify::new()),
        };
        let addr = spawn_bridge(state).await;
        let res = reqwest::Client::new()
            .post(format!("http://{addr}/v1/chat"))
            .json(&serde_json::json!({"text": "hi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), HttpStatus::BAD_GATEWAY);
        let body: ErrorBody = res.json().await.unwrap();
        assert!(body.error.contains("brain down"));
    }
}
