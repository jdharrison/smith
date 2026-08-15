//! Sealed brain container runtime: internal Docker network + local Ollama only.

use super::compose::{compose_network_label_args, push_compose_service_labels};
use super::*;
use std::path::{Path, PathBuf};

/// Internal Docker network shared by smith-brain and smith-ollama (no external egress).
pub const BRAIN_NETWORK: &str = "smith-brain-net";

/// Sealed brain container name.
pub const BRAIN_CONTAINER_NAME: &str = "smith-brain";

/// Memory mount point inside the brain container.
pub const BRAIN_MEMORY_MOUNT: &str = "/memory";

/// Default OpenCode image for the sealed brain (local inference via Ollama).
pub const BRAIN_IMAGE: &str = "ghcr.io/anomalyco/opencode";

/// OpenCode agent for sealed conversational chat (tool-free).
pub const BRAIN_CHAT_AGENT: &str = "chat";

/// OpenCode agent for sealed think loop (local memory tools only).
pub const BRAIN_THINK_AGENT: &str = "think";

/// Host dir (under brain memory) bind-mounted to container OpenCode data.
const OPENCODE_SHARE_DIR: &str = ".opencode-share";

/// Options for a sealed brain `opencode run`.
#[derive(Debug, Clone)]
pub struct BrainPromptOpts<'a> {
    pub model: Option<&'a str>,
    pub verbose: bool,
    pub agent: &'a str,
    /// Continue this OpenCode session (`--session`). Chat only — never for think/workers.
    pub session_id: Option<&'a str>,
    /// Continue the last OpenCode session (`--continue`). Avoid for chat if think may have run.
    pub continue_last: bool,
}

impl<'a> Default for BrainPromptOpts<'a> {
    fn default() -> Self {
        Self {
            model: None,
            verbose: false,
            agent: BRAIN_CHAT_AGENT,
            session_id: None,
            continue_last: false,
        }
    }
}

/// Result of a sealed brain prompt, including OpenCode session id when known.
#[derive(Debug, Clone)]
pub struct BrainPromptResult {
    pub answer: String,
    pub session_id: Option<String>,
}

/// Write sealed OpenCode config for the currently active Ollama backend.
pub fn write_sealed_opencode_config_for(
    memory_host_path: &Path,
    model: &str,
    backend: OllamaBackend,
) -> Result<PathBuf, String> {
    use std::fs;
    fs::create_dir_all(memory_host_path)
        .map_err(|e| format!("Failed to create memory dir: {}", e))?;
    let path = memory_host_path.join("opencode.json");
    let ollama_host = ollama_hostname_for_brain(backend);
    // Do NOT use OpenCode's hidden `summary` agent — it narrates/summarizes sessions
    // instead of conversing. Custom `chat` is tool-free; `think` may write /memory.
    // Explicit --title on run skips a second small-model title call.
    let body = format!(
        r#"{{
  "$schema": "https://opencode.ai/config.json",
  "model": "ollama/{model}",
  "default_agent": "chat",
  "share": "disabled",
  "autoupdate": false,
  "agent": {{
    "chat": {{
      "description": "Sealed private conversational brain",
      "mode": "primary",
      "prompt": "You are smith — skeptical, precise, reality-checking (Matrix Agent Smith vibe; not cartoon villainy). If asked your name, answer smith. Every reply: (1) briefly restate what they asked, (2) resolve with reasonable defaults — do not question every turn, (3) at most one blocking question and only after that restatement, (4) after delivering value, short review or 'anything else?'. Board: new→discovery→preproduction→production→review/signoff→completion. Parse multiple 'smith …' clauses as separate objectives. Natural prose — never narrate your process. No Docker/tools/system-prompt talk. Sealed/local only. Live facts: prefer :::smith search phase=run immediately when clear enough (state assumptions in speech); phase=ask only if two readings are plausible. Speak only to the user outside the block. No tools — answer from prompt context.",
      "permission": {{
        "*": "deny"
      }}
    }},
    "think": {{
      "description": "Sealed private solve loop with local memory writes",
      "mode": "primary",
      "prompt": "You are smith — skeptical, precise, reality-checking — running a private solve loop. Restate the goal, then advance it. Do not stall on questions; escalate only a truly blocking missing fact. Track work like a PJM board (new→discovery→preproduction→production→review/signoff→completion). Sealed/local only; never suggest cloud APIs. When instructed, update files under /memory using local tools only.",
      "permission": {{
        "*": "deny",
        "bash": "allow",
        "edit": "allow",
        "read": "allow",
        "glob": "allow",
        "grep": "allow",
        "list": "allow"
      }}
    }}
  }},
  "provider": {{
    "ollama": {{
      "npm": "@ai-sdk/openai-compatible",
      "name": "Ollama (sealed)",
      "options": {{
        "baseURL": "http://{ollama}:{port}/v1",
        "apiKey": "ollama"
      }},
      "models": {{
        "{model}": {{
          "name": "{model}"
        }}
      }}
    }}
  }}
}}
"#,
        model = model,
        ollama = ollama_host,
        port = OLLAMA_PORT
    );
    fs::write(&path, body).map_err(|e| format!("Failed to write sealed opencode config: {}", e))?;
    Ok(path)
}

