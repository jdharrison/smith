mod dagger_pipeline;
mod docker;
mod github;

use clap::{CommandFactory, Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// ANSI color codes for status output (circle: red = not built, blue = built, green = online).
const ANSI_RED: &str = "\x1b[31m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RESET: &str = "\x1b[0m";

/// Colored bullet for status/install output (avoids format! in format args).
const BULLET_GREEN: &str = "\x1b[32m●\x1b[0m";
const BULLET_BLUE: &str = "\x1b[34m●\x1b[0m";
const BULLET_RED: &str = "\x1b[31m●\x1b[0m";
const BULLET_YELLOW: &str = "\x1b[33m●\x1b[0m";

/// OSC 8 hyperlink so the URL is clickable in supported terminals (e.g. VS Code, iTerm2, Windows Terminal).
fn clickable_agent_url(port: u16) -> String {
    let url = format!("http://localhost:{}", port);
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, url)
}

/// active = container running, reachable = health endpoint responds (only meaningful when active).
/// is_cloud = cloud agents are always green (no build needed).
/// Local agents: red (none) -> blue (built) -> green (online).
fn status_circle(active: bool, reachable: Option<bool>, built: bool, is_cloud: bool) -> String {
    let bullet = "●";
    let color = if active && reachable == Some(false) {
        ANSI_YELLOW
    } else if active {
        ANSI_GREEN
    } else if is_cloud {
        ANSI_GREEN
    } else if built {
        ANSI_BLUE
    } else {
        ANSI_RED
    };
    format!("{}{}{}", color, bullet, ANSI_RESET)
}

/// When dropped, restores stderr. Used for quiet mode to hide Dagger progress. No-op on non-Unix.
struct StderrRedirectGuard(Option<std::os::unix::io::RawFd>);

#[cfg(unix)]
fn redirect_stderr_to_null() -> Option<StderrRedirectGuard> {
    let null_fd = std::fs::File::open("/dev/null").ok()?;
    let null_raw = null_fd.as_raw_fd();
    let saved = unsafe { libc::dup(libc::STDERR_FILENO) };
    if saved < 0 {
        return None;
    }
    if unsafe { libc::dup2(null_raw, libc::STDERR_FILENO) } < 0 {
        unsafe { libc::close(saved) };
        return None;
    }
    Some(StderrRedirectGuard(Some(saved)))
}

#[cfg(not(unix))]
fn redirect_stderr_to_null() -> Option<StderrRedirectGuard> {
    None
}

impl Drop for StderrRedirectGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(saved) = self.0.take() {
            unsafe {
                libc::dup2(saved, libc::STDERR_FILENO);
                libc::close(saved);
            }
        }
    }
}

/// Spinner chars; runs until done is true.
fn run_spinner_until(done: &AtomicBool) {
    const CHARS: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0usize;
    while !done.load(Ordering::Relaxed) {
        print!("\r  {}  ", CHARS[i % CHARS.len()]);
        let _ = std::io::Write::flush(&mut std::io::stdout());
        thread::sleep(Duration::from_millis(80));
        i += 1;
    }
    print!("\r    \r");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// When the Dagger SDK fails to parse an error response (e.g. "invalid type: sequence, expected a string"),
/// the real exec error is still in the response body. Extract and return a clearer message when possible.
fn clarify_dagger_error(err: &str) -> String {
    const PREFIX: &str = "The response body is: ";
    let Some(start) = err.find(PREFIX) else {
        return err.to_string();
    };
    let json_str = &err[start + PREFIX.len()..];
    let Ok(root): Result<Value, _> = serde_json::from_str(json_str) else {
        return err.to_string();
    };
    let Some(errors) = root.get("errors").and_then(Value::as_array) else {
        return err.to_string();
    };
    let Some(first) = errors.first() else {
        return err.to_string();
    };
    let mut out = String::new();
    if let Some(msg) = first.get("message").and_then(Value::as_str) {
        out.push_str(msg);
    }
    if let Some(ext) = first.get("extensions") {
        if let Some(stdout) = ext.get("stdout").and_then(Value::as_str) {
            if !stdout.trim().is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(stdout.trim());
            }
        }
        if let Some(stderr) = ext.get("stderr").and_then(Value::as_str) {
            if !stderr.trim().is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(stderr.trim());
            }
        }
    }
    if out.is_empty() {
        err.to_string()
    } else {
        out
    }
}

#[derive(Parser)]
#[command(name = "smith")]
#[command(about = "smith — open-source control plane for local agent orchestration", long_about = None)]
#[command(disable_help_flag = true)]
#[command(disable_version_flag = true)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Dependencies, agents, projects
    #[command(next_help_heading = "System")]
    Status {
        /// Show raw output (config path, docker version/info, agent details)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Docker, config dir, optional Dagger
    Install,
    /// Remove all data, containers, and optionally, smith entirely.
    Uninstall {
        /// Skip confirmation prompt (still prompts for config removal unless --remove-config)
        #[arg(short, long)]
        force: bool,
        /// Also remove the config directory (~/.config/smith)
        #[arg(long)]
        remove_config: bool,
        /// Also remove Docker images built by smith
        #[arg(long)]
        remove_images: bool,
    },
    /// Print help
    Help,
    /// Print version
    Version,
    #[command(next_help_heading = "Commands")]
    /// Agent containers (opencode serve)
    Agent {
        #[command(subcommand)]
        cmd: AgentCommands,
    },
    /// Repos and config for run pipelines
    Project {
        #[command(subcommand)]
        cmd: ProjectCommands,
    },
    /// Run a pipeline (agent + project scoped)
    Run {
        #[command(subcommand)]
        cmd: RunCommands,
    },
}

/// Built-in agent name and image when no config is set (OpenCode cloud wrapper).
const DEFAULT_AGENT_NAME: &str = "opencode";
const DEFAULT_AGENT_IMAGE: &str = "ghcr.io/anomalyco/opencode";

