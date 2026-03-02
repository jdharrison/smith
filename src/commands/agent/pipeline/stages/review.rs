use crate::*;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::Review {
            project,
            branch,
            limit,
            state,
            plan,
            reply,
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

            if reply.is_some() && plan.is_none() {
                eprintln!("Error: --reply requires --plan <id>");
                std::process::exit(1);
            }

            let resolved_plan = if let Some(plan_filter) = plan.as_ref() {
                match resolve_plan_id_filter(plan_filter, &plan_dirs) {
                    Ok(id) => Some(id),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            if let Some(reply_text) = reply.as_ref() {
                let selected_plan = resolved_plan.clone().expect("resolved plan must exist");
                let run_dir = format!("/state/{}", selected_plan);
                let manifest_path = format!("{}/manifest.json", run_dir);
                let raw = match docker::read_spawn_file(&project, &branch, &manifest_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: cannot load manifest for '{}': {}", selected_plan, e);
                        std::process::exit(1);
                    }
                };

                let mut manifest = match serde_json::from_str::<PlanManifest>(&raw) {
                    Ok(m) => m,
                    Err(e) => {
                        let mut inferred = PlanManifest::new(
                            selected_plan.clone(),
                            project.clone(),
                            branch.clone(),
                            "(unknown prompt)".to_string(),
                        );
                        inferred
                            .errors
                            .push(format!("Invalid manifest recovered during reply: {}", e));

                        let planner_path = format!("{}/planner.json", run_dir);
                        if let Ok(planner_raw) =
                            docker::read_spawn_file(&project, &branch, &planner_path)
                        {
                            let summary = extract_high_level_summary_from_planner(&planner_raw);
                            if !summary.is_empty() {
                                inferred.summary = summary;
                            }
                            inferred.issues = extract_plan_issues_from_planner(&planner_raw);
                        }

                        inferred
                    }
                };

                if !is_valid_short_plan_id(&manifest.short_id) {
                    manifest.short_id = short_plan_id_from_dir_name(&selected_plan);
                }

                manifest.replies.push(PlanReply {
                    submitted_at_unix: now_unix(),
                    text: reply_text.clone(),
                });
                if !manifest.issues.is_empty() {
                    for issue in &mut manifest.issues {
                        if issue.answer.is_none() {
                            issue.answer = Some(reply_text.clone());
                        }
                    }
                }
                manifest.updated_at_unix = now_unix();

                if let Err(e) = write_plan_manifest(&project, &branch, &run_dir, &manifest) {
                    eprintln!("Error: failed to save reply for '{}': {}", selected_plan, e);
                    std::process::exit(1);
                }

                println!(
                    "Saved reply for {} (id: {}).",
                    selected_plan,
                    effective_short_plan_id(&manifest, &selected_plan)
                );
            }

            plan_dirs.sort_by(|a, b| b.cmp(a));

            if let Some(selected_plan) = resolved_plan.as_ref() {
                plan_dirs.retain(|d| d == selected_plan);
            }

            let mut manifests: Vec<(String, PlanManifest, u64)> = Vec::new();
            for dir_name in plan_dirs {
                let ts_hint = plan_id_timestamp(&dir_name).unwrap_or(0);
                let manifest_path = format!("/state/{}/manifest.json", dir_name);
                match docker::read_spawn_file(&project, &branch, &manifest_path) {
                    Ok(raw) => match serde_json::from_str::<PlanManifest>(&raw) {
                        Ok(m) => {
                            let sort_key = if m.created_at_unix == 0 {
                                ts_hint
                            } else {
                                m.created_at_unix
                            };
                            manifests.push((dir_name.clone(), m, sort_key));
                        }
                        Err(parse_err) => {
                            let mut inferred = PlanManifest::new(
                                dir_name.clone(),
                                project.clone(),
                                branch.clone(),
                                "(unknown prompt)".to_string(),
                            );
                            inferred.created_at_unix = ts_hint;
                            inferred.updated_at_unix = ts_hint;
                            inferred.set_state("failed", "finalize");
                            inferred
                                .errors
                                .push(format!("Invalid manifest.json: {}", parse_err));
                            manifests.push((dir_name, inferred, ts_hint));
                        }
                    },
                    Err(_) => {
                        let mut inferred = PlanManifest::new(
                            dir_name.clone(),
                            project.clone(),
                            branch.clone(),
                            "(unknown prompt)".to_string(),
                        );
                        inferred.created_at_unix = ts_hint;
                        inferred.updated_at_unix = ts_hint;
                        inferred.set_state("not_started", "init");
                        inferred
                            .errors
                            .push("Missing manifest.json (inferred entry)".to_string());
                        manifests.push((dir_name, inferred, ts_hint));
                    }
                }
            }

            manifests.sort_by(|(_, _, a_key), (_, _, b_key)| b_key.cmp(a_key));

            let desired_state = state.as_ref().map(|s| s.to_lowercase());
            let mut shown = 0usize;
            for (dir_name, manifest, _) in manifests {
                if let Some(ref desired) = desired_state {
                    if manifest.state.to_lowercase() != *desired {
                        continue;
                    }
                }
                if let Some(max) = limit {
                    if shown >= max {
                        break;
                    }
                }

                if shown > 0 {
                    println!("\n------------------------------------------------------------\n");
                }

                let mut manifest = manifest;
                if manifest.summary.is_empty() || manifest.issues.is_empty() {
                    let run_dir = format!("/state/{}", dir_name);
                    let planner_path = format!("{}/{}", run_dir, manifest.artifacts.planner);
                    if let Ok(planner_raw) =
                        docker::read_spawn_file(&project, &branch, &planner_path)
                    {
                        if manifest.summary.is_empty() {
                            let summary = extract_high_level_summary_from_planner(&planner_raw);
                            if !summary.is_empty() {
                                manifest.summary = summary;
                            }
                        }
                        if manifest.issues.is_empty() {
                            manifest.issues = extract_plan_issues_from_planner(&planner_raw);
                        }
                    }
                }

                let assurance_path = format!("/state/{}/assurance.json", dir_name);
                let assurance_preview = docker::read_spawn_file(&project, &branch, &assurance_path)
                    .ok()
                    .and_then(|raw| extract_assurance_preview(&raw));

                print_plan_block(&dir_name, &manifest, assurance_preview.as_ref());

                shown += 1;
            }

            if shown == 0 {
                if let Some(s) = desired_state {
                    println!("No plans found matching state '{}'", s);
                } else {
                    println!("No plans found");
                }
            }
        }
        _ => unreachable!("non-review command routed to review handler"),
    }
}
