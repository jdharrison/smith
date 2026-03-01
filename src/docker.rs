//! Minimal Docker helpers for Doctor (check_docker_available), agent start/stop, and container list/stop/remove.
//! Ask/dev/review use the Dagger pipeline; these are for debugging and environment checks.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Once};

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
        return "Spawned agent container not found. Start it with `smith spawn start` and try again."
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

/// Check if Docker is available and running (Dagger uses it as container runtime).
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

/// Container name for the Ollama service.
pub const OLLAMA_CONTAINER_NAME: &str = "smith-ollama";

/// Default port for Ollama API.
pub const OLLAMA_PORT: u16 = 11434;

/// Start an Ollama container for local model serving.
/// Returns the port the Ollama API is available on.
pub fn start_ollama_container(model: &str, gpu: bool) -> Result<u16, String> {
    let container_name = OLLAMA_CONTAINER_NAME;

    // Check if already running
    let inspect = Command::new("docker")
        .args(["inspect", container_name])
        .output();

    if let Ok(output) = inspect {
        if output.status.success() {
            println!("  Ollama container already running");
            return Ok(OLLAMA_PORT);
        }
    }

    // Build docker run command
    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--name".to_string(),
        container_name.to_string(),
        "-p".to_string(),
        format!("{}:{}", OLLAMA_PORT, OLLAMA_PORT),
        "-v".to_string(),
        "smith-ollama:/root/.ollama".to_string(),
    ];

    if gpu {
        args.push("--gpus".to_string());
        args.push("all".to_string());
    }

    args.push("ollama/ollama".to_string());
    args.push("run".to_string());
    args.push(model.to_string());

    println!("  Starting Ollama container with model '{}'...", model);
    if gpu {
        print!("  (with GPU passthrough)");
    }
    println!();

    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start Ollama container: {}", e))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to start Ollama container: {}", error));
    }

    // Wait for Ollama to be ready
    println!("  Waiting for Ollama to be ready...");
    let max_attempts = 30;
    for attempt in 1..=max_attempts {
        let curl = Command::new("curl")
            .args(["-s", &format!("http://localhost:{}/api/tags", OLLAMA_PORT)])
            .output();

        if let Ok(out) = curl {
            if out.status.success() {
                println!("  Ollama is ready!");
                return Ok(OLLAMA_PORT);
            }
        }

        if attempt < max_attempts {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    Err("Ollama container started but API not responding".to_string())
}

/// Stop the Ollama container.
pub fn stop_ollama_container() -> Result<(), String> {
    let container_name = OLLAMA_CONTAINER_NAME;

    // Check if exists
    let inspect = Command::new("docker")
        .args(["inspect", container_name])
        .output();

    if let Ok(output) = inspect {
        if !output.status.success() {
            // Container doesn't exist, nothing to stop
            return Ok(());
        }
    } else {
        return Ok(());
    }

    println!("  Stopping Ollama container...");
    stop_container(container_name)
}

/// Check if Ollama container is running.
pub fn is_ollama_running() -> bool {
    let output = Command::new("docker")
        .args(["inspect", OLLAMA_CONTAINER_NAME, "-f", "{{.State.Running}}"])
        .output();

    output
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("true"))
        .unwrap_or(false)
}

/// Sanitize project or branch name for use in container name.
/// Docker names allow [a-zA-Z0-9][a-zA-Z0-9_.-]*. Replaces disallowed chars with -.
fn sanitize_for_container_name(s: &str) -> String {
    let safe: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    safe.trim_matches(|c| c == '-' || c == '_' || c == '.')
        .to_string()
}

/// Generate container name for a spawned agent: "agent_{project}_{branch}".
pub fn spawn_container_name(project: &str, branch: &str) -> String {
    let proj = sanitize_for_container_name(project);
    let bran = sanitize_for_container_name(branch);
    format!("agent_{}_{}", proj, bran)
}

/// Generate port for a spawned agent using hash-based allocation.
/// Uses project + branch to generate deterministic port in range [4096, 8191].
/// If port is in use, caller should try next available port.
pub fn spawn_container_port(project: &str, branch: &str) -> u16 {
    let mut hasher = DefaultHasher::new();
    project.hash(&mut hasher);
    branch.hash(&mut hasher);
    let hash = hasher.finish() as u16;
    SPAWN_PORT_MIN + (hash % SPAWN_PORT_RANGE)
}

/// Find next available port in spawn range, starting from the given port.
pub fn spawn_find_available_port(start_port: u16) -> Result<u16, String> {
    for port in start_port..=SPAWN_PORT_MAX {
        let url = format!("http://127.0.0.1:{}/global/health", port);
        let output = Command::new("curl")
            .args(["-sf", "--max-time", "1", &url])
            .output();
        if !output.map(|o| o.status.success()).unwrap_or(false) {
            return Ok(port);
        }
    }
    Err("No available ports in spawn range (4096-8191)".to_string())
}

