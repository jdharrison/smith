//! Dagger pipeline: Setup → Setup Check (loop) → Execute → Execute Check (loop) → Assurance.
//! Setup check and Execute check have feedback loops: on failure, feed error into fix step and retry.
//! Runs inside dagger_sdk::connect(); all container work is done via the Dagger engine.

use dagger_sdk::{Container, Query};
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Default)]
pub struct PipelineRoles {
    pub setup_run: Option<RoleInfo>,
    pub setup_check: Option<RoleInfo>,
    pub execute_run: Option<RoleInfo>,
    pub execute_check: Option<RoleInfo>,
    pub validate_run: Option<RoleInfo>,
    pub validate_check: Option<RoleInfo>,
    pub commit_run: Option<RoleInfo>,
    pub commit_check: Option<RoleInfo>,
}

#[derive(Clone)]
pub struct RoleInfo {
    pub agent: String,
    pub role: String,
    pub model: Option<String>,
    pub prompt: Option<String>,
}

impl RoleInfo {
    pub fn new(agent: String, role: String, model: Option<String>, prompt: Option<String>) -> Self {
        Self {
            agent,
            role,
            model,
            prompt,
        }
    }

    pub fn model_arg(&self) -> String {
        self.model
            .as_ref()
            .map(|m| format!(" -m '{}'", m.replace('\'', "'\"'\"'")))
            .unwrap_or_default()
    }

    pub fn prompt_arg(&self) -> String {
        self.prompt
            .as_ref()
            .map(|p| format!(" --prompt '{}'", p.replace('\'', "'\"'\"'")))
            .unwrap_or_default()
    }
}

impl PipelineRoles {
    pub fn get_for_step(&self, step: &str) -> Option<&RoleInfo> {
        match step {
            "setup_run" => self.setup_run.as_ref(),
            "setup_check" => self.setup_check.as_ref(),
            "execute_run" => self.execute_run.as_ref(),
            "execute_check" => self.execute_check.as_ref(),
            "validate_run" => self.validate_run.as_ref(),
            "validate_check" => self.validate_check.as_ref(),
            "commit_run" => self.commit_run.as_ref(),
            "commit_check" => self.commit_check.as_ref(),
            _ => None,
        }
    }
}

const MAX_SETUP_LOOP_RETRIES: u32 = 3;
const MAX_EXEC_CHECK_LOOP_RETRIES: u32 = 3;
const MAX_ASSURANCE_LOOP_RETRIES: u32 = 3;

/// Install deps by project type; exits 1 on failure (used for initial setup and after setup-fix).
const INSTALL_SCRIPT: &str = r#"
  if [ -f Cargo.toml ]; then
    cargo check 2>&1; r=$?;
    if [ $r -ne 0 ]; then echo "Setup failed: cargo check failed (exit $r). Dependencies could not be resolved or build failed. Aborting."; exit 1; fi
  elif [ -f package.json ]; then
    (npm install 2>&1 || yarn install 2>&1); r=$?;
    if [ $r -ne 0 ]; then echo "Setup failed: npm/yarn install failed (exit $r). Dependencies could not be resolved. Aborting."; exit 1; fi
  elif [ -f go.mod ]; then
    go mod download 2>&1; r=$?;
    if [ $r -ne 0 ]; then echo "Setup failed: go mod download failed (exit $r). Aborting."; exit 1; fi
  elif [ -f requirements.txt ] || [ -f pyproject.toml ]; then
    (pip install -r requirements.txt 2>/dev/null || pip3 install -r requirements.txt 2>/dev/null) 2>&1; r=$?;
    if [ $r -ne 0 ]; then echo "Setup failed: pip install failed (exit $r). Aborting."; exit 1; fi
  fi
"#;

/// Setup check: build + tests. Runs and echoes SMITH_SETUP_EXIT=0|1 then exits 0 so we can loop on failure.
const SETUP_CHECK_CAPTURE_SCRIPT: &str = r#"
  e=0
  if [ -f Cargo.toml ]; then
    cargo check 2>&1; r=$?; [ $r -ne 0 ] && e=1
    cargo test 2>&1; r=$?; [ $r -ne 0 ] && e=1
  elif [ -f package.json ]; then
    (npm run build 2>/dev/null || npm ls 2>/dev/null) 2>&1; r=$?; [ $r -ne 0 ] && e=1
    npm test 2>&1; r=$?; [ $r -ne 0 ] && e=1
  elif [ -f go.mod ]; then
    go build ./... 2>&1; r=$?; [ $r -ne 0 ] && e=1
  fi
  echo "SMITH_SETUP_EXIT=$e"
  exit 0
"#;

