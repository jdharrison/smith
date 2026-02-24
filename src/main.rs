mod dagger_pipeline;
mod docker;
mod github;

use clap::{CommandFactory, Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    let url = format!("http://host.docker.internal:{}", port);
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, url)
}

/// active = container running, reachable = health endpoint responds (only meaningful when active).
fn status_circle(active: bool, reachable: Option<bool>, built: bool) -> String {
    let bullet = "●";
    let color = if active && reachable == Some(false) {
        ANSI_YELLOW
    } else if active {
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
        /// Roles for this agent (comma-separated, e.g. "builder,analyst")
        #[arg(long, value_delimiter = ',')]
        roles: Option<Vec<String>>,
    },
    /// Show status of all configured agents (systemctl-style, compact table per agent)
    Status,
    /// Build Docker image for one agent or all (generate Dockerfile if missing, then docker build)
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
        /// Roles for this agent (comma-separated, e.g. "builder,analyst")
        #[arg(long, value_delimiter = ',')]
        roles: Option<Vec<String>>,
    },
    /// Remove an agent
    Remove {
        /// Agent name
        name: String,
    },
    /// Start cloud agents (opencode serve; 1 agent -> 1 container). Idempotent: skips if already running.
    Start {
        /// Print docker command and health-check details
        #[arg(short, long)]
        verbose: bool,
    },
    /// Stop all running agent containers
    Stop,
    /// Stream live logs from an agent container (docker logs -f)
    Logs {
        /// Agent name (e.g. opencode)
        name: String,
    },
    /// Reset all agents to default (opencode only). Prompts for confirmation unless --force.
    Reset {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
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
    },
    /// Remove a project
    Remove {
        /// Project name
        name: String,
    },
    /// Remove all projects. Prompts for confirmation unless --force.
    Reset {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
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
    /// Roles this agent can fulfill (e.g. ["builder", "analyst"]). If empty, agent can do all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    roles: Vec<String>,
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
    roles: Vec<String>,
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
    let port = port.or_else(|| Some(docker::OPENCODE_SERVER_PORT + agents.len() as u16));
    let enabled = enabled.or(Some(true));
    agents.push(AgentEntry {
        name: name.clone(),
        image,
        agent_type,
        model,
        small_model,
        provider,
        base_url,
        port,
        enabled,
        roles,
    });
    if cfg.current_agent.is_none() {
        cfg.current_agent = Some(name);
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

/// Column width for subcommand names so descriptions align (clap-style).
const HELP_NAME_WIDTH: usize = 18;

fn print_smith_help() {
    let c = Cli::command();
    if let Some(about) = c.get_about() {
        println!("{}\n", about);
    }
    println!("Usage: smith [COMMAND]");
    const SYSTEM: &[&str] = &["status", "install", "help", "version"];
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
            let agents: Vec<String> = if list.is_empty() {
                vec![DEFAULT_AGENT_NAME.to_string()]
            } else {
                list.iter().map(|e| e.name.clone()).collect()
            };
            let any_running = agents.iter().any(|n| running.contains(n));

            let smith_bullet = if installed && any_running {
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
            let any_built = agents.iter().any(|name| {
                docker::image_exists(&docker::agent_built_image_tag(name)).unwrap_or(false)
            });
            let agents_bullet = if any_running {
                BULLET_GREEN
            } else if any_built {
                BULLET_BLUE
            } else {
                BULLET_RED
            };
            println!("  {} agents", agents_bullet);
            for (i, name) in agents.iter().enumerate() {
                let active = running.contains(name);
                let built =
                    docker::image_exists(&docker::agent_built_image_tag(name)).unwrap_or(false);
                let port = list
                    .iter()
                    .find(|e| e.name == *name)
                    .map(|e| agent_port(e, i))
                    .unwrap_or_else(|| docker::OPENCODE_SERVER_PORT + i as u16);
                let reachable = if active {
                    Some(docker::check_agent_reachable(port))
                } else {
                    None
                };
                let (bullet, state) = if active && reachable == Some(false) {
                    (BULLET_YELLOW, "running (port unreachable)")
                } else if active {
                    (BULLET_GREEN, "running")
                } else if built {
                    (BULLET_BLUE, "built")
                } else {
                    (BULLET_RED, "not built")
                };
                if active {
                    println!(
                        "       {} agent/{} - {} {}",
                        bullet,
                        name,
                        state,
                        clickable_agent_url(port)
                    );
                } else {
                    println!(
                        "       {} agent/{} - {} (http://host.docker.internal:{})",
                        bullet, name, state, port
                    );
                }
            }
            if agents.is_empty() {
                println!("       {} (no cloud agents configured)", BULLET_RED);
            }

            // projects: workspace check per project, then summary bullet (green=all ok, yellow=some, red=none)
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
                            if output.contains(".git") {
                                project_results.push((name, true, "workspace OK".to_string()));
                            } else {
                                project_results.push((
                                    name,
                                    false,
                                    "workspace missing .git".to_string(),
                                ));
                            }
                        }
                        Err(_) => {
                            project_results.push((name, false, "clone failed".to_string()));
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
                    println!("       {} project/{} - {}", bullet, name, msg);
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
                println!("  Current agents: (none — opencode used by default)");
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
                    let roles_in = prompt_line(
                        "  Roles (comma-separated, e.g. builder,analyst, Enter for none): ",
                    );
                    let roles: Vec<String> = if roles_in.is_empty() {
                        vec![]
                    } else {
                        roles_in.split(',').map(|s| s.trim().to_string()).collect()
                    };
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
                        roles,
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
                roles,
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
                    roles.unwrap_or_default(),
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
                let current = cfg.current_agent.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
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
                    Vec<String>,
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
                        vec![],
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
                    let circle = status_circle(active, reachable, built);
                    let current_mark = if *name == current { " (current)" } else { "" };
                    println!("\n  {} agent/{} - {}{}", circle, name, image, current_mark);
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
                    let built_tag = docker::agent_built_image_tag(name);
                    let image_line = if built {
                        built_tag
                    } else {
                        format!("not built (uses {})", image)
                    };
                    println!("      Image:    {}", image_line);
                    let model_str = model.as_deref().unwrap_or("default");
                    let small_model_str = small_model.as_deref().unwrap_or("-");
                    let is_local = agent_type.as_deref() == Some("local");
                    let provider_str = provider.as_deref().unwrap_or("-");
                    let mode_str = if is_local { "local" } else { "cloud" };
                    println!(
                        "      Model:    {}  Small: {}  Provider: {}  Type: {}",
                        model_str, small_model_str, provider_str, mode_str
                    );
                    let roles_str = if roles.is_empty() {
                        "-".to_string()
                    } else {
                        roles.join(", ")
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
                roles,
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let agents = cfg.agents.get_or_insert_with(Vec::new);
                match agents.iter_mut().find(|a| a.name == name) {
                    Some(entry) => {
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
                        if let Some(ref r) = roles {
                            entry.roles = if r.is_empty() { vec![] } else { r.clone() };
                        }
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("Agent '{}' updated successfully", name);
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
                            None,
                            None,
                            None,
                            docker::OPENCODE_SERVER_PORT,
                        )],
                    }
                } else {
                    let n = name.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
                    let (base_image, model, small_model, provider, port) = cfg
                        .agents
                        .as_deref()
                        .and_then(|a| {
                            a.iter().position(|e| e.name == n).map(|idx| {
                                let e = &a[idx];
                                (
                                    e.image.clone(),
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
                for (agent_name, base_image, model, small_model, provider, port) in &agents {
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

                // Check if any agent uses local - get model from agent.model
                let local_agent = agents
                    .iter()
                    .find(|e| e.agent_type.as_deref() == Some("local"));
                let local_model = local_agent
                    .and_then(|a| a.model.clone())
                    .unwrap_or_else(|| "qwen3:8b".to_string());

                // Start Ollama if any agent uses local
                if !local_model.is_empty() {
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

                // Build agent list (1 agent : 1 container) - only enabled agents
                let enabled_agents: Vec<_> = if agents.is_empty() {
                    vec![(
                        DEFAULT_AGENT_NAME.to_string(),
                        DEFAULT_AGENT_IMAGE.to_string(),
                        None,
                        None,
                        true,
                        docker::OPENCODE_SERVER_PORT,
                    )]
                } else {
                    agents
                        .iter()
                        .filter(|e| e.enabled.unwrap_or(true))
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
                    println!("All {} cloud agent(s) started and tested successfully.", ok);
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
                match docker::stop_all_agent_containers() {
                    Ok(stopped) => {
                        if stopped.is_empty() {
                            println!("No running agent containers.");
                        } else {
                            for name in &stopped {
                                println!("  {}: stopped", name);
                            }
                            println!("Stopped {} container(s).", stopped.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
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
            AgentCommands::Reset { force } => {
                if !confirm_reset(
                    "Reset all agents to default (opencode only)? Type 'yes' to confirm: ",
                    force,
                ) {
                    eprintln!("Reset cancelled.");
                    std::process::exit(1);
                }
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                cfg.agents = None;
                cfg.agent = None;
                cfg.current_agent = None;
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("Agents reset to default (opencode).");
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
                let project = ProjectConfig {
                    name: name.clone(),
                    repo,
                    image,
                    ssh_key,
                    base_branch,
                    remote,
                    github_token,
                    script,
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
                            if output.contains(".git") {
                                println!("\n  {} {} - workspace OK", BULLET_GREEN, proj.name);
                                if verbose {
                                    println!("  ---");
                                    for line in output.lines() {
                                        println!("    {}", line);
                                    }
                                }
                            } else {
                                eprintln!(
                                    "  {} {} - failed: workspace missing .git (clone may have failed)",
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
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                let project = cfg.projects.iter_mut().find(|p| p.name == name);
                match project {
                    Some(proj) => {
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
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("Project '{}' updated successfully", name);
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
            ProjectCommands::Reset { force } => {
                if !confirm_reset("Remove all projects? Type 'yes' to confirm: ", force) {
                    eprintln!("Reset cancelled.");
                    std::process::exit(1);
                }
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                cfg.projects.clear();
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("All projects removed.");
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
                let result = dagger_pipeline::with_connection(move |conn| {
                    let repo = resolved_repo.clone();
                    let q = question.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let br = branch.clone();
                    let to = timeout_secs;
                    let scr = script.clone();
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
                let result = dagger_pipeline::with_connection(move |conn| {
                    let repo = resolved_repo.clone();
                    let br = branch.clone();
                    let base_br = resolved_base.clone();
                    let _rem = _resolved_remote.clone();
                    let img = agent_image.clone();
                    let ssh = ssh_key_path.clone();
                    let to = timeout_secs;
                    let scr = script.clone();
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
        });
        let serialized = toml::to_string(&cfg).unwrap();
        let deserialized: SmithConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.projects.len(), 1);
        assert_eq!(deserialized.projects[0].name, "test");
    }
}
