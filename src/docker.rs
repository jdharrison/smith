//! Minimal Docker helpers for Doctor (check_docker_available), agent start/stop, and container list/stop/remove.
//! Ask/dev/review use the Dagger pipeline; these are for debugging and environment checks.

use std::process::Command;

/// Prefix for agent containers: "smith-agent-<name>". Used by agent start/stop/list.
pub const AGENT_CONTAINER_PREFIX: &str = "smith-agent-";

/// Default port for OpenCode server. Additional agents use 4097, 4098, ...
pub const OPENCODE_SERVER_PORT: u16 = 4096;

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

/// Start an agent container running OpenCode in server mode. Exposes `port` on the host.
/// Uses `--entrypoint opencode` so the container runs exactly `opencode serve --hostname 0.0.0.0 --port N`.
/// If a container with this name already exists, tries to start it (e.g. after stop).
pub fn start_agent_container(agent_name: &str, image: &str, port: u16) -> Result<(), String> {
    let name = agent_container_name(agent_name);
    let port_str = port.to_string();
    let run = Command::new("docker")
        .args([
            "run",
            "-d",
            "--restart",
            "unless-stopped",
            "--name",
            &name,
            "-p",
            &format!("{}:{}", port, port),
            "--entrypoint",
            "opencode",
            image,
            "serve",
            "--hostname",
            "0.0.0.0",
            "--port",
            &port_str,
        ])
        .output()
        .map_err(|e| format!("Failed to run container: {}", e))?;
    if run.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&run.stderr);
    if stderr.contains("already in use") || stderr.contains("Conflict") {
        // Existing container may be from an old image or wrong port mapping. Remove and recreate.
        let _ = Command::new("docker").args(["rm", "-f", &name]).output();
        let run2 = Command::new("docker")
            .args([
                "run",
                "-d",
                "--restart",
                "unless-stopped",
                "--name",
                &name,
                "-p",
                &format!("{}:{}", port, port),
                "--entrypoint",
                "opencode",
                image,
                "serve",
                "--hostname",
                "0.0.0.0",
                "--port",
                &port_str,
            ])
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
