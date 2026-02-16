use crate::docker;
use serde::{Deserialize, Serialize};

/// Trait for agents that can execute in containers
pub trait Agent {
    /// Initialize the agent with a container
    fn initialize(&self, container_name: &str) -> Result<(), String>;

    /// Ask a question and get a response
    fn ask(&self, container_name: &str, question: &str) -> Result<String, String>;

    /// Checkout a branch
    fn checkout_branch(&self, container_name: &str, branch: &str) -> Result<(), String>;

    /// Execute a development action (read/write) with validation and commit
    fn dev(
        &self,
        container_name: &str,
        task: &str,
        branch: &str,
        base: Option<&str>,
    ) -> Result<String, String>;

    /// Review changes in a feature branch
    fn review(
        &self,
        container_name: &str,
        branch: &str,
        base: Option<&str>,
    ) -> Result<String, String>;
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

        if result.trim() != "ok" {
            return Err("Workspace not found in container".to_string());
        }

        // Initialize repository: install dependencies and validate
        docker::initialize_repository(container_name)?;

        Ok(())
    }

    fn ask(&self, container_name: &str, question: &str) -> Result<String, String> {
        // Pass question directly to agent - no parsing, just input -> output
        ask_agent(container_name, question)
    }

    fn checkout_branch(&self, container_name: &str, branch: &str) -> Result<(), String> {
        checkout_branch_simple(container_name, branch)
    }

    fn dev(
        &self,
        container_name: &str,
        task: &str,
        branch: &str,
        base: Option<&str>,
    ) -> Result<String, String> {
        // Execute development action with validation and commit
        dev_agent(container_name, task, branch, base)
    }

    fn review(
        &self,
        container_name: &str,
        branch: &str,
        base: Option<&str>,
    ) -> Result<String, String> {
        review_branch(container_name, branch, base)
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
    // Ensure Node.js is available first (installs if missing)
    docker::ensure_nodejs_available(container_name)?;

    // Check if OpenCode is already available
    let check_cmd = "which opencode || command -v opencode || echo 'not found'";
    let result = docker::exec_in_container(container_name, check_cmd)?;

    if !result.contains("not found") {
        // OpenCode is already available
        return Ok(());
    }

    // Check if npm is available (should be available after ensure_nodejs_available)
    let npm_check = "which npm || command -v npm || echo 'not found'";
    let npm_available = docker::exec_in_container(container_name, npm_check)?;

    if npm_available.contains("not found") {
        return Err(
            "npm not found in container after Node.js installation. This should not happen."
                .to_string(),
        );
    }

    // Check if npx is available (npx doesn't require global install)
    let npx_check = "which npx || command -v npx || echo 'not found'";
    let npx_available = docker::exec_in_container(container_name, npx_check)?;

    if !npx_available.contains("not found") {
        // npx is available, we can use it directly without installing
        // Test that npx can access opencode-ai
        let test_npx = docker::exec_in_container(
            container_name,
            "npx -y opencode-ai --version 2>&1 | head -1 || echo 'test failed'",
        )
        .unwrap_or_else(|_| "test failed".to_string());

        if !test_npx.contains("test failed") && !test_npx.trim().is_empty() {
            return Ok(());
        }
    }

    // Fallback: Try installing OpenCode via npm (optional, npx should work)
    let install_cmd = "npm install -g opencode-ai 2>&1";
    let install_output = docker::exec_in_container(container_name, install_cmd).unwrap_or_default();

    // Check if installation shows errors (but allow warnings)
    if install_output.contains("npm ERR!") {
        // Installation failed, but npx might still work
        println!("  ⚠ Global npm install failed, will try npx instead");
    }

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

    // If all else fails, npx should still work (it downloads on demand)
    // So we'll allow this to proceed and let execute_opencode handle it
    Ok(())
}

