//! Explicit cloud capabilities. Constructors perform no I/O; only the app worker uses them.
use crate::app::{AppError, AppResult, Message};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

#[path = "cloud_http.rs"]
mod http;
#[path = "cloud_keyring.rs"]
mod keyring;
pub(crate) use http::unique_json;
pub use http::{build_body, parse_response, OfficialHttp};
pub use keyring::SecretService;

pub const OUTPUT_TOKENS: u32 = 8192;
pub const BODY_LIMIT: usize = 1024 * 1024;
pub const HEADER_LIMIT: usize = 16 * 1024;
pub const POLICY: &str = "smith-cloud-text-2026-10-15-v1";
pub const VALIDATION_TEXT: &str = "Say hello in one short sentence.";
pub const BUDGET: Duration = Duration::from_secs(180);
pub const KEYRING_BUDGET: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Openai,
    Anthropic,
}
impl Vendor {
    pub const ALL: [Self; 2] = [Self::Openai, Self::Anthropic];
    pub fn name(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
        }
    }
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Openai => "https://api.openai.com/v1/responses",
            Self::Anthropic => "https://api.anthropic.com/v1/messages",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Openai => 0,
            Self::Anthropic => 1,
        }
    }
}
#[derive(Clone, Copy)]
pub struct Model {
    pub vendor: Vendor,
    pub id: &'static str,
    pub label: &'static str,
    pub context: usize,
}
pub const MODELS: &[Model] = &[
    Model {
        vendor: Vendor::Openai,
        id: "gpt-6-astra",
        label: "GPT-6 Astra",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Openai,
        id: "gpt-6-sol",
        label: "GPT-6 Sol",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Openai,
        id: "gpt-6.1-sol",
        label: "GPT-6.1 Sol",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Openai,
        id: "gpt-6-luna",
        label: "GPT-6 Luna",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Openai,
        id: "gpt-5.6-sol",
        label: "GPT-5.6 Sol",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Openai,
        id: "gpt-5.6-terra",
        label: "GPT-5.6 Terra",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Openai,
        id: "gpt-5.6-luna",
        label: "GPT-5.6 Luna",
        context: 1_050_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-sonnet-5-5",
        label: "Claude Sonnet 5.5",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-haiku-5-5",
        label: "Claude Haiku 5.5",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-opus-5-5",
        label: "Claude Opus 5.5",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-fable-5-1",
        label: "Claude Fable 5.1",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-opus-5",
        label: "Claude Opus 5",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-opus-4-8",
        label: "Claude Opus 4.8",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-opus-4-7",
        label: "Claude Opus 4.7",
        context: 1_000_000,
    },
    Model {
        vendor: Vendor::Anthropic,
        id: "claude-sonnet-5",
        label: "Claude Sonnet 5",
        context: 1_000_000,
    },
];
pub fn model(vendor: Vendor, id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.vendor == vendor && m.id == id)
}

