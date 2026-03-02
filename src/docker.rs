//! Minimal Docker helpers for Doctor (check_docker_available), agent start/stop, and container list/stop/remove.
//! Shared helpers for model and agent container lifecycle plus runtime checks.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::Value;

/// Prefix for agent containers: "smith-agent-<name>". Used by agent start/stop/list.
pub const AGENT_CONTAINER_PREFIX: &str = "smith-agent-";

/// Prefix for spawned containers: "agent_{project}_{branch}".
pub const SPAWN_CONTAINER_PREFIX: &str = "agent_";

/// Port range for spawned containers: 4096-8191.
pub const SPAWN_PORT_MIN: u16 = 4096;
pub const SPAWN_PORT_MAX: u16 = 8191;
pub const SPAWN_PORT_RANGE: u16 = SPAWN_PORT_MAX - SPAWN_PORT_MIN + 1;

/// Default port for OpenCode server. Additional agents use 4097, 4098, ...
pub const OPENCODE_SERVER_PORT: u16 = 4096;

pub fn host_opencode_config_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("opencode"))
}

static SPAWN_RUN_CANCELLED: AtomicBool = AtomicBool::new(false);
static SPAWN_RUN_SIGINT_INIT: Once = Once::new();

fn ensure_spawn_run_sigint_handler() {
    SPAWN_RUN_SIGINT_INIT.call_once(|| {
        let _ = ctrlc::set_handler(|| {
            SPAWN_RUN_CANCELLED.store(true, Ordering::SeqCst);
        });
    });
}

#[derive(Clone, Copy)]
enum StreamSource {
    Stdout,
    Stderr,
}

fn extract_string_field(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.to_string()),
        Some(Value::Object(map)) => map
            .get("message")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        _ => None,
    }
}

fn collect_text_parts(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            if !s.is_empty() {
                out.push(s.to_string());
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_text_parts(item, out);
            }
        }
        Value::Object(map) => {
            for key in [
                "text",
                "delta",
                "content",
                "output_text",
                "answer",
                "message",
            ] {
                if let Some(v) = map.get(key) {
                    collect_text_parts(v, out);
                }
            }
            for key in ["data", "result", "response", "choices", "parts"] {
                if let Some(v) = map.get(key) {
                    collect_text_parts(v, out);
                }
            }
        }
        _ => {}
    }
}

fn extract_retry_after_secs(raw: &str) -> Option<u64> {
    let lower = raw.to_lowercase();
    for marker in [
        "retry-after\":\"",
        "retry-after\":",
        "retry-after:",
        "retry_after\":",
    ] {
        if let Some(idx) = lower.find(marker) {
            let start = idx + marker.len();
            let rest = &lower[start..];
            let digits: String = rest
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(val) = digits.parse::<u64>() {
                return Some(val);
            }
        }
    }
    None
}

fn classify_spawn_run_error(raw: &str, exit_code: Option<i32>) -> String {
    let lower = raw.to_lowercase();

    if lower.contains("no such container") {
        return "Spawned agent container not found. Start it with `smith agent start` and try again."
            .to_string();
    }

    if lower.contains("freeusagelimiterror")
        || lower.contains("rate limit exceeded")
        || lower.contains("statuscode\":429")
        || lower.contains("status code 429")
    {
        if let Some(retry_after) = extract_retry_after_secs(raw) {
            return format!(
                "Agent request failed: provider rate limit/quota exceeded. Retry after about {} seconds.",
                retry_after
            );
        }
        return "Agent request failed: provider rate limit/quota exceeded. Please try again later or switch model/provider.".to_string();
    }

    if lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
    {
        return "Agent request failed: authentication error from provider. Check API key/provider settings.".to_string();
    }

    if let Some(code) = exit_code {
        return format!("Prompt command failed with exit code {}", code);
    }

    "Prompt command failed".to_string()
}

fn has_hard_failure_signal(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    lower.contains("freeusagelimiterror")
        || lower.contains("rate limit exceeded")
        || lower.contains("statuscode\":429")
        || lower.contains("status code 429")
        || lower.contains("ai_apicallerror")
        || lower.contains("error response from daemon")
}

/// Sanitize agent name for use in container name (Docker allows [a-zA-Z0-9][a-zA-Z0-9_.-]*).
pub fn agent_container_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = safe.trim_matches('_');
    if trimmed.is_empty() {
        format!("{}unnamed", AGENT_CONTAINER_PREFIX)
    } else {
        format!("{}{}", AGENT_CONTAINER_PREFIX, trimmed)
    }
}