/// Execute OpenCode with a question
/// OpenCode analyzes the workspace at /workspace directly
fn execute_opencode(container_name: &str, question: &str) -> Result<String, String> {
    // Try multiple ways to run OpenCode:
    // 1. Try npx opencode-ai (most reliable)
    // 2. Try opencode command if available
    // 3. Try /usr/local/bin/opencode

    let escaped_question = question.replace("'", "'\"'\"'");

    // Try npx first (most reliable, doesn't require global install)
    let npx_cmd = format!(
        "cd /workspace && timeout 300 npx -y opencode-ai run '{}' 2>&1",
        escaped_question
    );

    let npx_result = docker::exec_in_container(container_name, &npx_cmd);
    match npx_result {
        Ok(result) => {
            let trimmed = result.trim();
            if !trimmed.is_empty() {
                return Ok(result);
            }
            // Empty result from npx - log it but continue to try other methods
            println!("    ⚠ npx opencode-ai returned empty response");
        }
        Err(e) => {
            // npx failed - log but continue to try other methods
            println!("    ⚠ npx opencode-ai failed: {}", e);
        }
    }

    // Try opencode command if available
    let opencode_cmd = format!(
        "cd /workspace && timeout 300 (opencode run '{}' 2>&1 || /usr/local/bin/opencode run '{}' 2>&1)",
        escaped_question, escaped_question
    );

    let result = docker::exec_in_container(container_name, &opencode_cmd)?;

    let trimmed_result = result.trim();
    if trimmed_result.is_empty() {
        // Get detailed debug info
        let debug_commands = vec![
            ("npx check", "which npx || echo 'npx not found'"),
            (
                "opencode check",
                "which opencode || echo 'opencode not found'",
            ),
            ("npm check", "which npm || echo 'npm not found'"),
            ("workspace check", "ls -la /workspace | head -5"),
            (
                "opencode test",
                "npx -y opencode-ai --version 2>&1 || echo 'opencode test failed'",
            ),
        ];

        let mut debug_info = Vec::new();
        for (name, cmd) in debug_commands {
            let output = docker::exec_in_container(container_name, cmd)
                .unwrap_or_else(|_| format!("{}: command failed", name));
            debug_info.push(format!("{}: {}", name, output.trim()));
        }

        Err(format!(
            "OpenCode returned empty response.\nDebug info:\n{}",
            debug_info.join("\n")
        ))
    } else {
        Ok(result)
    }
}

/// JSON response structure for validation
#[derive(Debug, Serialize, Deserialize)]
struct ValidationResponse {
    success: bool,
    message: String,
}

/// Execute a development action with validation and commit
/// This performs read/write operations, validates them, and commits
fn dev_agent(
    container_name: &str,
    task: &str,
    branch: &str,
    base: Option<&str>,
) -> Result<String, String> {
    // Bootstrap OpenCode if needed
    bootstrap_opencode(container_name)?;

    // Handle branch management
    setup_branch(container_name, branch, base)?;

    // Execute the development task (OpenCode will make changes)
    println!("  Executing development task...");
    execute_opencode(container_name, task)?;

    // Validate the changes using OpenCode with JSON response
    println!("  Validating changes...");
    let max_attempts = 3;
    let mut attempt = 0;

    loop {
        attempt += 1;
        if attempt > max_attempts {
            return Err(format!(
                "Validation failed after {} attempts. Last error: {}",
                max_attempts, "Could not fix validation issues"
            ));
        }

        let validation_result = validate_with_opencode(container_name)?;

        if validation_result.success {
            println!("    ✓ Validation passed: {}", validation_result.message);
            break;
        } else {
            println!(
                "    ⚠ Validation failed (attempt {}/{}): {}",
                attempt, max_attempts, validation_result.message
            );

            if attempt < max_attempts {
                println!("    Attempting to fix issues...");
                let fix_task = format!("Fix the following issues: {}", validation_result.message);
                execute_opencode(container_name, &fix_task)?;
            } else {
                return Err(format!("Validation failed: {}", validation_result.message));
            }
        }
    }

    // Check if there are any changes to commit
    let status_cmd = "cd /workspace && git status --porcelain";
    let status = docker::exec_in_container(container_name, status_cmd)?;

    if status.trim().is_empty() {
        println!("  ⚠ No changes detected (git status --porcelain is clean)");
        println!("  Skipping commit and push");
        return Err("No changes were made by the development task".to_string());
    }

    // Commit the changes
    println!("  Committing changes...");
    let commit_hash = commit_changes(container_name, task)?;

    // Push to branch
    println!("  Pushing to branch...");
    push_to_branch(container_name, branch)?;

    Ok(commit_hash)
}