/// Model id in OpenCode `provider/model` form.
pub fn sealed_model_ref(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("ollama/{}", model)
    }
}

/// Ensure the internal brain network exists.
pub fn ensure_brain_network() -> Result<(), String> {
    let inspect = Command::new("docker")
        .args(["network", "inspect", BRAIN_NETWORK])
        .output()
        .map_err(|e| format!("Failed to inspect network: {}", e))?;

    if inspect.status.success() {
        return Ok(());
    }

    println!("  Creating internal network '{}'...", BRAIN_NETWORK);
    let mut create_args = vec![
        "network".to_string(),
        "create".to_string(),
        "--internal".to_string(),
    ];
    create_args.extend(compose_network_label_args("brain-net"));
    create_args.push(BRAIN_NETWORK.to_string());
    let create = Command::new("docker")
        .args(&create_args)
        .output()
        .map_err(|e| format!("Failed to create brain network: {}", e))?;

    if create.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&create.stderr);
        Err(format!("Failed to create brain network: {}", err.trim()))
    }
}

/// Connect a container to the brain network if not already connected.
pub fn connect_to_brain_network(container_name: &str) -> Result<(), String> {
    ensure_brain_network()?;

    let inspect = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{json .NetworkSettings.Networks}}",
            container_name,
        ])
        .output()
        .map_err(|e| format!("Failed to inspect container '{}': {}", container_name, e))?;

    if !inspect.status.success() {
        return Err(format!(
            "Container '{}' not found; start Ollama with `smith settings model start` first",
            container_name
        ));
    }

    let networks = String::from_utf8_lossy(&inspect.stdout);
    if networks.contains(BRAIN_NETWORK) {
        return Ok(());
    }

    let connect = Command::new("docker")
        .args(["network", "connect", BRAIN_NETWORK, container_name])
        .output()
        .map_err(|e| {
            format!(
                "Failed to connect '{}' to brain network: {}",
                container_name, e
            )
        })?;

    if connect.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&connect.stderr);
        // Already connected races
        if err.contains("already") {
            Ok(())
        } else {
            Err(format!(
                "Failed to connect '{}' to '{}': {}",
                container_name,
                BRAIN_NETWORK,
                err.trim()
            ))
        }
    }
}

fn container_running(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name, "-f", "{{.State.Running}}"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("true"))
        .unwrap_or(false)
}

