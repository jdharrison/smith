use crate::*;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::Develop {
            project,
            branch,
            base,
            plan,
            max_validate_passes,
            verbose,
            task,
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

            if max_validate_passes == 0 {
                eprintln!("Error: --max-validate-passes must be >= 1");
                std::process::exit(1);
            }

            let project_config =
                resolve_project_config(Some(project.clone())).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
            let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());
            let (commit_name, commit_email) = resolve_commit_author(project_config.as_ref());
            let pipeline_roles = resolve_pipeline_roles(project_config.as_ref(), "dev");

            if let Err(e) = docker::ensure_spawn_state_dir(&project, &branch) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let plan_dirs = match docker::list_spawn_plan_dirs(&project, &branch) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            if plan_dirs.is_empty() {
                eprintln!(
                    "Error: no plan runs found in /state for {}:{}; run `smith run plan` first",
                    project, branch
                );
                std::process::exit(1);
            }

            let selected_plan = match resolve_plan_id_filter(&plan, &plan_dirs) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let plan_dir = format!("/state/{}", selected_plan);
            let manifest_path = format!("{}/manifest.json", plan_dir);
            let manifest_raw = match docker::read_spawn_file(&project, &branch, &manifest_path) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "Error: failed to load plan manifest '{}': {}",
                        selected_plan, e
                    );
                    std::process::exit(1);
                }
            };
            let manifest = match serde_json::from_str::<PlanManifest>(&manifest_raw) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "Error: invalid plan manifest for '{}': {}",
                        selected_plan, e
                    );
                    std::process::exit(1);
                }
            };

            if manifest.state != "completed" {
                eprintln!(
                    "Error: plan '{}' is in state '{}'; expected completed",
                    selected_plan, manifest.state
                );
                std::process::exit(1);
            }
            if manifest.project != project || manifest.branch != branch {
                eprintln!(
                    "Error: plan target mismatch; plan is {}:{}, requested {}:{}",
                    manifest.project, manifest.branch, project, branch
                );
                std::process::exit(1);
            }

            let unresolved = unresolved_plan_issues(&manifest);
            if !unresolved.is_empty() {
                eprintln!(
                        "Error: plan '{}' has unresolved issues; reply to all blockers before develop:\n{}",
                        selected_plan,
                        unresolved.join("\n")
                    );
                std::process::exit(1);
            }

            let expected = [
                format!("{}/{}", plan_dir, manifest.artifacts.producer),
                format!("{}/{}", plan_dir, manifest.artifacts.architect),
                format!("{}/{}", plan_dir, manifest.artifacts.designer),
                format!("{}/{}", plan_dir, manifest.artifacts.planner),
            ];
            for path in &expected {
                match docker::spawn_file_exists(&project, &branch, path) {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("Error: required plan artifact missing: {}", path);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error: failed validating plan artifact '{}': {}", path, e);
                        std::process::exit(1);
                    }
                }
            }

            let planner_path = format!("{}/{}", plan_dir, manifest.artifacts.planner);
            let planner_raw = match docker::read_spawn_file(&project, &branch, &planner_path) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "Error: failed reading planner artifact '{}': {}",
                        planner_path, e
                    );
                    std::process::exit(1);
                }
            };
            if let Err(e) = planner_has_actionable_sections(&planner_raw) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let dev_run_id = format!("dev-{}-{}", now_unix(), generate_short_plan_id(0));
            let dev_run_dir = format!("/state/{}", dev_run_id);
            if let Err(e) = docker::ensure_spawn_dir(&project, &branch, &dev_run_dir) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let execution_brief_path = format!("{}/execution-brief.json", dev_run_dir);
            let execution_brief = serde_json::json!({
                "schema_version": 1,
                "task": task.clone(),
                "plan_id": selected_plan.clone(),
                "short_plan_id": effective_short_plan_id(&manifest, &selected_plan),
                "project": project.clone(),
                "branch": branch.clone(),
                "base": resolved_base.clone(),
                "plan_prompt": manifest.prompt.clone(),
                "plan_summary": manifest.summary.clone(),
                "plan_issues": manifest.issues.clone(),
                "plan_replies": manifest.replies.clone(),
                "artifacts": {
                    "producer": format!("{}/{}", plan_dir, manifest.artifacts.producer),
                    "architect": format!("{}/{}", plan_dir, manifest.artifacts.architect),
                    "designer": format!("{}/{}", plan_dir, manifest.artifacts.designer),
                    "planner": planner_path,
                },
            });
            let execution_brief_body = match serde_json::to_string_pretty(&execution_brief) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: failed to serialize execution brief: {}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = docker::write_spawn_file(
                &project,
                &branch,
                &execution_brief_path,
                &execution_brief_body,
            ) {
                eprintln!("Error: failed writing execution brief: {}", e);
                std::process::exit(1);
            }

            let mut dev_manifest = DevRunManifest::new(
                dev_run_id.clone(),
                project.clone(),
                branch.clone(),
                resolved_base.clone(),
                selected_plan.clone(),
                effective_short_plan_id(&manifest, &selected_plan),
                task.clone(),
                max_validate_passes,
            );
            if let Err(e) = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest) {
                eprintln!("Error: failed writing develop manifest: {}", e);
                std::process::exit(1);
            }

            let branch_escaped = branch.replace('\'', "'\"'\"'");
            let base_escaped = resolved_base.replace('\'', "'\"'\"'");
            let setup_script = format!(
                    "cd /workspace && git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {{ echo 'Not a git repo at /workspace'; exit 1; }} && git fetch origin 2>&1 && if git show-ref --verify --quiet 'refs/remotes/origin/{branch}'; then git checkout -B '{branch}' 'refs/remotes/origin/{branch}' 2>&1; else git show-ref --verify --quiet 'refs/remotes/origin/{base}' || {{ echo 'Missing remote base branch origin/{base}'; exit 1; }}; git checkout -B '{branch}' 'refs/remotes/origin/{base}' 2>&1; fi && git reset --hard HEAD 2>&1 && git clean -fd 2>&1 && test -z \"$(git status --porcelain)\" || {{ echo 'Workspace is not clean after setup'; exit 1; }}",
                    branch = branch_escaped,
                    base = base_escaped
                );
            dev_manifest.set_phase("setup");
            let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
            if let Err(e) = docker::run_spawn_shell(&project, &branch, &setup_script) {
                dev_manifest.errors.push(e.clone());
                dev_manifest.set_state("failed", "setup");
                let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            if verbose {
                println!(
                    "  {} Running spawn develop for {}:{} using plan {} (id: {})",
                    BULLET_BLUE, project, branch, selected_plan, dev_manifest.short_plan_id
                );
            }

            let mut latest_report: Option<DevAssuranceReport> = None;
            for attempt in 1..=max_validate_passes {
                let develop_artifact_path = format!("{}/develop-{}.json", dev_run_dir, attempt);
                let assurance_artifact_path = format!("{}/assurance-{}.json", dev_run_dir, attempt);

                dev_manifest.set_phase(&format!("develop-{}", attempt));
                let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                let develop_prompt = build_spawn_develop_prompt(
                    &task,
                    &plan_dir,
                    &execution_brief_path,
                    &develop_artifact_path,
                    attempt,
                );
                if let Err(e) = docker::run_prompt_in_spawned_container_with_options(
                    &project,
                    &branch,
                    &develop_prompt,
                    verbose,
                    pipeline_roles
                        .execute_run
                        .as_ref()
                        .and_then(|r| r.model.as_deref()),
                    pipeline_roles
                        .execute_run
                        .as_ref()
                        .and_then(|r| r.prompt.as_deref()),
                ) {
                    dev_manifest.errors.push(e.clone());
                    dev_manifest.set_state("failed", &format!("develop-{}", attempt));
                    let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }

                if !docker::spawn_file_exists(&project, &branch, &develop_artifact_path)
                    .unwrap_or(false)
                {
                    let msg = format!(
                        "Developer pass {} did not produce required artifact {}",
                        attempt, develop_artifact_path
                    );
                    dev_manifest.errors.push(msg.clone());
                    dev_manifest.set_state("failed", &format!("develop-{}", attempt));
                    let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                    eprintln!("Error: {}", msg);
                    std::process::exit(1);
                }

                dev_manifest.set_phase(&format!("validate-{}", attempt));
                let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                let assurance_prompt = build_spawn_assurance_prompt(
                    &task,
                    &plan_dir,
                    &execution_brief_path,
                    &develop_artifact_path,
                    &assurance_artifact_path,
                    attempt,
                );
                if let Err(e) = docker::run_prompt_in_spawned_container_with_options(
                    &project,
                    &branch,
                    &assurance_prompt,
                    verbose,
                    pipeline_roles
                        .validate_run
                        .as_ref()
                        .and_then(|r| r.model.as_deref()),
                    pipeline_roles
                        .validate_run
                        .as_ref()
                        .and_then(|r| r.prompt.as_deref()),
                ) {
                    dev_manifest.errors.push(e.clone());
                    dev_manifest.set_state("failed", &format!("validate-{}", attempt));
                    let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }

                let assurance_raw =
                    match docker::read_spawn_file(&project, &branch, &assurance_artifact_path) {
                        Ok(v) => v,
                        Err(e) => {
                            dev_manifest.errors.push(e.clone());
                            dev_manifest.set_state("failed", &format!("validate-{}", attempt));
                            let _ =
                                write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                            eprintln!(
                                "Error: assurance artifact missing for pass {}: {}",
                                attempt, e
                            );
                            std::process::exit(1);
                        }
                    };

                let report = match parse_dev_assurance_report(&assurance_raw) {
                    Ok(r) => r,
                    Err(e) => {
                        dev_manifest.errors.push(e.clone());
                        dev_manifest.set_state("failed", &format!("validate-{}", attempt));
                        let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                if verbose {
                    println!(
                        "  {} validate pass {} verdict={} blocking={} non_blocking={}",
                        BULLET_BLUE,
                        attempt,
                        report.verdict,
                        report.blocking_issues.len(),
                        report.non_blocking_issues.len()
                    );
                }

                dev_manifest.attempts.push(DevAttemptRecord {
                    attempt,
                    develop_artifact: develop_artifact_path,
                    assurance_artifact: assurance_artifact_path,
                    verdict: report.verdict.clone(),
                    blocking_issues: report.blocking_issues.len(),
                    non_blocking_issues: report.non_blocking_issues.len(),
                });
                dev_manifest.non_blocking_issues = report.non_blocking_issues.clone();
                dev_manifest.final_verdict = Some(report.verdict.clone());
                let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);

                let blocking = !report.blocking_issues.is_empty() || report.verdict == "fail";
                latest_report = Some(report);
                if !blocking {
                    break;
                }
                if attempt == max_validate_passes {
                    dev_manifest.errors.push(
                        "Validation failed: blocking issues remain after max passes".to_string(),
                    );
                    dev_manifest.set_state("failed", "validate");
                    let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                    eprintln!(
                        "Error: blocking issues remain after {} validate passes",
                        max_validate_passes
                    );
                    println!("  State Dir: {}", dev_run_dir);
                    std::process::exit(1);
                }
            }

            dev_manifest.set_phase("commit");
            let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);

            let commit_msg = format!(
                "{} [plan:{}]",
                task.replace('\n', " "),
                dev_manifest.short_plan_id
            )
            .replace('\'', "'\"'\"'");
            let git_name_cmd = match commit_name.as_deref() {
                Some(name) if !name.trim().is_empty() => format!(
                    "git config user.name '{}' && ",
                    name.replace('\'', "'\"'\"'")
                ),
                _ => "git config user.name 'Smith' && ".to_string(),
            };
            let git_email_cmd = match commit_email.as_deref() {
                Some(email) if !email.trim().is_empty() => format!(
                    "git config user.email '{}' && ",
                    email.replace('\'', "'\"'\"'")
                ),
                _ => "git config user.email 'smith@localhost' && ".to_string(),
            };
            let commit_script = format!(
                    "cd /workspace && test -n \"$(git status --porcelain)\" || {{ echo 'SMITH_NO_CHANGES'; exit 3; }} && {git_name}{git_email}git add -A && git commit -m '{msg}' 2>&1 && git fetch origin 2>&1 && if git show-ref --verify --quiet 'refs/remotes/origin/{branch}'; then git rebase 'refs/remotes/origin/{branch}' 2>&1 || {{ echo 'Rebase failed'; exit 1; }}; fi && git push origin 'HEAD:refs/heads/{branch}' 2>&1 && git rev-parse HEAD",
                    git_name = git_name_cmd,
                    git_email = git_email_cmd,
                    msg = commit_msg,
                    branch = branch_escaped
                );

            let commit_output = match docker::run_spawn_shell(&project, &branch, &commit_script) {
                Ok(v) => v,
                Err(e) => {
                    if e.contains("SMITH_NO_CHANGES") {
                        dev_manifest.set_state("failed", "commit");
                        dev_manifest
                            .errors
                            .push("No changes to commit after validation".to_string());
                        let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                        println!("\n⚠ No changes were made by the development task");
                        println!("  State Dir: {}", dev_run_dir);
                        std::process::exit(1);
                    }
                    dev_manifest.errors.push(e.clone());
                    dev_manifest.set_state("failed", "commit");
                    let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            let commit_hash = commit_output
                .lines()
                .rev()
                .map(str::trim)
                .find(|line| line.len() >= 7 && line.chars().all(|c| c.is_ascii_hexdigit()))
                .unwrap_or("")
                .to_string();

            if commit_hash.is_empty() {
                dev_manifest
                    .errors
                    .push("Commit succeeded but hash could not be parsed".to_string());
                dev_manifest.set_state("failed", "commit");
                let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);
                eprintln!("Error: unable to determine commit hash from commit output");
                std::process::exit(1);
            }

            dev_manifest.final_commit = Some(commit_hash.clone());
            dev_manifest.set_state("completed", "done");
            let _ = write_dev_manifest(&project, &branch, &dev_run_dir, &dev_manifest);

            if let Some(report) = latest_report {
                if !report.non_blocking_issues.is_empty() {
                    println!(
                        "  {} Non-blocking assurance issues: {}",
                        BULLET_YELLOW,
                        report.non_blocking_issues.len()
                    );
                }
            }

            println!("  {} Spawn develop completed", BULLET_GREEN);
            println!("  Commit: {}", commit_hash);
            println!(
                "  Plan: {} (id: {})",
                selected_plan, dev_manifest.short_plan_id
            );
            println!("  State Dir: {}", dev_run_dir);
        }
        _ => unreachable!("non-develop command routed to develop handler"),
    }
}