/// Information about a spawned container.
#[derive(Debug, Clone, Deserialize)]
pub struct SpawnInfo {
    pub project: String,
    pub branch: String,
    pub container_name: String,
    pub port: u16,
    pub status: String,
    pub image: String,
}

/// List all spawned containers (smith::*).
pub fn list_spawned_containers() -> Result<Vec<SpawnInfo>, String> {
    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name={}", SPAWN_CONTAINER_PREFIX),
            "--format",
            "{{.Names}}|{{.Status}}|{{.Image}}",
        ])
        .output()
        .map_err(|e| format!("Failed to list containers: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to list containers: {}", err));
    }

    let mut results = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split("|").collect();
        if parts.len() < 3 {
            continue;
        }
        let container_name = parts[0].trim();
        let status = parts[1].trim();
        let image = parts[2].trim();

        // Parse project and branch from name: "agent_{project}_{branch}"
        if let Some(stripped) = container_name.strip_prefix(SPAWN_CONTAINER_PREFIX) {
            let segments: Vec<&str> = stripped.splitn(2, '_').collect();
            if segments.len() >= 2 {
                let project = segments[0].to_string();
                let branch = segments[1..].join("_"); // Handle branches with _ in name

                // Find port from container config or skip
                let port = get_container_port(container_name).unwrap_or(0);

                results.push(SpawnInfo {
                    project,
                    branch,
                    container_name: container_name.to_string(),
                    port,
                    status: status.to_string(),
                    image: image.to_string(),
                });
            }
        }
    }

    Ok(results)
}

/// Get the host port mapped for a container.
fn get_container_port(container_name: &str) -> Result<u16, String> {
    let output = Command::new("docker")
        .args(["port", container_name])
        .output()
        .map_err(|e| format!("Failed to get port: {}", e))?;

    let port_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Output format: "4954/tcp -> 0.0.0.0:4954" - extract the host port
    if let Some(colon_pos) = port_str.rfind(':') {
        let host_port = &port_str[colon_pos + 1..];
        return host_port
            .parse::<u16>()
            .map_err(|_| format!("Failed to parse port from: {}", port_str));
    }
    // Fallback: try to parse as just a number
    port_str
        .parse::<u16>()
        .map_err(|_| format!("Failed to parse port from: {}", port_str))
}

/// Stop a spawned container by project and branch.
pub fn stop_spawned_container(project: &str, branch: &str) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    stop_container(&name)
}

/// Restart a spawned container by project and branch.
pub fn restart_spawned_container(project: &str, branch: &str) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    restart_container(&name)
}

