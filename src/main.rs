mod dagger_pipeline;
mod docker;
mod github;

use clap::{CommandFactory, Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

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
                    out.push_str("\n");
                }
                out.push_str(stdout.trim());
            }
        }
        if let Some(stderr) = ext.get("stderr").and_then(Value::as_str) {
            if !stderr.trim().is_empty() {
                if !out.is_empty() {
                    out.push_str("\n");
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
#[command(version)]
#[command(about = "Agent Smith — open-source control plane for coding orchestration and configuration", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate the local environment
    Doctor {
        /// Show full Dagger and pipeline output
        #[arg(long)]
        verbose: bool,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigCommands,
    },
    /// Manage projects registered with Agent Smith
    Project {
        #[command(subcommand)]
        cmd: ProjectCommands,
    },
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
    /// Manage containers
    Container {
        #[command(subcommand)]
        cmd: ContainerCommands,
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

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show config file path
    Path,
    /// Set GitHub API token
    SetGitHubToken {
        /// GitHub personal access token
        token: String,
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
        /// Docker image to use for this project (optional)
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for this project (optional)
        #[arg(long)]
        ssh_key: Option<String>,
    },
    /// List all registered projects
    List,
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
    },
    /// Remove a project
    Remove {
        /// Project name
        name: String,
    },
}

#[derive(Subcommand)]
enum ContainerCommands {
    /// List all smith containers
    List,
    /// Stop a container
    Stop {
        /// Container name
        name: String,
    },
    /// Remove a container
    Remove {
        /// Container name
        name: String,
    },
}

#[derive(Serialize, Deserialize, Default)]
struct SmithConfig {
    projects: Vec<ProjectConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<GitHubConfig>,
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
}

fn config_dir() -> Result<PathBuf, String> {
    ProjectDirs::from("com", "agentsmith", "smith")
        .ok_or_else(|| "Could not determine config directory".to_string())
        .map(|dirs| dirs.config_dir().to_path_buf())
}

fn config_file_path() -> Result<PathBuf, String> {
    config_dir().map(|dir| dir.join("config.toml"))
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

/// Resolve Docker image to use (for Dagger pipeline base image)
/// Priority: explicit flag > project config > default
fn resolve_image(explicit_image: Option<&str>, project_config: Option<&ProjectConfig>) -> String {
    explicit_image
        .map(|s| s.to_string())
        .or_else(|| project_config.and_then(|p| p.image.clone()))
        .unwrap_or_else(|| "alpine:latest".to_string())
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            // Show help when no command provided
            let mut cmd = Cli::command();
            cmd.print_help().unwrap();
            std::process::exit(0);
        }
        Some(Commands::Doctor { verbose }) => {
            if verbose {
                println!("Doctor: Validating environment...");
            }

            // Check config directory
            match config_dir() {
                Ok(dir) => {
                    if verbose {
                        println!("  ✓ Config directory accessible: {}", dir.display());
                    }
                }
                Err(e) => {
                    println!("  ✗ Config directory error: {}", e);
                }
            }

            // Check Docker (Dagger uses it as container runtime)
            match docker::check_docker_available() {
                Ok(_) => {
                    if verbose {
                        println!("  ✓ Docker is available and running");
                    }
                }
                Err(e) => {
                    println!("  ✗ Docker: {}", e);
                }
            }

            // Check Dagger
            let dagger_ok = {
                let _guard = if !verbose {
                    redirect_stderr_to_null()
                } else {
                    None
                };
                dagger_pipeline::with_connection(|conn| async move {
                    dagger_pipeline::run_doctor(&conn).await
                })
                .await
            };
            match dagger_ok {
                Ok(_) => {
                    if verbose {
                        println!("  ✓ Dagger engine is available");
                    } else {
                        println!("Doctor: ✓ Config, Docker, and Dagger OK");
                    }
                }
                Err(e) => {
                    println!("  ✗ Dagger: {}", e);
                    println!("    Run ask/dev/review with: dagger run smith <command> ...");
                }
            }
        }
        Some(Commands::Config { cmd }) => match cmd {
            ConfigCommands::Path => {
                let file = config_file_path().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("{}", file.display());
            }
            ConfigCommands::SetGitHubToken { token } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                cfg.github = Some(GitHubConfig { token });
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("GitHub token configured successfully");
                println!("  Note: Token must have 'repo' scope (classic tokens) or 'pull-requests:write' permission (fine-grained tokens) to create PRs");
            }
        },
        Some(Commands::Project { cmd }) => match cmd {
            ProjectCommands::Add {
                name,
                repo,
                image,
                ssh_key,
            } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                if cfg.projects.iter().any(|p| p.name == name) {
                    eprintln!("Error: Project '{}' already exists", name);
                    std::process::exit(1);
                }
                let ssh_key = ssh_key.filter(|s| !s.is_empty());
                cfg.projects.push(ProjectConfig {
                    name,
                    repo,
                    image,
                    ssh_key,
                });
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
                        println!("{}", parts);
                    }
                }
            }
            ProjectCommands::Update {
                name,
                repo,
                image,
                ssh_key,
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
        },
        Some(Commands::Ask {
            question,
            base,
            repo,
            project,
            image,
            ssh_key,
            keep_alive: _,
            timeout,
            verbose,
        }) => {
            let resolved_repo = resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
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
            let resolved_image = resolve_image(image.as_deref(), project_config.as_ref());
            let ssh_key_path = resolve_ssh_key(ssh_key.as_ref(), project_config.as_ref());

            if verbose {
                println!("Ask: {}", question);
                println!("  Repository: {}", resolved_repo);
                println!("  Image: {}", resolved_image);
            }

            let branch = base.as_deref().unwrap_or("main").to_string();
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
                let img = resolved_image.clone();
                let ssh = ssh_key_path.clone();
                let br = branch.clone();
                let to = timeout_secs;
                async move {
                    dagger_pipeline::run_ask(
                        &conn,
                        &repo,
                        Some(br.as_str()),
                        &q,
                        &img,
                        ssh.as_deref(),
                        to,
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
                    let msg = format!("{}", e);
                    eprintln!("Error: {}", clarify_dagger_error(&msg));
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Dev {
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
        }) => {
            let resolved_repo = resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
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
            let resolved_image = resolve_image(image.as_deref(), project_config.as_ref());
            let ssh_key_path = resolve_ssh_key(ssh_key.as_ref(), project_config.as_ref());

            if verbose {
                println!("Dev: {}", task);
                println!("  Repository: {}", resolved_repo);
                println!("  Image: {}", resolved_image);
            }

            let branch_out = branch.clone();
            let resolved_repo_pr = resolved_repo.clone();
            let base_pr = base.clone();
            let task_pr = task.clone();
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
                let img = resolved_image.clone();
                let ssh = ssh_key_path.clone();
                let base_br = base.clone();
                let to = timeout_secs;
                async move {
                    dagger_pipeline::run_dev(
                        &conn,
                        &repo,
                        &br,
                        base_br.as_deref(),
                        &t,
                        &img,
                        ssh.as_deref(),
                        verb_for_closure,
                        to,
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
                        let msg = format!("{}", e);
                        eprintln!("Error: {}", clarify_dagger_error(&msg));
                        std::process::exit(1);
                    }
                }
            }

            if pr {
                let cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error loading config: {}", e);
                    std::process::exit(1);
                });
                if let Some(github_config) = cfg.github {
                    if let Ok(repo_info) = github::extract_repo_info(&resolved_repo_pr) {
                        let base_branch = base_pr.as_deref().unwrap_or("main");
                        match github::create_or_update_pr(
                            &github_config.token,
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
                    eprintln!("  ⚠ GitHub token not configured. Use 'smith config set-github-token <token>'");
                }
            }

            if dev_result.is_err() && !pr {
                std::process::exit(1);
            }
        }
        Some(Commands::Container { cmd }) => match cmd {
            ContainerCommands::List => match docker::list_containers() {
                Ok(containers) => {
                    if containers.is_empty() {
                        println!("No smith containers found");
                    } else {
                        println!("Smith containers:");
                        for container in containers {
                            println!("  {}", container);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
            ContainerCommands::Stop { name } => match docker::stop_container(&name) {
                Ok(_) => println!("Container '{}' stopped", name),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
            ContainerCommands::Remove { name } => match docker::remove_container(&name) {
                Ok(_) => println!("Container '{}' removed", name),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
        },
        Some(Commands::Review {
            branch,
            base,
            repo,
            project,
            image,
            ssh_key,
            keep_alive: _,
            timeout,
            verbose,
        }) => {
            let resolved_repo = resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
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
            let resolved_image = resolve_image(image.as_deref(), project_config.as_ref());
            let ssh_key_path = resolve_ssh_key(ssh_key.as_ref(), project_config.as_ref());

            if verbose {
                println!("Review: {}", branch);
                println!("  Repository: {}", resolved_repo);
                println!("  Image: {}", resolved_image);
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

            let timeout_secs = timeout.unwrap_or(300);
            let result = dagger_pipeline::with_connection(move |conn| {
                let repo = resolved_repo.clone();
                let br = branch.clone();
                let base_br = base.clone();
                let img = resolved_image.clone();
                let ssh = ssh_key_path.clone();
                let to = timeout_secs;
                async move {
                    dagger_pipeline::run_review(
                        &conn,
                        &repo,
                        &br,
                        base_br.as_deref(),
                        &img,
                        ssh.as_deref(),
                        to,
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
                    let msg = format!("{}", e);
                    eprintln!("Error: {}", clarify_dagger_error(&msg));
                    std::process::exit(1);
                }
            }
        }
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
        });
        let serialized = toml::to_string(&cfg).unwrap();
        let deserialized: SmithConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.projects.len(), 1);
        assert_eq!(deserialized.projects[0].name, "test");
    }
}
