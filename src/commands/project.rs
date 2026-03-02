use crate::*;

pub async fn handle(cmd: ProjectCommands) {
    match cmd {
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
                let ssh_key_path = resolve_ssh_key(None, Some(proj));
                if verbose {
                    println!("Project: {} -> {}", proj.name, resolved_repo);
                    println!("  Branch: {}", base);
                }
                if let Some(path) = ssh_key_path.as_ref() {
                    if !path.exists() {
                        eprintln!(
                            "  {} {} - failed: ssh key not found at {}",
                            BULLET_RED,
                            proj.name,
                            path.display()
                        );
                        std::process::exit(1);
                    }
                }
                println!("\n  {} {} - ready", BULLET_GREEN, proj.name);
                if verbose {
                    println!("  ---");
                    println!("    repo: {}", resolved_repo);
                    println!("    base branch: {}", base);
                    if let Some(path) = ssh_key_path.as_ref() {
                        println!("    ssh key: {}", path.display());
                    } else {
                        println!("    ssh key: default");
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
    }
}
