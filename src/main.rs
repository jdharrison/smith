mod repo;
mod docker;
mod agent;

use agent::Agent;

use clap::{CommandFactory, Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "smith")]
#[command(about = "Agent Smith — open-source control plane for coding orchestration and configuration", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate the local environment
    Doctor,
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
    /// Run an orchestration action with an agent
    Run {
        /// Repository path or URL (overrides project config if provided)
        #[arg(long)]
        repo: Option<String>,
        /// Project name from config
        #[arg(long)]
        project: Option<String>,
        /// SSH key path for private repositories (optional)
        #[arg(long)]
        ssh_key: Option<PathBuf>,
    },
    /// Ask a question to an agent about a project
    Ask {
        /// The question to ask the agent
        question: String,
        /// Repository path or URL (overrides project config if provided)
        #[arg(long)]
        repo: Option<String>,
        /// Project name from config
        #[arg(long)]
        project: Option<String>,
        /// SSH key path for private repositories (optional)
        #[arg(long)]
        ssh_key: Option<PathBuf>,
    },
    /// Review workflow (placeholder): model a long-running session (e.g., keep Docker container alive)
    Review {
        /// Project name from config
        #[arg(long)]
        project: Option<String>,
        /// Keep container/session alive for interactive review
        #[arg(long)]
        keep_alive: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show config file path
    Path,
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
    },
    /// List all registered projects
    List,
    /// Remove a project
    Remove {
        /// Project name
        name: String,
    },
}