/// Checkout a branch (simple version for ask command)
fn checkout_branch_simple(container_name: &str, branch: &str) -> Result<(), String> {
    let checkout_cmd = format!("cd /workspace && git checkout {}", branch);
    docker::exec_in_container(container_name, &checkout_cmd)
        .map_err(|e| format!("Failed to checkout branch '{}': {}", branch, e))?;
    Ok(())
}

/// Set up branch: checkout base if provided, then create/checkout target branch
/// Supports both creating new branches and continuing work on existing branches
fn setup_branch(container_name: &str, branch: &str, base: Option<&str>) -> Result<(), String> {
    // Configure git user if not already configured
    let config_cmd = "cd /workspace && \
        git config user.name 'Agent Smith' 2>/dev/null || true && \
        git config user.email 'smith@agentsmith.dev' 2>/dev/null || true";
    let _ = docker::exec_in_container(container_name, config_cmd);

    // If base is provided, checkout that branch first
    if let Some(base_branch) = base {
        println!("  Checking out base branch: {}", base_branch);
        let checkout_base = format!("cd /workspace && git checkout {}", base_branch);
        docker::exec_in_container(container_name, &checkout_base)
            .map_err(|e| format!("Failed to checkout base branch '{}': {}", base_branch, e))?;
    }

    // Check if branch exists locally
    let check_local = format!(
        "cd /workspace && git show-ref --verify --quiet refs/heads/{} && echo 'exists' || echo 'not_exists'",
        branch
    );
    let local_exists = docker::exec_in_container(container_name, &check_local)?;

    if local_exists.trim() == "exists" {
        // Branch exists locally, checkout it (continue work)
        println!(
            "  Branch '{}' exists locally, checking out to continue work",
            branch
        );
        let checkout_cmd = format!("cd /workspace && git checkout {}", branch);
        docker::exec_in_container(container_name, &checkout_cmd)
            .map_err(|e| format!("Failed to checkout existing branch '{}': {}", branch, e))?;
        println!("  ✓ Checked out existing branch: {}", branch);
    } else {
        // Branch doesn't exist locally, check if it exists on remote
        let remote_check = "cd /workspace && git remote -v";
        let remotes = docker::exec_in_container(container_name, remote_check)?;

        if !remotes.trim().is_empty() {
            // Fetch remote refs to check for remote branch
            let _ = docker::exec_in_container(
                container_name,
                "cd /workspace && git fetch --quiet 2>/dev/null || true",
            );

            let remote_name = "origin";
            let check_remote = format!(
                "cd /workspace && git ls-remote --heads {} {} 2>/dev/null | wc -l",
                remote_name, branch
            );
            let remote_count = docker::exec_in_container(container_name, &check_remote)
                .unwrap_or_else(|_| "0".to_string())
                .trim()
                .parse::<u32>()
                .unwrap_or(0);

            if remote_count > 0 {
                // Branch exists on remote, checkout and track it (continue work on remote branch)
                println!(
                    "  Branch '{}' exists on remote, checking out and tracking",
                    branch
                );
                let checkout_remote = format!(
                    "cd /workspace && git checkout --track {}/{}",
                    remote_name, branch
                );
                docker::exec_in_container(container_name, &checkout_remote)
                    .map_err(|e| format!("Failed to checkout remote branch '{}': {}", branch, e))?;
                println!("  ✓ Checked out and tracking remote branch: {}", branch);
            } else {
                // Branch doesn't exist anywhere, create new branch
                println!("  Creating new branch: {}", branch);
                let create_cmd = format!("cd /workspace && git checkout -b {}", branch);
                docker::exec_in_container(container_name, &create_cmd)
                    .map_err(|e| format!("Failed to create new branch '{}': {}", branch, e))?;
                println!("  ✓ Created new branch: {}", branch);
            }
        } else {
            // No remote configured, create new branch
            println!("  Creating new branch: {}", branch);
            let create_cmd = format!("cd /workspace && git checkout -b {}", branch);
            docker::exec_in_container(container_name, &create_cmd)
                .map_err(|e| format!("Failed to create new branch '{}': {}", branch, e))?;
            println!("  ✓ Created new branch: {}", branch);
        }
    }

    Ok(())
}

