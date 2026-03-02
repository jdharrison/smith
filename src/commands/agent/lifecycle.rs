use crate::*;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::Start {
            project,
            branch,
            port,
        } => {
            // Auto-detect project from cwd if not provided
            let project = match project {
                Some(p) => p,
                None => match detect_project_from_cwd() {
                    Ok(Some(name)) => name,
                    Ok(None) => {
                        eprintln!("Error: No project specified and none detected from current directory. Use --project or run from a git repo that matches a configured project.");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
            };

            // Auto-detect branch from current git branch if not provided
            let branch = match branch {
                Some(b) => b,
                None => {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!("Error: No branch specified and failed to detect from git. Use --branch.");
                            std::process::exit(1);
                        }
                    }
                }
            };

            // Resolve project config
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let proj = cfg
                .projects
                .iter()
                .find(|p| p.name == project)
                .ok_or_else(|| format!("Project '{}' not found", project))
                .unwrap();

            let image = proj
                .image
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_IMAGE.to_string());
            let repo = proj.repo.clone();
            let ssh_key = proj.ssh_key.as_ref().map(PathBuf::from);
            let commit_name = proj.commit_name.clone();
            let commit_email = proj.commit_email.clone();

            // Determine port
            let final_port = match port {
                Some(p) => p,
                None => docker::spawn_container_port(&project, &branch),
            };

            println!(
                "  {} Starting agent for {}{}:{}",
                BULLET_BLUE, project, ANSI_RESET, branch
            );
            println!("       Image: {}", image);
            println!("       Repo: {}", repo);
            println!("       Port: {}", final_port);
            match docker::host_opencode_config_dir() {
                Some(path) if path.exists() => {
                    println!(
                        "       OpenCode config: {} (mounted read-only)",
                        path.display()
                    );
                }
                Some(path) => {
                    println!(
                        "       OpenCode config: {} (not found, mount skipped)",
                        path.display()
                    );
                }
                None => {
                    println!(
                        "       OpenCode config: unavailable (could not resolve host config dir)"
                    );
                }
            }

            match docker::start_spawned_container(
                &project,
                &branch,
                final_port,
                &image,
                &repo,
                ssh_key.as_deref(),
                commit_name.as_deref(),
                commit_email.as_deref(),
            ) {
                Ok(actual_port) => {
                    let url = clickable_agent_url(actual_port);
                    println!("  {} Agent ready at {}", BULLET_GREEN, url);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AgentCommands::Stop {
            project,
            branch,
            all,
        } => {
            if all {
                // Stop all spawned agents
                match docker::list_spawned_containers() {
                    Ok(containers) => {
                        let running: Vec<_> = containers
                            .iter()
                            .filter(|c| c.status.to_lowercase().contains("up"))
                            .collect();
                        if running.is_empty() {
                            println!("No running spawned agents");
                        } else {
                            println!("Stopping {} spawned agents:", running.len());
                            for c in &running {
                                if let Err(e) =
                                    docker::stop_spawned_container(&c.project, &c.branch)
                                {
                                    eprintln!("Error stopping {}::{}: {}", c.project, c.branch, e);
                                } else {
                                    println!(
                                        "  {} Stopped {}{}:{}",
                                        BULLET_GREEN, c.project, ANSI_RESET, c.branch
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Stop specific agent - auto-detect project/branch if not provided
                let project = match project {
                    Some(p) => p,
                    None => match detect_project_from_cwd() {
                        Ok(Some(name)) => name,
                        _ => {
                            eprintln!("Error: --project required (or use --all to stop all)");
                            std::process::exit(1);
                        }
                    },
                };
                let branch = match branch {
                    Some(b) => b,
                    None => {
                        let output = Command::new("git")
                            .args(["rev-parse", "--abbrev-ref", "HEAD"])
                            .output();
                        match output {
                            Ok(out) if out.status.success() => {
                                String::from_utf8_lossy(&out.stdout).trim().to_string()
                            }
                            _ => {
                                eprintln!("Error: --branch required (or use --all to stop all)");
                                std::process::exit(1);
                            }
                        }
                    }
                };
                match docker::stop_spawned_container(&project, &branch) {
                    Ok(()) => {
                        println!(
                            "  {} Stopped agent for {}{}:{}",
                            BULLET_GREEN, project, ANSI_RESET, branch
                        );
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        AgentCommands::Restart { project, branch } => {
            // Auto-detect project and branch if not provided
            let project = match project {
                Some(p) => p,
                None => match detect_project_from_cwd() {
                    Ok(Some(name)) => name,
                    _ => {
                        eprintln!("Error: --project required");
                        std::process::exit(1);
                    }
                },
            };
            let branch = match branch {
                Some(b) => b,
                None => {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!("Error: --branch required");
                            std::process::exit(1);
                        }
                    }
                }
            };

            println!(
                "  {} Restarting agent for {}{}:{}",
                BULLET_BLUE, project, ANSI_RESET, branch
            );

            match docker::restart_spawned_container(&project, &branch) {
                Ok(()) => {
                    println!(
                        "  {} Restarted agent for {}{}:{}",
                        BULLET_GREEN, project, ANSI_RESET, branch
                    );
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AgentCommands::Run {
            project,
            branch,
            verbose,
            prompt,
        } => {
            // Auto-detect project and branch if not provided
            let project = match project {
                Some(p) => p,
                None => match detect_project_from_cwd() {
                    Ok(Some(name)) => name,
                    _ => {
                        eprintln!("Error: --project required");
                        std::process::exit(1);
                    }
                },
            };
            let branch = match branch {
                Some(b) => b,
                None => {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!("Error: --branch required");
                            std::process::exit(1);
                        }
                    }
                }
            };

            match docker::run_prompt_in_spawned_container(&project, &branch, &prompt, verbose) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AgentCommands::List => match docker::list_spawned_containers() {
            Ok(containers) => {
                if containers.is_empty() {
                    println!("No spawned agents");
                } else {
                    println!("Spawned agents:");
                    for c in containers {
                        let status_color = if c.status.to_lowercase().contains("up") {
                            ANSI_GREEN
                        } else if c.status.to_lowercase().contains("exited") {
                            ANSI_YELLOW
                        } else {
                            ANSI_RED
                        };
                        println!(
                            "  {} {}::{} - {} (name: {}, id: {}, port: {}, image: {})",
                            status_color,
                            c.project,
                            c.branch,
                            c.status,
                            c.container_name,
                            c.container_id,
                            c.port,
                            c.image
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        AgentCommands::Clear {
            project,
            branch,
            all,
            plan,
            state,
        } => {
            let project = match project {
                Some(p) => p,
                None => match detect_project_from_cwd() {
                    Ok(Some(name)) => name,
                    _ => {
                        eprintln!("Error: --project required");
                        std::process::exit(1);
                    }
                },
            };
            let branch = match branch {
                Some(b) => b,
                None => {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!("Error: --branch required");
                            std::process::exit(1);
                        }
                    }
                }
            };

            if !all && plan.is_none() && state.is_none() {
                eprintln!(
                        "Error: specify at least one filter (--plan/--state) or use --all to clear all plans"
                    );
                std::process::exit(1);
            }

            if let Err(e) = docker::ensure_spawn_state_dir(&project, &branch) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let mut plan_dirs = match docker::list_spawn_plan_dirs(&project, &branch) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            if plan_dirs.is_empty() {
                println!("No plan runs found in /state for {}:{}", project, branch);
                return;
            }

            plan_dirs.sort();
            let state_filter = state.as_ref().map(|s| s.to_lowercase());
            let resolved_plan_filter = match plan.as_ref() {
                Some(filter) => match resolve_plan_id_filter(filter, &plan_dirs) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                None => None,
            };

            let mut candidates: Vec<String> = Vec::new();
            for dir_name in plan_dirs {
                if !all {
                    if let Some(target_plan) = resolved_plan_filter.as_ref() {
                        if &dir_name != target_plan {
                            continue;
                        }
                    }

                    if let Some(ref desired_state) = state_filter {
                        let manifest_path = format!("/state/{}/manifest.json", dir_name);
                        let actual_state =
                            match docker::read_spawn_file(&project, &branch, &manifest_path) {
                                Ok(raw) => serde_json::from_str::<PlanManifest>(&raw)
                                    .map(|m| m.state.to_lowercase())
                                    .unwrap_or_else(|_| "unknown".to_string()),
                                Err(_) => "unknown".to_string(),
                            };
                        if actual_state != *desired_state {
                            continue;
                        }
                    }
                }

                candidates.push(dir_name);
            }

            if candidates.is_empty() {
                println!("No matching plan runs to clear");
                return;
            }

            let mut removed = Vec::new();
            let mut failed = Vec::new();
            for dir_name in candidates {
                let path = format!("/state/{}", dir_name);
                match docker::remove_spawn_dir(&project, &branch, &path) {
                    Ok(()) => removed.push(dir_name),
                    Err(e) => failed.push(format!("{} ({})", dir_name, e)),
                }
            }

            if !removed.is_empty() {
                println!("Cleared {} plan run(s):", removed.len());
                for dir_name in removed {
                    println!("  - {}", dir_name);
                }
            }

            if !failed.is_empty() {
                eprintln!("Failed to clear {} plan run(s):", failed.len());
                for line in failed {
                    eprintln!("  - {}", line);
                }
                std::process::exit(1);
            }
        }
        AgentCommands::Logs {
            project,
            branch,
            follow,
        } => {
            // Auto-detect project and branch if not provided
            let project = match project {
                Some(p) => p,
                None => match detect_project_from_cwd() {
                    Ok(Some(name)) => name,
                    _ => {
                        eprintln!("Error: --project required");
                        std::process::exit(1);
                    }
                },
            };
            let branch = match branch {
                Some(b) => b,
                None => {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!("Error: --branch required");
                            std::process::exit(1);
                        }
                    }
                }
            };

            let name = docker::spawn_container_name(&project, &branch);

            if follow {
                let running = Command::new("docker")
                    .args(["inspect", &name, "-f", "{{.State.Running}}"])
                    .output();

                match running {
                    Ok(out) if out.status.success() => {
                        let is_running = String::from_utf8_lossy(&out.stdout).trim() == "true";
                        if !is_running {
                            eprintln!(
                                    "Error: container '{}' is not running. Start it with `smith agent start`.",
                                    name
                                );
                            std::process::exit(1);
                        }
                    }
                    Ok(_) => {
                        eprintln!("Error: container not found or not accessible");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error: Failed to inspect container: {}", e);
                        std::process::exit(1);
                    }
                }
            }

            let mut cmd = Command::new("docker");
            cmd.arg("logs");
            if follow {
                cmd.arg("-f");
            }
            cmd.arg(&name);

            let status = cmd.status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => {
                    eprintln!("Error: container not found or not accessible");
                    if let Some(code) = s.code() {
                        std::process::exit(code);
                    }
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error: Failed to get logs: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AgentCommands::Prune => match docker::prune_spawned_containers() {
            Ok(removed) => {
                if removed.is_empty() {
                    println!("No stopped containers to prune");
                } else {
                    println!("Pruned {} containers:", removed.len());
                    for c in removed {
                        println!("  - {}", c);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        _ => unreachable!("non-lifecycle command routed to lifecycle handler"),
    }
}