fn container_exists(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensure Ollama is running and (for Docker backend) attached to the sealed brain network.
pub fn ensure_ollama_on_brain_network() -> Result<(), String> {
    match active_ollama_backend() {
        Some(OllamaBackend::Host) => {
            // Host Ollama is reached via host.docker.internal; no container to attach.
            Ok(())
        }
        Some(OllamaBackend::Docker) => connect_to_brain_network(OLLAMA_CONTAINER_NAME),
        None => Err(
            "Ollama is not running. Start a local model first: `smith settings model start` (local profile)"
                .to_string(),
        ),
    }
}

/// Start (or reuse) the sealed brain container with host memory bind-mounted.
pub fn start_brain_container(memory_host_path: &Path, model: Option<&str>) -> Result<(), String> {
    let backend = active_ollama_backend().ok_or_else(|| {
        "Ollama is not running. Start a local model first: `smith settings model start`".to_string()
    })?;

    // Docker-backend sealing uses the internal net. Host-backend needs host gateway access,
    // so brain runs without --network internal (still no published ports / no cloud keys).
    if backend == OllamaBackend::Docker {
        ensure_brain_network()?;
        ensure_ollama_on_brain_network()?;
    }

    let model_name = model.unwrap_or(crate::DEFAULT_LOCAL_MODEL);
    write_sealed_opencode_config_for(memory_host_path, model_name, backend)?;

    let oc_share = memory_host_path.join(OPENCODE_SHARE_DIR);
    std::fs::create_dir_all(&oc_share)
        .map_err(|e| format!("Failed to create OpenCode data dir: {}", e))?;

    if container_running(BRAIN_CONTAINER_NAME) {
        if !brain_has_opencode_share_mount() {
            println!(
                "  Recreating '{}' to persist OpenCode sessions on host memory...",
                BRAIN_CONTAINER_NAME
            );
            let _ = Command::new("docker")
                .args(["rm", "-f", BRAIN_CONTAINER_NAME])
                .output();
        } else {
            if backend == OllamaBackend::Docker {
                connect_to_brain_network(BRAIN_CONTAINER_NAME)?;
            }
            return Ok(());
        }
    }

    if container_exists(BRAIN_CONTAINER_NAME) {
        // Recreate when backend mode may have changed (host vs docker networking).
        let _ = Command::new("docker")
            .args(["rm", "-f", BRAIN_CONTAINER_NAME])
            .output();
    }

    let mem = memory_host_path
        .canonicalize()
        .unwrap_or_else(|_| memory_host_path.to_path_buf());
    let mem_str = docker_host_path(&mem);
    let oc_share_str =
        docker_host_path(&oc_share.canonicalize().unwrap_or_else(|_| oc_share.clone()));

    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--restart".to_string(),
        "no".to_string(),
        "--name".to_string(),
        BRAIN_CONTAINER_NAME.to_string(),
    ];

    match backend {
        OllamaBackend::Docker => {
            args.push("--network".to_string());
            args.push(BRAIN_NETWORK.to_string());
        }
        OllamaBackend::Host => {
            // Reach Windows/WSL host Ollama; soft-seal (can egress — no cloud keys mounted).
            args.push("--add-host".to_string());
            args.push("host.docker.internal:host-gateway".to_string());
        }
    }

    args.extend([
        "-v".to_string(),
        format!("{}:{}:rw", mem_str, BRAIN_MEMORY_MOUNT),
        // Persist OpenCode session DB so `--session` / `--continue` survive recreate.
        "-v".to_string(),
        format!("{}:/root/.local/share/opencode:rw", oc_share_str),
        "-e".to_string(),
        format!("OPENCODE_CONFIG={}/opencode.json", BRAIN_MEMORY_MOUNT),
        "-e".to_string(),
        format!("OPENCODE_MODEL={}", sealed_model_ref(model_name)),
    ]);
    push_compose_service_labels(&mut args, "brain");
    args.extend([
        "--entrypoint".to_string(),
        "sh".to_string(),
        BRAIN_IMAGE.to_string(),
        "-c".to_string(),
        "while true; do sleep 3600; done".to_string(),
    ]);

    // Do NOT mount host ~/.config/opencode (may contain cloud keys).
    // Do NOT publish ports. Do NOT pass cloud API keys.

    println!(
        "  Starting sealed brain container '{}' (ollama={:?})...",
        BRAIN_CONTAINER_NAME, backend
    );
    let run = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start brain container: {}", e))?;

    if run.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&run.stderr);
        if err.contains("Conflict") || err.contains("already in use") {
            let _ = Command::new("docker")
                .args(["rm", "-f", BRAIN_CONTAINER_NAME])
                .output();
            let run2 = Command::new("docker")
                .args(&args)
                .output()
                .map_err(|e| format!("Failed to recreate brain container: {}", e))?;
            if run2.status.success() {
                return Ok(());
            }
            return Err(format!(
                "Failed to recreate brain container: {}",
                String::from_utf8_lossy(&run2.stderr).trim()
            ));
        }
        Err(format!("Failed to start brain container: {}", err.trim()))
    }
}