/// Validate changes using OpenCode with JSON response
fn validate_with_opencode(container_name: &str) -> Result<ValidationResponse, String> {
    let validation_prompt = "Validate compilation/build. Check if the code compiles, builds, and passes tests. Return ONLY a JSON object with this exact structure: {\"success\": true/false, \"message\": \"description\"}. Do not include any other text, only the JSON.";

    let response = execute_opencode(container_name, validation_prompt)?;

    // Try to parse JSON from the response
    // OpenCode might return JSON wrapped in markdown or other text, so we need to extract it
    let json_str = extract_json_from_response(&response);

    serde_json::from_str::<ValidationResponse>(&json_str).map_err(|e| {
        format!(
            "Failed to parse validation response as JSON: {}. Response was: {}",
            e, response
        )
    })
}

/// Extract JSON from OpenCode response (might be wrapped in markdown code blocks or other text)
fn extract_json_from_response(response: &str) -> String {
    let trimmed = response.trim();

    // Try to find JSON object in the response
    // Look for { ... } pattern
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return trimmed[start..=end].to_string();
            }
        }
    }

    // If no JSON found, return the whole response (will fail parsing but that's ok)
    trimmed.to_string()
}

/// Push changes to the specified branch
fn push_to_branch(container_name: &str, branch: &str) -> Result<(), String> {
    // Check if remote exists
    let remote_check = "cd /workspace && git remote -v";
    let remotes = docker::exec_in_container(container_name, remote_check)?;

    if remotes.trim().is_empty() {
        // No remote configured, skip push
        println!("    ⚠ No remote configured, skipping push");
        return Ok(());
    }

    // Check if there are any uncommitted changes (shouldn't happen after commit, but check anyway)
    let status_cmd = "cd /workspace && git status --porcelain";
    let status = docker::exec_in_container(container_name, status_cmd)?;

    if !status.trim().is_empty() {
        println!("    ⚠ Working directory has uncommitted changes, skipping push");
        println!("    (This should not happen after commit - there may be an issue)");
        return Ok(());
    }

    // Get the default remote name (usually 'origin')
    let remote_name = "origin"; // Could be made configurable in the future

    // Check if there are commits to push by comparing local branch to remote
    // If remote branch doesn't exist, we'll push (git push -u will create it)
    let remote_branch = format!("{}/{}", remote_name, branch);
    let check_remote_branch = format!(
        "cd /workspace && git ls-remote --heads {} {} 2>/dev/null | wc -l",
        remote_name, branch
    );
    let remote_exists = docker::exec_in_container(container_name, &check_remote_branch)
        .unwrap_or_else(|_| "0".to_string())
        .trim()
        .parse::<u32>()
        .unwrap_or(0);

    if remote_exists > 0 {
        // Remote branch exists, check if we're ahead
        let ahead_check = format!(
            "cd /workspace && git rev-list --count {}..{} 2>/dev/null || echo '0'",
            remote_branch, branch
        );
        let ahead_count = docker::exec_in_container(container_name, &ahead_check)
            .unwrap_or_else(|_| "0".to_string())
            .trim()
            .parse::<u32>()
            .unwrap_or(0);

        if ahead_count == 0 {
            println!("    ⚠ No commits to push (branch is up to date with remote)");
            return Ok(());
        }
    }

    // Configure git for HTTPS pushes (use credential helper to avoid prompts)
    // For public repos, we might still need authentication for push
    let _ = docker::exec_in_container(
        container_name,
        "cd /workspace && git config credential.helper 'store' 2>/dev/null || true",
    );

    // Push to branch
    let push_cmd = format!(
        "cd /workspace && git push -u {} {} 2>&1",
        remote_name, branch
    );

    match docker::exec_in_container(container_name, &push_cmd) {
        Ok(output) => {
            // Check if push actually succeeded (git push can return 0 even on some errors)
            if output.contains("error")
                || output.contains("fatal")
                || output.contains("Permission denied")
            {
                Err(format!("Push appeared to fail. Output: {}", output))
            } else {
                println!("    ✓ Pushed to {}/{}", remote_name, branch);
                Ok(())
            }
        }
        Err(e) => {
            // Push failed - provide helpful error message
            // For HTTPS repos, push requires authentication even if clone doesn't
            let error_msg = if e.contains("Permission denied") || e.contains("authentication") {
                format!(
                    "Push failed: Authentication required. For HTTPS repos, you may need to:\n\
                    1. Use SSH URL (git@github.com:user/repo.git) with --ssh-key\n\
                    2. Configure git credentials in the container\n\
                    Original error: {}",
                    e
                )
            } else {
                format!("Failed to push to branch '{}': {}", branch, e)
            };
            Err(error_msg)
        }
    }
}