/// Run a prompt in a spawned container and stream raw output to the terminal.
pub fn run_prompt_in_spawned_container(
    project: &str,
    branch: &str,
    prompt: &str,
    verbose: bool,
) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    let mut args = vec![
        "exec".to_string(),
        "-w".to_string(),
        "/workspace".to_string(),
        name,
        "opencode".to_string(),
        "run".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--print-logs".to_string(),
    ];
    if verbose {
        args.push("--thinking".to_string());
    }
    args.push(prompt.to_string());

    ensure_spawn_run_sigint_handler();
    SPAWN_RUN_CANCELLED.store(false, Ordering::SeqCst);

    let mut child = Command::new("docker")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run prompt in spawned container: {}", e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture prompt output".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture prompt errors".to_string())?;

    let (tx, rx) = mpsc::channel::<(StreamSource, String)>();

    let tx_out = tx.clone();
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx_out.send((StreamSource::Stdout, line)).is_err() {
                break;
            }
        }
    });

    let tx_err = tx.clone();
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if tx_err.send((StreamSource::Stderr, line)).is_err() {
                break;
            }
        }
    });
    drop(tx);

    let mut rendered_answer = String::new();
    let mut fallback_stdout = String::new();
    let mut error_context = String::new();

    loop {
        if SPAWN_RUN_CANCELLED.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err("Cancelled by user.".to_string());
        }

        match rx.recv_timeout(std::time::Duration::from_millis(120)) {
            Ok((source, line)) => {
                if line.trim().is_empty() {
                    continue;
                }

                match source {
                    StreamSource::Stdout => {
                        if let Ok(value) = serde_json::from_str::<Value>(&line) {
                            if let Some(err) = extract_string_field(value.get("error")) {
                                if !error_context.is_empty() {
                                    error_context.push('\n');
                                }
                                error_context.push_str(&err);
                            }
                            if let Some(msg) = extract_string_field(value.get("message")) {
                                if msg.to_lowercase().contains("error") {
                                    if !error_context.is_empty() {
                                        error_context.push('\n');
                                    }
                                    error_context.push_str(&msg);
                                }
                            }

                            let mut parts = Vec::new();
                            collect_text_parts(&value, &mut parts);
                            for part in parts {
                                rendered_answer.push_str(&part);
                                if verbose {
                                    print!("{}", part);
                                    let _ = std::io::stdout().flush();
                                }
                            }

                            if has_hard_failure_signal(&error_context) {
                                let _ = child.kill();
                                let _ = child.wait();
                                let _ = stdout_thread.join();
                                let _ = stderr_thread.join();
                                return Err(classify_spawn_run_error(&error_context, Some(1)));
                            }
                        } else {
                            if !fallback_stdout.is_empty() {
                                fallback_stdout.push('\n');
                            }
                            fallback_stdout.push_str(&line);
                            if verbose {
                                println!("{}", line);
                            }

                            if has_hard_failure_signal(&line) {
                                let _ = child.kill();
                                let _ = child.wait();
                                let _ = stdout_thread.join();
                                let _ = stderr_thread.join();
                                return Err(classify_spawn_run_error(&line, Some(1)));
                            }
                        }
                    }
                    StreamSource::Stderr => {
                        if !error_context.is_empty() {
                            error_context.push('\n');
                        }
                        error_context.push_str(&line);
                        if verbose {
                            eprintln!("{}", line);
                        }

                        if has_hard_failure_signal(&line) {
                            let _ = child.kill();
                            let _ = child.wait();
                            let _ = stdout_thread.join();
                            let _ = stderr_thread.join();
                            return Err(classify_spawn_run_error(&error_context, Some(1)));
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed waiting for prompt command: {}", e))?;

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    if has_hard_failure_signal(&error_context) {
        return Err(classify_spawn_run_error(&error_context, status.code()));
    }

    if status.success() {
        if !rendered_answer.trim().is_empty() {
            if !verbose {
                println!("{}", rendered_answer.trim());
            }
            return Ok(());
        }
        if !fallback_stdout.trim().is_empty() {
            println!("{}", fallback_stdout.trim());
        }
        return Ok(());
    }

    let mut raw = String::new();
    if !error_context.trim().is_empty() {
        raw.push_str(error_context.trim());
    }
    if !fallback_stdout.trim().is_empty() {
        if !raw.is_empty() {
            raw.push('\n');
        }
        raw.push_str(fallback_stdout.trim());
    }

    Err(classify_spawn_run_error(&raw, status.code()))
}

/// Prune (remove) all stopped spawned containers.
pub fn prune_spawned_containers() -> Result<Vec<String>, String> {
    let containers = list_spawned_containers()?;
    let mut removed = Vec::new();

    for container in containers {
        if container.status.to_lowercase().contains("exited")
            || container.status.to_lowercase().contains("created")
            || container.status.to_lowercase().contains("dead")
        {
            let output = Command::new("docker")
                .args(["rm", &container.container_name])
                .output()
                .map_err(|e| format!("Failed to remove container: {}", e))?;

            if output.status.success() {
                removed.push(container.container_name);
            }
        }
    }

    Ok(removed)
}

/// Start a spawned container for a project/branch.
/// Clones fresh repo into container workspace and starts opencode serve.
pub fn start_spawned_container(
    project: &str,
    branch: &str,
    port: u16,
    image: &str,
    repo_url: &str,
    ssh_key: Option<&Path>,
    commit_name: Option<&str>,
    commit_email: Option<&str>,
) -> Result<u16, String> {
    let container_name = spawn_container_name(project, branch);

    // Check if container already exists
    if container_exists(&container_name)? {
        // Check if it's running
        let output = Command::new("docker")
            .args(["inspect", &container_name, "-f", "{{.State.Running}}"])
            .output()
            .map_err(|e| format!("Failed to inspect container: {}", e))?;

        if output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .trim()
                .contains("true")
        {
            // Container already running, return its port
            let actual_port = get_container_port(&container_name)?;
            return Ok(actual_port);
        }

        // Container exists but not running - remove it so we can start fresh
        let _ = Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output();
    }

    // Ensure port is available
    let final_port = if check_agent_reachable(port) {
        spawn_find_available_port(port)?
    } else {
        port
    };

    // Build git config commands for identity (with fallback)
    let git_name_cmd = match commit_name {
        Some(name) => format!(
            "git config user.name '{}' 2>/dev/null || git config user.name 'Smith' 2>/dev/null || true",
            name.replace('\'', "'\"'\"'")
        ),
        None => "git config user.name 'Smith' 2>/dev/null || true".to_string(),
    };
    let git_email_cmd = match commit_email {
        Some(email) => format!(
            "git config user.email '{}' 2>/dev/null || git config user.email 'smith@localhost' 2>/dev/null || true",
            email.replace('\'', "'\"'\"'")
        ),
        None => "git config user.email 'smith@localhost' 2>/dev/null || true".to_string(),
    };

    // Build startup script that clones repo and starts opencode serve
    let branch_escaped = branch.replace('\'', "'\"'\"'");
    let repo_escaped = repo_url.replace('\'', "'\"'\"'");
    let startup_script = format!(
        r#"set -e
# Install git and openssh-client
apk add --no-cache git openssh-client 2>/dev/null || (apt-get update && apt-get install -y git openssh-client 2>/dev/null) || true

# Setup SSH: only create .ssh dir if not already mounted from host
if [ ! -d /root/.ssh ] || [ ! -f /root/.ssh/known_hosts ]; then
    mkdir -p /root/.ssh
    chmod 700 /root/.ssh
    ssh-keyscan github.com >> /root/.ssh/known_hosts 2>/dev/null || true
fi

# Set GIT_SSH_COMMAND if key file exists, otherwise rely on SSH_AUTH_SOCK
if [ -f /root/.ssh/id_rsa ]; then
    chmod 600 /root/.ssh/id_rsa
    export GIT_SSH_COMMAND="ssh -i /root/.ssh/id_rsa -o StrictHostKeyChecking=no"
fi

# Create workspace and prepare repo (idempotent on container restart)
mkdir -p /workspace

if [ -d /workspace/.git ]; then
    cd /workspace
else
    if [ -n "$(ls -A /workspace 2>/dev/null)" ]; then
        # Preserve pre-existing files; avoid failing restart on non-empty workspace
        cd /workspace
    else
        git clone '{repo}' /workspace
        cd /workspace
    fi
fi

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    # Ensure origin matches configured repo
    if git remote get-url origin >/dev/null 2>&1; then
        git remote set-url origin '{repo}' 2>/dev/null || true
    else
        git remote add origin '{repo}' 2>/dev/null || true
    fi

    # Fetch refs when available (non-fatal for restart resilience)
    git fetch origin 2>/dev/null || true

    # Prefer preserving local branch state; only create when missing
    if git show-ref --verify --quiet 'refs/heads/{branch}'; then
        git checkout '{branch}'
    elif git rev-parse --verify 'origin/{branch}' >/dev/null 2>&1; then
        git checkout -b '{branch}' 'origin/{branch}'
    else
        git checkout -b '{branch}'
    fi

    # Ensure git config for commits in this session
    # Use project-configured identity, or fallback to default
    {git_name}
    {git_email}
else
    echo "Warning: /workspace is not a git repo; skipping git setup"
fi

# Start opencode serve
exec opencode serve --hostname 0.0.0.0 --port {port}"#,
        repo = repo_escaped,
        branch = branch_escaped,
        port = final_port,
        git_name = git_name_cmd,
        git_email = git_email_cmd
    );

    // Build docker run command
    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--restart".to_string(),
        "unless-stopped".to_string(),
        "--name".to_string(),
        container_name.clone(),
        "-p".to_string(),
        format!("{}:{}", final_port, final_port),
    ];

    if let Some(cfg_dir) = host_opencode_config_dir().filter(|p| p.exists()) {
        args.extend([
            "-v".to_string(),
            format!("{}:/root/.config/opencode:ro", cfg_dir.to_string_lossy()),
        ]);
    }

    // Forward SSH agent if available
    if let Some(ssh_socket) = std::env::var("SSH_AUTH_SOCK").ok() {
        args.extend(["-e".to_string(), format!("SSH_AUTH_SOCK={}", ssh_socket)]);
    }

    // Mount SSH key or forward SSH agent
    if let Some(key_path) = ssh_key {
        if key_path.exists() {
            args.extend([
                "-v".to_string(),
                format!("{}:/root/.ssh/id_rsa:ro", key_path.display()),
                "-e".to_string(),
                "GIT_SSH_COMMAND=ssh -i /root/.ssh/id_rsa -o StrictHostKeyChecking=no".to_string(),
            ]);
        }
    } else if std::env::var("SSH_AUTH_SOCK").is_ok() {
        // No specific key but SSH agent available: mount ~/.ssh so agent can authenticate
        if let Ok(home) = std::env::var("HOME") {
            let ssh_dir = format!("{}/.ssh", home);
            if std::path::Path::new(&ssh_dir).exists() {
                args.extend(["-v".to_string(), format!("{}:/root/.ssh:rw", ssh_dir)]);
            }
        }
    }

    // Add image and startup script (image must come before command in docker run)
    args.extend([
        "--entrypoint".to_string(),
        "/bin/sh".to_string(),
        image.to_string(),
        "-c".to_string(),
        startup_script,
    ]);

    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run container: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to start spawned container: {}",
            stderr.trim()
        ));
    }

    // Wait for server to be ready
    test_agent_server(final_port)?;

    Ok(final_port)
}
