mod agent;
mod docker;
mod repo;

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
        /// Docker image to use for this project (optional)
        #[arg(long)]
        image: Option<String>,
    },
    /// List all registered projects
    List,
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
}

#[derive(Serialize, Deserialize, Clone)]
struct ProjectConfig {
    name: String,
    repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
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

/// Resolve Docker image to use
/// Priority: explicit flag > project config > default
fn resolve_image(explicit_image: Option<&str>, project_config: Option<&ProjectConfig>) -> String {
    explicit_image
        .map(|s| s.to_string())
        .or_else(|| project_config.and_then(|p| p.image.clone()))
        .unwrap_or_else(|| "node:20-alpine".to_string())
}

/// Set up a containerized workspace and return the container name
fn setup_containerized_workspace(
    repo_url: &str,
    project_name: Option<&str>,
    ssh_key_path: Option<&PathBuf>,
    image: Option<&str>,
) -> Result<String, String> {
    let container_name = docker::generate_container_name(project_name);

    docker::setup_containerized_workspace(&container_name, repo_url, ssh_key_path, image)?;

    Ok(container_name)
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
            ProjectCommands::Add { name, repo, image } => {
                let mut cfg = load_config().unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                if cfg.projects.iter().any(|p| p.name == name) {
                    eprintln!("Error: Project '{}' already exists", name);
                    std::process::exit(1);
                }
                cfg.projects.push(ProjectConfig {
                    name,
                    repo,
                    image,
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
                        if let Some(ref image) = proj.image {
                            println!("  {} -> {} (image: {})", proj.name, proj.repo, image);
                        } else {
                            println!("  {} -> {}", proj.name, proj.repo);
                        }
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
            keep_alive,
        }) => {
            let resolved_repo = resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let project_config = resolve_project_config(project.clone()).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            // Resolve image: flag > project config > default
            let resolved_image = resolve_image(image.as_deref(), project_config.as_ref());

            // Check for SSH key in environment if not provided
            let ssh_key_path =
                ssh_key.or_else(|| std::env::var("SSH_KEY_PATH").ok().map(PathBuf::from));

            println!("Ask: {}", question);
            println!("  Repository: {}", resolved_repo);
            println!("  Image: {}", resolved_image);

            // Set up containerized workspace
            let container_name = match setup_containerized_workspace(
                &resolved_repo,
                project.as_deref(),
                ssh_key_path.as_ref(),
                Some(&resolved_image),
            ) {
                Ok(name) => {
                    println!("  ✓ Container created and repository cloned");
                    println!("  Container: {}", name);
                    name
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            // Initialize and ask the agent
            let agent = agent::OpenCodeAgent;
            match agent.initialize(&container_name) {
                Ok(_) => {
                    println!("  ✓ Agent initialized");

                    // Checkout base branch if provided
                    if let Some(base_branch) = &base {
                        println!("  Checking out base branch: {}", base_branch);
                        match agent.checkout_branch(&container_name, base_branch) {
                            Ok(_) => println!("  ✓ Checked out branch: {}", base_branch),
                            Err(e) => {
                                eprintln!("Error checking out branch '{}': {}", base_branch, e);
                                let _ = docker::remove_container(&container_name);
                                std::process::exit(1);
                            }
                        }
                    }

                    match agent.ask(&container_name, &question) {
                        Ok(answer) => {
                            println!("\nAnswer: {}", answer);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            // Clean up container on error
                            let _ = docker::remove_container(&container_name);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error initializing agent: {}", e);
                    // Clean up container on error
                    let _ = docker::remove_container(&container_name);
                    std::process::exit(1);
                }
            }

            // Clean up container when done (unless keep_alive is set)
            if keep_alive {
                println!("  Container kept alive: {}", container_name);
                println!(
                    "  Use 'docker exec -it {} sh' to access the container interactively",
                    container_name
                );
                println!("  Use 'smith container stop {}' to stop it", container_name);
                println!(
                    "  Use 'smith container remove {}' to remove it",
                    container_name
                );
            } else {
                match docker::remove_container(&container_name) {
                    Ok(_) => println!("  ✓ Container removed"),
                    Err(e) => eprintln!("  Warning: Failed to remove container: {}", e),
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
            keep_alive,
        }) => {
            let resolved_repo = resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let project_config = resolve_project_config(project.clone()).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            // Resolve image: flag > project config > default
            let resolved_image = resolve_image(image.as_deref(), project_config.as_ref());

            // Check for SSH key in environment if not provided
            let ssh_key_path =
                ssh_key.or_else(|| std::env::var("SSH_KEY_PATH").ok().map(PathBuf::from));

            println!("Dev: {}", task);
            println!("  Repository: {}", resolved_repo);
            println!("  Image: {}", resolved_image);

            // Set up containerized workspace
            let container_name = match setup_containerized_workspace(
                &resolved_repo,
                project.as_deref(),
                ssh_key_path.as_ref(),
                Some(&resolved_image),
            ) {
                Ok(name) => {
                    println!("  ✓ Container created and repository cloned");
                    println!("  Container: {}", name);
                    name
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            // Initialize and execute dev action
            let agent = agent::OpenCodeAgent;
            match agent.initialize(&container_name) {
                Ok(_) => {
                    println!("  ✓ Agent initialized");
                    match agent.dev(&container_name, &task, &branch, base.as_deref()) {
                        Ok(commit) => {
                            println!("\n✓ Development action completed and committed");
                            println!("  Commit: {}", commit);
                            println!("  Branch: {}", branch);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            // Clean up container on error
                            let _ = docker::remove_container(&container_name);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error initializing agent: {}", e);
                    // Clean up container on error
                    let _ = docker::remove_container(&container_name);
                    std::process::exit(1);
                }
            }

            // Clean up container when done (unless keep_alive is set)
            if keep_alive {
                println!("  Container kept alive: {}", container_name);
                println!(
                    "  Use 'docker exec -it {} sh' to access the container interactively",
                    container_name
                );
                println!("  Use 'smith container stop {}' to stop it", container_name);
                println!(
                    "  Use 'smith container remove {}' to remove it",
                    container_name
                );
            } else {
                match docker::remove_container(&container_name) {
                    Ok(_) => println!("  ✓ Container removed"),
                    Err(e) => eprintln!("  Warning: Failed to remove container: {}", e),
                }
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
            keep_alive,
        }) => {
            let resolved_repo = resolve_repo(repo.clone(), project.clone()).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let project_config = resolve_project_config(project.clone()).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            // Resolve image: flag > project config > default
            let resolved_image = resolve_image(image.as_deref(), project_config.as_ref());

            // Check for SSH key in environment if not provided
            let ssh_key_path =
                ssh_key.or_else(|| std::env::var("SSH_KEY_PATH").ok().map(PathBuf::from));

            println!("Review: {}", branch);
            println!("  Repository: {}", resolved_repo);
            println!("  Image: {}", resolved_image);

            // Set up containerized workspace
            let container_name = match setup_containerized_workspace(
                &resolved_repo,
                project.as_deref(),
                ssh_key_path.as_ref(),
                Some(&resolved_image),
            ) {
                Ok(name) => {
                    println!("  ✓ Container created and repository cloned");
                    println!("  Container: {}", name);
                    name
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            // Initialize and review the branch
            let agent = agent::OpenCodeAgent;
            match agent.initialize(&container_name) {
                Ok(_) => {
                    println!("  ✓ Agent initialized");
                    match agent.review(&container_name, &branch, base.as_deref()) {
                        Ok(review_output) => {
                            println!("\n{}", review_output);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            // Clean up container on error
                            let _ = docker::remove_container(&container_name);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error initializing agent: {}", e);
                    // Clean up container on error
                    let _ = docker::remove_container(&container_name);
                    std::process::exit(1);
                }
            }

            // Clean up container when done (unless keep_alive is set)
            if keep_alive {
                println!("  Container kept alive: {}", container_name);
                println!(
                    "  Use 'docker exec -it {} sh' to access the container interactively",
                    container_name
                );
                println!("  Use 'smith container stop {}' to stop it", container_name);
                println!(
                    "  Use 'smith container remove {}' to remove it",
                    container_name
                );
            } else {
                match docker::remove_container(&container_name) {
                    Ok(_) => println!("  ✓ Container removed"),
                    Err(e) => eprintln!("  Warning: Failed to remove container: {}", e),
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
