use std::path::PathBuf;
use std::process::Command;

/// Run a Docker container to clone a repository
/// 
/// If `ssh_key_path` is provided, mounts the SSH key into the container.
/// Otherwise, assumes public repository.
pub fn run_container(
    image: &str,
    repo_url: &str,
    workdir: &PathBuf,
    ssh_key_path: Option<&PathBuf>,
    keep_alive: bool,
) -> Result<String, String> {
    let container_name = format!("smith-{}", std::process::id());
    
    let mut cmd = Command::new("docker");
    cmd.arg("run");
    cmd.arg("--rm");
    cmd.arg("--name").arg(&container_name);

    // Mount SSH key if provided
    if let Some(key_path) = ssh_key_path {
        // Mount private key
        cmd.arg("-v").arg(format!(
            "{}:/root/.ssh/id_ed25519:ro",
            key_path.display()
        ));
        
        // Try to mount public key if it exists
        let pub_key = key_path.with_extension("pub");
        if pub_key.exists() {
            cmd.arg("-v").arg(format!(
                "{}:/root/.ssh/id_ed25519.pub:ro",
                pub_key.display()
            ));
        }
    }

    // Mount workspace
    cmd.arg("-v").arg(format!("{}:/workspace", workdir.display()));
    cmd.arg("-w").arg("/workspace");

    // Override entrypoint to use shell (alpine/git has git as entrypoint)
    cmd.arg("--entrypoint").arg("/bin/sh");

    // Add image
    cmd.arg(image);

    // Build command to run in container
    if keep_alive {
        // Keep container alive for interactive use
        cmd.arg("-c").arg("while true; do sleep 3600; done");
    } else {
        // Clone repository
        let clone_cmd = if ssh_key_path.is_some() {
            // Setup SSH key permissions and clone
            format!(
                "chmod 600 /root/.ssh/id_ed25519 2>/dev/null; \
                 git clone {} .",
                repo_url
            )
        } else {
            // Just clone (public repo)
            format!("git clone {} .", repo_url)
        };
        cmd.arg("-c").arg(clone_cmd);
    }

    // Execute
    let output = cmd.output()
        .map_err(|e| format!("Failed to execute docker run: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("Docker run failed: {}", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_command_building() {
        // Test would require Docker to be running
        // Integration test would be needed
    }
}
