use crate::*;

pub async fn handle(cmd: WorkspaceCommands) {
    match cmd {
        WorkspaceCommands::Add { name, path, access } => {
            if let Err(e) = parse_workspace_access(access.as_deref()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            let host = expand_user_path(&path);
            if !host.exists() {
                eprintln!("Error: path does not exist: {}", host.display());
                std::process::exit(1);
            }
            if !host.is_dir() {
                eprintln!("Error: path is not a directory: {}", host.display());
                std::process::exit(1);
            }
            let stored = store_host_path(&host.canonicalize().unwrap_or(host));
            let access = access.filter(|s| !s.trim().is_empty());
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let workspace = WorkspaceConfig {
                name: name.clone(),
                path: stored,
                access,
            };
            if let Err(e) = add_workspace_to_config(&mut cfg, workspace) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            save_config(&cfg).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!(
                "Workspace '{}' added. Agents pick it up on next `smith settings agent start`.",
                name
            );
        }
        WorkspaceCommands::List => {
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            if cfg.workspaces.is_empty() {
                println!("No workspaces registered. Add one with `smith settings workspace add <name> --path <dir>`.");
                return;
            }
            for ws in &cfg.workspaces {
                let access = ws.access.as_deref().unwrap_or("readwrite");
                println!("  {} -> {} ({})", ws.name, ws.path, access);
            }
        }
        WorkspaceCommands::Status { workspace } => {
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let selected: Vec<&WorkspaceConfig> = match workspace.as_deref() {
                Some(name) => match cfg.workspaces.iter().find(|w| w.name == name) {
                    Some(w) => vec![w],
                    None => {
                        eprintln!("Error: Workspace '{}' not found", name);
                        std::process::exit(1);
                    }
                },
                None => {
                    if cfg.workspaces.is_empty() {
                        eprintln!("No workspaces registered. Add one with `smith settings workspace add`.");
                        std::process::exit(1);
                    }
                    cfg.workspaces.iter().collect()
                }
            };
            let mut failed = false;
            for ws in selected {
                let host = expand_user_path(&ws.path);
                match parse_workspace_access(ws.access.as_deref()) {
                    Err(e) => {
                        println!("  {} {} - {}", BULLET_RED, ws.name, e);
                        failed = true;
                    }
                    Ok(read_only) => {
                        if !host.exists() {
                            println!("  {} {} - missing {}", BULLET_RED, ws.name, host.display());
                            failed = true;
                        } else if !host.is_dir() {
                            println!(
                                "  {} {} - not a directory {}",
                                BULLET_RED,
                                ws.name,
                                host.display()
                            );
                            failed = true;
                        } else {
                            let access = if read_only { "read" } else { "readwrite" };
                            println!(
                                "  {} {} - {} ({})",
                                BULLET_GREEN,
                                ws.name,
                                host.display(),
                                access
                            );
                        }
                    }
                }
            }
            if failed {
                std::process::exit(1);
            }
        }
        WorkspaceCommands::Update { name, path, access } => {
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let Some(ws) = cfg.workspaces.iter_mut().find(|w| w.name == name) else {
                eprintln!("Error: Workspace '{}' not found", name);
                std::process::exit(1);
            };
            if let Some(new_path) = path {
                let host = expand_user_path(&new_path);
                if !host.exists() || !host.is_dir() {
                    eprintln!(
                        "Error: path must be an existing directory: {}",
                        host.display()
                    );
                    std::process::exit(1);
                }
                ws.path = store_host_path(&host.canonicalize().unwrap_or(host));
            }
            if let Some(new_access) = access {
                if new_access.trim().is_empty() {
                    ws.access = None;
                } else {
                    if let Err(e) = parse_workspace_access(Some(&new_access)) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                    ws.access = Some(new_access);
                }
            }
            save_config(&cfg).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!("Workspace '{}' updated", name);
        }
        WorkspaceCommands::Remove { name } => {
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let before = cfg.workspaces.len();
            cfg.workspaces.retain(|w| w.name != name);
            if cfg.workspaces.len() == before {
                eprintln!("Error: Workspace '{}' not found", name);
                std::process::exit(1);
            }
            save_config(&cfg).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!("Workspace '{}' removed", name);
        }
    }
}
