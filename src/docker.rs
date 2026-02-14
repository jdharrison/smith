use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique container name for parallel execution
pub fn generate_container_name(project_name: Option<&str>) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();

    if let Some(project) = project_name {
        format!("smith-{}-{}-{}", project, pid, timestamp)
    } else {
        format!("smith-{}-{}", pid, timestamp)
    }
}

/// Create a long-lived container with workspace inside
///
/// Creates a container that will have the repository cloned inside it.
/// The workspace exists inside the container, not on the host.
pub fn create_container(
    container_name: &str,
    image: &str,
    ssh_key_path: Option<&PathBuf>,
) -> Result<(), String> {
    let mut cmd = Command::new("docker");
    cmd.arg("create");
    cmd.arg("--name").arg(container_name);

    // Mount SSH key if provided
    if let Some(key_path) = ssh_key_path {
        // Mount private key
        cmd.arg("-v")
            .arg(format!("{}:/root/.ssh/id_ed25519:ro", key_path.display()));

        // Try to mount public key if it exists
        let pub_key = key_path.with_extension("pub");
        if pub_key.exists() {
            cmd.arg("-v").arg(format!(
                "{}:/root/.ssh/id_ed25519.pub:ro",
                pub_key.display()
            ));
        }
    }

    // Set working directory inside container
    cmd.arg("-w").arg("/workspace");

    // Override entrypoint to use shell
    cmd.arg("--entrypoint").arg("/bin/sh");

    // Keep container running
    cmd.arg(image);
    cmd.arg("-c").arg("while true; do sleep 3600; done");

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to create container: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to create container: {}", error))
    }
}

/// Start a container
pub fn start_container(container_name: &str) -> Result<(), String> {
    let output = Command::new("docker")
        .arg("start")
        .arg(container_name)
        .output()
        .map_err(|e| format!("Failed to start container: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to start container: {}", error))
    }
}

/// Clone a repository inside a container
pub fn clone_in_container(
    container_name: &str,
    repo_url: &str,
    ssh_key_path: Option<&PathBuf>,
) -> Result<String, String> {
    // Setup SSH key permissions if needed, then clone
    let clone_cmd = if ssh_key_path.is_some() {
        format!(
            "mkdir -p /workspace && cd /workspace && \
             chmod 600 /root/.ssh/id_ed25519 2>/dev/null; \
             chmod 644 /root/.ssh/id_ed25519.pub 2>/dev/null; \
             git clone {} .",
            repo_url
        )
    } else {
        format!(
            "mkdir -p /workspace && cd /workspace && \
             git clone {} .",
            repo_url
        )
    };

    exec_in_container(container_name, &clone_cmd)
}

/// Execute a command inside a running container
pub fn exec_in_container(container_name: &str, command: &str) -> Result<String, String> {
    let output = Command::new("docker")
        .arg("exec")
        .arg(container_name)
        .arg("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| format!("Failed to exec in container: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = if !stderr.trim().is_empty() {
            stderr.to_string()
        } else if !stdout.trim().is_empty() {
            stdout.to_string()
        } else {
            format!(
                "Command failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            )
        };
        Err(format!("Command failed in container: {}", error_msg))
    }
}

/// Stop a container
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

/// Remove a container
pub fn remove_container(container_name: &str) -> Result<(), String> {
    let output = Command::new("docker")
        .arg("rm")
        .arg("-f")
        .arg(container_name)
        .output()
        .map_err(|e| format!("Failed to remove container: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to remove container: {}", error))
    }
}

/// Check if a container exists
pub fn container_exists(container_name: &str) -> bool {
    Command::new("docker")
        .arg("ps")
        .arg("-a")
        .arg("--filter")
        .arg(format!("name={}", container_name))
        .arg("--format")
        .arg("{{.Names}}")
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.trim() == container_name)
        })
        .unwrap_or(false)
}

/// List all smith containers
pub fn list_containers() -> Result<Vec<String>, String> {
    let output = Command::new("docker")
        .arg("ps")
        .arg("-a")
        .arg("--filter")
        .arg("name=smith-")
        .arg("--format")
        .arg("{{.Names}}")
        .output()
        .map_err(|e| format!("Failed to list containers: {}", e))?;

    if output.status.success() {
        let containers: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(containers)
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to list containers: {}", error))
    }
}

/// Create, start, and clone repository in a container
/// Returns the container name for further operations
pub fn setup_containerized_workspace(
    container_name: &str,
    repo_url: &str,
    ssh_key_path: Option<&PathBuf>,
    image: Option<&str>,
) -> Result<String, String> {
    // Use node image by default (has npm and git)
    let base_image = image.unwrap_or("node:20-alpine");

    // Create container
    create_container(container_name, base_image, ssh_key_path)?;

    // Start container
    start_container(container_name)?;

    // Install git in node container if needed (node images usually have git)
    let _ = exec_in_container(
        container_name,
        "which git || apk add --no-cache git 2>/dev/null",
    );

    // Clone repository inside container
    clone_in_container(container_name, repo_url, ssh_key_path)?;

    Ok(container_name.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_docker_command_building() {
        // Test would require Docker to be running
        // Integration test would be needed
    }
}
