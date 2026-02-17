use dirs::home_dir;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Project type detected from repository files
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Rust,
    Go,
    Python,
    Node,
    Java,
    Unknown,
}

/// Sanitize a string for use in container names
/// Replaces invalid characters with underscores and limits length
pub fn sanitize_for_container_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == '/' {
                '_'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(50) // Limit length
        .collect()
}

/// Generate a unique container name for parallel execution
/// Format: smith_{command}_{sanitized_branch}_{timestamp}
pub fn generate_container_name(command: &str, branch_or_question: Option<&str>) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000_000; // Use last 9 digits for shorter names

    let sanitized_branch = branch_or_question
        .map(|s| sanitize_for_container_name(s))
        .unwrap_or_else(|| "default".to_string());

    format!("smith_{}_{}_{}", command, sanitized_branch, timestamp)
}

/// Get the host's SSH directory path
fn get_host_ssh_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join(".ssh"))
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

    // Mount SSH credentials: prefer explicit --ssh-key, otherwise use host's .ssh directory
    if let Some(key_path) = ssh_key_path {
        // Mount specific SSH key if provided (override)
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
    } else if let Some(ssh_dir) = get_host_ssh_dir() {
        // Mount host's entire .ssh directory if it exists
        if ssh_dir.exists() && ssh_dir.is_dir() {
            cmd.arg("-v")
                .arg(format!("{}:/root/.ssh_host:ro", ssh_dir.display()));
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
    _ssh_key_path: Option<&PathBuf>,
) -> Result<String, String> {
    // Clone repository (SSH setup is already done in setup_containerized_workspace)
    let clone_cmd = format!(
        "mkdir -p /workspace && cd /workspace && git clone {} .",
        repo_url
    );

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

/// Check if Docker is available and running
/// Returns Ok(()) if Docker is available and the daemon is running
pub fn check_docker_available() -> Result<(), String> {
    // First, check if docker command exists
    let version_output = Command::new("docker")
        .arg("--version")
        .output()
        .map_err(|e| format!("Docker command not found: {}", e))?;

    if !version_output.status.success() {
        return Err("Docker command failed to execute".to_string());
    }

    // Check if Docker daemon is running by trying to get Docker info
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
        .arg("name=smith_")
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

    // Check if repo URL uses SSH
    let uses_ssh = repo_url.starts_with("git@") || repo_url.starts_with("ssh://");

    // Create container
    create_container(container_name, base_image, ssh_key_path)?;

    // Start container
    start_container(container_name)?;

    // Install git if needed (try multiple package managers)
    let _ = exec_in_container(
        container_name,
        "which git || (apk add --no-cache git 2>/dev/null || apt-get update && apt-get install -y git 2>/dev/null || true)",
    );

    // Install SSH client if needed (for SSH-based git clones)
    if uses_ssh || ssh_key_path.is_some() {
        let _ = exec_in_container(
            container_name,
            "which ssh || (apk add --no-cache openssh-client 2>/dev/null || apt-get update && apt-get install -y openssh-client 2>/dev/null || true)",
        );

        // Set up SSH credentials from host or explicit key
        // Check if we have host SSH directory mounted or explicit key
        let has_explicit_key = ssh_key_path.is_some();
        let has_host_ssh = exec_in_container(
            container_name,
            "test -d /root/.ssh_host && echo 'yes' || echo 'no'",
        )
        .unwrap_or_else(|_| "no".to_string())
        .trim()
            == "yes";

        if has_explicit_key || has_host_ssh {
            // Create .ssh directory
            exec_in_container(
                container_name,
                "mkdir -p /root/.ssh && chmod 700 /root/.ssh",
            )?;

            if has_explicit_key {
                // Use explicit SSH key (mounted as id_ed25519)
                let key_check = exec_in_container(
                    container_name,
                    "test -f /root/.ssh/id_ed25519 && echo 'exists' || echo 'missing'",
                )
                .unwrap_or_else(|_| "missing".to_string());

                if !key_check.contains("exists") {
                    return Err(
                        "SSH key was not mounted properly. Expected /root/.ssh/id_ed25519 to exist."
                            .to_string(),
                    );
                }

                // Copy the mounted key to a new file with proper permissions
                exec_in_container(
                    container_name,
                    "cp /root/.ssh/id_ed25519 /root/.ssh/id_ed25519.key",
                )?;
                exec_in_container(container_name, "chmod 600 /root/.ssh/id_ed25519.key")?;

                // Copy public key if it exists
                let _ = exec_in_container(
                    container_name,
                    "test -f /root/.ssh/id_ed25519.pub && cp /root/.ssh/id_ed25519.pub /root/.ssh/id_ed25519.key.pub && chmod 644 /root/.ssh/id_ed25519.key.pub || true",
                ).ok();

                // Update SSH config to include the identity file
                let ssh_config = "Host github.com\n  StrictHostKeyChecking accept-new\n  UserKnownHostsFile /root/.ssh/known_hosts\n  IdentityFile /root/.ssh/id_ed25519.key\n";
                exec_in_container(
                    container_name,
                    &format!(
                        "echo '{}' > /root/.ssh/config && chmod 600 /root/.ssh/config",
                        ssh_config.replace("'", "'\"'\"'")
                    ),
                )?;
            } else if has_host_ssh {
                // Copy host SSH directory contents to container .ssh
                // Find and copy SSH keys from host directory
                exec_in_container(
                    container_name,
                    "cp -r /root/.ssh_host/* /root/.ssh/ 2>/dev/null || true",
                )?;

                // Fix permissions on copied files
                exec_in_container(
                    container_name,
                    "chmod 700 /root/.ssh && chmod 600 /root/.ssh/* 2>/dev/null || true",
                )?;
                exec_in_container(
                    container_name,
                    "chmod 644 /root/.ssh/*.pub /root/.ssh/known_hosts /root/.ssh/config 2>/dev/null || true",
                )?;

                // Ensure SSH config exists with proper settings
                let ssh_config_check = exec_in_container(
                    container_name,
                    "test -f /root/.ssh/config && echo 'exists' || echo 'missing'",
                )
                .unwrap_or_else(|_| "missing".to_string());

                if !ssh_config_check.contains("exists") {
                    // Create basic SSH config if host doesn't have one
                    let ssh_config = "Host github.com\n  StrictHostKeyChecking accept-new\n";
                    exec_in_container(
                        container_name,
                        &format!(
                            "echo '{}' > /root/.ssh/config && chmod 600 /root/.ssh/config",
                            ssh_config.replace("'", "'\"'\"'")
                        ),
                    )?;
                }
            }

            // Add GitHub to known_hosts if not already present
            let _ = exec_in_container(
                container_name,
                "ssh-keyscan github.com >> /root/.ssh/known_hosts 2>/dev/null || true",
            );
        } else if uses_ssh {
            // SSH URL but no credentials - create basic setup (will fail on clone, but setup is clean)
            exec_in_container(
                container_name,
                "mkdir -p /root/.ssh && chmod 700 /root/.ssh",
            )?;
            let ssh_config = "Host github.com\n  StrictHostKeyChecking accept-new\n  UserKnownHostsFile /root/.ssh/known_hosts\n";
            exec_in_container(
                container_name,
                &format!(
                    "echo '{}' > /root/.ssh/config && chmod 600 /root/.ssh/config",
                    ssh_config.replace("'", "'\"'\"'")
                ),
            )?;
        }
    }

    // Clone repository inside container
    // SSH key is required (validated in main.rs)
    clone_in_container(container_name, repo_url, ssh_key_path)?;

    // Ensure Node.js is available for OpenCode
    ensure_nodejs_available(container_name)?;

    Ok(container_name.to_string())
}

/// Detect project type by checking for key files in the repository
/// This is done after cloning, so we check inside the container
pub fn detect_project_type(container_name: &str) -> ProjectType {
    // Check for key files that indicate project type
    let checks = vec![
        ("test -f /workspace/Cargo.toml", ProjectType::Rust),
        ("test -f /workspace/go.mod", ProjectType::Go),
        ("test -f /workspace/package.json", ProjectType::Node),
        (
            "test -f /workspace/requirements.txt || test -f /workspace/pyproject.toml || test -f /workspace/setup.py",
            ProjectType::Python,
        ),
        (
            "test -f /workspace/pom.xml || test -f /workspace/build.gradle",
            ProjectType::Java,
        ),
    ];

    for (check_cmd, project_type) in checks {
        match exec_in_container(container_name, check_cmd) {
            Ok(output) => {
                // If command succeeds (exit code 0), the file exists
                if output.trim().is_empty() || !output.contains("not found") {
                    return project_type;
                }
            }
            Err(_) => {
                // Command failed, file doesn't exist, continue checking
                continue;
            }
        }
    }

    ProjectType::Unknown
}

/// Install Node.js and npm in a container at runtime
/// Detects the package manager and installs accordingly
pub fn ensure_nodejs_available(container_name: &str) -> Result<(), String> {
    // Check if Node.js is already available
    let check_cmd = "which node || command -v node || echo 'not found'";
    let check_result = exec_in_container(container_name, check_cmd)?;

    if !check_result.contains("not found") {
        // Node.js is already available
        return Ok(());
    }

    println!("  Installing Node.js in container...");

    // Detect package manager and install Node.js
    // Try Alpine (apk) first (most common for official images)
    let apk_cmd = "apk add --no-cache nodejs npm 2>&1";
    let apk_result = exec_in_container(container_name, apk_cmd);
    if apk_result.is_ok() {
        // Verify installation
        let verify = exec_in_container(container_name, "node --version 2>&1")?;
        if !verify.trim().is_empty() {
            println!("    ✓ Node.js installed via apk");
            return Ok(());
        }
    }

    // Try Debian/Ubuntu (apt)
    let apt_cmd = "apt-get update && apt-get install -y nodejs npm 2>&1";
    let apt_result = exec_in_container(container_name, apt_cmd);
    if apt_result.is_ok() {
        let verify = exec_in_container(container_name, "node --version 2>&1")?;
        if !verify.trim().is_empty() {
            println!("    ✓ Node.js installed via apt");
            return Ok(());
        }
    }

    // Try using nvm or other methods as last resort
    // For now, return error if we can't install
    Err("Failed to install Node.js. Container must have apk or apt package manager.".to_string())
}

/// Initialize repository by installing dependencies based on project type
/// This is called during Agent::initialize() to ensure dependencies are ready
pub fn initialize_repository(container_name: &str) -> Result<(), String> {
    println!("  Initializing repository dependencies...");

    // Detect project type
    let project_type = detect_project_type(container_name);
    println!("    Detected project type: {:?}", project_type);

    // Install dependencies based on type
    match project_type {
        ProjectType::Rust => {
            println!("    Installing Rust dependencies...");
            // Cargo will download dependencies on first build
            // Just verify cargo is available
            let cargo_check = exec_in_container(
                container_name,
                "which cargo || command -v cargo || echo 'not found'",
            )?;
            if cargo_check.contains("not found") {
                return Err("Cargo not found in container. Please use a Rust-based Docker image (e.g., rust:1.75-alpine).".to_string());
            }
            // Try a dry-run build to fetch dependencies
            let _ = exec_in_container(
                container_name,
                "cd /workspace && cargo check --message-format=short 2>&1 | head -20 || true",
            );
            println!("    ✓ Rust dependencies ready");
        }
        ProjectType::Node => {
            println!("    Installing Node.js dependencies...");
            // Check if package.json exists
            let has_package_json = exec_in_container(
                container_name,
                "test -f /workspace/package.json && echo 'yes' || echo 'no'",
            )?;
            if has_package_json.contains("yes") {
                // Try npm install
                let npm_result =
                    exec_in_container(container_name, "cd /workspace && npm install 2>&1");
                if npm_result.is_ok() {
                    println!("    ✓ Node.js dependencies installed");
                } else {
                    // Try yarn if npm fails
                    let yarn_result =
                        exec_in_container(container_name, "cd /workspace && yarn install 2>&1");
                    if yarn_result.is_ok() {
                        println!("    ✓ Node.js dependencies installed (via yarn)");
                    } else {
                        println!("    ⚠ Could not install Node.js dependencies (npm/yarn not available or failed)");
                    }
                }
            } else {
                println!("    ⚠ No package.json found, skipping dependency installation");
            }
        }
        ProjectType::Go => {
            println!("    Installing Go dependencies...");
            let go_check = exec_in_container(
                container_name,
                "which go || command -v go || echo 'not found'",
            )?;
            if go_check.contains("not found") {
                return Err("Go not found in container. Please use a Go-based Docker image (e.g., golang:1.21-alpine).".to_string());
            }
            // Download Go modules
            let _ = exec_in_container(
                container_name,
                "cd /workspace && go mod download 2>&1 || true",
            );
            println!("    ✓ Go dependencies ready");
        }
        ProjectType::Python => {
            println!("    Installing Python dependencies...");
            let python_check = exec_in_container(container_name, "which python3 || which python || command -v python3 || command -v python || echo 'not found'")?;
            if python_check.contains("not found") {
                return Err("Python not found in container. Please use a Python-based Docker image (e.g., python:3.11-alpine).".to_string());
            }
            // Try pip install
            let pip_result = exec_in_container(container_name, "cd /workspace && (pip install -r requirements.txt 2>&1 || pip3 install -r requirements.txt 2>&1 || true)");
            if pip_result.is_ok() {
                println!("    ✓ Python dependencies installed");
            } else {
                println!(
                    "    ⚠ Could not install Python dependencies (requirements.txt may not exist)"
                );
            }
        }
        ProjectType::Java => {
            println!("    Installing Java dependencies...");
            let java_check = exec_in_container(
                container_name,
                "which javac || command -v javac || echo 'not found'",
            )?;
            if java_check.contains("not found") {
                return Err("Java not found in container. Please use a Java-based Docker image (e.g., eclipse-temurin:21-jdk-alpine).".to_string());
            }
            // Maven or Gradle will download dependencies on first build
            println!("    ✓ Java dependencies ready (will be downloaded on first build)");
        }
        ProjectType::Unknown => {
            println!("    ⚠ Unknown project type, skipping dependency installation");
        }
    }

    // Validate that key tools are available
    validate_dependencies_installed(container_name, &project_type)?;

    Ok(())
}

/// Validate that dependencies are properly installed
fn validate_dependencies_installed(
    container_name: &str,
    project_type: &ProjectType,
) -> Result<(), String> {
    match project_type {
        ProjectType::Rust => {
            let cargo_version = exec_in_container(container_name, "cargo --version 2>&1")?;
            if cargo_version.trim().is_empty() {
                return Err("Cargo validation failed".to_string());
            }
        }
        ProjectType::Node => {
            let npm_version = exec_in_container(container_name, "npm --version 2>&1")?;
            if npm_version.trim().is_empty() {
                return Err("npm validation failed".to_string());
            }
        }
        ProjectType::Go => {
            let go_version = exec_in_container(container_name, "go version 2>&1")?;
            if go_version.trim().is_empty() {
                return Err("Go validation failed".to_string());
            }
        }
        ProjectType::Python => {
            let python_version = exec_in_container(
                container_name,
                "python3 --version 2>&1 || python --version 2>&1",
            )?;
            if python_version.trim().is_empty() {
                return Err("Python validation failed".to_string());
            }
        }
        ProjectType::Java => {
            let java_version = exec_in_container(container_name, "javac -version 2>&1")?;
            if java_version.trim().is_empty() {
                return Err("Java validation failed".to_string());
            }
        }
        ProjectType::Unknown => {
            // Skip validation for unknown types
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_docker_command_building() {
        // Test would require Docker to be running
        // Integration test would be needed
    }
}
