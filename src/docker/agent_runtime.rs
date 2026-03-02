use super::*;

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
    pub container_id: String,
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
            "{{.Names}}|{{.ID}}|{{.Status}}|{{.Image}}",
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
        if parts.len() < 4 {
            continue;
        }
        let container_name = parts[0].trim();
        let container_id = parts[1].trim();
        let status = parts[2].trim();
        let image = parts[3].trim();

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
                    container_id: container_id.to_string(),
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

/// Ensure a directory exists in a spawned container.
pub fn ensure_spawn_dir(project: &str, branch: &str, dir_path: &str) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    let command = format!("mkdir -p '{}'", dir_path.replace('\'', "'\"'\"'"));
    let output = Command::new("docker")
        .args(["exec", &name, "sh", "-lc", &command])
        .output()
        .map_err(|e| format!("Failed to ensure '{}' in container: {}", dir_path, e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Failed to ensure '{}' in container '{}': {}",
            dir_path,
            name,
            stderr.trim()
        ))
    }
}

/// Remove a directory tree in a spawned container.
pub fn remove_spawn_dir(project: &str, branch: &str, dir_path: &str) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    let command = format!("rm -rf '{}'", dir_path.replace('\'', "'\"'\"'"));
    let output = Command::new("docker")
        .args(["exec", &name, "sh", "-lc", &command])
        .output()
        .map_err(|e| format!("Failed to remove '{}' in container: {}", dir_path, e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Failed to remove '{}' in container '{}': {}",
            dir_path,
            name,
            stderr.trim()
        ))
    }
}

/// Ensure the shared planning state directory exists in a spawned container.
pub fn ensure_spawn_state_dir(project: &str, branch: &str) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    let command = "mkdir -p /workspace; if [ -L /state ]; then target=$(readlink /state || true); if [ \"$target\" = \"/workspace/state\" ]; then mkdir -p /workspace/state /state; cp -a /workspace/state/. /state/ 2>/dev/null || true; rm -f /state; mkdir -p /state; fi; fi; mkdir -p /state";
    let output = Command::new("docker")
        .args(["exec", &name, "sh", "-lc", command])
        .output()
        .map_err(|e| format!("Failed to initialize /state in container: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Failed to initialize /state in container '{}': {}",
            name,
            stderr.trim()
        ))
    }
}

/// Return true when a file exists in the spawned container.
pub fn spawn_file_exists(project: &str, branch: &str, file_path: &str) -> Result<bool, String> {
    let name = spawn_container_name(project, branch);
    let command = format!("test -f '{}'", file_path.replace('\'', "'\"'\"'"));
    let output = Command::new("docker")
        .args(["exec", &name, "sh", "-lc", &command])
        .output()
        .map_err(|e| format!("Failed checking file in container: {}", e))?;

    Ok(output.status.success())
}

/// List plan run directories under /state in a spawned container.
pub fn list_spawn_plan_dirs(project: &str, branch: &str) -> Result<Vec<String>, String> {
    let name = spawn_container_name(project, branch);
    let output = Command::new("docker")
        .args([
            "exec",
            &name,
            "sh",
            "-lc",
            "for d in /state/plan-*; do [ -d \"$d\" ] && basename \"$d\"; done; true",
        ])
        .output()
        .map_err(|e| format!("Failed listing plan directories: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed listing plan directories: {}",
            stderr.trim()
        ));
    }

    let mut dirs = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            dirs.push(trimmed.to_string());
        }
    }
    Ok(dirs)
}