/// Map provider name to the expected environment variable name for API key.
/// e.g., "anthropic" -> "ANTHROPIC_API_KEY", "openai" -> "OPENAI_API_KEY"
fn provider_api_key_env(provider: &str) -> String {
    let normalized = provider
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{}_API_KEY", normalized)
}

/// Start an agent container running OpenCode in server mode. Exposes `port` on the host.
/// Uses `--entrypoint opencode` so the container runs exactly `opencode serve --hostname 0.0.0.0 --port N`.
/// If a container with this name already exists, tries to start it (e.g. after stop).
/// If provider is Some, passes through the corresponding API key env var from host to container.
/// If base_url is Some, passes OPENCODE_BASE_URL env var to container.
pub fn start_agent_container(
    agent_name: &str,
    image: &str,
    port: u16,
    provider: Option<&str>,
    base_url: Option<&str>,
) -> Result<(), String> {
    let name = agent_container_name(agent_name);
    let port_str = port.to_string();

    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--restart".to_string(),
        "unless-stopped".to_string(),
        "--name".to_string(),
        name.clone(),
        "-p".to_string(),
        format!("{}:{}", port, port),
    ];

    if let Some(cfg_dir) = host_opencode_config_dir().filter(|p| p.exists()) {
        args.extend([
            "-v".to_string(),
            format!("{}:/root/.config/opencode:ro", cfg_dir.to_string_lossy()),
        ]);
    }

    if let Some(prov) = provider {
        let env_var = provider_api_key_env(prov);
        args.push("-e".to_string());
        args.push(env_var);
    }

    if let Some(url) = base_url {
        args.push("-e".to_string());
        args.push(format!("OPENCODE_BASE_URL={}", url));
    }

    args.extend([
        "--entrypoint".to_string(),
        "opencode".to_string(),
        image.to_string(),
        "serve".to_string(),
        "--hostname".to_string(),
        "0.0.0.0".to_string(),
        "--port".to_string(),
        port_str.clone(),
    ]);

    let run = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run container: {}", e))?;
    if run.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&run.stderr);
    if stderr.contains("already in use") || stderr.contains("Conflict") {
        let _ = Command::new("docker").args(["rm", "-f", &name]).output();

        let mut args2 = vec![
            "run".to_string(),
            "-d".to_string(),
            "--restart".to_string(),
            "unless-stopped".to_string(),
            "--name".to_string(),
            name.clone(),
            "-p".to_string(),
            format!("{}:{}", port, port),
        ];

        if let Some(cfg_dir) = host_opencode_config_dir().filter(|p| p.exists()) {
            args2.extend([
                "-v".to_string(),
                format!("{}:/root/.config/opencode:ro", cfg_dir.to_string_lossy()),
            ]);
        }

        if let Some(prov) = provider {
            let env_var = provider_api_key_env(prov);
            args2.push("-e".to_string());
            args2.push(env_var);
        }

        args2.extend([
            "--entrypoint".to_string(),
            "opencode".to_string(),
            image.to_string(),
            "serve".to_string(),
            "--hostname".to_string(),
            "0.0.0.0".to_string(),
            "--port".to_string(),
            port_str.clone(),
        ]);

        let run2 = Command::new("docker")
            .args(&args2)
            .output()
            .map_err(|e| format!("Failed to run container: {}", e))?;
        if run2.status.success() {
            return Ok(());
        }
        return Err(format!(
            "Failed to recreate agent '{}': {}",
            agent_name,
            String::from_utf8_lossy(&run2.stderr).trim()
        ));
    }
    Err(format!(
        "Failed to start agent '{}': {}",
        agent_name,
        stderr.trim()
    ))
}

/// Quick one-shot check: is the agent health endpoint reachable on the given port?
/// Single request, 2s timeout. Used by status to show warning when container is up but port unreachable.
pub fn check_agent_reachable(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/global/health", port);
    let output = Command::new("curl")
        .args(["-sf", "--max-time", "2", &url])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout);
            body.to_lowercase().contains("healthy")
        }
        _ => false,
    }
}

/// Test that an OpenCode server is responding at the given host port (e.g. after start).
/// Uses GET /global/health; returns Ok if we get a 200 and body contains "healthy".
/// Retries up to 8 times with 2s delay (server may need a few seconds to start).
pub fn test_agent_server(port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/global/health", port);
    let mut last_err = String::new();
    for attempt in 0..8 {
        let output = Command::new("curl")
            .args(["-sf", "--max-time", "5", &url])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let body = String::from_utf8_lossy(&out.stdout);
                if body.to_lowercase().contains("healthy") {
                    return Ok(());
                }
                last_err = format!("response did not contain 'healthy': {}", body.trim());
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                last_err = format!("HTTP/connection error: {}", stderr.trim());
            }
            Err(e) => last_err = format!("curl failed: {}", e),
        }
        if attempt < 7 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    Err(format!(
        "Health check failed after retries: server at {} - {}",
        url, last_err
    ))
}