/// Intentionally neither Debug nor Serialize nor Clone. Copies in transport libraries
/// and the trusted same-user process are outside the best-effort erasure guarantee.
pub struct Secret(Zeroizing<Vec<u8>>);
impl Secret {
    pub fn new(bytes: Vec<u8>) -> AppResult<Self> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty()
            || bytes.len() > 4096
            || !bytes.iter().all(|b| (0x21..=0x7e).contains(b))
        {
            return Err(error("INVALID_REQUEST"));
        }
        Ok(Self(bytes))
    }
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
/// Opaque, non-secret identity. The save ID is an attribute, never a key digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialRef {
    pub provider: Vendor,
    pub generation: String,
    pub save_request_id: String,
}
pub trait CredentialStore: Send + Sync + 'static {
    fn read(
        &self,
        identity: &CredentialRef,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> AppResult<Secret>;
    fn save(
        &self,
        identity: &CredentialRef,
        secret: &Secret,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> AppResult<()>;
}
/// HTTP seam returns only bounded wire bytes, not a vendor SDK result. The service
/// validates even synthetic responses through the production final-text parser.
pub struct HttpResponse {
    pub status: u16,
    pub header_bytes: usize,
    pub body: Vec<u8>,
}
pub struct HttpFailure {
    pub error: AppError,
    pub not_sent: bool,
}
pub trait HttpTransport: Send + Sync + 'static {
    fn post(
        &self,
        vendor: Vendor,
        body: Vec<u8>,
        secret: &Secret,
        deadline: Instant,
        dispatch_deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::result::Result<HttpResponse, HttpFailure>;
}
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;
    fn wall_ms(&self) -> u64;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wall_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(9_007_199_254_740_991) as u64
    }
}
pub struct Dependencies {
    pub credentials: Arc<dyn CredentialStore>,
    pub http: Arc<dyn HttpTransport>,
    pub clock: Arc<dyn Clock>,
    pub(crate) cancelled: AtomicBool,
}
impl Dependencies {
    pub fn new(credentials: Arc<dyn CredentialStore>, http: Arc<dyn HttpTransport>) -> Self {
        Self {
            credentials,
            http,
            clock: Arc::new(SystemClock),
            cancelled: AtomicBool::new(false),
        }
    }
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }
    pub fn production() -> Self {
        Self::new(Arc::new(SecretService), Arc::new(OfficialHttp))
    }
    pub fn disabled() -> Self {
        Self::new(Arc::new(Disabled), Arc::new(Disabled))
    }
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}
struct Disabled;
impl CredentialStore for Disabled {
    fn read(&self, _: &CredentialRef, _: Instant, _: &AtomicBool) -> AppResult<Secret> {
        Err(error("KEYRING_UNAVAILABLE"))
    }
    fn save(&self, _: &CredentialRef, _: &Secret, _: Instant, _: &AtomicBool) -> AppResult<()> {
        Err(error("KEYRING_UNAVAILABLE"))
    }
}
impl HttpTransport for Disabled {
    fn post(
        &self,
        _: Vendor,
        _: Vec<u8>,
        _: &Secret,
        _: Instant,
        _: Instant,
        _: &AtomicBool,
    ) -> std::result::Result<HttpResponse, HttpFailure> {
        Err(HttpFailure {
            error: error("CLOUD_UNAVAILABLE"),
            not_sent: true,
        })
    }
}
pub(crate) fn error(code: &str) -> AppError {
    let message = match code {
        "KEYRING_LOCKED" => "The credential keyring is locked; unlock it outside Smith",
        "KEYRING_MISSING" => "The exact saved credential is missing; no replacement was used",
        "KEYRING_CONFLICT" => "Credential identity is ambiguous or changed; no replacement was used",
        "KEYRING_PROMPT_REQUIRED" => "The keyring requires interaction; Smith will not unlock or prompt",
        "KEYRING_TIMEOUT" => "The bounded keyring operation did not complete",
        "KEYRING_UNAVAILABLE" => "An accessible unlocked same-user Secret Service is required",
        "CREDENTIAL_OUTCOME_UNKNOWN" => "Credential publication may have occurred; the old configuration is preserved; do not repeat automatically",
        "CREDENTIAL_CHANGED" => "Credential generation changed; prepare a new explicit request",
        "NOT_OBSERVED" => "Credential availability has not been observed in this service instance",
        "PROVIDER_NOT_CONFIGURED" => "Save a credential explicitly before using this provider",
        "PROVIDER_NOT_VALIDATED" => "This credential generation needs an explicit provider validation",
        "ROUTE_UNAVAILABLE" => "The selected route is unavailable; no other route was used",
        "CLOUD_AUTH" => "The provider rejected authentication or access; no other credential was used",
        "CLOUD_QUOTA" => "The provider rejected billing or quota; no retry was attempted",
        "CLOUD_RATE_LIMIT" => "The provider rate-limited the request; no retry was attempted",
        "CLOUD_UNAVAILABLE" => "The cloud request failed; no retry or fallback was attempted",
        "CLOUD_PROTOCOL" => "The provider returned an unsupported, invalid or oversized response",
        "REMOTE_OUTCOME_UNKNOWN" => "The provider may have processed or billed this request; it was not replayed",
        "MODEL_TRUNCATED" => "The provider did not return a complete final answer within the output budget",
        "MODEL_RESULT_LIMIT" => "The provider returned empty or oversized final text",
        "CONTEXT_LIMIT" => "The exact request cannot fit the bounded provider context",
        "PERMISSION_EXPIRED" => "Dispatch authorization expired; this request was not sent",
        "CONSENT_EXPIRED" => "Preparation expired; a new explicit request is required",
        "CONSENT_REQUIRED" => "Explicit consent is required before transferring this context",
        "CONSENT_MISMATCH" => "The request no longer matches its exact prepared consent",
        "UPGRADE_REQUIRED" => "Provider-aware schema 3 is required for this state",
        "INTERRUPTED" => "Operation interrupted; it was not replayed",
        _ => "Invalid application request",
    };
    AppError::new(code, message)
}