/// Execute check: fmt + build + tests. Echoes SMITH_EXEC_CHECK_EXIT=0|1 then exits 0 for loop.
const EXEC_CHECK_CAPTURE_SCRIPT: &str = r#"
  e=0
  if [ -f Cargo.toml ]; then
    cargo fmt --check 2>&1; r=$?; [ $r -ne 0 ] && e=1
    cargo check 2>&1; r=$?; [ $r -ne 0 ] && e=1
    cargo test 2>&1; r=$?; [ $r -ne 0 ] && e=1
  elif [ -f package.json ]; then
    npm run build 2>&1; r=$?; [ $r -ne 0 ] && e=1
    npm test 2>&1; r=$?; [ $r -ne 0 ] && e=1
  elif [ -f go.mod ]; then
    go build ./... 2>&1; r=$?; [ $r -ne 0 ] && e=1
    go test ./... 2>&1; r=$?; [ $r -ne 0 ] && e=1
  fi
  echo "SMITH_EXEC_CHECK_EXIT=$e"
  exit 0
"#;

const ASSURANCE_PROMPT: &str = "Briefly review the recent changes in this repo for obvious issues; list any major concerns or say OK if none.";

/// Ask assurance: filter/cleanup prompt. Trims unnecessary parts from an answer; feeds back into itself for one extra pass.
const ASK_CLEANUP_PROMPT_PREFIX: &str = "You are a filter. The following text is an answer to a user question. Remove any unnecessary preamble, meta-commentary, redundant sections, or cruft. Keep the substantive answer. Return only the cleaned response, nothing else.\n\n---\n";
const ASK_ASSURANCE_MAX_PASSES: u32 = 2;
const ASK_ASSURANCE_MAX_INPUT_CHARS: usize = 14_000;

/// Heuristic: assurance passed if review says OK or no issues (so we can break the loop).
fn assurance_passed(stdout: &str) -> bool {
    let s = stdout.to_lowercase();
    s.contains("ok")
        || s.contains("no major")
        || s.contains("no concerns")
        || s.contains("no issues")
        || s.contains("none.")
        || s.trim().is_empty()
}

/// Result type for pipeline operations (avoids exposing eyre in public API).
pub type PipelineResult<T> = Result<T, String>;

/// Map clone/branch-not-found errors to a clear message for read-only commands (ask/review).
fn map_branch_not_found_err(e: String, branch: &str) -> String {
    let s = e.to_lowercase();
    if s.contains("couldn't find remote ref")
        || s.contains("not found in upstream")
        || s.contains("remote branch")
    {
        format!(
            "Branch '{}' does not exist on remote. Ask and Review only work on existing branches.",
            branch
        )
    } else {
        e
    }
}

/// Run a minimal pipeline to verify the Dagger engine and API.
/// Returns Ok(()) if the engine is reachable and a simple container exec works.
pub async fn run_doctor(conn: &Query) -> eyre::Result<()> {
    let out: String = conn
        .container()
        .from("alpine:latest")
        .with_exec(vec!["echo", "dagger ok"])
        .stdout()
        .await?;
    if out.trim() != "dagger ok" {
        eyre::bail!("Unexpected output: {}", out);
    }
    Ok(())
}