/// Tag for the built agent image (smith/<name>:latest). Smith-managed wrapper of the source image.
pub fn agent_built_image_tag(agent_name: &str) -> String {
    let safe: String = agent_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = safe.trim_matches('_');
    let name = if trimmed.is_empty() {
        "unnamed"
    } else {
        trimmed
    };
    format!("smith/{}:latest", name)
}

/// Return true if a Docker image with the given reference exists locally.
pub fn image_exists(image_ref: &str) -> Result<bool, String> {
    let output = Command::new("docker")
        .args(["image", "inspect", image_ref])
        .output()
        .map_err(|e| format!("Failed to inspect image: {}", e))?;
    Ok(output.status.success())
}

/// List agent names that currently have a running container (smith-agent-*).
pub fn list_running_agent_containers() -> Result<Vec<String>, String> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--filter",
            &format!("name={}", AGENT_CONTAINER_PREFIX),
            "--format",
            "{{.Names}}",
        ])
        .output()
        .map_err(|e| format!("Failed to list containers: {}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to list containers: {}", err));
    }
    let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| {
            s.trim()
                .strip_prefix(AGENT_CONTAINER_PREFIX)
                .unwrap_or(s.trim())
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    Ok(names)
}

/// Return true if a container with the given name exists (running or stopped).
pub fn container_exists(container_name: &str) -> Result<bool, String> {
    let output = Command::new("docker")
        .args(["inspect", container_name])
        .output()
        .map_err(|e| format!("Failed to inspect container: {}", e))?;
    Ok(output.status.success())
}

/// Stop an agent's container by agent name.
pub fn stop_agent_container(agent_name: &str) -> Result<(), String> {
    let name = agent_container_name(agent_name);
    stop_container(&name)
}

/// Stop all running smith-agent-* containers. Returns list of agent names stopped.
pub fn stop_all_agent_containers() -> Result<Vec<String>, String> {
    let running = list_running_agent_containers()?;
    for name in &running {
        let _ = stop_agent_container(name);
    }
    Ok(running)
}

/// Check if Docker is available and running.
pub fn check_docker_available() -> Result<(), String> {
    let version_output = Command::new("docker")
        .arg("--version")
        .output()
        .map_err(|e| format!("Docker command not found: {}", e))?;

    if !version_output.status.success() {
        return Err("Docker command failed to execute".to_string());
    }

    let info_output = Command::new("docker")
        .arg("info")
        .output()
        .map_err(|e| format!("Failed to check Docker daemon: {}", e))?;

    if !info_output.status.success() {
        let error = String::from_utf8_lossy(&info_output.stderr);
        if error.contains("Cannot connect") || error.contains("Is the docker daemon running") {
            return Err("Docker daemon is not running".to_string());
        }
        return Err(format!("Docker daemon check failed: {}", error));
    }

    Ok(())
}

/// Stop a container by name.
pub fn stop_container(container_name: &str) -> Result<(), String> {
    let output = Command::new("docker")
        .arg("stop")
        .arg(container_name)
        .output()
        .map_err(|e| format!("Failed to stop container: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to stop container: {}", error))
    }
}

/// Restart a container by name.
pub fn restart_container(container_name: &str) -> Result<(), String> {
    let output = Command::new("docker")
        .arg("restart")
        .arg(container_name)
        .output()
        .map_err(|e| format!("Failed to restart container: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to restart container: {}", error))
    }
}

mod agent_runtime;
mod model_runtime;

pub use agent_runtime::{
    ensure_spawn_dir, ensure_spawn_state_dir, list_spawn_plan_dirs, list_spawned_containers,
    prune_spawned_containers, read_spawn_file, remove_spawn_dir, restart_spawned_container,
    run_prompt_in_spawned_container, run_prompt_in_spawned_container_with_options, run_spawn_shell,
    spawn_container_name, spawn_container_port, spawn_file_exists, start_spawned_container,
    stop_spawned_container, write_spawn_file,
};
pub use model_runtime::{
    is_ollama_running, start_ollama_container, stop_ollama_container, OLLAMA_PORT,
};