#[derive(Subcommand)]
enum AgentCommands {
    /// Show status of all configured agents
    Status,
    /// Add an agent
    Add {
        /// Agent name (id)
        name: String,
        /// Docker image (e.g. ghcr.io/anomalyco/opencode); default: official OpenCode image
        #[arg(long)]
        image: Option<String>,
        /// Agent type: "local" or "cloud" (default: cloud)
        #[arg(long)]
        agent_type: Option<String>,
        /// Model to use (e.g. "anthropic/claude-sonnet-4-5", "qwen3:8b")
        #[arg(long)]
        model: Option<String>,
        /// Smaller model for internal operations (reduces API costs)
        #[arg(long)]
        small_model: Option<String>,
        /// Provider name (e.g. "ollama", "anthropic", "openai", "openrouter")
        #[arg(long)]
        provider: Option<String>,
        /// Custom base URL for provider (for proxies/custom endpoints)
        #[arg(long)]
        base_url: Option<String>,
        /// Port for opencode serve (default: 4096 + index)
        #[arg(long)]
        port: Option<u16>,
        /// Whether this agent is enabled (default: true)
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Update an existing agent
    Update {
        /// Agent name
        name: String,
        /// Docker image (pass empty to clear)
        #[arg(long)]
        image: Option<String>,
        /// Agent type: "local" or "cloud" (pass empty to clear)
        #[arg(long)]
        agent_type: Option<String>,
        /// Model (pass empty to clear = use container default)
        #[arg(long)]
        model: Option<String>,
        /// Small model (pass empty to clear)
        #[arg(long)]
        small_model: Option<String>,
        /// Provider name (pass empty to clear)
        #[arg(long)]
        provider: Option<String>,
        /// Base URL (pass empty to clear)
        #[arg(long)]
        base_url: Option<String>,
        /// Port for opencode serve (pass empty to clear)
        #[arg(long)]
        port: Option<u16>,
        /// Whether this agent is enabled (pass empty to clear)
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Remove an agent
    Remove {
        /// Agent name
        name: String,
    },
    /// Build Docker image for local agents (generate Dockerfile if missing, then docker build)
    Build {
        /// Agent name to build (e.g. opencode); omit to build all
        #[arg()]
        name: Option<String>,
        /// Build all configured agents
        #[arg(short = 'a', long = "all", alias = "A")]
        all: bool,
        /// Remove existing image and build with --no-cache (clean build)
        #[arg(long)]
        force: bool,
        /// Print Dockerfile path and docker build command per agent
        #[arg(long)]
        verbose: bool,
    },
    /// Start local agent containers (1 agent -> 1 container). Idempotent: skips if already running.
    Start {
        /// Print docker command and health-check details
        #[arg(short, long)]
        verbose: bool,
    },
    /// Stop local agent containers
    Stop,
    /// Stream live logs from an agent container (docker logs -f)
    Logs {
        /// Agent name (e.g. opencode)
        name: String,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Add a new project
    Add {
        /// Project name
        name: String,
        /// Repository path or URL
        #[arg(long)]
        repo: String,
        /// Docker image to use for this project (required)
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for this project (optional)
        #[arg(long)]
        ssh_key: Option<String>,
        /// Base branch to use for clone/compare (optional, default: main)
        #[arg(long)]
        base_branch: Option<String>,
        /// Remote name for fetch/push (optional, default: origin)
        #[arg(long)]
        remote: Option<String>,
        /// GitHub personal access token for PR creation (optional)
        #[arg(long)]
        github_token: Option<String>,
        /// Script to run in container before pipeline (optional, e.g., install OpenCode)
        #[arg(long)]
        script: Option<String>,
        /// Git author name (optional, overrides local git config)
        #[arg(long)]
        commit_name: Option<String>,
        /// Git author email (optional, overrides local git config)
        #[arg(long)]
        commit_email: Option<String>,
    },
    /// List all registered projects
    List,
    /// Spin up Dagger, clone project, and list workspace files (validates project is loadable)
    Status {
        /// Project name (omit to run status for all projects)
        #[arg(long)]
        project: Option<String>,
        /// Show full Dagger output
        #[arg(long)]
        verbose: bool,
    },
    /// Update an existing project's repository URL, image, or SSH key
    Update {
        /// Project name
        name: String,
        /// New repository path or URL
        #[arg(long)]
        repo: Option<String>,
        /// New Docker image to use for this project
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for this project (pass empty to clear)
        #[arg(long)]
        ssh_key: Option<String>,
        /// Base branch (pass empty to clear)
        #[arg(long)]
        base_branch: Option<String>,
        /// Remote name (pass empty to clear)
        #[arg(long)]
        remote: Option<String>,
        /// GitHub token for PR creation (pass empty to clear)
        #[arg(long)]
        github_token: Option<String>,
        /// Script to run in container before pipeline (pass empty to clear)
        #[arg(long)]
        script: Option<String>,
        /// Git author name (pass empty to clear)
        #[arg(long)]
        commit_name: Option<String>,
        /// Git author email (pass empty to clear)
        #[arg(long)]
        commit_email: Option<String>,
        /// Agent name to use for this project (pass empty to clear)
        #[arg(long)]
        agent: Option<String>,
        /// Ask pipeline: setup_run and setup_check roles (e.g., "installer" or "installer analyst")
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        ask_setup: Option<Vec<String>>,
        /// Ask pipeline: execute_run and execute_check roles (e.g., "engineer" or "engineer analyst")
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        ask_execute: Option<Vec<String>>,
        /// Ask pipeline: validate_run and validate_check roles (e.g., "validator" or "validator reviewer")
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        ask_validate: Option<Vec<String>>,
        /// Dev pipeline: setup_run and setup_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_setup: Option<Vec<String>>,
        /// Dev pipeline: execute_run and execute_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_execute: Option<Vec<String>>,
        /// Dev pipeline: validate_run and validate_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_validate: Option<Vec<String>>,
        /// Dev pipeline: commit_run and commit_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_commit: Option<Vec<String>>,
        /// Review pipeline: setup_run and setup_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        review_setup: Option<Vec<String>>,
        /// Review pipeline: execute_run and execute_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        review_execute: Option<Vec<String>>,
        /// Review pipeline: validate_run and validate_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        review_validate: Option<Vec<String>>,
    },
    /// Remove a project
    Remove {
        /// Project name
        name: String,
    },
}

/// Pipeline commands: agent + project scoped (run via `smith run <cmd>`).
#[derive(Subcommand)]
enum RunCommands {
    /// Ask a question to an agent about a project
    Ask {
        /// The question to ask the agent
        question: String,
        /// Base branch to checkout before asking (optional, uses default if not provided)
        #[arg(long, required = false)]
        base: Option<String>,
        /// Repository path or URL (overrides project config if provided)
        #[arg(long)]
        repo: Option<String>,
        /// Project name from config
        #[arg(long)]
        project: Option<String>,
        /// Docker image to use (overrides project config if provided)
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for private repositories (optional)
        #[arg(long)]
        ssh_key: Option<PathBuf>,
        /// Keep container alive after answering (for debugging/inspection)
        #[arg(long)]
        keep_alive: bool,
        /// Timeout in seconds for the agent run (default: 300)
        #[arg(long)]
        timeout: Option<u64>,
        /// Show full Dagger and pipeline output
        #[arg(long)]
        verbose: bool,
    },
    /// Execute a development action (read/write) with validation and commit
    Dev {
        /// The development task/instruction to execute
        task: String,
        /// Target branch to create/use and push to (required)
        #[arg(long)]
        branch: String,
        /// Base branch to checkout before starting (optional, uses default if not provided)
        #[arg(long, required = false)]
        base: Option<String>,
        /// Repository path or URL (overrides project config if provided)
        #[arg(long)]
        repo: Option<String>,
        /// Project name from config
        #[arg(long)]
        project: Option<String>,
        /// Docker image to use (overrides project config if provided)
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for private repositories (optional)
        #[arg(long)]
        ssh_key: Option<PathBuf>,
        /// Keep container alive after completion (for debugging/inspection)
        #[arg(long)]
        keep_alive: bool,
        /// Create or update a pull request after pushing (requires GitHub token configured)
        #[arg(long)]
        pr: bool,
        /// Timeout in seconds for the agent run (default: 300)
        #[arg(long)]
        timeout: Option<u64>,
        /// Show verbose output from OpenCode agent
        #[arg(long)]
        verbose: bool,
    },
    /// Review changes in a feature branch
    Review {
        /// Feature branch name to review
        branch: String,
        /// Base branch to compare against (optional, will auto-detect if not provided)
        #[arg(long)]
        base: Option<String>,
        /// Repository path or URL (overrides project config if provided)
        #[arg(long)]
        repo: Option<String>,
        /// Project name from config
        #[arg(long)]
        project: Option<String>,
        /// Docker image to use (overrides project config if provided)
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for private repositories (optional)
        #[arg(long)]
        ssh_key: Option<PathBuf>,
        /// Keep container alive after review (for debugging/inspection)
        #[arg(long)]
        keep_alive: bool,
        /// Timeout in seconds for the agent run (default: 300)
        #[arg(long)]
        timeout: Option<u64>,
        /// Show full Dagger and pipeline output
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Serialize, Deserialize, Default)]
struct SmithConfig {
    projects: Vec<ProjectConfig>,
    /// Legacy: global github token (no longer used; PRs use project.github_token)
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<GitHubConfig>,
    /// Legacy: single agent image (used when agents list is empty)
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<AgentConfig>,
    /// Named agents (e.g. opencode = OpenCode image). When set, used for resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    agents: Option<Vec<AgentEntry>>,
    /// Which agent name to use for ask/dev/review (default: "opencode")
    #[serde(skip_serializing_if = "Option::is_none")]
    current_agent: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct AgentConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct AgentRole {
    /// Mode for this role (e.g., "build", "plan", "ask", "review", "edit")
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    /// Model override for this role (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Prompt prefix for this role
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct AgentEntry {
    /// Unique id/name for the agent
    name: String,
    /// Docker image (e.g. ghcr.io/anomalyco/opencode); custom images allowed
    image: String,
    /// Agent type: "local" or "cloud" (default: "cloud")
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_type: Option<String>,
    /// Model to use (e.g. "anthropic/claude-sonnet-4-5", "qwen3:8b")
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Smaller model for internal operations (reduces API costs)
    #[serde(skip_serializing_if = "Option::is_none")]
    small_model: Option<String>,
    /// Provider name: "ollama", "anthropic", "openai", "openrouter", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    /// Custom base URL (for proxies/custom endpoints)
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    /// Port for opencode serve (default: 4096 + index)
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    /// Whether this agent is enabled (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    /// Default role name for this agent
    #[serde(skip_serializing_if = "Option::is_none")]
    default_role: Option<String>,
    /// Roles defined for this agent (keyed by role name)
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<HashMap<String, AgentRole>>,
}

/// Resolve port for an agent: port if set, else OPENCODE_SERVER_PORT + index.
fn agent_port(entry: &AgentEntry, index: usize) -> u16 {
    entry
        .port
        .unwrap_or_else(|| docker::OPENCODE_SERVER_PORT + index as u16)
}

#[derive(Serialize, Deserialize, Clone)]
struct GitHubConfig {
    token: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ProjectConfig {
    name: String,
    repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ssh_key: Option<String>,
    /// Base branch to clone and compare against (e.g. main). All actions fetch and use remote ref.
    #[serde(skip_serializing_if = "Option::is_none")]
    base_branch: Option<String>,
    /// Remote name for fetch/push (e.g. origin). All comparisons use refs on this remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    remote: Option<String>,
    /// GitHub personal access token for PR creation (--pr). Per-repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    github_token: Option<String>,
    /// Script to run in container before pipeline (e.g., install OpenCode).
    /// Example: "curl -fsSL https://opencode.ai/install.sh | sh"
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<String>,
    /// Commit author name (overrides local git config)
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_name: Option<String>,
    /// Commit author email (overrides local git config)
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_email: Option<String>,
    /// Agent name to use for this project (overrides current_agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    /// Pipeline step: ask.setup.run
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_setup_run: Option<String>,
    /// Pipeline step: ask.setup.check
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_setup_check: Option<String>,
    /// Pipeline step: ask.execute.run
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_execute_run: Option<String>,
    /// Pipeline step: ask.execute.check
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_execute_check: Option<String>,
    /// Pipeline step: ask.validate.run
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_validate_run: Option<String>,
    /// Pipeline step: ask.validate.check
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_validate_check: Option<String>,
    /// Pipeline step: dev.setup.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_setup_run: Option<String>,
    /// Pipeline step: dev.setup.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_setup_check: Option<String>,
    /// Pipeline step: dev.execute.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_execute_run: Option<String>,
    /// Pipeline step: dev.execute.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_execute_check: Option<String>,
    /// Pipeline step: dev.validate.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_validate_run: Option<String>,
    /// Pipeline step: dev.validate.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_validate_check: Option<String>,
    /// Pipeline step: dev.commit.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_commit_run: Option<String>,
    /// Pipeline step: dev.commit.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_commit_check: Option<String>,
    /// Pipeline step: review.setup.run
    #[serde(skip_serializing_if = "Option::is_none")]
    review_setup_run: Option<String>,
    /// Pipeline step: review.setup.check
    #[serde(skip_serializing_if = "Option::is_none")]
    review_setup_check: Option<String>,
    /// Pipeline step: review.execute.run
    #[serde(skip_serializing_if = "Option::is_none")]
    review_execute_run: Option<String>,
    /// Pipeline step: review.execute.check
    #[serde(skip_serializing_if = "Option::is_none")]
    review_execute_check: Option<String>,
    /// Pipeline step: review.validate.run
    #[serde(skip_serializing_if = "Option::is_none")]
    review_validate_run: Option<String>,
    /// Pipeline step: review.validate.check
    #[serde(skip_serializing_if = "Option::is_none")]
    review_validate_check: Option<String>,
}

fn config_dir() -> Result<PathBuf, String> {
    ProjectDirs::from("com", "agent", "smith")
        .ok_or_else(|| "Could not determine config directory".to_string())
        .map(|dirs| dirs.config_dir().to_path_buf())
}

/// Build the Docker image for one agent: ensure agent dir and Dockerfile exist, then run docker build.
/// `port` is written into the Dockerfile (EXPOSE and CMD) and should match the agent's port or default.
#[allow(clippy::too_many_arguments)]
fn build_agent_image(
    config_dir: &Path,
    name: &str,
    base_image: &str,
    port: u16,
    model: Option<&str>,
    small_model: Option<&str>,
    _provider: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let agent_dir = config_dir.join("agents").join(name);
    fs::create_dir_all(&agent_dir).map_err(|e| format!("Failed to create agent dir: {}", e))?;
    let dockerfile_path = agent_dir.join("Dockerfile");

    let port_str = port.to_string();
    let mut env_lines = String::new();
    let mut cmd_args = vec!["serve", "--hostname", "0.0.0.0", "--port", &port_str];

    if let Some(m) = model {
        cmd_args.push("--model");
        cmd_args.push(m);
        env_lines.push_str(&format!("ENV OPENCODE_MODEL=\"{}\"\n", m));
    }

    if let Some(sm) = small_model {
        env_lines.push_str(&format!("ENV OPENCODE_SMALL_MODEL=\"{}\"\n", sm));
    }

    let opencode_config = if model.is_some() || small_model.is_some() {
        let mut cfg = String::from("{\n");
        if let Some(m) = model {
            cfg.push_str(&format!("  \"model\": \"{}\",\n", m));
        }
        if let Some(sm) = small_model {
            cfg.push_str(&format!("  \"small_model\": \"{}\",\n", sm));
        }
        cfg.push_str("\n}\n");
        Some(cfg)
    } else {
        None
    };

    if !dockerfile_path.exists() || force {
        let mut content = format!(
            r#"FROM {}

LABEL smith.agent.name="{}"

EXPOSE {}

"#,
            base_image, name, port
        );

        content.push_str(&env_lines);

        if let Some(ref cfg) = opencode_config {
            let config_path = agent_dir.join("opencode.jsonc");
            fs::write(&config_path, cfg)
                .map_err(|e| format!("Failed to write opencode config: {}", e))?;
            content.push_str("COPY opencode.jsonc /home/opencode.jsonc\n");
            content.push_str("ENV OPENCODE_CONFIG=/home/opencode.jsonc\n");
        }

        content.push_str(&format!(
            r#"
ENTRYPOINT ["opencode"]
CMD ["{}"]
"#,
            cmd_args.join("\", \"")
        ));

        fs::write(&dockerfile_path, content)
            .map_err(|e| format!("Failed to write Dockerfile: {}", e))?;
    }
    let tag = docker::agent_built_image_tag(name);
    if force {
        let _ = Command::new("docker").args(["rmi", "-f", &tag]).output();
    }
    let mut args = vec!["build", "-t", &tag];
    if force {
        args.push("--no-cache");
    }
    let output = Command::new("docker")
        .args(&args)
        .arg(agent_dir.as_path())
        .output()
        .map_err(|e| format!("Failed to run docker build: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("docker build failed: {}", stderr.trim()))
    }
}

fn config_file_path() -> Result<PathBuf, String> {
    config_dir().map(|dir| dir.join("config.toml"))
}

/// Prompt for confirmation; returns true if user types "yes"/"y" (case-insensitive) or if force is true.
fn confirm_reset(prompt: &str, force: bool) -> bool {
    if force {
        return true;
    }
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    let line = line.trim().to_lowercase();
    line == "yes" || line == "y"
}

/// Read a line from stdin (trimmed). For interactive install wizard.
fn prompt_line(prompt: &str) -> String {
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    line.trim().to_string()
}

/// Prompt [y/N]; returns true for y/yes, false for n/no or empty (default no).
fn prompt_yn(prompt: &str, default_no: bool) -> bool {
    let hint = if default_no { " [y/N]" } else { " [Y/n]" };
    let line = prompt_line(&format!("{}{}: ", prompt, hint)).to_lowercase();
    if line.is_empty() {
        return !default_no;
    }
    matches!(line.as_str(), "y" | "yes")
}

/// Prompt [y/N/skip]; returns Some(true)=y, Some(false)=n, None=skip.
fn prompt_yn_skip(prompt: &str) -> Option<bool> {
    let line = prompt_line(&format!("{} [y/N/skip]: ", prompt)).to_lowercase();
    if line.is_empty() || line == "n" || line == "no" {
        return Some(false);
    }
    if line == "skip" || line == "s" {
        return None;
    }
    if line == "y" || line == "yes" {
        return Some(true);
    }
    Some(false)
}

/// Add an agent to config (used by CLI `agent add` and install wizard). Does not save.
#[allow(clippy::too_many_arguments)]
fn add_agent_to_config(
    cfg: &mut SmithConfig,
    name: String,
    image: Option<String>,
    agent_type: Option<String>,
    model: Option<String>,
    small_model: Option<String>,
    provider: Option<String>,
    base_url: Option<String>,
    port: Option<u16>,
    enabled: Option<bool>,
    default_role: Option<String>,
    roles: Option<HashMap<String, AgentRole>>,
) -> Result<(), String> {
    if cfg
        .agents
        .as_ref()
        .is_some_and(|a| a.iter().any(|e| e.name == name))
    {
        return Err(format!("Agent '{}' already exists", name));
    }
    let image = image
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_IMAGE.to_string());
    let agent_type = agent_type.filter(|s| !s.is_empty());
    let model = model.filter(|s| !s.is_empty());
    let small_model = small_model.filter(|s| !s.is_empty());
    let provider = provider.filter(|s| !s.is_empty());
    let base_url = base_url.filter(|s| !s.is_empty());
    let agents = cfg.agents.get_or_insert_with(Vec::new);

    // First agent becomes "default" if no name specified
    let agent_name = if name.is_empty() && agents.is_empty() {
        "default".to_string()
    } else {
        name
    };

    // If no roles provided, create default "*" role
    let roles = if roles.is_none() {
        let mut default_roles = HashMap::new();
        default_roles.insert(
            "*".to_string(),
            AgentRole {
                mode: None,
                model: model.clone(),
                prompt: None,
            },
        );
        Some(default_roles)
    } else {
        roles
    };

    let default_role = default_role.or_else(|| Some("*".to_string()));
    let port = port.or_else(|| Some(docker::OPENCODE_SERVER_PORT + agents.len() as u16));
    let enabled = enabled.or(Some(true));
    agents.push(AgentEntry {
        name: agent_name.clone(),
        image,
        agent_type,
        model,
        small_model,
        provider,
        base_url,
        port,
        enabled,
        default_role,
        roles,
    });
    if cfg.current_agent.is_none() {
        cfg.current_agent = Some(agent_name);
    }
    Ok(())
}

/// Add a project to config (used by CLI `project add` and install wizard). Does not save.
fn add_project_to_config(cfg: &mut SmithConfig, project: ProjectConfig) -> Result<(), String> {
    if cfg.projects.iter().any(|p| p.name == project.name) {
        return Err(format!("Project '{}' already exists", project.name));
    }
    cfg.projects.push(project);
    Ok(())
}

fn load_config() -> Result<SmithConfig, String> {
    let file = config_file_path()?;
    if !file.exists() {
        return Ok(SmithConfig::default());
    }
    let content = fs::read_to_string(&file).map_err(|e| format!("Failed to read config: {}", e))?;
    toml::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
}

fn save_config(config: &SmithConfig) -> Result<(), String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config directory: {}", e))?;
    let file = config_file_path()?;
    let content =
        toml::to_string_pretty(config).map_err(|e| format!("Failed to serialize config: {}", e))?;

    // Atomic write: write to temp file then rename. On EXDEV (cross-filesystem), fall back to copy + remove.
    let temp_file = file.with_extension("toml.tmp");
    fs::write(&temp_file, content).map_err(|e| format!("Failed to write config: {}", e))?;
    if let Err(e) = fs::rename(&temp_file, &file) {
        // EXDEV = cross-filesystem rename not supported (MSRV 1.83: avoid ErrorKind::CrossesDevices)
        let is_cross_device = e.raw_os_error() == Some(libc::EXDEV);
        if is_cross_device {
            fs::copy(&temp_file, &file).map_err(|e| format!("Failed to copy config: {}", e))?;
            fs::remove_file(&temp_file)
                .map_err(|e| format!("Failed to remove temp config: {}", e))?;
        } else {
            return Err(format!("Failed to finalize config: {}", e));
        }
    }
    Ok(())
}

const INSTALLED_MARKER: &str = ".smith-installed";

/// True if the user has run `smith install` (marker file in config dir).
fn is_installed() -> bool {
    config_dir()
        .map(|d| d.join(INSTALLED_MARKER).exists())
        .unwrap_or(false)
}

/// Version last recorded by `smith install` (for migrations). None = not installed; Some("") = legacy empty marker.
fn installed_version() -> Option<String> {
    let path = config_dir().ok()?.join(INSTALLED_MARKER);
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

/// If Docker is not available, try to install it via the official get.docker.com script (Linux only).
/// Requires sudo. On success, starts the docker service. Non-fatal on failure.
#[cfg(target_os = "linux")]
fn try_install_docker() {
    if docker::check_docker_available().is_ok() {
        println!("  {} docker - available", BULLET_GREEN);
        return;
    }
    println!(
        "  {} docker - installing (https://get.docker.com) ...",
        BULLET_BLUE
    );
    let script_path = std::env::temp_dir().join("get-docker.sh");
    let curl = Command::new("curl")
        .args(["-fsSL", "https://get.docker.com", "-o"])
        .arg(&script_path)
        .status();
    if let Ok(s) = curl {
        if !s.success() {
            eprintln!(
                "  {} docker - install failed (could not download script)",
                BULLET_RED
            );
            return;
        }
    } else {
        eprintln!("  {} docker - install skipped (curl not found)", BULLET_RED);
        return;
    }
    let install = Command::new("sudo")
        .args(["sh", script_path.to_str().unwrap_or("get-docker.sh")])
        .status();
    let _ = fs::remove_file(&script_path);
    match install {
        Ok(s) if s.success() => {
            let _ = Command::new("sudo")
                .args(["systemctl", "start", "docker"])
                .status();
            let _ = Command::new("sudo")
                .args(["systemctl", "enable", "docker"])
                .status();
            println!(
                "  {} docker - available",
                BULLET_GREEN
            );
            println!("       Log out and back in to use Docker without sudo (docker group).");
        }
        _ => eprintln!(
            "  {} docker - install failed (run manually: curl -fsSL https://get.docker.com | sudo sh)",
            BULLET_RED
        ),
    }
}

#[cfg(not(target_os = "linux"))]
fn try_install_docker() {}

/// On Linux, ensure Docker service is started and enabled (so it runs on boot). No-op if Docker not available. Non-fatal.
#[cfg(target_os = "linux")]
fn ensure_docker_started_and_enabled() {
    if docker::check_docker_available().is_err() {
        return;
    }
    let _ = Command::new("sudo")
        .args(["systemctl", "start", "docker"])
        .status();
    let _ = Command::new("sudo")
        .args(["systemctl", "enable", "docker"])
        .status();
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
fn ensure_docker_started_and_enabled() {}

/// If Dagger CLI is not in PATH, try to install it via the official install script (Linux only).
/// Installs to $HOME/.local/bin. Non-fatal on failure.
#[cfg(target_os = "linux")]
fn try_install_dagger() {
    if Command::new("dagger")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        println!("  {} dagger - running", BULLET_GREEN);
        return;
    }
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            eprintln!("  {} dagger - install skipped (HOME not set)", BULLET_RED);
            return;
        }
    };
    let bin_dir = format!("{}/.local/bin", home);
    if fs::create_dir_all(&bin_dir).is_err() {
        eprintln!(
            "  {} dagger - install skipped (could not create {})",
            BULLET_RED, bin_dir
        );
        return;
    }
    println!("  {} dagger - installing to {} ...", BULLET_BLUE, bin_dir);
    let status = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://dl.dagger.io/dagger/install.sh | sh")
        .env("BIN_DIR", &bin_dir)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!(
                "  {} dagger - running (ensure {} is in PATH)",
                BULLET_GREEN,
                bin_dir
            );
        }
        _ => eprintln!(
            "  {} dagger - install failed (run manually: curl -fsSL https://dl.dagger.io/dagger/install.sh | BIN_DIR=$HOME/.local/bin sh)",
            BULLET_RED
        ),
    }
}

#[cfg(not(target_os = "linux"))]
fn try_install_dagger() {}

/// Write config dir and install marker with current version (called at end of install wizard).
/// Storing the version allows future migrations to run when upgrading.
fn run_install_finish() -> Result<(), String> {
    if let Ok(dir) = config_dir() {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
        let version = env!("CARGO_PKG_VERSION");
        fs::write(dir.join(INSTALLED_MARKER), version)
            .map_err(|e| format!("Failed to write install marker: {}", e))?;
    }
    Ok(())
}

fn resolve_repo(repo: Option<String>, project: Option<String>) -> Result<String, String> {
    if let Some(r) = repo {
        return Ok(r);
    }
    if let Some(p) = project {
        let cfg = load_config()?;
        let proj = cfg
            .projects
            .iter()
            .find(|pr| pr.name == p)
            .ok_or_else(|| format!("Project '{}' not found", p))?;
        return Ok(proj.repo.clone());
    }
    Err("Either --repo or --project must be provided".to_string())
}

fn resolve_project_config(project: Option<String>) -> Result<Option<ProjectConfig>, String> {
    if let Some(p) = project {
        let cfg = load_config()?;
        let proj = cfg
            .projects
            .iter()
            .find(|pr| pr.name == p)
            .ok_or_else(|| format!("Project '{}' not found", p))?;
        return Ok(Some(proj.clone()));
    }
    Ok(None)
}

/// Resolve agent container image. Priority: explicit --image > named agent (current or "opencode") > legacy agent.image > DEFAULT_AGENT_IMAGE.
fn resolve_agent_image(explicit_image: Option<&str>) -> String {
    if let Some(s) = explicit_image {
        return s.to_string();
    }
    let cfg = match load_config() {
        Ok(c) => c,
        Err(_) => return DEFAULT_AGENT_IMAGE.to_string(),
    };
    let name = cfg.current_agent.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
    if let Some(ref agents) = cfg.agents {
        if let Some(entry) = agents.iter().find(|a| a.name == name) {
            return entry.image.clone();
        }
    }
    cfg.agent
        .as_ref()
        .and_then(|a| a.image.clone())
        .unwrap_or_else(|| DEFAULT_AGENT_IMAGE.to_string())
}

/// Resolve SSH key path: explicit --ssh-key > project ssh_key > SSH_KEY_PATH env
fn resolve_ssh_key(
    explicit: Option<&PathBuf>,
    project_config: Option<&ProjectConfig>,
) -> Option<PathBuf> {
    explicit
        .cloned()
        .or_else(|| {
            project_config
                .and_then(|p| p.ssh_key.as_ref())
                .map(PathBuf::from)
        })
        .or_else(|| std::env::var("SSH_KEY_PATH").ok().map(PathBuf::from))
}

/// Resolve base branch: explicit CLI --base > project base_branch > "main"
fn resolve_base_branch(explicit: Option<&str>, project_config: Option<&ProjectConfig>) -> String {
    explicit
        .map(String::from)
        .or_else(|| project_config.and_then(|p| p.base_branch.clone()))
        .unwrap_or_else(|| "main".to_string())
}

/// Resolve project script: project script > None
fn resolve_project_script(project_config: Option<&ProjectConfig>) -> Option<String> {
    project_config.and_then(|p| p.script.clone())
}

/// Resolve remote name: project remote > "origin"
fn resolve_remote(project_config: Option<&ProjectConfig>) -> String {
    project_config
        .and_then(|p| p.remote.clone())
        .unwrap_or_else(|| "origin".to_string())
}

/// Resolve commit name/email from project config (returns None if not set = use local git)
fn resolve_commit_author(
    project_config: Option<&ProjectConfig>,
) -> (Option<String>, Option<String>) {
    let commit_name = project_config.and_then(|p| p.commit_name.clone());
    let commit_email = project_config.and_then(|p| p.commit_email.clone());
    (commit_name, commit_email)
}

/// Resolve pipeline step role: returns (agent_name, role_name, mode, model, prompt)
/// Looks up step in project config, parses "agent:role", resolves role from agent config
fn resolve_pipeline_role(
    project_config: Option<&ProjectConfig>,
    step: &str,
    current_agent: Option<&str>,
) -> Option<(
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Get agent name from project or current_agent
    let agent_name = project_config
        .and_then(|p| p.agent.clone())
        .or_else(|| current_agent.map(String::from))?;

    // Get step mapping from project config
    let step_mapping = match step {
        "ask_setup_run" => project_config?.ask_setup_run.clone(),
        "ask_setup_check" => project_config?.ask_setup_check.clone(),
        "ask_execute_run" => project_config?.ask_execute_run.clone(),
        "ask_execute_check" => project_config?.ask_execute_check.clone(),
        "ask_validate_run" => project_config?.ask_validate_run.clone(),
        "ask_validate_check" => project_config?.ask_validate_check.clone(),
        "dev_setup_run" => project_config?.dev_setup_run.clone(),
        "dev_setup_check" => project_config?.dev_setup_check.clone(),
        "dev_execute_run" => project_config?.dev_execute_run.clone(),
        "dev_execute_check" => project_config?.dev_execute_check.clone(),
        "dev_validate_run" => project_config?.dev_validate_run.clone(),
        "dev_validate_check" => project_config?.dev_validate_check.clone(),
        "dev_commit_run" => project_config?.dev_commit_run.clone(),
        "dev_commit_check" => project_config?.dev_commit_check.clone(),
        "review_setup_run" => project_config?.review_setup_run.clone(),
        "review_setup_check" => project_config?.review_setup_check.clone(),
        "review_execute_run" => project_config?.review_execute_run.clone(),
        "review_execute_check" => project_config?.review_execute_check.clone(),
        "review_validate_run" => project_config?.review_validate_run.clone(),
        "review_validate_check" => project_config?.review_validate_check.clone(),
        _ => None,
    };

    // Parse "agent:role" or just "role" (use project agent)
    let (resolved_agent, role_name) = if let Some(ref mapping) = step_mapping {
        if mapping.contains(':') {
            let parts: Vec<&str> = mapping.split(':').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (agent_name.clone(), mapping.clone())
        }
    } else {
        // Fall back to agent's default_role
        let agents = cfg.agents.as_ref()?;
        let agent = agents.iter().find(|a| a.name == agent_name)?;
        let default_role = agent.default_role.as_ref()?.clone();
        (agent_name, default_role)
    };

    // Get role config from agent, with * fallback
    let agents = cfg.agents.as_ref()?;
    let agent = agents.iter().find(|a| a.name == resolved_agent)?;
    let roles = agent.roles.as_ref()?;

    // Try the specified role first, then fall back to "*"
    let role = roles.get(&role_name).or_else(|| roles.get("*"))?;

    Some((
        resolved_agent,
        role_name,
        role.mode.clone(),
        role.model.clone(),
        role.prompt.clone(),
    ))
}

fn resolve_pipeline_roles(
    project_config: Option<&ProjectConfig>,
    pipeline_type: &str,
) -> dagger_pipeline::PipelineRoles {
    use dagger_pipeline::{PipelineRoles as PR, RoleInfo as RI};
    let mut roles = PR::default();

    let steps = [
        ("setup_run", format!("{}_setup_run", pipeline_type)),
        ("setup_check", format!("{}_setup_check", pipeline_type)),
        ("execute_run", format!("{}_execute_run", pipeline_type)),
        ("execute_check", format!("{}_execute_check", pipeline_type)),
        ("validate_run", format!("{}_validate_run", pipeline_type)),
        (
            "validate_check",
            format!("{}_validate_check", pipeline_type),
        ),
        ("commit_run", format!("{}_commit_run", pipeline_type)),
        ("commit_check", format!("{}_commit_check", pipeline_type)),
    ];

    for (step_key, step_name) in steps {
        if let Some((_, _, _, model, prompt)) =
            resolve_pipeline_role(project_config, &step_name, None)
        {
            let role_info = RI::new(model, prompt);
            match step_key {
                "setup_run" => roles.setup_run = Some(role_info),
                "setup_check" => roles.setup_check = Some(role_info),
                "execute_run" => roles.execute_run = Some(role_info),
                "execute_check" => roles.execute_check = Some(role_info),
                "validate_run" => roles.validate_run = Some(role_info),
                "validate_check" => roles.validate_check = Some(role_info),
                "commit_run" => roles.commit_run = Some(role_info),
                "commit_check" => roles.commit_check = Some(role_info),
                _ => {}
            }
        }
    }

    roles
}

/// Column width for subcommand names so descriptions align (clap-style).
const HELP_NAME_WIDTH: usize = 18;

fn print_smith_help() {
    let c = Cli::command();
    if let Some(about) = c.get_about() {
        println!("{}\n", about);
    }
    println!("Usage: smith [COMMAND]");
    const SYSTEM: &[&str] = &["status", "install", "uninstall", "help", "version"];
    const COMMANDS: &[&str] = &["agent", "project", "run"];
    println!("\nCommands:");
    for sub in c.get_subcommands() {
        let name = sub.get_name();
        if SYSTEM.contains(&name) {
            let short = sub.get_about().unwrap_or_default();
            println!("  {name:<HELP_NAME_WIDTH$}  {short}");
        }
    }
    println!();
    for sub in c.get_subcommands() {
        let name = sub.get_name();
        if COMMANDS.contains(&name) {
            let short = sub.get_about().unwrap_or_default();
            println!("  {name:<HELP_NAME_WIDTH$}  {short}");
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            print_smith_help();
            std::process::exit(0);
        }
        Some(Commands::Status { verbose }) => {
            let docker_ok = docker::check_docker_available().is_ok();
            let dagger_ok = {
                let _guard = if verbose {
                    None
                } else {
                    redirect_stderr_to_null()
                };
                dagger_pipeline::with_connection(|conn| async move {
                    dagger_pipeline::run_doctor(&conn).await
                })
                .await
                .is_ok()
            };
            let installed = is_installed();

            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let running = docker::list_running_agent_containers().unwrap_or_default();
            let list = cfg.agents.as_deref().unwrap_or(&[]);

            let smith_bullet = if installed && docker_ok {
                BULLET_GREEN
            } else if installed {
                BULLET_BLUE
            } else {
                BULLET_RED
            };
            println!("{} smith", smith_bullet);
            let d_bullet = if docker_ok { BULLET_GREEN } else { BULLET_RED };
            println!(
                "     {} docker - {}",
                d_bullet,
                if docker_ok {
                    "available"
                } else {
                    "unavailable"
                }
            );
            let g_bullet = if dagger_ok { BULLET_GREEN } else { BULLET_RED };
            println!(
                "     {} dagger - {}",
                g_bullet,
                if dagger_ok { "running" } else { "not running" }
            );
            if list.is_empty() {
                println!("  {} agents", BULLET_BLUE);
                println!("       (none)");
            } else {
                // First pass: determine aggregate status
                let mut agents_bullet = BULLET_BLUE;
                let mut has_cloud_or_running = false;
                for agent_entry in list.iter() {
                    let active = running.contains(&agent_entry.name);
                    let built =
                        docker::image_exists(&docker::agent_built_image_tag(&agent_entry.name)).unwrap_or(false);
                    let is_cloud = agent_entry
                        .agent_type
                        .as_deref()
                        .map(|t| t != "local")
                        .unwrap_or(true);
                    if is_cloud || active {
                        has_cloud_or_running = true;
                    } else if !built {
                        agents_bullet = BULLET_RED;
                    }
                }
                // Print header with aggregate status
                if agents_bullet == BULLET_RED {
                    println!("  {} agents", BULLET_RED);
                } else if has_cloud_or_running {
                    println!("  {} agents", BULLET_GREEN);
                } else {
                    println!("  {} agents", BULLET_BLUE);
                }
                // Second pass: print each agent
                for (i, agent_entry) in list.iter().enumerate() {
                    let name = &agent_entry.name;
                    let active = running.contains(name);
                    let built =
                        docker::image_exists(&docker::agent_built_image_tag(name)).unwrap_or(false);
                    let is_cloud = agent_entry
                        .agent_type
                        .as_deref()
                        .map(|t| t != "local")
                        .unwrap_or(true);
                    let port = agent_port(agent_entry, i);
                    let reachable = if active {
                        Some(docker::check_agent_reachable(port))
                    } else {
                        None
                    };
                    let bullet = if is_cloud {
                        BULLET_GREEN
                    } else if active && reachable == Some(false) {
                        BULLET_YELLOW
                    } else if active {
                        BULLET_GREEN
                    } else if built {
                        BULLET_BLUE
                    } else {
                        BULLET_RED
                    };
                    let state = if is_cloud {
                        "cloud"
                    } else if active && reachable == Some(false) {
                        "running (port unreachable)"
                    } else if active {
                        "running"
                    } else if built {
                        "built"
                    } else {
                        "not built"
                    };
                    if is_cloud {
                        println!("       {} {} - {}", bullet, name, state);
                    } else if active {
                        println!(
                            "       {} {} - {} {}",
                            bullet,
                            name,
                            state,
                            clickable_agent_url(port)
                        );
                    } else {
                        println!(
                            "       {} {} - {} (http://localhost:{})",
                            bullet, name, state, port
                        );
                    }
                }
            }

            // projects:
            let mut project_results: Vec<(String, bool, String)> = Vec::new();
            if !cfg.projects.is_empty() && dagger_ok {
                for proj in &cfg.projects {
                    let resolved_repo = &proj.repo;
                    let name = proj.name.clone();
                    if resolved_repo.starts_with("https://") {
                        project_results.push((
                            name,
                            false,
                            "skipped (HTTPS not supported)".to_string(),
                        ));
                        continue;
                    }
                    let base = resolve_base_branch(None, Some(proj));
                    let agent_image = resolve_agent_image(proj.image.as_deref());
                    let ssh_key_path = resolve_ssh_key(None, Some(proj));
                    let project_script = resolve_project_script(Some(proj));
                    let done = std::sync::Arc::new(AtomicBool::new(false));
                    let done_clone = done.clone();
                    let spinner_handle = if !verbose {
                        Some(thread::spawn(move || run_spinner_until(&done_clone)))
                    } else {
                        None
                    };
                    let _guard = if !verbose {
                        redirect_stderr_to_null()
                    } else {
                        None
                    };
                    let repo = resolved_repo.clone();
                    let branch = base.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let scr = project_script.clone();
                    let result = dagger_pipeline::with_connection(move |conn| {
                        let repo = repo.clone();
                        let branch = branch.clone();
                        let img = img.clone();
                        let ssh = ssh.clone();
                        async move {
                            dagger_pipeline::run_project_status(
                                &conn,
                                &repo,
                                &branch,
                                &img,
                                ssh.as_deref(),
                                scr.as_deref(),
                            )
                            .await
                            .map_err(|e| eyre::eyre!("{}", e))
                        }
                    })
                    .await;
                    if !verbose {
                        done.store(true, Ordering::Relaxed);
                        if let Some(h) = spinner_handle {
                            let _: std::thread::Result<()> = h.join();
                        }
                    }
                    drop(_guard);
                    match result {
                        Ok(output) => {
                            let has_git = !output.contains("no-git") && output.len() > 2;
                            let has_opencode = output.contains('.') && output.chars().filter(|c| *c == '.').count() >= 2;
                            if has_git && has_opencode {
                                project_results.push((name, true, "ready".to_string()));
                            } else if !has_git {
                                project_results.push((
                                    name,
                                    false,
                                    "clone failed".to_string(),
                                ));
                            } else {
                                project_results.push((
                                    name,
                                    false,
                                    "opencode not available".to_string(),
                                ));
                            }
                        }
                        Err(e) => {
                            project_results.push((name, false, format!("clone failed: {}", e)));
                        }
                    }
                }
            }
            let project_ok_count = project_results.iter().filter(|(_, ok, _)| *ok).count();
            let project_total = project_results.len();
            let projects_bullet = if project_total == 0 {
                BULLET_RED
            } else if project_ok_count == project_total {
                BULLET_GREEN
            } else if project_ok_count > 0 {
                BULLET_YELLOW
            } else {
                BULLET_RED
            };
            println!("  {} projects", projects_bullet);
            if project_results.is_empty() {
                if cfg.projects.is_empty() {
                    println!("       (none)");
                } else if !dagger_ok {
                    println!("       (dagger not running)");
                }
            } else {
                for (name, ok, msg) in &project_results {
                    let bullet = if *ok { BULLET_GREEN } else { BULLET_RED };
                    println!("       {} {} - {}", bullet, name, msg);
                }
            }

            if verbose {
                println!();
                println!("  --- verbose ---");
                if let Ok(dir) = config_dir() {
                    let config_path = dir.join("config.toml");
                    println!("  config: {}", config_path.display());
                }
                if let Some(v) = installed_version() {
                    println!(
                        "  installed_version: {}",
                        if v.is_empty() {
                            "(legacy, unknown)"
                        } else {
                            &v
                        }
                    );
                } else {
                    println!("  installed_version: (not installed)");
                }
                if docker_ok {
                    if let Ok(o) = Command::new("docker").arg("--version").output() {
                        let out = String::from_utf8_lossy(&o.stdout);
                        let v = out.trim();
                        if !v.is_empty() {
                            println!("  docker: {}", v);
                        }
                    }
                    if let Ok(o) = Command::new("docker").arg("info").output() {
                        let out = String::from_utf8_lossy(&o.stdout);
                        for line in out.lines().take(15) {
                            println!("    {}", line);
                        }
                        if out.lines().count() > 15 {
                            println!("    ...");
                        }
                    }
                }
                println!("  running agent containers: {:?}", running);
                let agents_list: Vec<&AgentEntry> = list.iter().collect();
                if agents_list.is_empty() {
                    println!(
                        "  agent config: [{} (default)] {}",
                        DEFAULT_AGENT_NAME,
                        clickable_agent_url(docker::OPENCODE_SERVER_PORT)
                    );
                } else {
                    for (i, e) in agents_list.iter().enumerate() {
                        let port = agent_port(e, i);
                        println!(
                            "  agent config: {} -> image={} port={} {}",
                            e.name,
                            e.image,
                            port,
                            clickable_agent_url(port)
                        );
                    }
                }
            }
        }
        Some(Commands::Install) => {
            println!("{} smith install", BULLET_GREEN);
            println!();
            // --- Dependencies ---
            println!("  Dependencies:");
            try_install_docker();
            try_install_dagger();
            println!();
            // --- Docker always run (Linux) ---
            #[cfg(target_os = "linux")]
            {
                if docker::check_docker_available().is_ok() {
                    println!("  Always run Docker at boot so agents stay available after restart?");
                    println!("  (Requires sudo / password to run systemctl enable docker)");
                    if prompt_yn("Enable Docker at boot?", true) {
                        ensure_docker_started_and_enabled();
                        println!("  {} Docker - enabled at boot", BULLET_GREEN);
                    }
                    println!();
                }
            }
            // --- Config and agents ---
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let agents_list = cfg.agents.as_deref().unwrap_or(&[]);
            if agents_list.is_empty() {
                println!("  Current agents: (none)");
                println!();
                match prompt_yn("Create default opencode agent?", true) {
                    false => {
                        println!("  No agent created. Use 'smith agent add' later.");
                    }
                    true => {
                        let new_agent = AgentEntry {
                            name: DEFAULT_AGENT_NAME.to_string(),
                            image: DEFAULT_AGENT_IMAGE.to_string(),
                            agent_type: Some("cloud".to_string()),
                            model: None,
                            small_model: None,
                            provider: None,
                            base_url: None,
                            port: None,
                            enabled: Some(true),
                            default_role: None,
                            roles: None,
                        };
                        cfg.agents = Some(vec![new_agent]);
                        if let Err(e) = save_config(&cfg) {
                            eprintln!("Error saving config: {}", e);
                        } else {
                            println!("  {} Created default agent: {}", BULLET_GREEN, DEFAULT_AGENT_NAME);
                        }
                    }
                }
            } else {
                println!("  Current agents:");
                for a in agents_list {
                    println!("    - {} (image: {})", a.name, a.image);
                }
            }
            println!();
            match prompt_yn_skip("Add more agents?") {
                None => println!("  Skipping agents."),
                Some(false) => {}
                Some(true) => loop {
                    let name = prompt_line("  Agent name: ");
                    if name.is_empty() {
                        println!("  Agent name cannot be empty.");
                        continue;
                    }
                    let image_default = DEFAULT_AGENT_IMAGE.to_string();
                    let image_in = prompt_line(&format!("  Image [{}]: ", image_default));
                    let image = if image_in.is_empty() {
                        None
                    } else {
                        Some(image_in)
                    };
                    let model_in =
                        prompt_line("  Model (e.g. anthropic/claude-sonnet-4-5, Enter to skip): ");
                    let model = if model_in.is_empty() {
                        None
                    } else {
                        Some(model_in)
                    };
                    let small_model_in =
                        prompt_line("  Small model for internal ops (optional, Enter to skip): ");
                    let small_model = if small_model_in.is_empty() {
                        None
                    } else {
                        Some(small_model_in)
                    };
                    let provider_in = prompt_line(
                        "  Provider (e.g. ollama, anthropic, openai, Enter for cloud default): ",
                    );
                    let provider = if provider_in.is_empty() {
                        None
                    } else {
                        Some(provider_in)
                    };
                    let base_url_in =
                        prompt_line("  Base URL for provider (optional, Enter to skip): ");
                    let base_url = if base_url_in.is_empty() {
                        None
                    } else {
                        Some(base_url_in)
                    };
                    let type_in = prompt_line("  Type (local or cloud, Enter for cloud): ");
                    let agent_type = if type_in.is_empty() {
                        None
                    } else {
                        Some(type_in)
                    };
                    let port_in =
                        prompt_line("  Port for opencode serve (Enter for default 4096): ");
                    let port = if port_in.is_empty() {
                        None
                    } else {
                        port_in.parse().ok()
                    };
                    let enabled = Some(true);
                    match add_agent_to_config(
                        &mut cfg,
                        name.clone(),
                        image,
                        agent_type,
                        model,
                        small_model,
                        provider,
                        base_url,
                        port,
                        enabled,
                        None,
                        None,
                    ) {
                        Ok(()) => println!("  {} Added agent '{}'", BULLET_GREEN, name),
                        Err(e) => eprintln!("  {} {}", BULLET_RED, e),
                    }
                    save_config(&cfg).unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });
                    if !prompt_yn("Add another agent?", false) {
                        break;
                    }
                },
            }
            println!();
            // --- Projects ---
            if cfg.projects.is_empty() {
                println!("  Current projects: (none)");
            } else {
                println!("  Current projects:");
                for p in &cfg.projects {
                    println!("    - {} ({})", p.name, p.repo);
                }
            }
            println!();
            match prompt_yn_skip("Add any projects?") {
                None => println!("  Skipping projects."),
                Some(false) => {}
                Some(true) => loop {
                    let name = prompt_line("  Project name: ");
                    if name.is_empty() {
                        println!("  Project name cannot be empty.");
                        continue;
                    }
                    let repo = prompt_line("  Repository (URL or path): ");
                    if repo.is_empty() {
                        println!("  Repository is required.");
                        continue;
                    }
                    let image_in = prompt_line("  Image (optional, Enter to skip): ");
                    let image = if image_in.is_empty() {
                        None
                    } else {
                        Some(image_in)
                    };
                    let ssh_key_in = prompt_line("  SSH key path (optional): ");
                    let ssh_key = if ssh_key_in.is_empty() {
                        None
                    } else {
                        Some(ssh_key_in)
                    };
                    let base_in = prompt_line("  Base branch [main]: ");
                    let base_branch = if base_in.is_empty() {
                        None
                    } else {
                        Some(base_in)
                    };
                    let remote_in = prompt_line("  Remote name [origin]: ");
                    let remote = if remote_in.is_empty() {
                        None
                    } else {
                        Some(remote_in)
                    };
                    let token_in = prompt_line("  GitHub token for PRs (optional): ");
                    let github_token = if token_in.is_empty() {
                        None
                    } else {
                        Some(token_in)
                    };
                    let script_in = prompt_line("  Script to run in container (optional, e.g. curl -fsSL https://opencode.ai/install | sh): ");
                    let script = if script_in.is_empty() {
                        None
                    } else {
                        Some(script_in)
                    };
                    let project = ProjectConfig {
                        name: name.clone(),
                        repo,
                        image,
                        ssh_key,
                        base_branch,
                        remote,
                        github_token,
                        script,
                        commit_name: None,
                        commit_email: None,
                        agent: None,
                        ask_setup_run: None,
                        ask_setup_check: None,
                        ask_execute_run: None,
                        ask_execute_check: None,
                        ask_validate_run: None,
                        ask_validate_check: None,
                        dev_setup_run: None,
                        dev_setup_check: None,
                        dev_execute_run: None,
                        dev_execute_check: None,
                        dev_validate_run: None,
                        dev_validate_check: None,
                        dev_commit_run: None,
                        dev_commit_check: None,
                        review_setup_run: None,
                        review_setup_check: None,
                        review_execute_run: None,
                        review_execute_check: None,
                        review_validate_run: None,
                        review_validate_check: None,
                    };
                    match add_project_to_config(&mut cfg, project) {
                        Ok(()) => {
                            save_config(&cfg).unwrap_or_else(|e| {
                                eprintln!("Error: {}", e);
                                std::process::exit(1);
                            });
                            println!("  {} Added project '{}'", BULLET_GREEN, name);
                        }
                        Err(e) => eprintln!("  {} {}", BULLET_RED, e),
                    }
                    if !prompt_yn("Add another project?", false) {
                        break;
                    }
                },
            }
            println!();
            if let Err(e) = run_install_finish() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!(
                "  {} You're ready to run agentic pipelines via `smith run`",
                BULLET_GREEN
            );
            println!("     (e.g. smith run dev, smith run ask, smith run review)");
        }
        Some(Commands::Help) => {
            print_smith_help();
            std::process::exit(0);
        }
        Some(Commands::Version) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Uninstall {
            force,
            remove_config,
            remove_images,
        }) => {
            let prompt = "This will stop all agent containers and Ollama. Continue? Type 'yes' to confirm: ";
            if !confirm_reset(prompt, force) {
                eprintln!("Uninstall cancelled.");
                std::process::exit(1);
            }

            println!("{} smith uninstall", BULLET_YELLOW);
            println!();

            if let Err(e) = docker::check_docker_available() {
                eprintln!("Warning: Docker not available - skipping container cleanup: {}", e);
            } else {
                println!("  Stopping agent containers...");
                match docker::stop_all_agent_containers() {
                    Ok(stopped) => {
                        if stopped.is_empty() {
                            println!("    (no running agent containers)");
                        } else {
                            for name in &stopped {
                                println!("    {}: stopped", name);
                            }
                            println!("    Stopped {} container(s).", stopped.len());
                        }
                    }
                    Err(e) => eprintln!("    Warning: {}", e),
                }

                if docker::is_ollama_running() {
                    println!("  Stopping Ollama container...");
                    if let Err(e) = docker::stop_ollama_container() {
                        eprintln!("    Warning: {}", e);
                    } else {
                        println!("    Ollama: stopped");
                    }
                }

                if remove_images {
                    println!("  Removing Docker images...");
                    let _ = Command::new("docker")
                        .args(["rmi", "-f", "smith/unnamed:latest"])
                        .output();
                    let cfg = load_config().unwrap_or_default();
                    if let Some(agents) = cfg.agents {
                        for agent in &agents {
                            let tag = docker::agent_built_image_tag(&agent.name);
                            let _ = Command::new("docker").args(["rmi", "-f", &tag]).output();
                            println!("    {}: removed", tag);
                        }
                    }
                    if docker::image_exists("ollama/ollama").unwrap_or(false) {
                        let _ = Command::new("docker").args(["rmi", "-f", "ollama/ollama"]).output();
                        println!("    ollama/ollama: removed");
                    }
                }
            }

            let remove_config = if remove_config {
                true
            } else {
                let prompt = "Remove all config and profile data (~/.config/smith)? Type 'yes' to confirm: ";
                confirm_reset(prompt, false)
            };

            if remove_config {
                println!("  Removing config directory...");
                match config_dir() {
                    Ok(dir) => {
                        if dir.exists() {
                            if let Err(e) = fs::remove_dir_all(&dir) {
                                eprintln!("    Failed to remove config: {}", e);
                            } else {
                                println!("    {}: removed", dir.display());
                            }
                        } else {
                            println!("    (config directory does not exist)");
                        }
                    }
                    Err(e) => eprintln!("    Warning: {}", e),
                }
            }

            println!();
            println!("  {} Uninstalled successfully", BULLET_GREEN);
            if !remove_config {
                println!("     (config preserved - run with --remove-config to delete)");
            }
            if !remove_images {
                println!("     (images preserved - run with --remove-images to delete)");
            }

            let prompt = "Remove the smith binary? Type 'yes' to run 'cargo uninstall smith': ";
            if confirm_reset(prompt, force) {
                println!("  Running cargo uninstall smith...");
                let status = Command::new("cargo")
                    .args(["uninstall", "smith"])
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        println!("    smith binary removed");
                    }
                    _ => {
                        eprintln!("    Warning: cargo uninstall failed");
                        println!("    To remove manually, run: which smith");
                    }
                }
            }
        }
        Some(Commands::Agent { cmd }) => match cmd {
            AgentCommands::Add {
                name,
                image,
                agent_type,
                model,
                small_model,
                provider,
                base_url,
                port,
                enabled,
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                if let Err(e) = add_agent_to_config(
                    &mut cfg,
                    name.clone(),
                    image,
                    agent_type,
                    model,
                    small_model,
                    provider,
                    base_url,
                    port,
                    enabled,
                    None,
                    None,
                ) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("Agent '{}' added successfully", name);
            }
            AgentCommands::Status => {
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let running = docker::list_running_agent_containers().unwrap_or_default();
                let _current = cfg.current_agent.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
                let base = docker::OPENCODE_SERVER_PORT;
                let list = cfg.agents.as_deref().unwrap_or(&[]);
                #[allow(clippy::type_complexity)]
                let agents: Vec<(
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<bool>,
                    Option<HashMap<String, AgentRole>>,
                    u16,
                )> = if list.is_empty() {
                    vec![(
                        DEFAULT_AGENT_NAME.to_string(),
                        DEFAULT_AGENT_IMAGE.to_string(),
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some(true),
                        None,
                        base,
                    )]
                } else {
                    list.iter()
                        .enumerate()
                        .map(|(i, e)| {
                            (
                                e.name.clone(),
                                e.image.clone(),
                                e.agent_type.clone(),
                                e.model.clone(),
                                e.small_model.clone(),
                                e.provider.clone(),
                                e.base_url.clone(),
                                e.enabled,
                                e.roles.clone(),
                                agent_port(e, i),
                            )
                        })
                        .collect()
                };
                for (
                    name,
                    image,
                    agent_type,
                    model,
                    small_model,
                    provider,
                    _base_url,
                    enabled,
                    roles,
                    port,
                ) in &agents
                {
                    let active = running.contains(name);
                    let reachable = if active {
                        Some(docker::check_agent_reachable(*port))
                    } else {
                        None
                    };
                    let built =
                        docker::image_exists(&docker::agent_built_image_tag(name)).unwrap_or(false);
                    let is_cloud = agent_type.as_deref() != Some("local");
                    let circle = status_circle(active, reachable, built, is_cloud);
                    println!("\n  {} {}", circle, name);
                    if active {
                        if reachable == Some(false) {
                            println!("      Active:   active (running)");
                            println!(
                                "      Port:     {} {} (warning: unreachable)",
                                port,
                                clickable_agent_url(*port)
                            );
                        } else {
                            println!("      Active:   active (running)");
                            println!("      Port:     {} {}", port, clickable_agent_url(*port));
                        }
                    } else if is_cloud {
                        println!("      Active:   cloud (config only)");
                    } else {
                        println!(
                            "      Active:   {}",
                            if enabled.unwrap_or(true) {
                                "inactive"
                            } else {
                                "disabled"
                            }
                        );
                    }
                    if !is_cloud {
                        let built_tag = docker::agent_built_image_tag(name);
                        let image_line = if built {
                            built_tag
                        } else {
                            format!("not built (uses {})", image)
                        };
                        println!("      Image:    {}", image_line);
                    }
                    let model_str = model.as_deref().unwrap_or("default");
                    let small_model_str = small_model.as_deref().unwrap_or("-");
                    let is_local = agent_type.as_deref() == Some("local");
                    let provider_str = provider.as_deref().unwrap_or("-");
                    let mode_str = if is_local { "local" } else { "cloud" };
                    println!(
                        "      Model:    {}  Small: {}  Provider: {}  Type: {}",
                        model_str, small_model_str, provider_str, mode_str
                    );
                    let roles_str = match roles.as_ref() {
                        Some(r) if !r.is_empty() => {
                            r.keys().cloned().collect::<Vec<_>>().join(", ")
                        }
                        _ => "-".to_string(),
                    };
                    println!("      Roles:    {}", roles_str);
                }
                if agents.is_empty() {
                    println!("\n  (no cloud agents configured)");
                } else {
                    println!();
                }
            }
            AgentCommands::Update {
                name,
                image,
                agent_type,
                model,
                small_model,
                provider,
                base_url,
                port,
                enabled,
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agents = cfg.agents.get_or_insert_with(Vec::new);
                match agents.iter_mut().find(|a| a.name == name) {
                    Some(entry) => {
                        let is_wizard = image.is_none()
                            && agent_type.is_none()
                            && model.is_none()
                            && small_model.is_none()
                            && provider.is_none()
                            && base_url.is_none()
                            && port.is_none()
                            && enabled.is_none();
                        if is_wizard {
                            println!("  Updating agent '{}'", entry.name);
                            let image_in = prompt_line(&format!(
                                "  Image [{}]: ",
                                entry.image
                            ));
                            if !image_in.is_empty() {
                                entry.image = image_in;
                            }
                            let type_in = prompt_line(&format!(
                                "  Type (local/cloud) [{}]: ",
                                entry.agent_type.as_deref().unwrap_or("cloud")
                            ));
                            if !type_in.is_empty() {
                                entry.agent_type = Some(type_in);
                            } else if entry.agent_type.is_some() && type_in.is_empty() {
                                entry.agent_type = None;
                            }
                            let model_in = prompt_line(&format!(
                                "  Model [{}]: ",
                                entry.model.as_deref().unwrap_or("(none)")
                            ));
                            if !model_in.is_empty() {
                                entry.model = Some(model_in);
                            } else if entry.model.is_some() && model_in.is_empty() {
                                entry.model = None;
                            }
                            let small_model_in = prompt_line(&format!(
                                "  Small model [{}]: ",
                                entry.small_model.as_deref().unwrap_or("(none)")
                            ));
                            if !small_model_in.is_empty() {
                                entry.small_model = Some(small_model_in);
                            } else if entry.small_model.is_some() && small_model_in.is_empty() {
                                entry.small_model = None;
                            }
                            let provider_in = prompt_line(&format!(
                                "  Provider [{}]: ",
                                entry.provider.as_deref().unwrap_or("(none)")
                            ));
                            if !provider_in.is_empty() {
                                entry.provider = Some(provider_in);
                            } else if entry.provider.is_some() && provider_in.is_empty() {
                                entry.provider = None;
                            }
                            let base_url_in = prompt_line(&format!(
                                "  Base URL [{}]: ",
                                entry.base_url.as_deref().unwrap_or("(none)")
                            ));
                            if !base_url_in.is_empty() {
                                entry.base_url = Some(base_url_in);
                            } else if entry.base_url.is_some() && base_url_in.is_empty() {
                                entry.base_url = None;
                            }
                            let port_in = prompt_line(&format!(
                                "  Port [{}]: ",
                                entry.port.map(|p| p.to_string()).unwrap_or_else(|| "4096".to_string())
                            ));
                            if !port_in.is_empty() {
                                if let Ok(p) = port_in.parse() {
                                    entry.port = Some(p);
                                }
                            } else if entry.port.is_some() && port_in.is_empty() {
                                entry.port = None;
                            }
                            let enabled_in = prompt_line(&format!(
                                "  Enabled (true/false) [{}]: ",
                                entry.enabled.unwrap_or(true)
                            ));
                            if !enabled_in.is_empty() {
                                entry.enabled = Some(enabled_in == "true");
                            }
                            save_config(&cfg).unwrap_or_else(|e| {
                                eprintln!("Error: {}", e);
                                std::process::exit(1);
                            });
                            println!("  {} Agent '{}' updated", BULLET_GREEN, name);
                        } else {
                            if let Some(ref s) = image {
                                entry.image = if s.is_empty() {
                                    DEFAULT_AGENT_IMAGE.to_string()
                                } else {
                                    s.clone()
                                };
                            }
                            if let Some(ref s) = agent_type {
                                entry.agent_type = if s.is_empty() { None } else { Some(s.clone()) };
                            }
                            if let Some(ref s) = model {
                                entry.model = if s.is_empty() { None } else { Some(s.clone()) };
                            }
                            if let Some(ref s) = small_model {
                                entry.small_model = if s.is_empty() { None } else { Some(s.clone()) };
                            }
                            if let Some(ref s) = provider {
                                entry.provider = if s.is_empty() { None } else { Some(s.clone()) };
                            }
                            if let Some(ref s) = base_url {
                                entry.base_url = if s.is_empty() { None } else { Some(s.clone()) };
                            }
                            if let Some(p) = port {
                                entry.port = Some(p);
                            }
                            if let Some(e) = enabled {
                                entry.enabled = Some(e);
                            }
                            save_config(&cfg).unwrap_or_else(|e| {
                                eprintln!("Error: {}", e);
                                std::process::exit(1);
                            });
                            println!("Agent '{}' updated successfully", name);
                        }
                    }
                    None => {
                        eprintln!("Error: Agent '{}' not found", name);
                        std::process::exit(1);
                    }
                }
            }
            AgentCommands::Remove { name } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agents = cfg.agents.get_or_insert_with(Vec::new);
                let initial_len = agents.len();
                agents.retain(|a| a.name != name);
                if agents.len() == initial_len {
                    eprintln!("Error: Agent '{}' not found", name);
                    std::process::exit(1);
                }
                if cfg.current_agent.as_deref() == Some(name.as_str()) {
                    cfg.current_agent = Some(
                        agents
                            .first()
                            .map(|e| e.name.clone())
                            .unwrap_or_else(|| DEFAULT_AGENT_NAME.to_string()),
                    );
                }
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("Agent '{}' removed successfully", name);
            }
            AgentCommands::Build {
                name,
                all,
                force,
                verbose,
            } => {
                if let Err(e) = docker::check_docker_available() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                #[allow(clippy::type_complexity)]
                let agents: Vec<(
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    u16,
                )> = if all || name.is_none() {
                    match cfg.agents.as_deref() {
                        Some(a) if !a.is_empty() => a
                            .iter()
                            .enumerate()
                            .map(|(i, e)| {
                                (
                                    e.name.clone(),
                                    e.image.clone(),
                                    e.agent_type.clone(),
                                    e.model.clone(),
                                    e.small_model.clone(),
                                    e.provider.clone(),
                                    agent_port(e, i),
                                )
                            })
                            .collect(),
                        _ => vec![(
                            DEFAULT_AGENT_NAME.to_string(),
                            DEFAULT_AGENT_IMAGE.to_string(),
                            Some("cloud".to_string()),
                            None,
                            None,
                            None,
                            docker::OPENCODE_SERVER_PORT,
                        )],
                    }
                } else {
                    let n = name.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
                    let (base_image, agent_type, model, small_model, provider, port) = cfg
                        .agents
                        .as_deref()
                        .and_then(|a| {
                            a.iter().position(|e| e.name == n).map(|idx| {
                                let e = &a[idx];
                                (
                                    e.image.clone(),
                                    e.agent_type.clone(),
                                    e.model.clone(),
                                    e.small_model.clone(),
                                    e.provider.clone(),
                                    agent_port(e, idx),
                                )
                            })
                        })
                        .unwrap_or_else(|| {
                            if n == DEFAULT_AGENT_NAME {
                                (
                                    DEFAULT_AGENT_IMAGE.to_string(),
                                    Some("cloud".to_string()),
                                    None,
                                    None,
                                    None,
                                    docker::OPENCODE_SERVER_PORT,
                                )
                            } else {
                                eprintln!("Error: Agent '{}' not found", n);
                                std::process::exit(1);
                            }
                        });
                    vec![(
                        n.to_string(),
                        base_image,
                        agent_type,
                        model,
                        small_model,
                        provider,
                        port,
                    )]
                };
                let dir = config_dir().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let mut ok = 0usize;
                let mut failed = Vec::new();
                for (agent_name, base_image, agent_type, model, small_model, provider, port) in &agents {
                    let is_cloud = agent_type.as_deref() != Some("local");
                    if is_cloud {
                        println!("  {} Skipping cloud agent '{}'", BULLET_BLUE, agent_name);
                        continue;
                    }
                    if verbose {
                        let agent_dir = dir.join("agents").join(agent_name);
                        let dockerfile = agent_dir.join("Dockerfile");
                        let tag = docker::agent_built_image_tag(agent_name);
                        println!(
                            "  {}: agent_dir={} Dockerfile={} image={} port={}",
                            agent_name,
                            agent_dir.display(),
                            dockerfile.display(),
                            tag,
                            port
                        );
                    }
                    match build_agent_image(
                        dir.as_path(),
                        agent_name,
                        base_image,
                        *port,
                        model.as_deref(),
                        small_model.as_deref(),
                        provider.as_deref(),
                        force,
                    ) {
                        Ok(()) => {
                            let tag = docker::agent_built_image_tag(agent_name);
                            if verbose {
                                println!(
                                    "  {}: docker build -t {} {}",
                                    agent_name,
                                    tag,
                                    dir.join("agents").join(agent_name).display()
                                );
                            }
                            println!("  {}: built {}", agent_name, tag);
                            ok += 1;
                        }
                        Err(e) => {
                            eprintln!("  {}: build failed - {}", agent_name, e);
                            failed.push((agent_name.clone(), e));
                        }
                    }
                }
                if failed.is_empty() {
                    println!("Built {} agent image(s) successfully.", ok);
                } else {
                    println!("Built {}; {} failed.", ok, failed.len());
                    std::process::exit(1);
                }
            }
            AgentCommands::Start { verbose } => {
                if let Err(e) = docker::check_docker_available() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agents = cfg.agents.as_deref().unwrap_or(&[]);

                // Start Ollama for each local agent (each gets its own container with its model)
                let local_agents: Vec<_> = agents
                    .iter()
                    .filter(|e| e.agent_type.as_deref() == Some("local"))
                    .collect();
                for local in &local_agents {
                    let local_model = local
                        .model
                        .clone()
                        .unwrap_or_else(|| "qwen3:8b".to_string());
                    if docker::is_ollama_running() {
                        println!("  Ollama already running");
                    } else {
                        match docker::start_ollama_container(&local_model, true) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error starting Ollama: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                }

                // Build agent list (1 agent : 1 container) - only enabled local agents (skip cloud)
                let enabled_agents: Vec<_> = if agents.is_empty() {
                    // Default agent is cloud, so no containers to start
                    vec![]
                } else {
                    agents
                        .iter()
                        .filter(|e| {
                            let is_enabled = e.enabled.unwrap_or(true);
                            let is_local = e.agent_type.as_deref() == Some("local");
                            is_enabled && is_local
                        })
                        .enumerate()
                        .map(|(i, e)| {
                            let image =
                                if docker::image_exists(&docker::agent_built_image_tag(&e.name))
                                    .unwrap_or(false)
                                {
                                    docker::agent_built_image_tag(&e.name)
                                } else {
                                    e.image.clone()
                                };
                            let is_local = e.agent_type.as_deref() == Some("local");
                            let provider = if is_local {
                                Some("ollama".to_string())
                            } else {
                                e.provider.clone()
                            };
                            let base_url = if is_local {
                                Some(format!(
                                    "http://host.docker.internal:{}",
                                    docker::OLLAMA_PORT
                                ))
                            } else {
                                e.base_url.clone()
                            };
                            (
                                e.name.clone(),
                                image,
                                provider,
                                base_url,
                                e.enabled.unwrap_or(true),
                                agent_port(e, i),
                            )
                        })
                        .collect()
                };
                // Print skipping messages for cloud agents
                if !agents.is_empty() {
                    for agent in agents.iter() {
                        let is_local = agent.agent_type.as_deref() == Some("local");
                        if !is_local {
                            println!("  {} Skipping cloud agent '{}'", BULLET_BLUE, agent.name);
                        }
                    }
                }
                let running = docker::list_running_agent_containers().unwrap_or_default();
                if verbose {
                    println!("Agents: {}", enabled_agents.len());
                    for (name, image, provider, base_url, _enabled, port) in &enabled_agents {
                        let status = if running.contains(name) {
                            "already running"
                        } else {
                            "will start"
                        };
                        let provider_str = provider.as_deref().unwrap_or("-");
                        let mode = if base_url.is_some() { "local" } else { "cloud" };
                        println!(
                            "  {} -> {} port={} provider={} mode={} {} [{}]",
                            name,
                            image,
                            port,
                            provider_str,
                            mode,
                            clickable_agent_url(*port),
                            status
                        );
                    }
                    if !running.is_empty() {
                        println!("Already running: {}", running.join(", "));
                    }
                }
                let mut ok = 0usize;
                let mut failed = Vec::new();
                for (name, image, provider, base_url, _enabled, port) in &enabled_agents {
                    if running.contains(name) {
                        println!(
                            "  {}: already running (port {} {})",
                            name,
                            port,
                            clickable_agent_url(*port)
                        );
                        ok += 1;
                        continue;
                    }
                    if verbose {
                        let container_name = docker::agent_container_name(name);
                        let env_vars = if let Some(ref p) = provider {
                            let mut vars = format!(" -e {}_API_KEY={}", p.to_uppercase(), "dummy");
                            if let Some(ref url) = base_url {
                                vars.push_str(&format!(" -e OPENCODE_BASE_URL={}", url));
                            }
                            vars
                        } else {
                            String::new()
                        };
                        println!(
                                "  {}: docker run -d --name {} -p {}:{}{} --entrypoint opencode {} serve --hostname 0.0.0.0 --port {}",
                                name, container_name, port, port, env_vars, image, port
                            );
                    }
                    match docker::start_agent_container(
                        name,
                        image,
                        *port,
                        provider.as_deref(),
                        base_url.as_deref(),
                    ) {
                        Ok(()) => {
                            println!(
                                "  {}: started (port {} {})",
                                name,
                                port,
                                clickable_agent_url(*port)
                            );
                            if verbose {
                                println!("  {}: waiting 3s before health check...", name);
                            }
                            std::thread::sleep(std::time::Duration::from_secs(3));
                            if verbose {
                                println!("  {}: GET {}", name, clickable_agent_url(*port));
                            }
                            match docker::test_agent_server(*port) {
                                Ok(()) => {
                                    println!("  {}: health check OK", name);
                                    ok += 1;
                                }
                                Err(e) => {
                                    eprintln!("  {}: health check failed - {}", name, e);
                                    failed.push((name.clone(), e));
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("  {}: failed to start - {}", name, e);
                            failed.push((name.clone(), e));
                        }
                    }
                }
                if failed.is_empty() {
                    if ok == 0 {
                        println!("No local agents to start.");
                    } else {
                        println!("All {} local agent(s) started and tested successfully.", ok);
                    }
                } else {
                    println!("Started {}; {} failed.", ok, failed.len());
                    std::process::exit(1);
                }
            }
            AgentCommands::Stop => {
                if let Err(e) = docker::check_docker_available() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                // Print skipping messages for cloud agents
                let all_agents = cfg.agents.as_deref().unwrap_or(&[]);
                for agent in all_agents.iter() {
                    let is_local = agent.agent_type.as_deref() == Some("local");
                    if !is_local {
                        println!("  {} Skipping cloud agent '{}'", BULLET_BLUE, agent.name);
                    }
                }
                // Only stop local agent containers (skip cloud)
                let running = docker::list_running_agent_containers().unwrap_or_default();
                let agents = cfg.agents.as_deref().unwrap_or(&[]);
                let local_agent_names: std::collections::HashSet<_> = agents
                    .iter()
                    .filter(|e| e.agent_type.as_deref() == Some("local"))
                    .map(|e| e.name.clone())
                    .collect();
                let to_stop: Vec<_> = running
                    .into_iter()
                    .filter(|name| local_agent_names.contains(name))
                    .collect();
                let mut stopped = Vec::new();
                for name in &to_stop {
                    if docker::stop_agent_container(name).is_ok() {
                        stopped.push(name.clone());
                    }
                }
                if stopped.is_empty() {
                    println!("No running local agent containers.");
                } else {
                    for name in &stopped {
                        println!("  {}: stopped", name);
                    }
                    println!("Stopped {} container(s).", stopped.len());
                }
                // Also stop Ollama if it's running
                if docker::is_ollama_running() {
                    if let Err(e) = docker::stop_ollama_container() {
                        eprintln!("Warning: failed to stop Ollama: {}", e);
                    } else {
                        println!("  Ollama: stopped");
                    }
                }
            }
            AgentCommands::Logs { name } => {
                if let Err(e) = docker::check_docker_available() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                let container_name = docker::agent_container_name(&name);
                if !docker::container_exists(&container_name).unwrap_or(false) {
                    eprintln!(
                        "Error: Container '{}' not found. Start the agent with 'smith agent start'.",
                        container_name
                    );
                    std::process::exit(1);
                }
                let status = Command::new("docker")
                    .args(["logs", "-f", &container_name])
                    .status()
                    .unwrap_or_else(|e| {
                        eprintln!("Error: Failed to run docker logs: {}", e);
                        std::process::exit(1);
                    });
                if let Some(code) = status.code() {
                    std::process::exit(code);
                }
                std::process::exit(1);
            }
        },
        Some(Commands::Project { cmd }) => match cmd {
            ProjectCommands::Add {
                name,
                repo,
                image,
                ssh_key,
                base_branch,
                remote,
                github_token,
                script,
                commit_name,
                commit_email,
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let ssh_key = ssh_key.filter(|s| !s.is_empty());
                let base_branch = base_branch.filter(|s| !s.is_empty());
                let remote = remote.filter(|s| !s.is_empty());
                let github_token = github_token.filter(|s| !s.is_empty());
                let script = script.filter(|s| !s.is_empty());
                let commit_name = commit_name.filter(|s| !s.is_empty());
                let commit_email = commit_email.filter(|s| !s.is_empty());
                let project = ProjectConfig {
                    name: name.clone(),
                    repo,
                    image,
                    ssh_key,
                    base_branch,
                    remote,
                    github_token,
                    script,
                    commit_name,
                    commit_email,
                    agent: None,
                    ask_setup_run: None,
                    ask_setup_check: None,
                    ask_execute_run: None,
                    ask_execute_check: None,
                    ask_validate_run: None,
                    ask_validate_check: None,
                    dev_setup_run: None,
                    dev_setup_check: None,
                    dev_execute_run: None,
                    dev_execute_check: None,
                    dev_validate_run: None,
                    dev_validate_check: None,
                    dev_commit_run: None,
                    dev_commit_check: None,
                    review_setup_run: None,
                    review_setup_check: None,
                    review_execute_run: None,
                    review_execute_check: None,
                    review_validate_run: None,
                    review_validate_check: None,
                };
                if let Err(e) = add_project_to_config(&mut cfg, project) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("Project added successfully");
            }
            ProjectCommands::List => {
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                if cfg.projects.is_empty() {
                    println!("No projects registered");
                } else {
                    for proj in &cfg.projects {
                        let mut parts = format!("  {} -> {}", proj.name, proj.repo);
                        if let Some(ref image) = proj.image {
                            parts.push_str(&format!(" (image: {})", image));
                        }
                        if let Some(ref sk) = proj.ssh_key {
                            parts.push_str(&format!(" (ssh_key: {})", sk));
                        }
                        if let Some(ref bb) = proj.base_branch {
                            parts.push_str(&format!(" (base_branch: {})", bb));
                        }
                        if let Some(ref r) = proj.remote {
                            parts.push_str(&format!(" (remote: {})", r));
                        }
                        if proj.github_token.is_some() {
                            parts.push_str(" (github-token: set)");
                        }
                        if let Some(ref script) = proj.script {
                            let truncated = if script.len() > 40 {
                                format!("{}...", &script[..40])
                            } else {
                                script.clone()
                            };
                            parts.push_str(&format!(" (script: {})", truncated));
                        }
                        println!("{}", parts);
                    }
                }
            }
            ProjectCommands::Status { project, verbose } => {
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let projects: Vec<&ProjectConfig> = match project.as_deref() {
                    Some(name) => match cfg.projects.iter().find(|p| p.name == name) {
                        Some(p) => vec![p],
                        None => {
                            eprintln!("Error: Project '{}' not found", name);
                            std::process::exit(1);
                        }
                    },
                    None => {
                        if cfg.projects.is_empty() {
                            eprintln!("No projects registered. Add one with `smith project add`.");
                            std::process::exit(1);
                        }
                        cfg.projects.iter().collect()
                    }
                };
                for proj in projects {
                    let resolved_repo = &proj.repo;
                    if resolved_repo.starts_with("https://") {
                        eprintln!("Error: HTTPS URLs are not supported. Use SSH URLs (git@github.com:user/repo.git).");
                        std::process::exit(1);
                    }
                    let base = resolve_base_branch(None, Some(proj));
                    let agent_image = resolve_agent_image(proj.image.as_deref());
                    let ssh_key_path = resolve_ssh_key(None, Some(proj));
                    let project_script = resolve_project_script(Some(proj));
                    if verbose {
                        println!("Project: {} -> {}", proj.name, resolved_repo);
                        println!("  Branch: {}  Image: {}", base, agent_image);
                    }
                    let done = std::sync::Arc::new(AtomicBool::new(false));
                    let done_clone = done.clone();
                    let spinner_handle = if !verbose {
                        Some(thread::spawn(move || run_spinner_until(&done_clone)))
                    } else {
                        None
                    };
                    let _guard = if !verbose {
                        redirect_stderr_to_null()
                    } else {
                        None
                    };
                    let repo = resolved_repo.clone();
                    let branch = base.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let scr = project_script.clone();
                    let result = dagger_pipeline::with_connection(move |conn| {
                        let repo = repo.clone();
                        let branch = branch.clone();
                        let img = img.clone();
                        let ssh = ssh.clone();
                        async move {
                            dagger_pipeline::run_project_status(
                                &conn,
                                &repo,
                                &branch,
                                &img,
                                ssh.as_deref(),
                                scr.as_deref(),
                            )
                            .await
                            .map_err(|e| eyre::eyre!("{}", e))
                        }
                    })
                    .await;
                    if !verbose {
                        done.store(true, Ordering::Relaxed);
                        if let Some(h) = spinner_handle {
                            let _: std::thread::Result<()> = h.join();
                        }
                    }
                    drop(_guard);
                    match result {
                        Ok(output) => {
                            let has_git = !output.contains("no-git") && output.len() > 2;
                            let has_opencode = output.contains('.') && output.chars().filter(|c| *c == '.').count() >= 2;
                            if has_git && has_opencode {
                                println!("\n  {} {} - ready", BULLET_GREEN, proj.name);
                                if verbose {
                                    println!("  ---");
                                    for line in output.lines() {
                                        println!("    {}", line);
                                    }
                                }
                            } else if !has_git {
                                eprintln!(
                                    "  {} {} - failed: clone failed",
                                    BULLET_RED, proj.name
                                );
                                std::process::exit(1);
                            } else {
                                eprintln!(
                                    "  {} {} - failed: opencode not available",
                                    BULLET_RED, proj.name
                                );
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "  {} {} - failed: {}",
                                BULLET_RED,
                                proj.name,
                                clarify_dagger_error(&e)
                            );
                            std::process::exit(1);
                        }
                    }
                }
            }
            ProjectCommands::Update {
                name,
                repo,
                image,
                ssh_key,
                base_branch,
                remote,
                github_token,
                script,
                commit_name,
                commit_email,
                agent,
                ask_setup,
                ask_execute,
                ask_validate,
                dev_setup,
                dev_execute,
                dev_validate,
                dev_commit,
                review_setup,
                review_execute,
                review_validate,
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let project = cfg.projects.iter_mut().find(|p| p.name == name);
                match project {
                    Some(proj) => {
                        let is_wizard = repo.is_none()
                            && image.is_none()
                            && ssh_key.is_none()
                            && base_branch.is_none()
                            && remote.is_none()
                            && github_token.is_none()
                            && script.is_none()
                            && commit_name.is_none()
                            && commit_email.is_none()
                            && agent.is_none()
                            && ask_setup.is_none()
                            && ask_execute.is_none()
                            && ask_validate.is_none()
                            && dev_setup.is_none()
                            && dev_execute.is_none()
                            && dev_validate.is_none()
                            && dev_commit.is_none()
                            && review_setup.is_none()
                            && review_execute.is_none()
                            && review_validate.is_none();
                        if is_wizard {
                            println!("  Updating project '{}'", proj.name);
                            let repo_in = prompt_line(&format!("  Repository [{}]: ", proj.repo));
                            if !repo_in.is_empty() {
                                proj.repo = repo_in;
                            }
                            let image_in = prompt_line(&format!(
                                "  Image [{}]: ",
                                proj.image.as_deref().unwrap_or("opencode")
                            ));
                            if !image_in.is_empty() {
                                proj.image = Some(image_in);
                            } else if proj.image.is_some() && image_in.is_empty() {
                                proj.image = None;
                            }
                            let ssh_in = prompt_line(&format!(
                                "  SSH key [{}]: ",
                                proj.ssh_key.as_deref().unwrap_or("(none)")
                            ));
                            if !ssh_in.is_empty() {
                                proj.ssh_key = Some(ssh_in);
                            } else if proj.ssh_key.is_some() && ssh_in.is_empty() {
                                proj.ssh_key = None;
                            }
                            let base_branch_in = prompt_line(&format!(
                                "  Base branch [{}]: ",
                                proj.base_branch.as_deref().unwrap_or("main")
                            ));
                            if !base_branch_in.is_empty() {
                                proj.base_branch = Some(base_branch_in);
                            } else if proj.base_branch.is_some() && base_branch_in.is_empty() {
                                proj.base_branch = None;
                            }
                            let remote_in = prompt_line(&format!(
                                "  Remote [{}]: ",
                                proj.remote.as_deref().unwrap_or("origin")
                            ));
                            if !remote_in.is_empty() {
                                proj.remote = Some(remote_in);
                            } else if proj.remote.is_some() && remote_in.is_empty() {
                                proj.remote = None;
                            }
                            let github_in = prompt_line("  GitHub token [******]: ");
                            if !github_in.is_empty() {
                                proj.github_token = Some(github_in);
                            } else if proj.github_token.is_some() && github_in.is_empty() {
                                proj.github_token = None;
                            }
                            let script_in = prompt_line(&format!(
                                "  Script [{}]: ",
                                proj.script.as_deref().unwrap_or("(none)")
                            ));
                            if !script_in.is_empty() {
                                proj.script = Some(script_in);
                            } else if proj.script.is_some() && script_in.is_empty() {
                                proj.script = None;
                            }
                            let commit_name_in = prompt_line(&format!(
                                "  Commit name [{}]: ",
                                proj.commit_name.as_deref().unwrap_or("(none)")
                            ));
                            if !commit_name_in.is_empty() {
                                proj.commit_name = Some(commit_name_in);
                            } else if proj.commit_name.is_some() && commit_name_in.is_empty() {
                                proj.commit_name = None;
                            }
                            let commit_email_in = prompt_line(&format!(
                                "  Commit email [{}]: ",
                                proj.commit_email.as_deref().unwrap_or("(none)")
                            ));
                            if !commit_email_in.is_empty() {
                                proj.commit_email = Some(commit_email_in);
                            } else if proj.commit_email.is_some() && commit_email_in.is_empty() {
                                proj.commit_email = None;
                            }
                            let agent_in = prompt_line(&format!(
                                "  Agent [{}]: ",
                                proj.agent.as_deref().unwrap_or("(none)")
                            ));
                            if !agent_in.is_empty() {
                                proj.agent = Some(agent_in);
                            } else if proj.agent.is_some() && agent_in.is_empty() {
                                proj.agent = None;
                            }
                            println!("  Roles (Enter to keep current):");
                           let ask_setup_in = prompt_line(&format!(
                                "    ask.setup [{} {}]: ",
                                proj.ask_setup_run.as_deref().unwrap_or("-"),
                                proj.ask_setup_check.as_deref().unwrap_or("-")
                            ));
                            if !ask_setup_in.is_empty() {
                                let parts: Vec<_> = ask_setup_in.split_whitespace().collect();
                                proj.ask_setup_run = parts.first().map(|s| s.to_string());
                                proj.ask_setup_check = parts.get(1).map(|s| s.to_string());
                            }
                            let ask_execute_in = prompt_line(&format!(
                                "    ask.execute [{} {}]: ",
                                proj.ask_execute_run.as_deref().unwrap_or("-"),
                                proj.ask_execute_check.as_deref().unwrap_or("-")
                            ));
                            if !ask_execute_in.is_empty() {
                                let parts: Vec<_> = ask_execute_in.split_whitespace().collect();
                                proj.ask_execute_run = parts.first().map(|s| s.to_string());
                                proj.ask_execute_check = parts.get(1).map(|s| s.to_string());
                            }
                            let ask_validate_in = prompt_line(&format!(
                                "    ask.validate [{} {}]: ",
                                proj.ask_validate_run.as_deref().unwrap_or("-"),
                                proj.ask_validate_check.as_deref().unwrap_or("-")
                            ));
                            if !ask_validate_in.is_empty() {
                                let parts: Vec<_> = ask_validate_in.split_whitespace().collect();
                                proj.ask_validate_run = parts.first().map(|s| s.to_string());
                                proj.ask_validate_check = parts.get(1).map(|s| s.to_string());
                            }
                            let dev_setup_in = prompt_line(&format!(
                                "    dev.setup [{} {}]: ",
                                proj.dev_setup_run.as_deref().unwrap_or("-"),
                                proj.dev_setup_check.as_deref().unwrap_or("-")
                            ));
                            if !dev_setup_in.is_empty() {
                                let parts: Vec<_> = dev_setup_in.split_whitespace().collect();
                                proj.dev_setup_run = parts.first().map(|s| s.to_string());
                                proj.dev_setup_check = parts.get(1).map(|s| s.to_string());
                            }
                            let dev_execute_in = prompt_line(&format!(
                                "    dev.execute [{} {}]: ",
                                proj.dev_execute_run.as_deref().unwrap_or("-"),
                                proj.dev_execute_check.as_deref().unwrap_or("-")
                            ));
                            if !dev_execute_in.is_empty() {
                                let parts: Vec<_> = dev_execute_in.split_whitespace().collect();
                                proj.dev_execute_run = parts.first().map(|s| s.to_string());
                                proj.dev_execute_check = parts.get(1).map(|s| s.to_string());
                            }
                            let dev_validate_in = prompt_line(&format!(
                                "    dev.validate [{} {}]: ",
                                proj.dev_validate_run.as_deref().unwrap_or("-"),
                                proj.dev_validate_check.as_deref().unwrap_or("-")
                            ));
                            if !dev_validate_in.is_empty() {
                                let parts: Vec<_> = dev_validate_in.split_whitespace().collect();
                                proj.dev_validate_run = parts.first().map(|s| s.to_string());
                                proj.dev_validate_check = parts.get(1).map(|s| s.to_string());
                            }
                            let dev_commit_in = prompt_line(&format!(
                                "    dev.commit [{} {}]: ",
                                proj.dev_commit_run.as_deref().unwrap_or("-"),
                                proj.dev_commit_check.as_deref().unwrap_or("-")
                            ));
                            if !dev_commit_in.is_empty() {
                                let parts: Vec<_> = dev_commit_in.split_whitespace().collect();
                                proj.dev_commit_run = parts.first().map(|s| s.to_string());
                                proj.dev_commit_check = parts.get(1).map(|s| s.to_string());
                            }
                            let review_setup_in = prompt_line(&format!(
                                "    review.setup [{} {}]: ",
                                proj.review_setup_run.as_deref().unwrap_or("-"),
                                proj.review_setup_check.as_deref().unwrap_or("-")
                            ));
                            if !review_setup_in.is_empty() {
                                let parts: Vec<_> = review_setup_in.split_whitespace().collect();
                                proj.review_setup_run = parts.first().map(|s| s.to_string());
                                proj.review_setup_check = parts.get(1).map(|s| s.to_string());
                            }
                            let review_execute_in = prompt_line(&format!(
                                "    review.execute [{} {}]: ",
                                proj.review_execute_run.as_deref().unwrap_or("-"),
                                proj.review_execute_check.as_deref().unwrap_or("-")
                            ));
                            if !review_execute_in.is_empty() {
                                let parts: Vec<_> = review_execute_in.split_whitespace().collect();
                                proj.review_execute_run = parts.first().map(|s| s.to_string());
                                proj.review_execute_check = parts.get(1).map(|s| s.to_string());
                            }
                            let review_validate_in = prompt_line(&format!(
                                "    review.validate [{} {}]: ",
                                proj.review_validate_run.as_deref().unwrap_or("-"),
                                proj.review_validate_check.as_deref().unwrap_or("-")
                            ));
                            if !review_validate_in.is_empty() {
                                let parts: Vec<_> = review_validate_in.split_whitespace().collect();
                                proj.review_validate_run = parts.first().map(|s| s.to_string());
                                proj.review_validate_check = parts.get(1).map(|s| s.to_string());
                            }
                            save_config(&cfg).unwrap_or_else(|e| {
                                eprintln!("Error: {}", e);
                                std::process::exit(1);
                            });
                            println!("  {} Project '{}' updated", BULLET_GREEN, name);
                        } else {
                        if let Some(new_repo) = repo {
                            proj.repo = new_repo;
                        }
                        if let Some(new_image) = image {
                            proj.image = Some(new_image);
                        } else if image.is_some() {
                            // Explicitly set to None if --image flag was provided with empty value
                        }
                        if let Some(new_ssh) = ssh_key {
                            proj.ssh_key = if new_ssh.is_empty() {
                                None
                            } else {
                                Some(new_ssh)
                            };
                        }
                        if let Some(new_bb) = base_branch {
                            proj.base_branch = if new_bb.is_empty() {
                                None
                            } else {
                                Some(new_bb)
                            };
                        }
                        if let Some(new_remote) = remote {
                            proj.remote = if new_remote.is_empty() {
                                None
                            } else {
                                Some(new_remote)
                            };
                        }
                        if let Some(new_gt) = github_token {
                            proj.github_token = if new_gt.is_empty() {
                                None
                            } else {
                                Some(new_gt)
                            };
                        }
                        if let Some(new_script) = script {
                            proj.script = if new_script.is_empty() {
                                None
                            } else {
                                Some(new_script)
                            };
                        }
                        if let Some(new_commit_name) = commit_name {
                            proj.commit_name = if new_commit_name.is_empty() {
                                None
                            } else {
                                Some(new_commit_name)
                            };
                        }
                        if let Some(new_commit_email) = commit_email {
                            proj.commit_email = if new_commit_email.is_empty() {
                                None
                            } else {
                                Some(new_commit_email)
                            };
                        }
                        if let Some(new_agent) = agent {
                            proj.agent = if new_agent.is_empty() {
                                None
                            } else {
                                Some(new_agent)
                            };
                        }
                        // Parse role pairs: first is run, second is check (if provided)
                        if let Some(ref roles) = ask_setup {
                            proj.ask_setup_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.ask_setup_check = roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = ask_execute {
                            proj.ask_execute_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.ask_execute_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = ask_validate {
                            proj.ask_validate_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.ask_validate_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = dev_setup {
                            proj.dev_setup_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_setup_check = roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = dev_execute {
                            proj.dev_execute_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_execute_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = dev_validate {
                            proj.dev_validate_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_validate_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = dev_commit {
                            proj.dev_commit_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_commit_check = roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = review_setup {
                            proj.review_setup_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.review_setup_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = review_execute {
                            proj.review_execute_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.review_execute_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(ref roles) = review_validate {
                            proj.review_validate_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.review_validate_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = ask_execute {
                            proj.ask_execute_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.ask_execute_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = ask_validate {
                            proj.ask_validate_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.ask_validate_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = dev_setup {
                            proj.dev_setup_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_setup_check = roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = dev_execute {
                            proj.dev_execute_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_execute_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = dev_validate {
                            proj.dev_validate_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_validate_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = dev_commit {
                            proj.dev_commit_run = roles.first().cloned().filter(|s| !s.is_empty());
                            proj.dev_commit_check = roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = review_setup {
                            proj.review_setup_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.review_setup_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = review_execute {
                            proj.review_execute_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.review_execute_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        if let Some(roles) = review_validate {
                            proj.review_validate_run =
                                roles.first().cloned().filter(|s| !s.is_empty());
                            proj.review_validate_check =
                                roles.get(1).cloned().filter(|s| !s.is_empty());
                        }
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("Project '{}' updated successfully", name);
                        }
                    }
                    None => {
                        eprintln!("Error: Project '{}' not found", name);
                        std::process::exit(1);
                    }
                }
            }
            ProjectCommands::Remove { name } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let initial_len = cfg.projects.len();
                cfg.projects.retain(|p| p.name != name);
                if cfg.projects.len() == initial_len {
                    eprintln!("Error: Project '{}' not found", name);
                    std::process::exit(1);
                }
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("Project removed successfully");
            }
        },
        Some(Commands::Run { cmd }) => match cmd {
            RunCommands::Ask {
                question,
                base,
                repo,
                project,
                image,
                ssh_key,
                keep_alive: _,
                timeout,
                verbose,
            } => {
                let resolved_repo =
                    resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                if resolved_repo.starts_with("https://") {
                    eprintln!("Error: HTTPS URLs are not supported. Use SSH URLs (git@github.com:user/repo.git).");
                    std::process::exit(1);
                }

                let project_config = resolve_project_config(project.clone()).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agent_image = resolve_agent_image(image.as_deref());
                let ssh_key_path = resolve_ssh_key(ssh_key.as_ref(), project_config.as_ref());
                let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());
                let _resolved_remote = resolve_remote(project_config.as_ref());
                let project_script = resolve_project_script(project_config.as_ref());
                let pipeline_roles = resolve_pipeline_roles(project_config.as_ref(), "ask");

                if verbose {
                    println!("Ask: {}", question);
                    println!("  Repository: {}", resolved_repo);
                    println!("  Agent image: {}", agent_image);
                }

                let branch = resolved_base.clone();
                let script = project_script.clone();
                let done = std::sync::Arc::new(AtomicBool::new(false));
                let done_clone = done.clone();
                let spinner_handle = if !verbose {
                    Some(thread::spawn(move || run_spinner_until(&done_clone)))
                } else {
                    None
                };
                let _guard = if !verbose {
                    redirect_stderr_to_null()
                } else {
                    None
                };

                let timeout_secs = timeout.unwrap_or(300);
                let roles = pipeline_roles.clone();
                let result = dagger_pipeline::with_connection(move |conn| {
                    let repo = resolved_repo.clone();
                    let q = question.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let br = branch.clone();
                    let to = timeout_secs;
                    let scr = script.clone();
                    let r = roles.clone();
                    async move {
                        dagger_pipeline::run_ask(
                            &conn,
                            &repo,
                            Some(br.as_str()),
                            &q,
                            &img,
                            ssh.as_deref(),
                            to,
                            scr.as_deref(),
                            &r,
                        )
                        .await
                        .map_err(|e| eyre::eyre!("{}", e))
                    }
                })
                .await;

                if !verbose {
                    done.store(true, Ordering::Relaxed);
                    if let Some(h) = spinner_handle {
                        let _: std::thread::Result<()> = h.join();
                    }
                }
                drop(_guard);

                match result {
                    Ok(answer) => println!("\nAnswer: {}", answer),
                    Err(e) => {
                        let msg = e.to_string();
                        eprintln!("Error: {}", clarify_dagger_error(&msg));
                        std::process::exit(1);
                    }
                }
            }
            RunCommands::Dev {
                task,
                branch,
                base,
                repo,
                project,
                image,
                ssh_key,
                keep_alive: _,
                pr,
                timeout,
                verbose,
            } => {
                let resolved_repo =
                    resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                if resolved_repo.starts_with("https://") {
                    eprintln!("Error: HTTPS URLs are not supported. Use SSH URLs (git@github.com:user/repo.git) with --ssh-key.");
                    std::process::exit(1);
                }

                let project_config = resolve_project_config(project.clone()).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agent_image = resolve_agent_image(image.as_deref());
                let ssh_key_path = resolve_ssh_key(ssh_key.as_ref(), project_config.as_ref());
                let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());
                let _resolved_remote = resolve_remote(project_config.as_ref());
                let project_script = resolve_project_script(project_config.as_ref());
                let (commit_name, commit_email) = resolve_commit_author(project_config.as_ref());
                let pipeline_roles = resolve_pipeline_roles(project_config.as_ref(), "dev");

                if verbose {
                    println!("Dev: {}", task);
                    println!("  Repository: {}", resolved_repo);
                    println!("  Agent image: {}", agent_image);
                }

                let branch_out = branch.clone();
                let resolved_repo_pr = resolved_repo.clone();
                let base_pr = resolved_base.clone();
                let task_pr = task.clone();
                let script = project_script.clone();
                let done = std::sync::Arc::new(AtomicBool::new(false));
                let done_clone = done.clone();
                let spinner_handle = if !verbose {
                    Some(thread::spawn(move || run_spinner_until(&done_clone)))
                } else {
                    None
                };
                let _guard = if !verbose {
                    redirect_stderr_to_null()
                } else {
                    None
                };

                let verb_for_closure = verbose;
                let timeout_secs = timeout.unwrap_or(300);
                let roles = pipeline_roles.clone();
                let dev_result = dagger_pipeline::with_connection(move |conn| {
                    let repo = resolved_repo.clone();
                    let t = task.clone();
                    let br = branch.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let base_br = resolved_base.clone();
                    let _rem = _resolved_remote.clone();
                    let to = timeout_secs;
                    let scr = script.clone();
                    let c_name = commit_name.clone();
                    let c_email = commit_email.clone();
                    let r = roles.clone();
                    async move {
                        dagger_pipeline::run_dev(
                            &conn,
                            &repo,
                            &br,
                            Some(base_br.as_str()),
                            &t,
                            &img,
                            ssh.as_deref(),
                            verb_for_closure,
                            to,
                            scr.as_deref(),
                            c_name.as_deref(),
                            c_email.as_deref(),
                            &r,
                        )
                        .await
                        .map_err(|e| eyre::eyre!("{}", e))
                    }
                })
                .await;

                if !verbose {
                    done.store(true, Ordering::Relaxed);
                    if let Some(h) = spinner_handle {
                        let _: std::thread::Result<()> = h.join();
                    }
                }
                drop(_guard);

                match &dev_result {
                    Ok(commit) => {
                        println!("\n✓ Development action completed and committed");
                        println!("  Commit: {}", commit);
                        println!("  Branch: {}", branch_out);
                    }
                    Err(e) => {
                        if e.contains("No changes to commit") {
                            println!("\n⚠ No changes were made by the development task");
                        } else {
                            let msg = e.to_string();
                            eprintln!("Error: {}", clarify_dagger_error(&msg));
                            std::process::exit(1);
                        }
                    }
                }

                if pr {
                    let token = project_config
                        .as_ref()
                        .and_then(|p| p.github_token.as_deref());
                    if let Some(token) = token {
                        if let Ok(repo_info) = github::extract_repo_info(&resolved_repo_pr) {
                            let base_branch = base_pr.as_str();
                            match github::create_or_update_pr(
                                token,
                                &repo_info.owner,
                                &repo_info.name,
                                &branch_out,
                                base_branch,
                                &task_pr,
                            )
                            .await
                            {
                                Ok(pr_url) => println!("  ✓ Pull request: {}", pr_url),
                                Err(e) => {
                                    eprintln!("  ⚠ Failed to create/update PR: {}", e);
                                    if e.contains("403") || e.contains("Resource not accessible") {
                                        eprintln!(
                                            "     Your token may be missing required permissions."
                                        );
                                    }
                                }
                            }
                        } else {
                            eprintln!(
                                "  ⚠ Could not extract repository info from URL: {}",
                                resolved_repo_pr
                            );
                        }
                    } else {
                        eprintln!("  ⚠ GitHub token not configured for this repository. Use --project with a project that has a token, or add/update the project with --github-token <token>.");
                    }
                }

                if dev_result.is_err() && !pr {
                    std::process::exit(1);
                }
            }
            RunCommands::Review {
                branch,
                base,
                repo,
                project,
                image,
                ssh_key,
                keep_alive: _,
                timeout,
                verbose,
            } => {
                let resolved_repo =
                    resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                if resolved_repo.starts_with("https://") {
                    eprintln!("Error: HTTPS URLs are not supported. Use SSH URLs (git@github.com:user/repo.git) with --ssh-key.");
                    std::process::exit(1);
                }

                let project_config = resolve_project_config(project.clone()).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agent_image = resolve_agent_image(image.as_deref());
                let ssh_key_path = resolve_ssh_key(ssh_key.as_ref(), project_config.as_ref());
                let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());
                let _resolved_remote = resolve_remote(project_config.as_ref());
                let project_script = resolve_project_script(project_config.as_ref());
                let pipeline_roles = resolve_pipeline_roles(project_config.as_ref(), "review");

                if verbose {
                    println!("Review: {}", branch);
                    println!("  Repository: {}", resolved_repo);
                    println!("  Agent image: {}", agent_image);
                }

                let script = project_script.clone();
                let done = std::sync::Arc::new(AtomicBool::new(false));
                let done_clone = done.clone();
                let spinner_handle = if !verbose {
                    Some(thread::spawn(move || run_spinner_until(&done_clone)))
                } else {
                    None
                };
                let _guard = if !verbose {
                    redirect_stderr_to_null()
                } else {
                    None
                };

                let timeout_secs = timeout.unwrap_or(300);
                let roles = pipeline_roles.clone();
                let result = dagger_pipeline::with_connection(move |conn| {
                    let repo = resolved_repo.clone();
                    let br = branch.clone();
                    let base_br = resolved_base.clone();
                    let _rem = _resolved_remote.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let to = timeout_secs;
                    let scr = script.clone();
                    let r = roles.clone();
                    async move {
                        dagger_pipeline::run_review(
                            &conn,
                            &repo,
                            &br,
                            Some(base_br.as_str()),
                            &img,
                            ssh.as_deref(),
                            to,
                            scr.as_deref(),
                            &r,
                        )
                        .await
                        .map_err(|e| eyre::eyre!("{}", e))
                    }
                })
                .await;

                if !verbose {
                    done.store(true, Ordering::Relaxed);
                    if let Some(h) = spinner_handle {
                        let _: std::thread::Result<()> = h.join();
                    }
                }
                drop(_guard);

                match result {
                    Ok(review_output) => println!("\n{}", review_output),
                    Err(e) => {
                        let msg = e.to_string();
                        eprintln!("Error: {}", clarify_dagger_error(&msg));
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_repo_prefers_explicit_repo() {
        assert_eq!(
            resolve_repo(Some("explicit".to_string()), Some("project".to_string())).unwrap(),
            "explicit"
        );
    }

    #[test]
    fn resolve_repo_requires_input() {
        assert!(resolve_repo(None, None).is_err());
    }

    #[test]
    fn toml_round_trip() {
        let mut cfg = SmithConfig::default();
        cfg.projects.push(ProjectConfig {
            name: "test".to_string(),
            repo: "https://example.com/repo".to_string(),
            image: None,
            ssh_key: None,
            base_branch: None,
            remote: None,
            github_token: None,
            script: None,
            commit_name: None,
            commit_email: None,
            agent: None,
            ask_setup_run: None,
            ask_setup_check: None,
            ask_execute_run: None,
            ask_execute_check: None,
            ask_validate_run: None,
            ask_validate_check: None,
            dev_setup_run: None,
            dev_setup_check: None,
            dev_execute_run: None,
            dev_execute_check: None,
            dev_validate_run: None,
            dev_validate_check: None,
            dev_commit_run: None,
            dev_commit_check: None,
            review_setup_run: None,
            review_setup_check: None,
            review_execute_run: None,
            review_execute_check: None,
            review_validate_run: None,
            review_validate_check: None,
        });
        let serialized = toml::to_string(&cfg).unwrap();
        let deserialized: SmithConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.projects.len(), 1);
        assert_eq!(deserialized.projects[0].name, "test");
    }
}