/// Validate changes made by the development action
/// Checks: format, compile, build, tests
/// NOTE: This function is kept for reference but is no longer used.
/// Validation is now done via OpenCode with JSON responses.
#[allow(dead_code)]
fn validate_changes(container_name: &str) -> Result<(), String> {
    // Check if there are any changes to validate
    let status_cmd = "cd /workspace && git status --porcelain";
    let status = docker::exec_in_container(container_name, status_cmd)?;

    if status.trim().is_empty() {
        return Err("No changes detected to validate".to_string());
    }

    // 1. Format/clean check
    println!("    Checking code format...");
    if let Err(e) = check_format(container_name) {
        return Err(format!("Format check failed: {}", e));
    }
    println!("    ✓ Format check passed");

    // 2. Compile check
    println!("    Checking compilation...");
    if let Err(e) = check_compile(container_name) {
        return Err(format!("Compilation check failed: {}", e));
    }
    println!("    ✓ Compilation check passed");

    // 3. Build check
    println!("    Checking build...");
    if let Err(e) = check_build(container_name) {
        return Err(format!("Build check failed: {}", e));
    }
    println!("    ✓ Build check passed");

    // 4. Test check
    println!("    Running tests...");
    if let Err(e) = check_tests(container_name) {
        return Err(format!("Test check failed: {}", e));
    }
    println!("    ✓ Tests passed");

    Ok(())
}

/// Check code format (language-agnostic approach)
/// NOTE: This function is kept for reference but is no longer used.
/// Validation is now done via OpenCode with JSON responses.
#[allow(dead_code)]
fn check_format(container_name: &str) -> Result<(), String> {
    // Try common formatters based on project type
    // Rust: cargo fmt --check
    // JavaScript/TypeScript: prettier --check or eslint --fix
    // Python: black --check
    // Go: gofmt -l

    let format_checks = vec![
        ("cargo fmt --check", "Rust"),
        ("npx prettier --check . 2>/dev/null || true", "Prettier"),
        ("npx eslint --fix --dry-run . 2>/dev/null || true", "ESLint"),
        ("black --check . 2>/dev/null || true", "Black"),
        (
            "gofmt -l . 2>/dev/null | head -1 | grep -q . && echo 'needs format' || echo 'ok'",
            "Go",
        ),
    ];

    let mut any_applicable = false;
    let mut format_errors = Vec::new();

    for (cmd, name) in format_checks {
        let full_cmd = format!("cd /workspace && {} 2>&1", cmd);
        match docker::exec_in_container(container_name, &full_cmd) {
            Ok(output) => {
                // Check if the command exists and ran (not just "command not found")
                if !output.contains("command not found") && !output.contains("not found") {
                    any_applicable = true;
                    // Check for format issues
                    if output.contains("Diff in")
                        || output.contains("needs format")
                        || output.contains("would reformat")
                        || output.contains("Code style issues")
                    {
                        format_errors.push(format!("{} found formatting issues", name));
                    }
                }
            }
            Err(_) => {
                // Command doesn't exist or failed, skip
            }
        }
    }

    if !any_applicable {
        // No formatters found, skip format check
        return Ok(());
    }

    if !format_errors.is_empty() {
        return Err(format_errors.join(", "));
    }

    Ok(())
}

