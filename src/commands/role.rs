use crate::*;

pub async fn handle(cmd: RoleCommands) {
    match cmd {
        RoleCommands::List { verbose } => {
            let roles = list_role_files().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            if roles.is_empty() {
                let dir = opencode_roles_dir().unwrap_or_else(|_| PathBuf::from("(unknown)"));
                println!("No roles found in {}", dir.display());
                return;
            }

            println!("Roles:");
            for (name, path) in roles {
                let content = fs::read_to_string(&path).unwrap_or_default();
                let mode_marker = if role_content_has_subagent_mode(&content) {
                    "mode=subagent"
                } else {
                    "mode=missing"
                };
                let role_kind = if is_core_role(&name) {
                    "core"
                } else {
                    "custom"
                };
                println!(
                    "  - {} [{}] {} ({})",
                    name,
                    role_kind,
                    path.display(),
                    mode_marker
                );

                if verbose {
                    println!("    instructions:");
                    for line in content.lines() {
                        println!("      {}", line);
                    }
                    if content.is_empty() {
                        println!("      (empty)");
                    }
                }
            }
        }
        RoleCommands::Add { name, from } => {
            let normalized = validate_role_name(&name).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            if is_core_role(&normalized) {
                eprintln!(
                    "Error: '{}' is a reserved core role name and cannot be added",
                    normalized
                );
                std::process::exit(1);
            }

            let existing = list_role_files().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            if existing.iter().any(|(n, _)| n == &normalized) {
                eprintln!("Error: role '{}' already exists", normalized);
                std::process::exit(1);
            }

            let content = fs::read_to_string(&from).unwrap_or_else(|e| {
                eprintln!("Error: failed reading '{}': {}", from.display(), e);
                std::process::exit(1);
            });
            if !role_content_has_subagent_mode(&content) {
                eprintln!(
                    "Error: role file '{}' must include 'mode: subagent'",
                    from.display()
                );
                std::process::exit(1);
            }

            let roles_dir = opencode_roles_dir().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            fs::create_dir_all(&roles_dir).unwrap_or_else(|e| {
                eprintln!(
                    "Error: failed creating roles directory '{}': {}",
                    roles_dir.display(),
                    e
                );
                std::process::exit(1);
            });

            let target = role_file_path(&normalized).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            fs::write(&target, content).unwrap_or_else(|e| {
                eprintln!("Error: failed writing '{}': {}", target.display(), e);
                std::process::exit(1);
            });

            println!("Added role '{}' at {}", normalized, target.display());
        }
        RoleCommands::Update { name, from } => {
            let normalized = validate_role_name(&name).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let target = role_file_path(&normalized).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            if !target.exists() {
                eprintln!("Error: role '{}' not found", normalized);
                std::process::exit(1);
            }

            let content = fs::read_to_string(&from).unwrap_or_else(|e| {
                eprintln!("Error: failed reading '{}': {}", from.display(), e);
                std::process::exit(1);
            });
            if !role_content_has_subagent_mode(&content) {
                eprintln!(
                    "Error: role file '{}' must include 'mode: subagent'",
                    from.display()
                );
                std::process::exit(1);
            }

            fs::write(&target, content).unwrap_or_else(|e| {
                eprintln!("Error: failed writing '{}': {}", target.display(), e);
                std::process::exit(1);
            });

            println!("Updated role '{}'", normalized);
        }
        RoleCommands::Remove { name, force } => {
            let normalized = validate_role_name(&name).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            if is_core_role(&normalized) && !force {
                eprintln!(
                    "Error: '{}' is a core role and cannot be removed without --force",
                    normalized
                );
                std::process::exit(1);
            }

            let target = role_file_path(&normalized).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            if !target.exists() {
                eprintln!("Error: role '{}' not found", normalized);
                std::process::exit(1);
            }

            fs::remove_file(&target).unwrap_or_else(|e| {
                eprintln!("Error: failed removing '{}': {}", target.display(), e);
                std::process::exit(1);
            });
            println!("Removed role '{}'", normalized);
        }
        RoleCommands::Sync { from, force } => {
            let source_dir = from.unwrap_or_else(|| PathBuf::from("roles"));
            let source_roles = list_role_files_in_dir(&source_dir).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let target_dir = opencode_roles_dir().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            fs::create_dir_all(&target_dir).unwrap_or_else(|e| {
                eprintln!(
                    "Error: failed creating roles directory '{}': {}",
                    target_dir.display(),
                    e
                );
                std::process::exit(1);
            });

            let mut copied = 0usize;
            let mut skipped = 0usize;
            let mut overwritten = 0usize;

            for (role_name, source_path) in source_roles {
                let content = fs::read_to_string(&source_path).unwrap_or_else(|e| {
                    eprintln!("Error: failed reading '{}': {}", source_path.display(), e);
                    std::process::exit(1);
                });
                if !role_content_has_subagent_mode(&content) {
                    eprintln!(
                        "Error: role file '{}' must include 'mode: subagent'",
                        source_path.display()
                    );
                    std::process::exit(1);
                }

                let target_path = target_dir.join(format!("{}.md", role_name));
                if target_path.exists() {
                    let existing = fs::read_to_string(&target_path).unwrap_or_default();
                    if existing == content {
                        skipped += 1;
                        continue;
                    }
                    if !force {
                        skipped += 1;
                        continue;
                    }
                    overwritten += 1;
                } else {
                    copied += 1;
                }

                fs::write(&target_path, content).unwrap_or_else(|e| {
                    eprintln!("Error: failed writing '{}': {}", target_path.display(), e);
                    std::process::exit(1);
                });
            }

            println!(
                "Synced roles from {} -> {}",
                source_dir.display(),
                target_dir.display()
            );
            println!(
                "  copied: {} | overwritten: {} | skipped: {}",
                copied, overwritten, skipped
            );
            if skipped > 0 && !force {
                println!("  Note: use --force to overwrite changed existing role files");
            }
        }
    }
}