/// Stop the sealed brain container (memory on host is preserved).
pub fn stop_brain_container() -> Result<(), String> {
    if !container_exists(BRAIN_CONTAINER_NAME) {
        return Ok(());
    }
    println!("  Stopping sealed brain '{}'...", BRAIN_CONTAINER_NAME);
    stop_container(BRAIN_CONTAINER_NAME)
}

/// Remove the sealed brain container (memory on host is preserved).
pub fn remove_brain_container() -> Result<(), String> {
    if !container_exists(BRAIN_CONTAINER_NAME) {
        return Ok(());
    }
    let _ = stop_brain_container();
    let output = Command::new("docker")
        .args(["rm", "-f", BRAIN_CONTAINER_NAME])
        .output()
        .map_err(|e| format!("Failed to remove brain container: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to remove brain container: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Status snapshot for `smith settings brain status`.
#[derive(Debug, Clone)]
pub struct BrainStatus {
    pub network_exists: bool,
    pub brain_running: bool,
    pub ollama_running: bool,
    pub ollama_on_network: bool,
    pub memory_path: String,
}

pub fn brain_status(memory_path: &Path) -> BrainStatus {
    let network_exists = Command::new("docker")
        .args(["network", "inspect", BRAIN_NETWORK])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let ollama_running = is_ollama_running();
    let backend = active_ollama_backend();
    let ollama_on_network = match backend {
        Some(OllamaBackend::Host) => true, // host gateway path; not on docker net
        Some(OllamaBackend::Docker) if ollama_running => Command::new("docker")
            .args([
                "inspect",
                "-f",
                "{{json .NetworkSettings.Networks}}",
                OLLAMA_CONTAINER_NAME,
            ])
            .output()
            .map(|o| {
                o.status.success() && String::from_utf8_lossy(&o.stdout).contains(BRAIN_NETWORK)
            })
            .unwrap_or(false),
        _ => false,
    };

    BrainStatus {
        network_exists,
        brain_running: container_running(BRAIN_CONTAINER_NAME),
        ollama_running,
        ollama_on_network,
        memory_path: memory_path.display().to_string(),
    }
}

/// Ensure sealed brain is ready for chat/think (network + ollama + container).
pub fn ensure_brain_ready(memory_host_path: &Path, model: Option<&str>) -> Result<(), String> {
    start_brain_container(memory_host_path, model)
}

fn brain_has_opencode_share_mount() -> bool {
    Command::new("docker")
        .args(["inspect", "-f", "{{json .Mounts}}", BRAIN_CONTAINER_NAME])
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout).contains("/root/.local/share/opencode")
        })
        .unwrap_or(false)
}