/// Check if code compiles
/// NOTE: This function is kept for reference but is no longer used.
/// Validation is now done via OpenCode with JSON responses.
#[allow(dead_code)]
fn check_compile(container_name: &str) -> Result<(), String> {
    // Try common compilers based on project type
    let compile_checks = vec![
        ("cargo check", "Rust"),
        ("tsc --noEmit 2>/dev/null || true", "TypeScript"),
        ("go build ./... 2>&1", "Go"),
        ("javac -version 2>&1 && find . -name '*.java' | head -1 | xargs javac 2>&1 || echo 'no java files'", "Java"),
    ];

    let mut any_applicable = false;
    let mut compile_errors = Vec::new();

    for (cmd, name) in compile_checks {
        let full_cmd = format!("cd /workspace && {} 2>&1", cmd);
        match docker::exec_in_container(container_name, &full_cmd) {
            Ok(output) => {
                // Check if the command exists and ran
                if !output.contains("command not found") && !output.contains("not found") {
                    any_applicable = true;
                    // Check for compilation errors
                    if output.contains("error")
                        || output.contains("Error")
                        || output.contains("ERROR")
                        || output.contains("failed")
                    {
                        // Check if it's just a "no files" message
                        if !output.contains("no java files") && !output.contains("no files found") {
                            compile_errors.push(format!("{} compilation failed", name));
                        }
                    }
                }
            }
            Err(e) => {
                // If it's a real error (not just command not found), it might be a compile error
                if !e.contains("command not found") && !e.contains("not found") {
                    any_applicable = true;
                    compile_errors.push(format!("{} compilation failed: {}", name, e));
                }
            }
        }
    }

    if !any_applicable {
        // No compilers found, skip compile check
        return Ok(());
    }

    if !compile_errors.is_empty() {
        return Err(compile_errors.join(", "));
    }

    Ok(())
}

/// Check if project builds
/// NOTE: This function is kept for reference but is no longer used.
/// Validation is now done via OpenCode with JSON responses.
#[allow(dead_code)]
fn check_build(container_name: &str) -> Result<(), String> {
    // Try common build commands
    let build_checks = vec![
        ("cargo build", "Rust"),
        ("npm run build 2>&1 || yarn build 2>&1 || true", "Node.js"),
        ("go build ./... 2>&1", "Go"),
        ("make build 2>&1 || true", "Make"),
    ];

    let mut any_applicable = false;
    let mut build_errors = Vec::new();

    for (cmd, name) in build_checks {
        let full_cmd = format!("cd /workspace && {} 2>&1", cmd);
        match docker::exec_in_container(container_name, &full_cmd) {
            Ok(output) => {
                // Check if the command exists and ran
                if !output.contains("command not found") && !output.contains("not found") {
                    any_applicable = true;
                    // Check for build errors
                    if output.contains("error")
                        || output.contains("Error")
                        || output.contains("ERROR")
                        || output.contains("failed")
                        || output.contains("FAILED")
                    {
                        build_errors.push(format!("{} build failed", name));
                    }
                }
            }
            Err(e) => {
                // If it's a real error (not just command not found), it might be a build error
                if !e.contains("command not found") && !e.contains("not found") {
                    any_applicable = true;
                    build_errors.push(format!("{} build failed: {}", name, e));
                }
            }
        }
    }

    if !any_applicable {
        // No build commands found, skip build check
        return Ok(());
    }

    if !build_errors.is_empty() {
        return Err(build_errors.join(", "));
    }

    Ok(())
}

