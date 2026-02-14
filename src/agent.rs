use crate::docker;

/// Trait for agents that can execute in containers
pub trait Agent {
    /// Initialize the agent with a container
    fn initialize(&self, container_name: &str) -> Result<(), String>;

    /// Ask a question and get a response
    fn ask(&self, container_name: &str, question: &str) -> Result<String, String>;
}

/// OpenCode agent implementation
pub struct OpenCodeAgent;

impl Agent for OpenCodeAgent {
    fn initialize(&self, container_name: &str) -> Result<(), String> {
        // Verify container exists and is running
        if !docker::container_exists(container_name) {
            return Err(format!("Container '{}' does not exist", container_name));
        }

        // Check if workspace exists inside container
        let check_cmd = "test -d /workspace && echo 'ok' || echo 'missing'";
        let result = docker::exec_in_container(container_name, check_cmd)?;

        if result.trim() == "ok" {
            Ok(())
        } else {
            Err("Workspace not found in container".to_string())
        }
    }

    fn ask(&self, container_name: &str, question: &str) -> Result<String, String> {
        // Pass question directly to agent - no parsing, just input -> output
        ask_agent(container_name, question)
    }
}

/// Ask the agent a question - direct pass-through, no parsing
/// OpenCode runs inside the container and analyzes the codebase directly
fn ask_agent(container_name: &str, question: &str) -> Result<String, String> {
    // Call OpenCode agent with question
    // OpenCode has direct access to /workspace and analyzes it directly
    call_opencode_agent(container_name, question)
}

/// Call OpenCode agent to answer the question
/// OpenCode runs inside the container and analyzes the codebase directly
fn call_opencode_agent(container_name: &str, question: &str) -> Result<String, String> {
    // Bootstrap OpenCode in the container if needed
    bootstrap_opencode(container_name)?;

    // Call OpenCode with the question
    // OpenCode has access to the full workspace at /workspace
    let answer = execute_opencode(container_name, question)?;

    Ok(answer)
}

/// Bootstrap OpenCode in the container
/// Installs/ensures OpenCode is available in the container
fn bootstrap_opencode(container_name: &str) -> Result<(), String> {
    // Check if OpenCode is already available
    let check_cmd = "which opencode || command -v opencode || echo 'not found'";
    let result = docker::exec_in_container(container_name, check_cmd)?;

    if !result.contains("not found") {
        // OpenCode is already available
        return Ok(());
    }

    // Check if npm is available (should be in node container)
    let npm_check = "which npm || command -v npm || echo 'not found'";
    let npm_available = docker::exec_in_container(container_name, npm_check)?;

    if npm_available.contains("not found") {
        return Err(
            "npm not found in container. Please use a node-based Docker image.".to_string(),
        );
    }

    // Install OpenCode via npm
    let install_cmd = "npm install -g opencode-ai 2>&1";
    let install_output = docker::exec_in_container(container_name, install_cmd)
        .map_err(|e| format!("npm install command failed: {}", e))?;

    // Check if installation shows errors (but allow warnings)
    if install_output.contains("npm ERR!") {
        return Err(format!(
            "npm install failed with errors:\n{}",
            install_output
        ));
    }

    // npm installs global packages to /usr/local/bin (in node containers)
    // Check if opencode is now available
    let check = docker::exec_in_container(
        container_name,
        "which opencode || test -f /usr/local/bin/opencode && echo 'found' || echo 'not found'",
    )?;
    if !check.contains("not found")
        && (check.contains("/usr/local/bin/opencode") || check.contains("found"))
    {
        return Ok(());
    }

    // Final check - try to run it directly
    let test_run = docker::exec_in_container(
        container_name,
        "/usr/local/bin/opencode --version 2>&1 || opencode --version 2>&1 || echo 'not found'",
    )
    .unwrap_or_default();
    if !test_run.contains("not found") && !test_run.trim().is_empty() {
        return Ok(());
    }

    // If installation failed, provide helpful error
    Err("Failed to bootstrap OpenCode in container.\n\
        npm install completed but opencode command not found.\n\
        \n\
        You may need to:\n\
        1. Ensure npm is available in the container\n\
        2. Check network connectivity from within the container\n\
        3. Verify the package name 'opencode-ai' is correct\n\
        4. Try running 'npx opencode-ai' directly"
        .to_string())
}

/// Execute OpenCode with a question
/// OpenCode analyzes the workspace at /workspace directly
fn execute_opencode(container_name: &str, question: &str) -> Result<String, String> {
    // OpenCode is installed at /usr/local/bin/opencode in node containers
    // OpenCode can analyze the codebase directly from /workspace
    // Use 'opencode run' command to execute with a message
    let cmd = format!(
        "cd /workspace && opencode run '{}'",
        question.replace("'", "'\"'\"'")
    );

    let result = docker::exec_in_container(container_name, &cmd)?;

    if result.trim().is_empty() {
        Err("OpenCode returned empty response".to_string())
    } else {
        Ok(result)
    }
}