/// Read a UTF-8 file from a spawned container.
pub fn read_spawn_file(project: &str, branch: &str, file_path: &str) -> Result<String, String> {
    let name = spawn_container_name(project, branch);
    let command = format!("cat '{}'", file_path.replace('\'', "'\"'\"'"));
    let output = Command::new("docker")
        .args(["exec", &name, "sh", "-lc", &command])
        .output()
        .map_err(|e| format!("Failed reading '{}' in container: {}", file_path, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed reading '{}' in container: {}",
            file_path,
            stderr.trim()
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 while reading '{}': {}", file_path, e))
}

/// Write UTF-8 content into a file in the spawned container using docker cp.
pub fn write_spawn_file(
    project: &str,
    branch: &str,
    file_path: &str,
    content: &str,
) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    let parent = file_path
        .rsplit_once('/')
        .map(|(p, _)| p)
        .filter(|p| !p.is_empty())
        .unwrap_or("/");
    ensure_spawn_dir(project, branch, parent)?;

    let mut tmp_path = std::env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    tmp_path.push(format!(
        "smith-spawn-write-{}-{}.tmp",
        std::process::id(),
        now
    ));

    fs::write(&tmp_path, content)
        .map_err(|e| format!("Failed writing temp file '{}': {}", tmp_path.display(), e))?;

    let destination = format!("{}:{}", name, file_path);
    let copy_result = Command::new("docker")
        .arg("cp")
        .arg(tmp_path.as_os_str())
        .arg(&destination)
        .output();

    let _ = fs::remove_file(&tmp_path);

    let output = copy_result.map_err(|e| format!("Failed to copy file into container: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Failed writing '{}' in container '{}': {}",
            file_path,
            name,
            stderr.trim()
        ))
    }
}

/// Execute a shell command in a spawned container and return stdout.
pub fn run_spawn_shell(project: &str, branch: &str, script: &str) -> Result<String, String> {
    let name = spawn_container_name(project, branch);
    let output = Command::new("docker")
        .args(["exec", &name, "sh", "-lc", script])
        .output()
        .map_err(|e| format!("Failed running command in spawned container: {}", e))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 output from container command: {}", e))
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut details = String::new();
        if !stdout.trim().is_empty() {
            details.push_str(stdout.trim());
        }
        if !stderr.trim().is_empty() {
            if !details.is_empty() {
                details.push('\n');
            }
            details.push_str(stderr.trim());
        }
        if details.is_empty() {
            details = "unknown error".to_string();
        }
        Err(format!(
            "Container command failed in '{}': {}",
            name, details
        ))
    }
}

/// Run a prompt in a spawned container and stream raw output to the terminal.
pub fn run_prompt_in_spawned_container(
    project: &str,
    branch: &str,
    prompt: &str,
    verbose: bool,
) -> Result<(), String> {
    run_prompt_in_spawned_container_with_options(project, branch, prompt, verbose, None, None)
}

/// Run a prompt in a spawned container with optional model and prompt-prefix overrides.
pub fn run_prompt_in_spawned_container_with_options(
    project: &str,
    branch: &str,
    prompt: &str,
    verbose: bool,
    model: Option<&str>,
    prompt_prefix: Option<&str>,
) -> Result<(), String> {
    let name = spawn_container_name(project, branch);
    let mut args = vec![
        "exec".to_string(),
        "-w".to_string(),
        "/".to_string(),
        name,
        "opencode".to_string(),
        "run".to_string(),
        "--dir".to_string(),
        "/".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--print-logs".to_string(),
    ];
    if let Some(model) = model {
        if !model.trim().is_empty() {
            args.push("-m".to_string());
            args.push(model.to_string());
        }
    }
    if let Some(prefix) = prompt_prefix {
        if !prefix.trim().is_empty() {
            args.push("--prompt".to_string());
            args.push(prefix.to_string());
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
#[allow(clippy::too_many_arguments)]
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

# Create state/workspace directories and prepare repo (idempotent on container restart)
mkdir -p /workspace
if [ -L /state ]; then
    target=$(readlink /state || true)
    if [ "$target" = "/workspace/state" ]; then
        mkdir -p /workspace/state /state
        cp -a /workspace/state/. /state/ 2>/dev/null || true
        rm -f /state
    fi
fi
mkdir -p /state

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
    if let Ok(ssh_socket) = std::env::var("SSH_AUTH_SOCK") {
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
