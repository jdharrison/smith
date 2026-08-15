//! Loopback chat bridge for native clients (Flutter later).
//!
//! Smith owns the turn (same session as `smith chat`). The client owns UI / STT / TTS.
//! Bind is localhost only — not a public API.

use crate::brain::reset_chat_session;
use crate::commands::chat::{ensure_ready, run_chat_turn};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Default bridge port (avoid Ollama 11434 / OpenCode 4096).
pub const BRIDGE_DEFAULT_PORT: u16 = 4783;

const BRIDGE_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_SESSION: &str = "default";

type ChatFn = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

#[derive(Clone)]
struct BridgeState {
    chat: ChatFn,
    echo: bool,
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
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatResponse {
    pub v: u32,
    pub text: String,
    pub session: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    pub echo: bool,
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

fn echo_chat(text: &str) -> Result<String, String> {
    Ok(text.to_string())
}

fn bridge_router(state: BridgeState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/chat", post(chat))
        .with_state(state)
}

async fn health(State(state): State<BridgeState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "smith-bridge".to_string(),
        echo: state.echo,
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
    let chat = state.chat.clone();
    let answer = tokio::task::spawn_blocking(move || chat(&text))
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
        text: answer,
        session: DEFAULT_SESSION.to_string(),
    }))
}

pub async fn handle_serve(port: u16, echo: bool) {
    let chat: ChatFn = if echo {
        Arc::new(echo_chat)
    } else {
        let (_mem, model) = match ensure_ready() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        };
        Arc::new(move |text: &str| run_chat_turn(text, &model))
    };

    let addr = SocketAddr::new(BRIDGE_HOST, port);
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: failed to bind smith bridge on {addr}: {e}");
            std::process::exit(1);
        }
    };

    println!("smith bridge listening on http://{addr}");
    if echo {
        println!("  mode: echo (no brain — client integration)");
    } else {
        println!("  mode: chat (same session as `smith chat`)");
    }
    println!("  GET  /v1/health");
    println!(
        "  POST /v1/chat   {{\"v\":1,\"text\":\"...\",\"session\":\"default\",\"new\":false}}"
    );
    println!("  loopback only. Ctrl-C to stop.");

    let app = bridge_router(BridgeState { chat, echo });
    if let Err(e) = axum::serve(listener, app).await {
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
        }
    }

    async fn spawn_bridge(state: BridgeState) -> SocketAddr {
        let listener = TcpListener::bind(SocketAddr::new(BRIDGE_HOST, 0))
            .await
            .expect("bind test port");
        let addr = listener.local_addr().expect("local addr");
        let app = bridge_router(state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        addr
    }

    #[test]
    fn validate_rejects_empty_text_and_bad_session() {
        assert!(validate_chat_request(&ChatRequest {
            v: Some(1),
            text: "  ".into(),
            session: None,
            new: false,
        })
        .is_err());
        assert!(validate_chat_request(&ChatRequest {
            v: Some(1),
            text: "hi".into(),
            session: Some("other".into()),
            new: false,
        })
        .is_err());
        assert!(validate_chat_request(&ChatRequest {
            v: Some(2),
            text: "hi".into(),
            session: None,
            new: false,
        })
        .is_err());
        assert!(validate_chat_request(&ChatRequest {
            v: Some(1),
            text: "hi".into(),
            session: Some(DEFAULT_SESSION.into()),
            new: false,
        })
        .is_ok());
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
        assert_eq!(health.service, "smith-bridge");
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
        assert_eq!(
            chat,
            ChatResponse {
                v: 1,
                text: "hello from client".into(),
                session: DEFAULT_SESSION.into(),
            }
        );
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
            chat: Arc::new(|_| Err("brain down".into())),
            echo: false,
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
