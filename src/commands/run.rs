use crate::*;

pub async fn handle(cmd: RunCommands) {
    let mut post_pr: Option<(String, String, String, String)> = None;
    let mut post_release_pr: Option<(Option<String>, String, String, String)> = None;

    match &cmd {
        RunCommands::Develop {
            project,
            branch,
            base,
            pr,
            task,
            ..
        } => {
            if *pr {
                let detected_project = if project.is_none() {
                    match detect_project_from_cwd() {
                        Ok(Some(name)) => Some(name),
                        _ => None,
                    }
                } else {
                    project.clone()
                };

                let project_config = resolve_project_config(detected_project.clone())
                    .unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                let resolved_repo =
                    resolve_repo(None, detected_project.clone()).unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                let resolved_branch = branch.clone().unwrap_or_else(|| {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!(
                                "Error: --pr requested but no branch provided and auto-detection failed"
                            );
                            std::process::exit(1);
                        }
                    }
                });

                let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());
                post_pr = Some((resolved_repo, resolved_branch, resolved_base, task.clone()));
            }
        }
        RunCommands::Release {
            project,
            branch,
            base,
            pr,
            ..
        } => {
            if *pr {
                let detected_project = if project.is_none() {
                    match detect_project_from_cwd() {
                        Ok(Some(name)) => Some(name),
                        _ => None,
                    }
                } else {
                    project.clone()
                };

                let project_config = resolve_project_config(detected_project.clone())
                    .unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                let resolved_repo =
                    resolve_repo(None, detected_project.clone()).unwrap_or_else(|e| {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });

                let resolved_branch = branch.clone().unwrap_or_else(|| {
                    let output = Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).trim().to_string()
                        }
                        _ => {
                            eprintln!(
                                "Error: --pr requested but no branch provided and auto-detection failed"
                            );
                            std::process::exit(1);
                        }
                    }
                });

                let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());
                post_release_pr = Some((
                    detected_project,
                    resolved_repo,
                    resolved_branch,
                    resolved_base,
                ));
            }
        }
        _ => {}
    }

    commands::pipeline::handle(cmd).await;

    if let Some((resolved_repo, branch_out, base_branch, task_pr)) = post_pr {
        let project = detect_project_from_cwd().ok().flatten();
        let project_config = resolve_project_config(project).unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        });
        let token = project_config
            .as_ref()
            .and_then(|p| p.github_token.as_deref());

        if let Some(token) = token {
            if let Ok(repo_info) = github::extract_repo_info(&resolved_repo) {
                match github::create_or_update_pr(
                    token,
                    &repo_info.owner,
                    &repo_info.name,
                    &branch_out,
                    &base_branch,
                    &task_pr,
                )
                .await
                {
                    Ok(pr_url) => println!("  {} Pull request: {}", BULLET_GREEN, pr_url),
                    Err(e) => {
                        eprintln!("  {} Failed to create/update PR: {}", BULLET_YELLOW, e);
                        if e.contains("403") || e.contains("Resource not accessible") {
                            eprintln!("     Your token may be missing required permissions.");
                        }
                    }
                }
            } else {
                eprintln!(
                    "  {} Could not extract repository info from URL: {}",
                    BULLET_YELLOW, resolved_repo
                );
            }
        } else {
            eprintln!(
                "  {} GitHub token not configured for this project; skipping PR creation",
                BULLET_YELLOW
            );
        }
    }

    if let Some((project_for_token, resolved_repo, branch_out, base_branch)) = post_release_pr {
        let project_config = resolve_project_config(project_for_token).unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        });
        let token = project_config
            .as_ref()
            .and_then(|p| p.github_token.as_deref());

        if let Some(token) = token {
            if let Ok(repo_info) = github::extract_repo_info(&resolved_repo) {
                match github::close_pr_for_branch(
                    token,
                    &repo_info.owner,
                    &repo_info.name,
                    &branch_out,
                    &base_branch,
                    "Integrated via smith run release",
                )
                .await
                {
                    Ok(Some(pr_url)) => {
                        println!("  {} Closed pull request: {}", BULLET_GREEN, pr_url)
                    }
                    Ok(None) => {
                        println!(
                            "  {} No open pull request found for branch '{}'",
                            BULLET_BLUE, branch_out
                        );
                    }
                    Err(e) => {
                        eprintln!("  {} Failed to close pull request: {}", BULLET_YELLOW, e);
                    }
                }
            } else {
                eprintln!(
                    "  {} Could not extract repository info from URL: {}",
                    BULLET_YELLOW, resolved_repo
                );
            }
        } else {
            eprintln!(
                "  {} GitHub token not configured for this project; skipping PR close",
                BULLET_YELLOW
            );
        }
    }
}