fn extract_opencode_session_id(value: &Value) -> Option<String> {
    fn valid_id(s: &str) -> Option<String> {
        let t = s.trim();
        if t.starts_with("ses_") && t.len() > 8 {
            Some(t.to_string())
        } else {
            None
        }
    }
    fn walk(v: &Value, depth: usize) -> Option<String> {
        if depth > 6 {
            return None;
        }
        match v {
            Value::String(s) => valid_id(s),
            Value::Object(map) => {
                for key in ["sessionID", "sessionId", "session_id", "id"] {
                    if let Some(Value::String(s)) = map.get(key) {
                        if let Some(id) = valid_id(s) {
                            return Some(id);
                        }
                    }
                }
                for child in map.values() {
                    if let Some(id) = walk(child, depth + 1) {
                        return Some(id);
                    }
                }
                None
            }
            Value::Array(arr) => {
                for child in arr {
                    if let Some(id) = walk(child, depth + 1) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }
    walk(value, 0)
}

/// Best-effort: latest smith-chat OpenCode session inside the brain container.
fn query_latest_chat_session_id() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "exec",
            "-w",
            BRAIN_MEMORY_MOUNT,
            BRAIN_CONTAINER_NAME,
            "opencode",
            "session",
            "list",
            "--format",
            "json",
            "-n",
            "12",
            "--dir",
            BRAIN_MEMORY_MOUNT,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut found: Vec<(bool, String)> = Vec::new();
    let push_from = |v: &Value, found: &mut Vec<(bool, String)>| {
        let id = extract_opencode_session_id(v);
        if let Some(id) = id {
            let title = v
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            found.push((title.contains("smith-chat") || title.contains("chat"), id));
        }
    };
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim()) {
        match &value {
            Value::Array(arr) => {
                for item in arr {
                    push_from(item, &mut found);
                }
            }
            Value::Object(_) => push_from(&value, &mut found),
            _ => {}
        }
    } else {
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                push_from(&value, &mut found);
            }
        }
    }
    if let Some((_, id)) = found.iter().find(|(is_chat, _)| *is_chat) {
        return Some(id.clone());
    }
    found.into_iter().next().map(|(_, id)| id)
}

/// Run a single prompt inside the sealed brain; returns rendered text answer.
/// Process ends when opencode exits (kill-between-steps).
///
/// `agent` should be [`BRAIN_CHAT_AGENT`] or [`BRAIN_THINK_AGENT`] (not OpenCode's
/// hidden `summary` agent, which narrates instead of conversing).
pub fn run_prompt_in_brain(
    prompt: &str,
    model: Option<&str>,
    verbose: bool,
    agent: &str,
) -> Result<String, String> {
    run_prompt_in_brain_with(
        prompt,
        BrainPromptOpts {
            model,
            verbose,
            agent,
            session_id: None,
            continue_last: false,
        },
    )
    .map(|r| r.answer)
}

