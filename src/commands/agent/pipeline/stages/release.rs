use crate::*;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::Release {
            project,
            branch,
            base,
            plan,
            verbose,
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

            let project_config =
                resolve_project_config(Some(project.clone())).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
            let resolved_base = resolve_base_branch(base.as_deref(), project_config.as_ref());

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
            let plan_manifest_path = format!("{}/manifest.json", plan_dir);
            let plan_manifest_raw =
                match docker::read_spawn_file(&project, &branch, &plan_manifest_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: failed to read plan manifest: {}", e);
                        std::process::exit(1);
                    }
                };
            let mut plan_manifest = match serde_json::from_str::<PlanManifest>(&plan_manifest_raw) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: invalid plan manifest '{}': {}", selected_plan, e);
                    std::process::exit(1);
                }
            };

            if plan_manifest.project != project || plan_manifest.branch != branch {
                eprintln!(
                    "Error: plan target mismatch; plan is {}:{}, requested {}:{}",
                    plan_manifest.project, plan_manifest.branch, project, branch
                );
                std::process::exit(1);
            }
            if plan_manifest.state != "completed" && plan_manifest.state != "released" {
                eprintln!(
                    "Error: plan '{}' is in state '{}'; expected completed or released",
                    selected_plan, plan_manifest.state
                );
                std::process::exit(1);
            }

            let (dev_run_id, dev_manifest) =
                match find_latest_completed_dev_run_for_plan(&project, &branch, &selected_plan) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
            let dev_run_dir = format!("/state/{}", dev_run_id);

            let latest_attempt = match dev_manifest.attempts.last() {
                Some(v) => v,
                None => {
                    eprintln!(
                        "Error: develop run '{}' has no recorded attempts",
                        dev_manifest.dev_run_id
                    );
                    std::process::exit(1);
                }
            };

            let develop_artifact_path = latest_attempt.develop_artifact.clone();
            let assurance_artifact_path = latest_attempt.assurance_artifact.clone();
            for path in [&develop_artifact_path, &assurance_artifact_path] {
                match docker::spawn_file_exists(&project, &branch, path) {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("Error: required develop artifact missing: {}", path);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error: failed checking artifact '{}': {}", path, e);
                        std::process::exit(1);
                    }
                }
            }

            let assurance_raw =
                match docker::read_spawn_file(&project, &branch, &assurance_artifact_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!(
                            "Error: failed reading assurance artifact '{}': {}",
                            assurance_artifact_path, e
                        );
                        std::process::exit(1);
                    }
                };
            if let Err(e) = parse_dev_assurance_report(&assurance_raw) {
                eprintln!("Error: assurance artifact is invalid: {}", e);
                std::process::exit(1);
            }

            let release_run_id = format!("release-{}-{}", now_unix(), generate_short_plan_id(0));
            let release_run_dir = format!("/state/{}", release_run_id);
            if let Err(e) = docker::ensure_spawn_dir(&project, &branch, &release_run_dir) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let review_artifact_path = format!("{}/review.json", release_run_dir);
            let integrate_artifact_path = format!("{}/integrate.json", release_run_dir);
            let sync_artifact_path = format!("{}/sync.json", release_run_dir);

            let mut release_manifest = ReleaseRunManifest::new(
                release_run_id.clone(),
                project.clone(),
                branch.clone(),
                resolved_base.clone(),
                selected_plan.clone(),
                effective_short_plan_id(&plan_manifest, &selected_plan),
                dev_run_id.clone(),
            );
            if let Err(e) =
                write_release_manifest(&project, &branch, &release_run_dir, &release_manifest)
            {
                eprintln!("Error: failed writing release manifest: {}", e);
                std::process::exit(1);
            }

            release_manifest.set_phase("review");
            let _ = write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);
            let review_prompt = build_spawn_release_review_prompt(
                &dev_manifest.task,
                &plan_dir,
                &dev_run_dir,
                &develop_artifact_path,
                &assurance_artifact_path,
                &review_artifact_path,
            );
            if let Err(e) =
                docker::run_prompt_in_spawned_container(&project, &branch, &review_prompt, verbose)
            {
                release_manifest.errors.push(e.clone());
                release_manifest.set_state("failed", "review");
                let _ =
                    write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let review_raw = match docker::read_spawn_file(&project, &branch, &review_artifact_path)
            {
                Ok(v) => v,
                Err(e) => {
                    release_manifest.errors.push(e.clone());
                    release_manifest.set_state("failed", "review");
                    let _ = write_release_manifest(
                        &project,
                        &branch,
                        &release_run_dir,
                        &release_manifest,
                    );
                    eprintln!("Error: review artifact missing: {}", e);
                    std::process::exit(1);
                }
            };
            let review_report = match parse_release_review_report(&review_raw) {
                Ok(v) => v,
                Err(e) => {
                    release_manifest.errors.push(e.clone());
                    release_manifest.set_state("failed", "review");
                    let _ = write_release_manifest(
                        &project,
                        &branch,
                        &release_run_dir,
                        &release_manifest,
                    );
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            release_manifest.review_ready = Some(review_report.release_ready);
            release_manifest.non_blocking_issues = review_report.non_blocking_issues.clone();
            let _ = write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);

            let mut integration_failed = false;
            let mut integration_blocked = false;

            if !review_report.release_ready || !review_report.blocking_issues.is_empty() {
                integration_blocked = true;
                release_manifest.integration_status = Some("blocked".to_string());
                let integrate_json = serde_json::json!({
                    "schema_version": 1,
                    "status": "blocked",
                    "reason": "release_review_blocked",
                    "strategy": Value::Null,
                    "merge_commit": Value::Null,
                    "pushed": false,
                    "generated_at_unix": now_unix(),
                });
                let integrate_body = serde_json::to_string_pretty(&integrate_json)
                    .map_err(|e| format!("Failed to serialize integrate artifact: {}", e))
                    .unwrap_or_else(|e| {
                        release_manifest.errors.push(e.clone());
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });
                if let Err(e) = docker::write_spawn_file(
                    &project,
                    &branch,
                    &integrate_artifact_path,
                    &integrate_body,
                ) {
                    release_manifest.errors.push(e.clone());
                    release_manifest.set_state("failed", "integrate");
                    let _ = write_release_manifest(
                        &project,
                        &branch,
                        &release_run_dir,
                        &release_manifest,
                    );
                    eprintln!("Error: failed writing integrate artifact: {}", e);
                    std::process::exit(1);
                }
            } else {
                release_manifest.set_phase("integrate");
                let _ =
                    write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);

                let branch_escaped = branch.replace('\'', "'\"'\"'");
                let base_escaped = resolved_base.replace('\'', "'\"'\"'");
                let merge_msg = format!(
                    "Merge '{}' into '{}' [plan:{}]",
                    branch, resolved_base, release_manifest.short_plan_id
                )
                .replace('\'', "'\"'\"'");
                let integrate_script = format!(
                    r#"cd /workspace && status='ok' && reason='' && strategy='' && merge_commit='' && pushed='false' && git fetch origin 2>&1 || {{ status='failed'; reason='fetch_failed'; }} && if [ "$status" = 'ok' ] && ! git show-ref --verify --quiet 'refs/remotes/origin/{base}'; then status='failed'; reason='missing_base_branch'; fi && if [ "$status" = 'ok' ] && ! git show-ref --verify --quiet 'refs/remotes/origin/{branch}'; then status='failed'; reason='missing_feature_branch'; fi && if [ "$status" = 'ok' ]; then git checkout -B '{base}' 'refs/remotes/origin/{base}' 2>&1 || {{ status='failed'; reason='checkout_base_failed'; }}; fi && if [ "$status" = 'ok' ]; then git reset --hard 'refs/remotes/origin/{base}' 2>&1 || {{ status='failed'; reason='reset_base_failed'; }}; fi && if [ "$status" = 'ok' ]; then if git merge --ff-only 'refs/remotes/origin/{branch}' 2>&1; then strategy='ff_only'; merge_commit=$(git rev-parse HEAD 2>/dev/null || true); else if git merge --no-ff -m '{merge_msg}' 'refs/remotes/origin/{branch}' 2>&1; then strategy='merge_commit'; merge_commit=$(git rev-parse HEAD 2>/dev/null || true); else git merge --abort 2>/dev/null || true; status='conflict'; reason='merge_conflict'; fi; fi; fi && if [ "$status" = 'ok' ]; then if git push origin 'HEAD:refs/heads/{base}' 2>&1; then pushed='true'; else status='failed'; reason='push_failed'; fi; fi && echo "SMITH_RELEASE_STATUS=$status" && echo "SMITH_RELEASE_REASON=$reason" && echo "SMITH_RELEASE_STRATEGY=$strategy" && echo "SMITH_RELEASE_MERGE_COMMIT=$merge_commit" && echo "SMITH_RELEASE_PUSHED=$pushed""#,
                    base = base_escaped,
                    branch = branch_escaped,
                    merge_msg = merge_msg
                );

                let integrate_raw =
                    match docker::run_spawn_shell(&project, &branch, &integrate_script) {
                        Ok(v) => v,
                        Err(e) => {
                            release_manifest.errors.push(e.clone());
                            release_manifest.set_state("failed", "integrate");
                            let _ = write_release_manifest(
                                &project,
                                &branch,
                                &release_run_dir,
                                &release_manifest,
                            );
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    };

                let status = extract_kv_line(&integrate_raw, "SMITH_RELEASE_STATUS")
                    .unwrap_or("failed")
                    .trim()
                    .to_string();
                let reason = extract_kv_line(&integrate_raw, "SMITH_RELEASE_REASON")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let strategy = extract_kv_line(&integrate_raw, "SMITH_RELEASE_STRATEGY")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let merge_commit = extract_kv_line(&integrate_raw, "SMITH_RELEASE_MERGE_COMMIT")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let pushed = extract_kv_line(&integrate_raw, "SMITH_RELEASE_PUSHED")
                    .unwrap_or("false")
                    .trim()
                    == "true";

                release_manifest.integration_status = Some(status.clone());
                if !strategy.is_empty() {
                    release_manifest.merge_strategy = Some(strategy.clone());
                }
                if !merge_commit.is_empty() {
                    release_manifest.merge_commit = Some(merge_commit.clone());
                }

                let integrate_json = serde_json::json!({
                    "schema_version": 1,
                    "status": status,
                    "reason": if reason.is_empty() { Value::Null } else { Value::String(reason.clone()) },
                    "strategy": if strategy.is_empty() { Value::Null } else { Value::String(strategy.clone()) },
                    "merge_commit": if merge_commit.is_empty() { Value::Null } else { Value::String(merge_commit.clone()) },
                    "pushed": pushed,
                    "raw_output": integrate_raw,
                    "generated_at_unix": now_unix(),
                });
                let integrate_body = serde_json::to_string_pretty(&integrate_json)
                    .map_err(|e| format!("Failed to serialize integrate artifact: {}", e))
                    .unwrap_or_else(|e| {
                        release_manifest.errors.push(e.clone());
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    });
                if let Err(e) = docker::write_spawn_file(
                    &project,
                    &branch,
                    &integrate_artifact_path,
                    &integrate_body,
                ) {
                    release_manifest.errors.push(e.clone());
                    release_manifest.set_state("failed", "integrate");
                    let _ = write_release_manifest(
                        &project,
                        &branch,
                        &release_run_dir,
                        &release_manifest,
                    );
                    eprintln!("Error: failed writing integrate artifact: {}", e);
                    std::process::exit(1);
                }

                if release_manifest.integration_status.as_deref() != Some("ok") {
                    integration_failed = true;
                }
            }

            release_manifest.set_phase("sync");
            let _ = write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);
            let sync_prompt = build_spawn_release_sync_prompt(
                &plan_dir,
                &review_artifact_path,
                &integrate_artifact_path,
                &sync_artifact_path,
            );
            if let Err(e) =
                docker::run_prompt_in_spawned_container(&project, &branch, &sync_prompt, verbose)
            {
                release_manifest.errors.push(e.clone());
                release_manifest.set_state("failed", "sync");
                let _ =
                    write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            if !docker::spawn_file_exists(&project, &branch, &sync_artifact_path).unwrap_or(false) {
                let msg = "Sync phase did not produce required sync.json artifact".to_string();
                release_manifest.errors.push(msg.clone());
                release_manifest.set_state("failed", "sync");
                let _ =
                    write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }

            let final_failed = integration_failed;
            let final_blocked = integration_blocked;
            if final_blocked {
                plan_manifest.set_state("release_blocked", "release_review");
            } else if final_failed {
                plan_manifest.set_state("release_failed", "release_sync");
            } else {
                plan_manifest.set_state("released", "release_sync");
            }
            if let Err(e) = write_plan_manifest(&project, &branch, &plan_dir, &plan_manifest) {
                release_manifest.errors.push(e.clone());
                release_manifest.set_state("failed", "sync");
                let _ =
                    write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);
                eprintln!("Error: failed updating plan manifest: {}", e);
                std::process::exit(1);
            }

            if final_blocked || final_failed {
                release_manifest.set_state("failed", "done");
            } else {
                release_manifest.set_state("completed", "done");
            }
            let _ = write_release_manifest(&project, &branch, &release_run_dir, &release_manifest);

            if final_blocked {
                eprintln!(
                    "Release blocked by review findings for plan {} (id: {})",
                    selected_plan, release_manifest.short_plan_id
                );
                println!("  State Dir: {}", release_run_dir);
                std::process::exit(1);
            }
            if final_failed {
                eprintln!(
                    "Release integration failed for plan {} (id: {})",
                    selected_plan, release_manifest.short_plan_id
                );
                println!("  State Dir: {}", release_run_dir);
                std::process::exit(1);
            }

            println!("  {} Spawn release completed", BULLET_GREEN);
            if let Some(strategy) = release_manifest.merge_strategy.as_deref() {
                println!("  Merge Strategy: {}", strategy);
            }
            if let Some(commit) = release_manifest.merge_commit.as_deref() {
                println!("  Merge Commit: {}", commit);
            }
            println!(
                "  Plan: {} (id: {})",
                selected_plan, release_manifest.short_plan_id
            );
            println!("  State Dir: {}", release_run_dir);
        }
        _ => unreachable!("non-release command routed to release handler"),
    }
}