/// Build container after Setup: clone repo, install deps, bootstrap OpenCode.
/// Setup check is run in a loop (run_setup_loop) so failures can be fed back into a fix step.
/// SSH: use explicit --ssh-key if provided; else for git@ URLs mount the host's ~/.ssh so whatever works on the host works in the pipeline.
/// - clone_branch: branch to clone from remote (must exist).
/// - work_branch: if Some(b), after clone run `git checkout -b b` (for dev on a new branch); if None, stay on clone_branch (for ask/review on existing branch).
/// - script: optional script to run in container (e.g., install OpenCode)
fn build_setup_container(
    conn: &Query,
    repo_url: &str,
    clone_branch: &str,
    work_branch: Option<&str>,
    base_image: &str,
    ssh_key_path: Option<&std::path::Path>,
    script: Option<&str>,
) -> PipelineResult<Container> {
    let key_content = ssh_key_path
        .map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .transpose()?;

    let uses_ssh_url = repo_url.starts_with("git@");
    let mut c = conn.container().from(base_image);

    // Note: Network access to host uses host.docker.internal or bridge IP by default

    if let Some(ref key) = key_content {
        // Explicit key: mount as secret. Use a unique secret name per invocation to avoid
        // a fixed identifier that could be logged or searched (secret value is never logged).
        let secret_name = format!(
            "dk_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let secret = conn.set_secret(&secret_name, key.clone());
        c = c
            .with_mounted_secret("/root/.ssh/id_ed25519", secret)
            .with_exec(vec![
                "sh",
                "-c",
                "mkdir -p /root/.ssh && chmod 700 /root/.ssh && chmod 600 /root/.ssh/id_ed25519 || { echo 'Setup failed: could not set SSH key permissions (600). Aborting.'; exit 1; }",
            ]);
    } else if uses_ssh_url {
        // No explicit key but SSH URL: mount host's ~/.ssh and optionally forward ssh-agent
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let ssh_dir = format!("{}/.ssh", home);
        let host_ssh = conn.host().directory(ssh_dir);
        c = c.with_mounted_directory("/root/.ssh", host_ssh);
        // If host uses ssh-agent, forward the socket so clone/auth works in the container
        if let Ok(sock) = std::env::var("SSH_AUTH_SOCK") {
            let host_socket = conn.host().unix_socket(&sock);
            c = c
                .with_unix_socket("/ssh-agent", host_socket)
                .with_env_variable("SSH_AUTH_SOCK", "/ssh-agent");
        }
    }

    // Install git, openssh, curl, ca-certificates, tar, bash (OpenCode install script needs bash; no Node unless project has package.json)
    c = c.with_exec(vec![
        "sh",
        "-c",
        "apk add --no-cache git openssh-client curl ca-certificates tar bash 2>/dev/null || (apt-get update && apt-get install -y git openssh-client curl ca-certificates tar bash) 2>/dev/null || true",
    ]);
    // OpenCode install disabled for now: isolate Dagger + clone testing (smith project status).
    // c = c.with_exec(vec![
    //     "sh",
    //     "-c",
    //     "OPENCODE_INSTALL_DIR=/usr/local/bin curl -fsSL https://opencode.ai/install | bash -s -- --no-modify-path 2>&1 || { echo 'Setup failed: opencode install failed. Aborting.'; exit 1; }",
    // ]);
    // Ensure opencode and cargo (when installed) are on PATH. Include /usr/local/cargo/bin
    // so Rust official images (e.g. rust:1-bookworm) keep cargo; alpine gets it from rustup in /root/.cargo/bin.
    c = c.with_env_variable(
        "PATH",
        "/root/.cargo/bin:/usr/local/cargo/bin:/root/.opencode/bin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );

    // When we didn't mount host .ssh, create .ssh and populate known_hosts so clone can at least verify the host
    if key_content.is_some() || !uses_ssh_url {
        c = c.with_exec(vec![
            "sh",
            "-c",
            "mkdir -p /root/.ssh && chmod 700 /root/.ssh",
        ]);
        c = c.with_exec(vec![
            "sh",
            "-c",
            "ssh-keyscan github.com >> /root/.ssh/known_hosts 2>/dev/null || true",
        ]);
    }

    let clone_branch_escaped = clone_branch.replace('\'', "'\"'\"'");
    let repo_escaped = repo_url.replace('\'', "'\"'\"'");
    let clone_cmd = format!(
        "git clone --depth 1 --branch '{}' '{}' /workspace",
        clone_branch_escaped, repo_escaped
    );
    c = c.with_exec(vec!["sh", "-c", &clone_cmd]);
    c = c.with_workdir("/workspace");
    // When dev specifies a work branch: fetch origin, then create branch from origin/<branch> if it exists so HEAD is based off origin; else create new branch from current HEAD.
    if let Some(wb) = work_branch {
        let wb_escaped = wb.replace('\'', "'\"'\"'");
        let remote_ref = format!("refs/remotes/origin/{}", wb);
        let remote_ref_escaped = remote_ref.replace('\'', "'\"'\"'");
        let branch_setup_cmd = format!(
            r#"git fetch origin 2>&1 || {{ echo 'Setup failed: could not fetch from origin. Aborting.'; exit 1; }}
if git rev-parse --verify "{}" >/dev/null 2>&1; then
  git checkout -b '{}' "{}" 2>&1 || {{ echo 'Setup failed: could not checkout branch from origin. Aborting.'; exit 1; }}
else
  git checkout -b '{}' 2>&1 || {{ echo 'Setup failed: could not create branch. Aborting.'; exit 1; }}
fi"#,
            remote_ref_escaped, wb_escaped, remote_ref_escaped, wb_escaped
        );
        c = c.with_exec(vec!["sh", "-c", &branch_setup_cmd]);
    }

    // Run project script if provided (e.g., install OpenCode) - runs after repo clone so codebase is available
    if let Some(script) = script {
        c = c.with_exec(vec!["sh", "-c", script]);
    }

    // Install Rust (rustup + default toolchain) when the project has Cargo.toml; need build-base/build-essential for native deps
    c = c.with_exec(vec![
        "sh",
        "-c",
        "[ -f /workspace/Cargo.toml ] && (apk add --no-cache build-base 2>/dev/null || (apt-get update && apt-get install -y build-essential) 2>/dev/null) && (curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable) || true",
    ]);
    // Install Node/npm only when the project has package.json (Rust/Go/Python-only repos stay Node-free)
    c = c.with_exec(vec![
        "sh",
        "-c",
        "[ -f /workspace/package.json ] && (apk add --no-cache nodejs npm 2>/dev/null || (apt-get update && apt-get install -y nodejs npm) 2>/dev/null) || true",
    ]);
    // Install dependencies by project type. Fail with a clear message if install cannot be resolved.
    c = c.with_exec(vec!["sh", "-c", INSTALL_SCRIPT]);

    // Bootstrap OpenCode (required for ask/dev/review). Fail if unavailable.
    c = c.with_exec(vec![
        "sh",
        "-c",
        "opencode --version 2>&1 || { echo 'Setup failed: opencode could not be run. Aborting.'; exit 1; }",
    ]);

    Ok(c)
}

/// Branch-sync check: when origin/<branch> exists, HEAD must equal origin/<branch>. Exit 1 with message otherwise; exit 0 when in sync or branch is new on remote.
fn branch_sync_check_script(branch: &str) -> String {
    let branch_escaped = branch.replace('\'', "'\"'\"'");
    let remote_ref = format!("refs/remotes/origin/{}", branch);
    let remote_ref_escaped = remote_ref.replace('\'', "'\"'\"'");
    format!(
        r#"cd /workspace && if git rev-parse --verify "{}" >/dev/null 2>&1; then
  HEAD=$(git rev-parse HEAD) && ORIGIN=$(git rev-parse "{}") && if [ "$HEAD" != "$ORIGIN" ]; then
    echo 'Setup check failed: HEAD is not at origin/{} (branch exists on remote). Sync or resolve before continuing.'; exit 1;
  fi;
fi"#,
        remote_ref_escaped, remote_ref_escaped, branch_escaped
    )
}

/// Parse "SMITH_SETUP_EXIT=0" or "SMITH_SETUP_EXIT=1" from stdout; default 1 if not found.
fn parse_setup_exit(stdout: &str) -> u32 {
    stdout
        .lines()
        .rev()
        .find_map(|l| {
            let l = l.trim();
            if l.starts_with("SMITH_SETUP_EXIT=") {
                l.trim_start_matches("SMITH_SETUP_EXIT=")
                    .parse::<u32>()
                    .ok()
            } else {
                None
            }
        })
        .unwrap_or(1)
}

/// Parse "SMITH_EXEC_CHECK_EXIT=0" or "SMITH_EXEC_CHECK_EXIT=1" from stdout; default 1 if not found.
fn parse_exec_check_exit(stdout: &str) -> u32 {
    stdout
        .lines()
        .rev()
        .find_map(|l| {
            let l = l.trim();
            if l.starts_with("SMITH_EXEC_CHECK_EXIT=") {
                l.trim_start_matches("SMITH_EXEC_CHECK_EXIT=")
                    .parse::<u32>()
                    .ok()
            } else {
                None
            }
        })
        .unwrap_or(1)
}

/// Setup check feedback loop: run setup check; if it fails, run opencode with the error, re-install, retry.
/// When branch_sync is Some(branch), we first verify HEAD is at origin/branch (when that ref exists); we never pass setup if not in sync.
async fn run_setup_loop(
    mut c: Container,
    branch_sync: Option<&str>,
    setup_role: Option<&RoleInfo>,
) -> PipelineResult<Container> {
    if let Some(branch) = branch_sync {
        let sync_script = branch_sync_check_script(branch);
        let _ = c
            .with_exec(vec!["sh", "-c", &sync_script])
            .stdout()
            .await
            .map_err(|e| e.to_string())?;
    }
    for attempt in 0..MAX_SETUP_LOOP_RETRIES {
        let out: String = c
            .with_exec(vec!["sh", "-c", SETUP_CHECK_CAPTURE_SCRIPT])
            .stdout()
            .await
            .map_err(|e| e.to_string())?;
        if parse_setup_exit(&out) == 0 {
            if let Some(branch) = branch_sync {
                let sync_script = branch_sync_check_script(branch);
                let _ = c
                    .with_exec(vec!["sh", "-c", &sync_script])
                    .stdout()
                    .await
                    .map_err(|e| e.to_string())?;
            }
            return Ok(c);
        }
        if attempt + 1 >= MAX_SETUP_LOOP_RETRIES {
            return Err(format!(
                "Setup check failed after {} attempts. Last output: {}",
                MAX_SETUP_LOOP_RETRIES,
                out.lines().take(50).collect::<Vec<_>>().join("\n")
            ));
        }
        let fix_prompt = format!(
            "The project's build or tests failed. Fix the project (dependencies, configuration, or code) so that build and tests pass. Apply your changes. Output from the failed check:\n\n---\n{}",
            out.lines().take(80).collect::<Vec<_>>().join("\n")
        );
        let fix_escaped = fix_prompt.replace('\'', "'\"'\"'");
        let model_arg = setup_role.map(|r| r.model_arg()).unwrap_or_default();
        let prompt_arg = setup_role.map(|r| r.prompt_arg()).unwrap_or_default();
        let fix_cmd = format!(
            "cd /workspace && timeout 300 opencode run{} {} '{}' 2>&1",
            model_arg, prompt_arg, fix_escaped
        );
        c = c.with_exec(vec!["sh", "-c", &fix_cmd]);
        let _ = c.stdout().await.map_err(|e| e.to_string())?;
        c = c.with_exec(vec!["sh", "-c", INSTALL_SCRIPT]);
        let _ = c.stdout().await.map_err(|e| e.to_string())?;
    }
    Err("Setup loop exhausted".to_string())
}

/// Execute check feedback loop: run execute check; if it fails, run opencode with the error, retry.
async fn run_execute_check_loop(
    mut c: Container,
    task_summary: &str,
    check_role: Option<&RoleInfo>,
) -> PipelineResult<Container> {
    for attempt in 0..MAX_EXEC_CHECK_LOOP_RETRIES {
        let out: String = c
            .with_exec(vec!["sh", "-c", EXEC_CHECK_CAPTURE_SCRIPT])
            .stdout()
            .await
            .map_err(|e| e.to_string())?;
        if parse_exec_check_exit(&out) == 0 {
            return Ok(c);
        }
        if attempt + 1 >= MAX_EXEC_CHECK_LOOP_RETRIES {
            return Err(format!(
                "Execute check failed after {} attempts (format/build). Last output: {}",
                MAX_EXEC_CHECK_LOOP_RETRIES,
                out.lines().take(50).collect::<Vec<_>>().join("\n")
            ));
        }
        let summary: String = task_summary.chars().take(200).collect();
        let fix_prompt = format!(
            "The format or build check failed after the task \"{}\". Fix the code so that cargo fmt --check and cargo check (or npm run build) pass. Apply your changes. Output from the failed check:\n\n---\n{}",
            summary,
            out.lines().take(80).collect::<Vec<_>>().join("\n")
        );
        let fix_escaped = fix_prompt.replace('\'', "'\"'\"'");
        let model_arg = check_role.map(|r| r.model_arg()).unwrap_or_default();
        let prompt_arg = check_role.map(|r| r.prompt_arg()).unwrap_or_default();
        let fix_cmd = format!(
            "cd /workspace && timeout 300 opencode run{} {} '{}' 2>&1",
            model_arg, prompt_arg, fix_escaped
        );
        c = c.with_exec(vec!["sh", "-c", &fix_cmd]);
        let _ = c.stdout().await.map_err(|e| e.to_string())?;
    }
    Err("Execute check loop exhausted".to_string())
}

/// Assurance feedback loop: run review; if issues reported, run opencode to address them, re-run execute check, then re-run assurance.
async fn run_assurance_loop(
    mut c: Container,
    task_summary: &str,
    validate_role: Option<&RoleInfo>,
    check_role: Option<&RoleInfo>,
) -> PipelineResult<Container> {
    let assurance_escaped = ASSURANCE_PROMPT.replace('\'', "'\"'\"'");
    let model_arg = validate_role.map(|r| r.model_arg()).unwrap_or_default();
    let prompt_arg = validate_role.map(|r| r.prompt_arg()).unwrap_or_default();
    for attempt in 0..MAX_ASSURANCE_LOOP_RETRIES {
        let assurance_cmd = format!(
            "cd /workspace && timeout 90 opencode run{} {} '{}' 2>&1",
            model_arg, prompt_arg, assurance_escaped
        );
        let out: String = c
            .with_exec(vec!["sh", "-c", &assurance_cmd])
            .stdout()
            .await
            .map_err(|e| e.to_string())?;
        if assurance_passed(&out) {
            return Ok(c);
        }
        if attempt + 1 >= MAX_ASSURANCE_LOOP_RETRIES {
            return Ok(c);
        }
        let summary: String = task_summary.chars().take(100).collect();
        let fix_prompt = format!(
            "A review of recent changes reported issues. Address any valid concerns and apply fixes. Task was: \"{}\". Review output:\n\n---\n{}",
            summary,
            out.lines().take(60).collect::<Vec<_>>().join("\n")
        );
        let fix_escaped = fix_prompt.replace('\'', "'\"'\"'");
        let fix_model_arg = validate_role.map(|r| r.model_arg()).unwrap_or_default();
        let fix_prompt_arg = validate_role.map(|r| r.prompt_arg()).unwrap_or_default();
        let fix_cmd = format!(
            "cd /workspace && timeout 180 opencode run{} {} '{}' 2>&1",
            fix_model_arg, fix_prompt_arg, fix_escaped
        );
        c = c.with_exec(vec!["sh", "-c", &fix_cmd]);
        let _ = c.stdout().await.map_err(|e| e.to_string())?;
        c = run_execute_check_loop(c, "assurance fixes", check_role).await?;
    }
    Ok(c)
}

/// One pass of ask assurance: run opencode to clean/trim the given text. Returns cleaned string or None on failure (never fail the pipeline).
async fn run_ask_assurance_pass(c: &Container, text: &str) -> Option<String> {
    let truncated: String = text.chars().take(ASK_ASSURANCE_MAX_INPUT_CHARS).collect();
    let full_prompt = format!("{}{}", ASK_CLEANUP_PROMPT_PREFIX, truncated);
    let escaped = full_prompt.replace('\'', "'\"'\"'");
    let cmd = format!(
        "cd /workspace && timeout 120 opencode run '{}' 2>&1",
        escaped
    );
    let out: String = c.with_exec(vec!["sh", "-c", &cmd]).stdout().await.ok()?;
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Ask assurance: filter step that trims the answer; feeds back into itself for up to ASK_ASSURANCE_MAX_PASSES. Never fails - returns last good or original.
async fn run_ask_assurance(c: Container, raw_answer: String) -> String {
    let mut current = raw_answer.clone();
    for _ in 0..ASK_ASSURANCE_MAX_PASSES {
        if let Some(cleaned) = run_ask_assurance_pass(&c, &current).await {
            current = cleaned;
        } else {
            break;
        }
    }
    current
}

/// Run the Ask pipeline: setup → setup loop → opencode run (read-only) → ask assurance (cleanup filter). Returns the agent's answer.
/// OpenCode runs locally in the container - model/provider is configured in the container image.
#[allow(clippy::too_many_arguments)]
pub async fn run_ask(
    conn: &Query,
    repo_url: &str,
    branch: Option<&str>,
    question: &str,
    base_image: &str,
    ssh_key_path: Option<&std::path::Path>,
    timeout_secs: u64,
    script: Option<&str>,
    roles: &PipelineRoles,
) -> PipelineResult<String> {
    let branch = branch.unwrap_or("main");
    let c = build_setup_container(
        conn,
        repo_url,
        branch,
        None,
        base_image,
        ssh_key_path,
        script,
    )
    .map_err(|e| map_branch_not_found_err(e, branch))?;
    let c = run_setup_loop(c, None, roles.setup_run.as_ref())
        .await
        .map_err(|e| map_branch_not_found_err(e, branch))?;

    // Run opencode for the question
    let escaped = question.replace('\'', "'\"'\"'");
    let role = roles.execute_run.as_ref();
    let model_arg = role.map(|r| r.model_arg()).unwrap_or_default();
    let prompt_arg = role.map(|r| r.prompt_arg()).unwrap_or_default();
    let cmd = format!(
        "cd /workspace && timeout {} opencode run{} {} '{}' 2>&1",
        timeout_secs, model_arg, prompt_arg, escaped
    );
    let raw = c
        .with_exec(vec!["sh", "-c", &cmd])
        .stdout()
        .await
        .map_err(|e| e.to_string())?;

    let answer = run_ask_assurance(c, raw.trim().to_string()).await;
    Ok(answer)
}

/// Run the Dev pipeline: setup → setup loop → execute → execute check loop → assurance → commit/push.
/// Returns the commit hash on success.
/// OpenCode runs locally in the container - model/provider is configured in the container image.
#[allow(clippy::too_many_arguments)]
pub async fn run_dev(
    conn: &Query,
    repo_url: &str,
    branch: &str,
    base: Option<&str>,
    task: &str,
    base_image: &str,
    ssh_key_path: Option<&std::path::Path>,
    _verbose: bool,
    timeout_secs: u64,
    script: Option<&str>,
    commit_name: Option<&str>,
    commit_email: Option<&str>,
    roles: &PipelineRoles,
) -> PipelineResult<String> {
    let clone_branch = base.unwrap_or("main");
    let c = build_setup_container(
        conn,
        repo_url,
        clone_branch,
        Some(branch),
        base_image,
        ssh_key_path,
        script,
    )?;
    let c = run_setup_loop(c, Some(branch), roles.setup_run.as_ref()).await?;

    // Configure git user if commit_name/commit_email are provided
    let git_user_config = match (commit_name, commit_email) {
        (Some(name), Some(email)) => format!(
            "git config user.name '{}' && git config user.email '{}' && ",
            name.replace('\'', "'\"'\"'"),
            email.replace('\'', "'\"'\"'")
        ),
        (Some(name), None) => format!(
            "git config user.name '{}' && ",
            name.replace('\'', "'\"'\"'")
        ),
        (None, Some(email)) => format!(
            "git config user.email '{}' && ",
            email.replace('\'', "'\"'\"'")
        ),
        (None, None) => String::new(),
    };

    // Run opencode for the task
    let escaped = task.replace('\'', "'\"'\"'");
    let role = roles.execute_run.as_ref();
    let model_arg = role.map(|r| r.model_arg()).unwrap_or_default();
    let prompt_arg = role.map(|r| r.prompt_arg()).unwrap_or_default();
    let exec_cmd = format!(
        "cd /workspace && {} timeout {} opencode run{} {} '{}' 2>&1",
        git_user_config, timeout_secs, model_arg, prompt_arg, escaped
    );
    let c = c.with_exec(vec!["sh", "-c", &exec_cmd]);
    let _ = c.stdout().await.map_err(|e| e.to_string())?;

    let c = run_execute_check_loop(c, task, roles.execute_check.as_ref()).await?;
    let c = run_assurance_loop(
        c,
        task,
        roles.validate_run.as_ref(),
        roles.execute_check.as_ref(),
    )
    .await?;
    // Commit and push (requires SSH for push)
    let commit_git_user_config = match (commit_name, commit_email) {
        (Some(name), Some(email)) => format!(
            "git config user.name '{}' && git config user.email '{}' && ",
            name.replace('\'', "'\"'\"'"),
            email.replace('\'', "'\"'\"'")
        ),
        (Some(name), None) => format!(
            "git config user.name '{}' && ",
            name.replace('\'', "'\"'\"'")
        ),
        (None, Some(email)) => format!(
            "git config user.email '{}' && ",
            email.replace('\'', "'\"'\"'")
        ),
        (None, None) => String::new(),
    };
    let commit_cmd = format!(
        "cd /workspace && {} git add -A && git status --porcelain | head -1 && git commit -m '{}' 2>&1 || echo 'no-commit'",
        commit_git_user_config,
        task.replace('\'', "'\"'\"'").replace('\n', " ")
    );
    let c = c.with_exec(vec!["sh", "-c", &commit_cmd]);
    let out: String = c
        .with_exec(vec![
            "sh",
            "-c",
            "cd /workspace && git rev-parse HEAD 2>/dev/null || echo 'no-commit'",
        ])
        .stdout()
        .await
        .map_err(|e| e.to_string())?;
    let hash = out.trim().to_string();
    if hash == "no-commit" {
        return Err("No changes to commit".to_string());
    }
    // Bust cache so we always run fetch+rebase+push (remote state changes; cached result would be stale).
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Fetch all refs so we see if remote branch exists; rebase onto it when it does so push is never behind.
    let branch_escaped = branch.replace('\'', "'\"'\"'");
    let remote_ref = format!("refs/remotes/origin/{}", branch);
    let remote_ref_escaped = remote_ref.replace('\'', "'\"'\"'");
    let rebase_cmd = format!(
        r#"cd /workspace && : run_{} && git fetch origin 2>&1 && if git rev-parse --verify "{}" >/dev/null 2>&1; then git rebase "{}" 2>&1 || {{ echo 'Push aborted: rebase onto origin/{} failed (conflicts or network). Resolve conflicts or retry.'; exit 1; }}; fi"#,
        run_id, remote_ref_escaped, remote_ref_escaped, branch_escaped
    );
    let c = c.with_exec(vec!["sh", "-c", &rebase_cmd]);
    let _ = c.stdout().await.map_err(|e| e.to_string())?;
    // Push; surface failure (e.g. non-fast-forward, permission denied).
    let push_cmd = format!(
        "cd /workspace && : run_{} && git push origin HEAD 2>&1 || {{ echo 'Push failed: remote rejected (e.g. non-fast-forward) or network/SSH error.'; exit 1; }}",
        run_id
    );
    let c = c.with_exec(vec!["sh", "-c", &push_cmd]);
    let _ = c.stdout().await.map_err(|e| e.to_string())?;
    Ok(hash)
}

/// Run the Review pipeline: setup → setup loop → opencode analysis. Returns the review text.
/// Clones the base branch first, then checks out the feature branch so git diff can compare them.
/// OpenCode runs locally in the container - model/provider is configured in the container image.
#[allow(clippy::too_many_arguments)]
pub async fn run_review(
    conn: &Query,
    repo_url: &str,
    branch: &str,
    base: Option<&str>,
    base_image: &str,
    ssh_key_path: Option<&std::path::Path>,
    timeout_secs: u64,
    script: Option<&str>,
    roles: &PipelineRoles,
) -> PipelineResult<String> {
    let base_branch = base.unwrap_or("main");
    // Clone base branch first, then checkout feature branch for comparison
    let c = build_setup_container(
        conn,
        repo_url,
        base_branch,
        None,
        base_image,
        ssh_key_path,
        script,
    )
    .map_err(|e| map_branch_not_found_err(e, base_branch))?;
    // Fetch all refs and checkout the feature branch to compare against base
    let branch_escaped = branch.replace('\'', "'\"'\"'");
    let remote_ref = format!("refs/remotes/origin/{}", branch);
    let remote_ref_escaped = remote_ref.replace('\'', "'\"'\"'");
    let checkout_cmd = format!(
        r#"cd /workspace && git fetch origin 2>&1 || {{ echo 'Setup failed: could not fetch from origin. Aborting.'; exit 1; }}
if git rev-parse --verify "{}" >/dev/null 2>&1; then
  git checkout -b '{}' "{}" 2>&1 || {{ echo 'Setup failed: could not checkout branch from origin. Aborting.'; exit 1; }}
else
  git checkout -b '{}' 2>&1 || {{ echo 'Setup failed: could not create branch. Aborting.'; exit 1; }}
fi"#,
        remote_ref_escaped, branch_escaped, remote_ref_escaped, branch_escaped
    );
    let c = c.with_exec(vec!["sh", "-c", &checkout_cmd]);
    let _ = c.stdout().await.map_err(|e| e.to_string())?;
    let c = run_setup_loop(c, None, roles.setup_run.as_ref())
        .await
        .map_err(|e| map_branch_not_found_err(e, branch))?;
    let review_prompt = format!(
        "Analyze the changes in branch '{}' compared to base branch '{}'. Report any issues, security concerns, or suggested improvements.",
        branch, base_branch
    );

    // Run opencode for review
    let escaped = review_prompt.replace('\'', "'\"'\"'");
    let role = roles.execute_run.as_ref();
    let model_arg = role.map(|r| r.model_arg()).unwrap_or_default();
    let prompt_arg = role.map(|r| r.prompt_arg()).unwrap_or_default();
    let cmd = format!(
        "cd /workspace && timeout {} opencode run{} {} '{}' 2>&1",
        timeout_secs, model_arg, prompt_arg, escaped
    );
    let out = c
        .with_exec(vec!["sh", "-c", &cmd])
        .stdout()
        .await
        .map_err(|e| e.to_string())?;
    Ok(out.trim().to_string())
}

/// Run project status: spin up Dagger, clone repo into workspace, list files. Validates project is loadable.
pub async fn run_project_status(
    conn: &Query,
    repo_url: &str,
    branch: &str,
    base_image: &str,
    ssh_key_path: Option<&std::path::Path>,
    script: Option<&str>,
) -> PipelineResult<String> {
    let c = build_setup_container(
        conn,
        repo_url,
        branch,
        None,
        base_image,
        ssh_key_path,
        script,
    )?;
    let out: String = c
        .with_exec(vec![
            "sh",
            "-c",
            "ls -la /workspace && echo '---' && find /workspace -maxdepth 2 -type f 2>/dev/null | head -40",
        ])
        .stdout()
        .await
        .map_err(|e| e.to_string())?;
    Ok(out.trim().to_string())
}

/// Connect to Dagger and run the given async closure, mapping eyre errors to String.
pub async fn with_connection<F, Fut, T>(f: F) -> PipelineResult<T>
where
    F: FnOnce(Query) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = eyre::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let result: Arc<Mutex<Option<eyre::Result<T>>>> = Arc::new(Mutex::new(None));
    let result_c = result.clone();
    dagger_sdk::connect(move |conn| {
        let result = result_c.clone();
        async move {
            let r = f(conn).await;
            *result.lock().unwrap() = Some(r);
            Ok(())
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    let mut guard = result.lock().unwrap();
    let taken = guard
        .take()
        .unwrap_or_else(|| Err(eyre::eyre!("pipeline did not set result")));
    taken.map_err(|e: eyre::Report| e.to_string())
}
