use crate::*;
use std::collections::HashSet;
use std::io::IsTerminal;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::Plan {
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

            if let Err(e) = docker::ensure_spawn_state_dir(&project, &branch) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let existing_plan_dirs =
                docker::list_spawn_plan_dirs(&project, &branch).unwrap_or_default();
            let existing_set: HashSet<String> = existing_plan_dirs.into_iter().collect();
            let mut run_id = String::new();
            for attempt in 0..256 {
                let short = generate_short_plan_id(attempt);
                let candidate = format!("plan-{}", short);
                if !existing_set.contains(&candidate) {
                    run_id = candidate;
                    break;
                }
            }
            if run_id.is_empty() {
                eprintln!("Error: failed to allocate unique short plan id");
                std::process::exit(1);
            }
            let run_dir = format!("/state/{}", run_id);

            if let Err(e) = docker::ensure_spawn_dir(&project, &branch, &run_dir) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            println!(
                "  {} Planning artifacts will be written to {}",
                BULLET_BLUE, run_dir
            );

            let mut manifest = PlanManifest::new(
                run_id.clone(),
                project.clone(),
                branch.clone(),
                prompt.clone(),
            );
            if let Err(e) = write_plan_manifest(&project, &branch, &run_dir, &manifest) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            manifest.set_state("in_progress", "planner");
            if let Err(e) = write_plan_manifest(&project, &branch, &run_dir, &manifest) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            let plan_started = Instant::now();
            let mut tracker_handle = None;
            let mut tracker_stop = None;
            if !verbose && io::stdout().is_terminal() {
                let stop = Arc::new(AtomicBool::new(false));
                tracker_handle = Some(spawn_plan_progress_tracker(
                    project.clone(),
                    branch.clone(),
                    run_dir.clone(),
                    stop.clone(),
                ));
                tracker_stop = Some(stop);
            }

            let plan_prompt = build_spawn_plan_prompt(&prompt, &run_dir);
            match docker::run_prompt_in_spawned_container(&project, &branch, &plan_prompt, verbose)
            {
                Ok(()) => {
                    if let Some(stop) = tracker_stop.take() {
                        stop.store(true, Ordering::SeqCst);
                    }
                    if let Some(handle) = tracker_handle.take() {
                        let _ = handle.join();
                    }
                    if !verbose {
                        print!("\r\x1b[2K");
                        let _ = io::stdout().flush();
                    }

                    let expected = [
                        (
                            "producer",
                            format!("{}/{}", run_dir, manifest.artifacts.producer.as_str()),
                        ),
                        (
                            "architect",
                            format!("{}/{}", run_dir, manifest.artifacts.architect.as_str()),
                        ),
                        (
                            "designer",
                            format!("{}/{}", run_dir, manifest.artifacts.designer.as_str()),
                        ),
                        (
                            "planner",
                            format!("{}/{}", run_dir, manifest.artifacts.planner.as_str()),
                        ),
                    ];

                    let mut missing = Vec::new();
                    for (role, path) in expected {
                        match docker::spawn_file_exists(&project, &branch, &path) {
                            Ok(true) => {
                                manifest
                                    .role_status
                                    .insert(role.to_string(), "ok".to_string());
                            }
                            Ok(false) => {
                                manifest
                                    .role_status
                                    .insert(role.to_string(), "failed".to_string());
                                missing.push(path);
                            }
                            Err(e) => {
                                manifest
                                    .role_status
                                    .insert(role.to_string(), "failed".to_string());
                                missing.push(path.clone());
                                manifest.errors.push(e);
                            }
                        }
                    }

                    if missing.is_empty() {
                        let planner_path =
                            format!("{}/{}", run_dir, manifest.artifacts.planner.as_str());
                        match docker::read_spawn_file(&project, &branch, &planner_path) {
                            Ok(raw) => {
                                let summary = extract_high_level_summary_from_planner(&raw);
                                if !summary.is_empty() {
                                    manifest.summary = summary;
                                }
                                manifest.issues = extract_plan_issues_from_planner(&raw);
                            }
                            Err(e) => {
                                manifest.errors.push(format!(
                                    "Unable to read planner summary from '{}': {}",
                                    planner_path, e
                                ));
                            }
                        }

                        manifest.set_state("completed", "finalize");
                        if let Err(e) = write_plan_manifest(&project, &branch, &run_dir, &manifest)
                        {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                        println!(
                            "  {} Plan run completed in {:.1}s",
                            BULLET_GREEN,
                            plan_started.elapsed().as_secs_f32()
                        );
                        print_plan_block(&run_id, &manifest, None);
                        println!("  State Dir: {}", run_dir);
                    } else {
                        manifest.errors.push(format!(
                            "Missing required artifacts: {}",
                            missing.join(", ")
                        ));
                        manifest.set_state("failed", "finalize");
                        let _ = write_plan_manifest(&project, &branch, &run_dir, &manifest);
                        eprintln!(
                            "Error: missing required artifacts in {}:\n{}",
                            run_dir,
                            missing.join("\n")
                        );
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    if let Some(stop) = tracker_stop.take() {
                        stop.store(true, Ordering::SeqCst);
                    }
                    if let Some(handle) = tracker_handle.take() {
                        let _ = handle.join();
                    }
                    if !verbose {
                        print!("\r\x1b[2K");
                        let _ = io::stdout().flush();
                    }

                    manifest.set_state("failed", "planner");
                    manifest.errors.push(e.clone());
                    let _ = write_plan_manifest(&project, &branch, &run_dir, &manifest);
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ => unreachable!("non-plan command routed to plan handler"),
    }
}