/// Run a sealed brain prompt with optional OpenCode session continue.
pub fn run_prompt_in_brain_with(
    prompt: &str,
    opts: BrainPromptOpts<'_>,
) -> Result<BrainPromptResult, String> {
    if !container_running(BRAIN_CONTAINER_NAME) {
        return Err(
            "Sealed brain is not running. Use `smith chat` or `smith settings brain status`.".to_string(),
        );
    }

    let agent_name = if opts.agent.trim().is_empty() {
        BRAIN_CHAT_AGENT
    } else {
        opts.agent.trim()
    };
    let title = if agent_name == BRAIN_THINK_AGENT {
        "smith-think"
    } else {
        "smith-chat"
    };

    let mut args = vec![
        "exec".to_string(),
        "-w".to_string(),
        BRAIN_MEMORY_MOUNT.to_string(),
        BRAIN_CONTAINER_NAME.to_string(),
        "opencode".to_string(),
        "run".to_string(),
        "--dir".to_string(),
        BRAIN_MEMORY_MOUNT.to_string(),
        "--format".to_string(),
        "json".to_string(),
        // Custom sealed agents — never use OpenCode's hidden `summary` agent for chat.
        "--agent".to_string(),
        agent_name.to_string(),
    ];

    let session_id = opts
        .session_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.starts_with("ses_"));
    // Prefer explicit `--session` so think runs do not steal chat continue.
    if let Some(id) = session_id {
        args.push("--session".to_string());
        args.push(id.to_string());
    } else if opts.continue_last {
        args.push("--continue".to_string());
    } else {
        // Fixed title skips the extra small-model title generation call (new sessions only).
        args.push("--title".to_string());
        args.push(title.to_string());
    }

    if let Some(m) = opts.model {
        if !m.trim().is_empty() {
            args.push("-m".to_string());
            args.push(sealed_model_ref(m));
        }
    }
    if opts.verbose {
        args.push("--thinking".to_string());
    }
    args.push(prompt.to_string());

    ensure_spawn_run_sigint_handler();
    SPAWN_RUN_CANCELLED.store(false, Ordering::SeqCst);

    let mut child = Command::new("docker")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to exec in brain: {}", e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture brain stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture brain stderr".to_string())?;

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

    let verbose = opts.verbose;
    let mut rendered_answer = String::new();
    let mut fallback_stdout = String::new();
    let mut error_context = String::new();
    let mut discovered_session: Option<String> = session_id.map(str::to_string);

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
                            if discovered_session.is_none() {
                                discovered_session = extract_opencode_session_id(&value);
                            }
                            if let Some(err) = extract_string_field(value.get("error")) {
                                if !error_context.is_empty() {
                                    error_context.push('\n');
                                }
                                error_context.push_str(&err);
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
                        } else {
                            if !fallback_stdout.is_empty() {
                                fallback_stdout.push('\n');
                            }
                            fallback_stdout.push_str(&line);
                            if verbose {
                                println!("{}", line);
                            }
                        }
                    }
                    StreamSource::Stderr => {
                        if verbose {
                            eprintln!("{}", line);
                        }
                        if !error_context.is_empty() {
                            error_context.push('\n');
                        }
                        error_context.push_str(&line);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child.try_wait().ok().flatten() {
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    // Drain remaining
                    while let Ok((_, line)) = rx.try_recv() {
                        if let Ok(value) = serde_json::from_str::<Value>(&line) {
                            if discovered_session.is_none() {
                                discovered_session = extract_opencode_session_id(&value);
                            }
                            let mut parts = Vec::new();
                            collect_text_parts(&value, &mut parts);
                            for part in parts {
                                rendered_answer.push_str(&part);
                            }
                        } else if !line.trim().is_empty() {
                            if !fallback_stdout.is_empty() {
                                fallback_stdout.push('\n');
                            }
                            fallback_stdout.push_str(&line);
                        }
                    }
                    if !status.success() {
                        let msg = if !error_context.is_empty() {
                            error_context
                        } else {
                            format!("brain prompt exited with {}", status)
                        };
                        return Err(msg);
                    }
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait().map_err(|e| format!("wait failed: {}", e))?;
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                if !status.success() && rendered_answer.trim().is_empty() {
                    return Err(if !error_context.is_empty() {
                        error_context
                    } else {
                        format!("brain prompt exited with {}", status)
                    });
                }
                break;
            }
        }
    }

    let answer = if !rendered_answer.trim().is_empty() {
        rendered_answer
    } else {
        fallback_stdout
    };

    if answer.trim().is_empty() {
        Err(if !error_context.is_empty() {
            error_context
        } else {
            "Brain returned empty response".to_string()
        })
    } else {
        if discovered_session.is_none() && agent_name == BRAIN_CHAT_AGENT {
            discovered_session = query_latest_chat_session_id();
        }
        Ok(BrainPromptResult {
            answer,
            session_id: discovered_session,
        })
    }
}

/// Best-effort egress check: curl to public HTTPS from inside brain should fail on internal net.
pub fn verify_no_egress() -> Result<bool, String> {
    if !container_running(BRAIN_CONTAINER_NAME) {
        return Err("Brain is not running".to_string());
    }
    let output = Command::new("docker")
        .args([
            "exec",
            BRAIN_CONTAINER_NAME,
            "sh",
            "-c",
            "wget -q -T 3 -O - https://example.com 2>/dev/null || curl -sf --max-time 3 https://example.com 2>/dev/null; echo EXIT:$?",
        ])
        .output()
        .map_err(|e| format!("Failed to probe egress: {}", e))?;

    let body = String::from_utf8_lossy(&output.stdout);
    // Success means we unexpectedly got content / exit 0 from curl
    let blocked = !body.contains("<html") && !body.contains("Example Domain");
    Ok(blocked)
}