/// Check if tests pass
/// NOTE: This function is kept for reference but is no longer used.
/// Validation is now done via OpenCode with JSON responses.
#[allow(dead_code)]
fn check_tests(container_name: &str) -> Result<(), String> {
    // Try common test commands
    let test_commands = vec![
        ("cargo test", "Rust"),
        ("npm test 2>&1 || yarn test 2>&1 || true", "Node.js"),
        ("go test ./... 2>&1", "Go"),
        (
            "python -m pytest 2>&1 || python -m unittest discover 2>&1 || true",
            "Python",
        ),
        ("make test 2>&1 || true", "Make"),
    ];

    let mut any_applicable = false;
    let mut test_errors = Vec::new();

    for (cmd, name) in test_commands {
        let full_cmd = format!("cd /workspace && {} 2>&1", cmd);
        match docker::exec_in_container(container_name, &full_cmd) {
            Ok(output) => {
                // Check if the command exists and ran
                if !output.contains("command not found") && !output.contains("not found") {
                    any_applicable = true;
                    // Check for test failures
                    if output.contains("FAILED")
                        || output.contains("failed")
                        || output.contains("FAIL")
                        || output.contains("Error")
                        || (output.contains("test") && output.contains("FAIL"))
                    {
                        // But allow "no tests found" messages
                        if !output.contains("no tests found") && !output.contains("No tests found")
                        {
                            test_errors.push(format!("{} tests failed", name));
                        }
                    }
                }
            }
            Err(e) => {
                // If it's a real error (not just command not found), it might be a test failure
                if !e.contains("command not found") && !e.contains("not found") {
                    any_applicable = true;
                    test_errors.push(format!("{} tests failed: {}", name, e));
                }
            }
        }
    }

    if !any_applicable {
        // No test commands found, skip test check
        return Ok(());
    }

    if !test_errors.is_empty() {
        return Err(test_errors.join(", "));
    }

    Ok(())
}

/// Review changes in a feature branch
/// Returns formatted output with header, high-level analysis, and git diff
fn review_branch(container_name: &str, branch: &str, base: Option<&str>) -> Result<String, String> {
    // Bootstrap OpenCode if needed
    bootstrap_opencode(container_name)?;

    // Fetch all branches to ensure we have remote refs
    let _ = docker::exec_in_container(
        container_name,
        "cd /workspace && git fetch --all --quiet 2>/dev/null || true",
    );

    // Find the base branch
    let base_branch = if let Some(b) = base {
        b.to_string()
    } else {
        find_base_branch(container_name, branch)?
    };

    // Checkout the feature branch
    println!("  Checking out feature branch: {}", branch);
    checkout_branch_simple(container_name, branch)?;

    // Get the git diff
    println!("  Getting diff between {} and {}...", base_branch, branch);
    let diff = get_git_diff(container_name, &base_branch, branch)?;

    if diff.trim().is_empty() {
        return Err(format!(
            "No differences found between {} and {}",
            base_branch, branch
        ));
    }

    // Use agent to analyze the changes
    println!("  Analyzing changes with agent...");
    let analysis_prompt = format!(
        "Analyze the following git diff and provide a high-level summary of the code changes. \
         Focus on: what files were changed, what functionality was added/modified/removed, \
         and any notable patterns or concerns. Keep it concise and structured.\n\n\
         Git diff:\n{}",
        diff
    );

    let analysis = execute_opencode(container_name, &analysis_prompt)?;

    // Format the output
    let output = format!(
        "╔═══════════════════════════════════════════════════════════════╗\n\
         ║                    CODE REVIEW REPORT                          ║\n\
         ╚═══════════════════════════════════════════════════════════════╝\n\n\
         Branch: {}\n\
         Base:   {}\n\n\
         ───────────────────────────────────────────────────────────────\n\
         HIGH-LEVEL CHANGES\n\
         ───────────────────────────────────────────────────────────────\n\n\
         {}\n\n\
         ───────────────────────────────────────────────────────────────\n\
         CODE DELTA (git diff)\n\
         ───────────────────────────────────────────────────────────────\n\n\
         {}",
        branch, base_branch, analysis, diff
    );

    Ok(output)
}