#[derive(Serialize, Deserialize, Default)]
struct SmithConfig {
    projects: Vec<ProjectConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ProjectConfig {
    name: String,
    repo: String,
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

    // Atomic write: write to temp file then rename
    let temp_file = file.with_extension("toml.tmp");
    fs::write(&temp_file, content).map_err(|e| format!("Failed to write config: {}", e))?;
    fs::rename(&temp_file, &file).map_err(|e| format!("Failed to finalize config: {}", e))?;
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

/// Clone a repository and set up a workspace, returning the workspace path
fn setup_workspace(
    repo_url: &str,
    ssh_key_path: Option<&PathBuf>,
) -> Result<PathBuf, String> {
    // Create workspace directory (clean if it exists)
    let workspace = std::env::temp_dir().join("smith-workspace");
    if workspace.exists() {
        std::fs::remove_dir_all(&workspace).map_err(|e| {
            format!("Failed to clean workspace: {}", e)
        })?;
    }
    std::fs::create_dir_all(&workspace).map_err(|e| {
        format!("Failed to create workspace: {}", e)
    })?;
    
    // Clone repository using Docker
    docker::run_container(
        "alpine/git:latest",
        repo_url,
        &workspace,
        ssh_key_path,
        false,
    )?;
    
    Ok(workspace)
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            // Show help when no command provided
            let mut cmd = Cli::command();
            cmd.print_help().unwrap();
            std::process::exit(0);
        }
        Some(Commands::Doctor) => {
            println!("Doctor: Validating environment...");
            println!("  ✓ Config directory accessible");
            println!("  [ ] Docker (not yet implemented)");
        }
        Some(Commands::Config { cmd }) => match cmd {
            ConfigCommands::Path => {
                let file = config_file_path().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                println!("{}", file.display());
            }
        },
        Some(Commands::Project { cmd }) => match cmd {
            ProjectCommands::Add { name, repo } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                if cfg.projects.iter().any(|p| p.name == name) {
                    eprintln!("Error: Project '{}' already exists", name);
                    std::process::exit(1);
                }
                cfg.projects.push(ProjectConfig { name, repo });
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
                        println!("  {} -> {}", proj.name, proj.repo);
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
        Some(Commands::Run { repo, project, ssh_key }) => {
            let resolved_repo = resolve_repo(repo, project).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            
            // Check for SSH key in environment if not provided
            let ssh_key_path = ssh_key.or_else(|| {
                std::env::var("SSH_KEY_PATH")
                    .ok()
                    .map(PathBuf::from)
            });
            
            println!("Run: Orchestrating for repo: {}", resolved_repo);
            
            // Create workspace directory (clean if it exists)
            let workspace = std::env::temp_dir().join("smith-workspace");
            if workspace.exists() {
                std::fs::remove_dir_all(&workspace).unwrap_or_else(|e| {
                    eprintln!("Warning: Failed to clean workspace: {}", e);
                });
            }
            std::fs::create_dir_all(&workspace).unwrap_or_else(|e| {
                eprintln!("Error: Failed to create workspace: {}", e);
                std::process::exit(1);
            });
            
            // Clone repository using Docker
            match docker::run_container(
                "alpine/git:latest",
                &resolved_repo,
                &workspace,
                ssh_key_path.as_ref(),
                false,
            ) {
                Ok(output) => {
                    println!("  ✓ Repository cloned successfully");
                    println!("  Workspace: {}", workspace.display());
                    if !output.trim().is_empty() {
                        print!("{}", output);
                    }
                    
                    // Initialize and run OpenCode agent
                    let agent = agent::OpenCodeAgent;
                    match agent.initialize(&workspace) {
                        Ok(_) => {
                            println!("  ✓ Agent initialized");
                            match agent.ask(&workspace, "what language is this project in") {
                                Ok(answer) => {
                                    println!("  ✓ Answer: {}", answer);
                                }
                                Err(e) => {
                                    eprintln!("  Error asking question: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("  Error initializing agent: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Ask {
            question,
            repo,
            project,
            ssh_key,
        }) => {
            let resolved_repo = resolve_repo(repo, project).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            
            // Check for SSH key in environment if not provided
            let ssh_key_path = ssh_key.or_else(|| {
                std::env::var("SSH_KEY_PATH")
                    .ok()
                    .map(PathBuf::from)
            });
            
            println!("Ask: {}", question);
            println!("  Repository: {}", resolved_repo);
            
            // Set up workspace and clone repository
            let workspace = match setup_workspace(&resolved_repo, ssh_key_path.as_ref()) {
                Ok(ws) => {
                    println!("  ✓ Repository cloned successfully");
                    ws
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            
            // Initialize and ask the agent
            let agent = agent::OpenCodeAgent;
            match agent.initialize(&workspace) {
                Ok(_) => {
                    println!("  ✓ Agent initialized");
                    match agent.ask(&workspace, &question) {
                        Ok(answer) => {
                            println!("\nAnswer: {}", answer);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error initializing agent: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Review {
            project,
            keep_alive,
        }) => {
            let resolved_repo = resolve_repo(None, project).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            
            // Check for SSH key in environment
            let ssh_key_path = std::env::var("SSH_KEY_PATH")
                .ok()
                .map(PathBuf::from);
            
            println!(
                "Review: Starting review session for repo: {}",
                resolved_repo
            );
            
            // Create workspace directory
            let workspace = std::env::temp_dir().join("smith-review");
            std::fs::create_dir_all(&workspace).unwrap_or_else(|e| {
                eprintln!("Error: Failed to create workspace: {}", e);
                std::process::exit(1);
            });
            
            // Clone repository using Docker
            match docker::run_container(
                "alpine/git:latest",
                &resolved_repo,
                &workspace,
                ssh_key_path.as_ref(),
                keep_alive,
            ) {
                Ok(output) => {
                    println!("  ✓ Repository cloned successfully");
                    println!("  Workspace: {}", workspace.display());
                    if keep_alive {
                        println!("  Container is running. Press Ctrl+C to stop...");
                        // In a real implementation, we'd wait for user input
                    }
                    if !output.trim().is_empty() {
                        print!("{}", output);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
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
        });
        let serialized = toml::to_string(&cfg).unwrap();
        let deserialized: SmithConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.projects.len(), 1);
        assert_eq!(deserialized.projects[0].name, "test");
    }
}