/// Runtime/future creation happens on the worker. Cancellation drops network/D-Bus
/// futures; DNS blocking helpers cannot keep the worker alive during runtime teardown.
fn bounded<T, F, Fut>(
    deadline: Instant,
    cancelled: &AtomicBool,
    timeout_code: &str,
    create: F,
) -> AppResult<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    if cancelled.load(Ordering::SeqCst) {
        return Err(error("INTERRUPTED"));
    }
    if Instant::now() >= deadline {
        return Err(error(timeout_code));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| error(timeout_code))?;
    let result = rt.block_on(async {
        let future = create();
        tokio::pin!(future);
        loop {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline.into()) => return Err(error(timeout_code)),
                result = &mut future => return result,
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    if cancelled.load(Ordering::SeqCst) { return Err(error("INTERRUPTED")); }
                }
            }
        }
    });
    rt.shutdown_timeout(Duration::ZERO);
    if cancelled.load(Ordering::SeqCst) {
        return Err(error("INTERRUPTED"));
    }
    if Instant::now() >= deadline {
        return Err(error(timeout_code));
    }
    result
}

pub fn context(
    vendor: Vendor,
    id: &str,
    history: &[Message],
    input: &str,
) -> AppResult<Vec<Message>> {
    context_with_instructions(vendor, id, history, input, "")
}

pub fn context_with_instructions(
    vendor: Vendor,
    id: &str,
    history: &[Message],
    input: &str,
    instructions: &str,
) -> AppResult<Vec<Message>> {
    let m = model(vendor, id).ok_or_else(|| error("ROUTE_UNAVAILABLE"))?;
    if instructions.len() > 2048
        || input.trim().is_empty()
        || input.len() > crate::app::MAX_TEXT
        || history.len() > 32
        || history.len() % 2 != 0
    {
        return Err(error("CONTEXT_LIMIT"));
    }
    if history
        .iter()
        .enumerate()
        .any(|(i, m)| m.role != if i % 2 == 0 { "user" } else { "assistant" })
    {
        return Err(error("CONTEXT_LIMIT"));
    }
    let mut messages = history.to_vec();
    loop {
        let bytes = messages.iter().map(|m| m.content.len()).sum::<usize>()
            + input.len()
            + instructions.len();
        if bytes <= crate::app::MAX_CONTEXT
            && bytes
                + (messages.len() + 1 + usize::from(!instructions.is_empty())) * 256
                + 4096
                + OUTPUT_TOKENS as usize
                <= m.context
        {
            break;
        }
        if messages.is_empty() {
            return Err(error("CONTEXT_LIMIT"));
        }
        messages.drain(..2);
    }
    messages.push(Message {
        role: "user".into(),
        content: input.into(),
    });
    Ok(messages)
}