/// Find the base branch for a feature branch
/// Tries: provided base, main, master, or merge-base with main/master
fn find_base_branch(container_name: &str, branch: &str) -> Result<String, String> {
    // Try to find merge-base with common base branches
    let common_bases = vec!["main", "master", "develop"];

    for base in &common_bases {
        // Check if base branch exists
        let check_cmd = format!(
            "cd /workspace && git show-ref --verify --quiet refs/heads/{} && echo 'exists' || \
             (git show-ref --verify --quiet refs/remotes/origin/{} && echo 'remote' || echo 'not_found')",
            base, base
        );
        let result = docker::exec_in_container(container_name, &check_cmd)
            .unwrap_or_else(|_| "not_found".to_string());

        if result.contains("exists") || result.contains("remote") {
            // Try to find merge-base
            let merge_base_cmd = format!(
                "cd /workspace && git merge-base {} {} 2>/dev/null | head -1",
                base, branch
            );
            let merge_base =
                docker::exec_in_container(container_name, &merge_base_cmd).unwrap_or_default();

            if !merge_base.trim().is_empty() {
                println!("  Found base branch: {} (merge-base found)", base);
                return Ok(base.to_string());
            }
        }
    }

    // If no merge-base found, default to main or master
    for base in &common_bases {
        let check_cmd = format!(
            "cd /workspace && (git show-ref --verify --quiet refs/heads/{} || \
             git show-ref --verify --quiet refs/remotes/origin/{}) && echo 'exists' || echo 'not_found'",
            base, base
        );
        let result = docker::exec_in_container(container_name, &check_cmd)
            .unwrap_or_else(|_| "not_found".to_string());

        if result.contains("exists") {
            println!("  Using default base branch: {}", base);
            return Ok(base.to_string());
        }
    }

    Err("Could not determine base branch. Please specify with --base".to_string())
}

/// Get git diff between two branches
fn get_git_diff(container_name: &str, base: &str, branch: &str) -> Result<String, String> {
    // Ensure we have the latest refs
    let _ = docker::exec_in_container(
        container_name,
        "cd /workspace && git fetch --all --quiet 2>/dev/null || true",
    );

    // Try to get diff, handling both local and remote branches
    let diff_cmd = format!("cd /workspace && git diff {}...{} 2>&1", base, branch);

    let diff = docker::exec_in_container(container_name, &diff_cmd)?;

    // If diff is empty, try with origin/ prefix
    if diff.trim().is_empty() {
        let remote_diff_cmd = format!(
            "cd /workspace && (git diff origin/{}...origin/{} 2>&1 || git diff {}...origin/{} 2>&1 || git diff origin/{}...{} 2>&1)",
            base, branch, base, branch, base, branch
        );
        return docker::exec_in_container(container_name, &remote_diff_cmd);
    }

    Ok(diff)
}

/// Commit changes with a message
fn commit_changes(container_name: &str, task: &str) -> Result<String, String> {
    // Configure git user if not already configured
    let config_cmd = "cd /workspace && \
        git config user.name 'Agent Smith' 2>/dev/null || true && \
        git config user.email 'smith@agentsmith.dev' 2>/dev/null || true";
    let _ = docker::exec_in_container(container_name, config_cmd);

    // Stage all changes
    let stage_cmd = "cd /workspace && git add -A";
    docker::exec_in_container(container_name, stage_cmd)?;

    // Check if there are staged changes
    let status_cmd =
        "cd /workspace && git diff --cached --quiet && echo 'no changes' || echo 'has changes'";
    let status = docker::exec_in_container(container_name, status_cmd)?;

    if status.trim() == "no changes" {
        return Err("No changes to commit".to_string());
    }

    // Create commit message from task (use task directly as commit message)
    let commit_cmd = format!(
        "cd /workspace && git commit -m '{}'",
        task.replace("'", "'\"'\"'")
    );

    docker::exec_in_container(container_name, &commit_cmd)?;

    // Get commit hash
    let hash_cmd = "cd /workspace && git rev-parse HEAD";
    let commit_hash = docker::exec_in_container(container_name, hash_cmd)?;

    Ok(commit_hash.trim().to_string())
}
