//! Sealed brain container runtime: internal Docker network + local Ollama only.

use super::*;
use std::path::Path;

/// Internal Docker network shared by smith-brain and smith-ollama (no external egress).
pub const BRAIN_NETWORK: &str = "smith-brain-net";

/// Sealed brain container name.
pub const BRAIN_CONTAINER_NAME: &str = "smith-brain";

/// Memory mount point inside the brain container.
pub const BRAIN_MEMORY_MOUNT: &str = "/memory";

/// Default OpenCode image for the sealed brain (local inference via Ollama).
pub const BRAIN_IMAGE: &str = "ghcr.io/anomalyco/opencode";

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
    let create = Command::new("docker")
        .args(["network", "create", "--internal", BRAIN_NETWORK])
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
            "Container '{}' not found; start Ollama with `smith model start` first",
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

/// Ensure Ollama is running and attached to the sealed brain network.
pub fn ensure_ollama_on_brain_network() -> Result<(), String> {
    if !is_ollama_running() {
        return Err(
            "Ollama is not running. Start a local model first: `smith model start` (local profile)"
                .to_string(),
        );
    }
    connect_to_brain_network(OLLAMA_CONTAINER_NAME)
}

/// Start (or reuse) the sealed brain container with host memory bind-mounted.
pub fn start_brain_container(memory_host_path: &Path, model: Option<&str>) -> Result<(), String> {
    ensure_brain_network()?;
    ensure_ollama_on_brain_network()?;

    if container_running(BRAIN_CONTAINER_NAME) {
        // Ensure still on network
        connect_to_brain_network(BRAIN_CONTAINER_NAME)?;
        return Ok(());
    }

    if container_exists(BRAIN_CONTAINER_NAME) {
        // Start existing then attach network
        let start = Command::new("docker")
            .args(["start", BRAIN_CONTAINER_NAME])
            .output()
            .map_err(|e| format!("Failed to start brain container: {}", e))?;
        if start.status.success() {
            connect_to_brain_network(BRAIN_CONTAINER_NAME)?;
            return Ok(());
        }
        // Recreate if start failed
        let _ = Command::new("docker")
            .args(["rm", "-f", BRAIN_CONTAINER_NAME])
            .output();
    }

    let mem = memory_host_path
        .canonicalize()
        .unwrap_or_else(|_| memory_host_path.to_path_buf());
    let mem_str = mem.to_string_lossy().replace('\\', "/");

    let model_name = model.unwrap_or(crate::DEFAULT_LOCAL_MODEL);

    let args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--name".to_string(),
        BRAIN_CONTAINER_NAME.to_string(),
        "--network".to_string(),
        BRAIN_NETWORK.to_string(),
        "-v".to_string(),
        format!("{}:{}:rw", mem_str, BRAIN_MEMORY_MOUNT),
        "-e".to_string(),
        format!(
            "OPENCODE_BASE_URL=http://{}:{}",
            OLLAMA_CONTAINER_NAME, OLLAMA_PORT
        ),
        "-e".to_string(),
        "OPENCODE_PROVIDER=ollama".to_string(),
        "-e".to_string(),
        format!("OPENCODE_MODEL={}", model_name),
        "--entrypoint".to_string(),
        "sh".to_string(),
        BRAIN_IMAGE.to_string(),
        "-c".to_string(),
        "while true; do sleep 3600; done".to_string(),
    ];

    // Do NOT mount host opencode config (may contain cloud keys).
    // Do NOT publish ports. Do NOT pass cloud API keys.

    println!(
        "  Starting sealed brain container '{}'...",
        BRAIN_CONTAINER_NAME
    );
    let run = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start brain container: {}", e))?;

    if run.status.success() {
        Ok(())
    } else {
        // Conflict: remove and retry once
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

/// Status snapshot for `smith brain status`.
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
    let ollama_on_network = if ollama_running {
        Command::new("docker")
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
            .unwrap_or(false)
    } else {
        false
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

/// Run a single prompt inside the sealed brain; returns rendered text answer.
/// Process ends when opencode exits (kill-between-steps).
pub fn run_prompt_in_brain(
    prompt: &str,
    model: Option<&str>,
    verbose: bool,
) -> Result<String, String> {
    if !container_running(BRAIN_CONTAINER_NAME) {
        return Err(
            "Sealed brain is not running. Use `smith chat` or `smith brain status`.".to_string(),
        );
    }

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
    ];

    if let Some(m) = model {
        if !m.trim().is_empty() {
            args.push("-m".to_string());
            args.push(m.to_string());
        }
    }
    if verbose {
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
        Ok(answer)
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
