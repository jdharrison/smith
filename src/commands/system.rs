use crate::*;

/// Validate local environment (config, Docker, install marker). Always available.
pub async fn handle_doctor(verbose: bool) {
    println!("{} smith doctor", BULLET_GREEN);
    println!();

    let mut ok = true;

    match config_dir() {
        Ok(dir) => {
            println!("  {} config - {}", BULLET_GREEN, dir.display());
            if verbose {
                let config_path = dir.join("config.toml");
                println!(
                    "       config.toml: {}",
                    if config_path.exists() {
                        "present"
                    } else {
                        "missing"
                    }
                );
            }
        }
        Err(e) => {
            ok = false;
            println!("  {} config - {}", BULLET_RED, e);
        }
    }

    match docker::check_docker_available() {
        Ok(()) => {
            println!("  {} docker - available", BULLET_GREEN);
            if verbose {
                if let Ok(o) = Command::new("docker").arg("--version").output() {
                    let v = String::from_utf8_lossy(&o.stdout);
                    let v = v.trim();
                    if !v.is_empty() {
                        println!("       {}", v);
                    }
                }
            }
        }
        Err(e) => {
            ok = false;
            println!("  {} docker - {}", BULLET_RED, e);
        }
    }

    if is_installed() {
        match installed_version() {
            Some(v) if !v.is_empty() => {
                println!("  {} install - {}", BULLET_GREEN, v);
            }
            _ => println!("  {} install - marked (legacy)", BULLET_GREEN),
        }
    } else {
        println!(
            "  {} install - not installed (run `smith install`)",
            BULLET_YELLOW
        );
    }

    match service::collect_status() {
        Ok(snap) => {
            println!("  {} services", BULLET_GREEN);
            println!(
                "       configured autostart policy: {}",
                if snap.autostart { "on" } else { "off" }
            );
            crate::commands::service::print_supervisor_status(&snap.supervisor, "       ");
            println!("       compose project: {}", docker::COMPOSE_PROJECT);
            for row in &snap.services {
                let b = if row.healthy {
                    BULLET_GREEN
                } else if row.running {
                    BULLET_YELLOW
                } else {
                    BULLET_BLUE
                };
                let url = row.url.as_deref().unwrap_or("");
                println!(
                    "       {} {} [{}] {}{}",
                    b,
                    row.id,
                    row.mode,
                    if row.running { "up" } else { "down" },
                    if url.is_empty() {
                        String::new()
                    } else {
                        format!(" {url}")
                    }
                );
            }
        }
        Err(e) => println!("  {} services - {}", BULLET_YELLOW, e),
    }

    println!();
    if ok {
        println!("  {} environment OK", BULLET_GREEN);
    } else {
        println!("  {} environment has issues", BULLET_RED);
        std::process::exit(1);
    }
}

pub async fn handle_status(verbose: bool) {
    let docker_ok = docker::check_docker_available().is_ok();
    let installed = is_installed();

    let cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    let running = docker::list_running_agent_containers().unwrap_or_default();
    let list = cfg.agents.as_deref().unwrap_or(&[]);

    let smith_bullet = if installed && docker_ok {
        BULLET_GREEN
    } else if installed {
        BULLET_BLUE
    } else {
        // Not installed yet — ready to install, not a hard error.
        BULLET_YELLOW
    };
    println!("{} smith", smith_bullet);
    println!(
        "     runtime: {}{}",
        cfg.runtime().as_str(),
        match cfg.runtime() {
            RuntimeMode::Host => " (native process; you contain this machine)",
            RuntimeMode::Container => " (process in Docker — not the same as network policy)",
        }
    );
    println!(
        "     network: {}{}",
        cfg.network().as_str(),
        match cfg.network() {
            NetworkMode::Private => " (brain offline; SearXNG for live facts)",
            NetworkMode::Public => " (brain may reach the internet)",
        }
    );
    if cfg.runtime().is_host() {
        println!("     chat: native  |  coding/pipeline/search: Docker");
    } else {
        println!("     chat: Docker  |  coding/pipeline/search: Docker");
    }
    println!("     supervisor:");
    crate::commands::service::print_supervisor_status(
        &service::os_register::observed_status(),
        "       ",
    );
    let d_bullet = if docker_ok { BULLET_GREEN } else { BULLET_RED };
    println!(
        "     {} docker - {}",
        d_bullet,
        if docker_ok {
            "available"
        } else {
            "unavailable"
        }
    );
    if docker_ok {
        if let Ok(st) = docker::sandbox_status() {
            let c_b = if st.coding_running {
                BULLET_GREEN
            } else if st.coding_configured {
                BULLET_YELLOW
            } else {
                BULLET_BLUE
            };
            println!(
                "     {} coding agent - {}",
                c_b,
                if st.coding_running {
                    "running"
                } else if st.coding_configured {
                    "ready (start on chat / agent start coding)"
                } else {
                    "not configured"
                }
            );
            let (sx_run, sx_ok) = docker::searxng_status();
            let sx_b = if sx_ok {
                BULLET_GREEN
            } else if sx_run {
                BULLET_YELLOW
            } else {
                BULLET_BLUE
            };
            println!(
                "     {} searxng - {}",
                sx_b,
                if sx_ok {
                    format!("ready http://127.0.0.1:{}", docker::SEARXNG_PORT)
                } else if sx_run {
                    "starting…".to_string()
                } else {
                    "not running (starts on search / install)".to_string()
                }
            );
            if st.sandbox_running {
                println!(
                    "     {} sandbox - {}",
                    BULLET_GREEN,
                    st.published_port
                        .map(|p| format!("http://127.0.0.1:{p}"))
                        .unwrap_or_else(|| "running".to_string())
                );
            }
        }
    }
    if list.is_empty() {
        println!("  {} agents", BULLET_BLUE);
        println!("       (none)");
    } else {
        // Aggregate: green if any online/cloud; yellow if any degraded; else blue (ready).
        let mut has_online = false;
        let mut has_degraded = false;
        for agent_entry in list.iter() {
            let active = running.contains(&agent_entry.name);
            let is_cloud = agent_entry
                .agent_type
                .as_deref()
                .map(|t| t != "local")
                .unwrap_or(true);
            if is_cloud || active {
                has_online = true;
            }
            if active {
                let i = list
                    .iter()
                    .position(|a| a.name == agent_entry.name)
                    .unwrap_or(0);
                let port = agent_port(agent_entry, i);
                if !docker::check_agent_reachable(port) {
                    has_degraded = true;
                }
            }
        }
        let agents_bullet = if has_degraded {
            BULLET_YELLOW
        } else if has_online {
            BULLET_GREEN
        } else {
            BULLET_BLUE
        };
        println!("  {} agents", agents_bullet);
        for (i, agent_entry) in list.iter().enumerate() {
            let name = &agent_entry.name;
            let active = running.contains(name);
            let built = docker::image_exists(&docker::agent_built_image_tag(name)).unwrap_or(false);
            let is_cloud = agent_entry
                .agent_type
                .as_deref()
                .map(|t| t != "local")
                .unwrap_or(true);
            let port = agent_port(agent_entry, i);
            let reachable = if active {
                Some(docker::check_agent_reachable(port))
            } else {
                None
            };
            let bullet = if is_cloud {
                BULLET_GREEN
            } else if active && reachable == Some(false) {
                BULLET_YELLOW
            } else if active {
                BULLET_GREEN
            } else {
                // not built / built idle — ready, not an error
                BULLET_BLUE
            };
            let state = if is_cloud {
                "cloud"
            } else if active && reachable == Some(false) {
                "running (port unreachable)"
            } else if active {
                "running"
            } else if built {
                "built"
            } else {
                "not built"
            };
            if is_cloud {
                println!("       {} {} - {}", bullet, name, state);
            } else if active {
                println!(
                    "       {} {} - {} {}",
                    bullet,
                    name,
                    state,
                    clickable_agent_url(port)
                );
            } else {
                println!(
                    "       {} {} - {} (http://localhost:{})",
                    bullet, name, state, port
                );
            }
        }
    }

    // projects:
    let project_results: Vec<(String, bool, String)> = cfg
        .projects
        .iter()
        .map(|proj| {
            if !project_repo_is_remote(&proj.repo) {
                let host = expand_user_path(&proj.repo);
                if host.exists() {
                    (proj.name.clone(), true, format!("local {}", host.display()))
                } else {
                    (
                        proj.name.clone(),
                        false,
                        format!("missing {}", host.display()),
                    )
                }
            } else if proj.repo.starts_with("https://") {
                (
                    proj.name.clone(),
                    false,
                    "unsupported repo URL (use SSH or a local path)".to_string(),
                )
            } else {
                (proj.name.clone(), true, "git configured".to_string())
            }
        })
        .collect();
    let project_ok_count = project_results.iter().filter(|(_, ok, _)| *ok).count();
    let project_total = project_results.len();
    let projects_bullet = if project_total == 0 {
        BULLET_BLUE
    } else if project_ok_count == project_total {
        BULLET_GREEN
    } else if project_ok_count > 0 {
        BULLET_YELLOW
    } else {
        BULLET_RED
    };
    println!("  {} projects", projects_bullet);
    if project_results.is_empty() {
        if cfg.projects.is_empty() {
            println!("       (none)");
        }
    } else {
        for (name, ok, msg) in &project_results {
            let bullet = if *ok { BULLET_GREEN } else { BULLET_RED };
            println!("       {} {} - {}", bullet, name, msg);
        }
    }

    // workspaces:
    let workspace_results: Vec<(String, bool, String)> = cfg
        .workspaces
        .iter()
        .map(|ws| {
            let host = expand_user_path(&ws.path);
            match parse_workspace_access(ws.access.as_deref()) {
                Err(e) => (ws.name.clone(), false, e),
                Ok(read_only) => {
                    let access = if read_only { "read" } else { "readwrite" };
                    if host.is_dir() {
                        (
                            ws.name.clone(),
                            true,
                            format!("{} ({})", host.display(), access),
                        )
                    } else {
                        (
                            ws.name.clone(),
                            false,
                            format!("missing {}", host.display()),
                        )
                    }
                }
            }
        })
        .collect();
    let workspace_ok = workspace_results.iter().filter(|(_, ok, _)| *ok).count();
    let workspace_total = workspace_results.len();
    let workspaces_bullet = if workspace_total == 0 {
        BULLET_BLUE
    } else if workspace_ok == workspace_total {
        BULLET_GREEN
    } else if workspace_ok > 0 {
        BULLET_YELLOW
    } else {
        BULLET_RED
    };
    println!("  {} workspaces", workspaces_bullet);
    if workspace_results.is_empty() {
        println!("       (none)");
    } else {
        for (name, ok, msg) in &workspace_results {
            let bullet = if *ok { BULLET_GREEN } else { BULLET_RED };
            println!("       {} {} - {}", bullet, name, msg);
        }
    }

    if verbose {
        println!();
        println!("  --- verbose ---");
        if let Ok(dir) = config_dir() {
            let config_path = dir.join("config.toml");
            println!("  config: {}", config_path.display());
        }
        if let Some(v) = installed_version() {
            println!(
                "  installed_version: {}",
                if v.is_empty() {
                    "(legacy, unknown)"
                } else {
                    &v
                }
            );
        } else {
            println!("  installed_version: (not installed)");
        }
        if docker_ok {
            if let Ok(o) = Command::new("docker").arg("--version").output() {
                let out = String::from_utf8_lossy(&o.stdout);
                let v = out.trim();
                if !v.is_empty() {
                    println!("  docker: {}", v);
                }
            }
            if let Ok(o) = Command::new("docker").arg("info").output() {
                let out = String::from_utf8_lossy(&o.stdout);
                for line in out.lines().take(15) {
                    println!("    {}", line);
                }
                if out.lines().count() > 15 {
                    println!("    ...");
                }
            }
        }
        println!("  running agent containers: {:?}", running);
        let agents_list: Vec<&AgentEntry> = list.iter().collect();
        if agents_list.is_empty() {
            println!(
                "  agent config: [{} (default)] {}",
                DEFAULT_AGENT_NAME,
                clickable_agent_url(docker::OPENCODE_SERVER_PORT)
            );
        } else {
            for (i, e) in agents_list.iter().enumerate() {
                let port = agent_port(e, i);
                println!(
                    "  agent config: {} -> image={} port={} {}",
                    e.name,
                    e.image,
                    port,
                    clickable_agent_url(port)
                );
            }
        }
    }
}

/// Install summary: coding worker vs optional/legacy named OpenCode profiles.
fn print_install_workers_summary(agents: &[AgentEntry]) {
    if agents.is_empty() {
        println!("  Workers: (none configured yet)");
        return;
    }
    println!("  Workers (named OpenCode profiles):");
    for a in agents {
        let kind = a.agent_type.as_deref().unwrap_or("cloud");
        let (label, note) = match a.name.as_str() {
            n if n == docker::CODING_AGENT_NAME => (
                format!("{} [agents.coding]", a.name),
                "demos / code intents".to_string(),
            ),
            "search" => (
                a.name.clone(),
                "legacy name — web search is services.searxng".to_string(),
            ),
            "local" => (
                a.name.clone(),
                "legacy local profile — prefer coding".to_string(),
            ),
            _ => (a.name.clone(), format!("{kind} worker")),
        };
        println!("    - {label}  ({kind})  {note}");
    }
    println!(
        "  {} Warm/on-demand policy: smith settings service status",
        BULLET_GREEN
    );
}

pub async fn handle_install() {
    println!("{} smith install", BULLET_GREEN);
    println!();
    let already_installed = is_installed();
    if already_installed {
        println!("  Re-run: skipping prompts for sections that are already initialized.");
        println!();
    }

    // --- Dependencies ---
    println!("  Dependencies:");
    try_install_docker();
    println!();

    // --- Docker always run (Linux) ---
    #[cfg(target_os = "linux")]
    {
        if docker::check_docker_available().is_ok() {
            if docker_enabled_at_boot() {
                println!("  {} Docker - already enabled at boot", BULLET_GREEN);
            } else if !already_installed {
                println!("  Always run Docker at boot so workers stay available after restart?");
                println!("  (Requires sudo / password to run systemctl enable docker)");
                if prompt_yn("Enable Docker at boot?", true) {
                    ensure_docker_started_and_enabled();
                    println!("  {} Docker - enabled at boot", BULLET_GREEN);
                }
            } else {
                println!(
                    "  {} Docker - not enabled at boot (use systemctl enable docker)",
                    BULLET_BLUE
                );
            }
            println!();
        }
    }

    // --- Workers (coding + optional named OpenCode profiles) ---
    let mut cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    print_install_workers_summary(cfg.agents.as_deref().unwrap_or(&[]));
    if cfg.agents.as_deref().unwrap_or(&[]).is_empty() && !already_installed {
        println!("  Coding worker (agents.coding) is offered in the sealed-brain step.");
        println!("  Extra named OpenCode workers (optional): smith settings agent add");
    }
    println!();

    let has_coding = cfg
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .any(|a| a.name == docker::CODING_AGENT_NAME);
    // First install only: optional extras when coding already exists in config.
    if !already_installed && has_coding {
        match prompt_yn_skip(
            "Add extra named OpenCode workers? (optional — search is services.searxng)",
        ) {
            None => println!("  Skipping extra workers."),
            Some(false) => {}
            Some(true) => loop {
                let name = prompt_line("  Worker name: ");
                if name.is_empty() {
                    println!("  Name cannot be empty.");
                    continue;
                }
                if name == "search" {
                    println!("  Tip: web search uses services.searxng, not an OpenCode worker.");
                }
                let image_default = DEFAULT_AGENT_IMAGE.to_string();
                let image_in = prompt_line(&format!("  Image [{}]: ", image_default));
                let image = if image_in.is_empty() {
                    None
                } else {
                    Some(image_in)
                };
                let model_in =
                    prompt_line("  Model (e.g. anthropic/claude-sonnet-4-5, Enter to skip): ");
                let model = if model_in.is_empty() {
                    None
                } else {
                    Some(model_in)
                };
                let small_model_in =
                    prompt_line("  Small model for internal ops (optional, Enter to skip): ");
                let small_model = if small_model_in.is_empty() {
                    None
                } else {
                    Some(small_model_in)
                };
                let provider_in = prompt_line(
                    "  Provider (e.g. ollama, anthropic, openai, Enter for cloud default): ",
                );
                let provider = if provider_in.is_empty() {
                    None
                } else {
                    Some(provider_in)
                };
                let base_url_in =
                    prompt_line("  Base URL for provider (optional, Enter to skip): ");
                let base_url = if base_url_in.is_empty() {
                    None
                } else {
                    Some(base_url_in)
                };
                let type_in = prompt_line("  Type (local or cloud, Enter for cloud): ");
                let agent_type = if type_in.is_empty() {
                    None
                } else {
                    Some(type_in)
                };
                let port_in = prompt_line("  Port for opencode serve (Enter for default 4096): ");
                let port = if port_in.is_empty() {
                    None
                } else {
                    port_in.parse().ok()
                };
                let enabled = Some(true);
                match add_agent_to_config(
                    &mut cfg,
                    name.clone(),
                    image,
                    agent_type,
                    model,
                    small_model,
                    provider,
                    base_url,
                    port,
                    enabled,
                    None,
                    None,
                    None,
                    None,
                    None,
                ) {
                    Ok(()) => println!("  {} Added worker '{}'", BULLET_GREEN, name),
                    Err(e) => eprintln!("  {} {}", BULLET_RED, e),
                }
                save_config(&cfg).unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                });
                if !prompt_yn("Add another worker?", false) {
                    break;
                }
            },
        }
        println!();
    }

    // --- Projects ---
    if ensure_default_smith_project_in(&mut cfg) {
        if let Err(e) = save_config(&cfg) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        println!(
            "  {} default project '{}' → {} (base {})",
            BULLET_GREEN,
            DEFAULT_SMITH_PROJECT_NAME,
            DEFAULT_SMITH_PROJECT_REPO,
            DEFAULT_SMITH_PROJECT_BASE_BRANCH
        );
    }
    if cfg.projects.is_empty() {
        println!("  Current projects: (none)");
    } else {
        println!("  Current projects:");
        for p in &cfg.projects {
            println!("    - {} ({})", p.name, p.repo);
        }
        println!("  {} projects - configured", BULLET_GREEN);
    }
    println!();
    // Always seed default smith; only prompt for *additional* projects on first install.
    if already_installed {
        // skip prompts
    } else {
        match prompt_yn_skip("Add more projects?") {
            None => println!("  Skipping additional projects."),
            Some(false) => {}
            Some(true) => loop {
                let name = prompt_line("  Project name: ");
                if name.is_empty() {
                    println!("  Project name cannot be empty.");
                    continue;
                }
                let repo = prompt_line("  Repository (URL or path): ");
                if repo.is_empty() {
                    println!("  Repository is required.");
                    continue;
                }
                let image_in = prompt_line("  Image (optional, Enter to skip): ");
                let image = if image_in.is_empty() {
                    None
                } else {
                    Some(image_in)
                };
                let ssh_key_in = prompt_line("  SSH key path (optional): ");
                let ssh_key = if ssh_key_in.is_empty() {
                    None
                } else {
                    Some(ssh_key_in)
                };
                let base_in = prompt_line("  Base branch [main]: ");
                let base_branch = if base_in.is_empty() {
                    None
                } else {
                    Some(base_in)
                };
                let remote_in = prompt_line("  Remote name [origin]: ");
                let remote = if remote_in.is_empty() {
                    None
                } else {
                    Some(remote_in)
                };
                let token_in = prompt_line("  GitHub token for PRs (optional): ");
                let github_token = if token_in.is_empty() {
                    None
                } else {
                    Some(token_in)
                };
                let script_in = prompt_line("  Script to run in container (optional, e.g. curl -fsSL https://opencode.ai/install | sh): ");
                let script = if script_in.is_empty() {
                    None
                } else {
                    Some(script_in)
                };
                let model_in = prompt_line(&format!(
                    "  Model profile for this project [{}]: ",
                    cfg.current_agent.as_deref().unwrap_or(DEFAULT_AGENT_NAME)
                ));
                let model = if model_in.is_empty() {
                    cfg.current_agent
                        .clone()
                        .or_else(|| Some(DEFAULT_AGENT_NAME.to_string()))
                } else {
                    Some(model_in)
                };
                let project = ProjectConfig {
                    name: name.clone(),
                    repo,
                    image,
                    ssh_key,
                    base_branch,
                    remote,
                    github_token,
                    script,
                    commit_name: None,
                    commit_email: None,
                    model,
                    ..Default::default()
                };
                match add_project_to_config(&mut cfg, project) {
                    Ok(()) => {
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("  {} Added project '{}'", BULLET_GREEN, name);
                    }
                    Err(e) => eprintln!("  {} {}", BULLET_RED, e),
                }
                if !prompt_yn("Add another project?", false) {
                    break;
                }
            },
        }
        println!();
    }

    // --- Workspaces ---
    if cfg.workspaces.is_empty() {
        println!("  Current workspaces: (none)");
    } else {
        println!("  Current workspaces:");
        for w in &cfg.workspaces {
            println!("    - {} ({})", w.name, w.path);
        }
        println!("  {} workspaces - already configured", BULLET_GREEN);
    }
    println!();
    if already_installed || !cfg.workspaces.is_empty() {
        // skip
    } else {
        match prompt_yn_skip("Add any host workspaces (existing directories to mount into agents)?")
        {
            None => println!("  Skipping workspaces."),
            Some(false) => {}
            Some(true) => loop {
                let name = prompt_line("  Workspace name: ");
                if name.is_empty() {
                    println!("  Workspace name cannot be empty.");
                    continue;
                }
                let path_in = prompt_line("  Host directory: ");
                if path_in.is_empty() {
                    println!("  Path is required.");
                    continue;
                }
                let host = expand_user_path(&path_in);
                if !host.is_dir() {
                    println!("  Path must be an existing directory: {}", host.display());
                    continue;
                }
                let access_in = prompt_line("  Access [readwrite] (read|write|readwrite): ");
                let access = if access_in.is_empty() {
                    None
                } else {
                    match parse_workspace_access(Some(&access_in)) {
                        Ok(_) => Some(access_in),
                        Err(e) => {
                            println!("  {}", e);
                            continue;
                        }
                    }
                };
                let workspace = WorkspaceConfig {
                    name: name.clone(),
                    path: store_host_path(&host.canonicalize().unwrap_or(host)),
                    access,
                };
                match add_workspace_to_config(&mut cfg, workspace) {
                    Ok(()) => {
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("  {} Added workspace '{}'", BULLET_GREEN, name);
                    }
                    Err(e) => eprintln!("  {} {}", BULLET_RED, e),
                }
                if !prompt_yn("Add another workspace?", false) {
                    break;
                }
            },
        }
        println!();
    }

    // --- Whisper STT ---
    crate::commands::whisper::ensure_whisper_install();
    println!();

    // --- Kokoro TTS ---
    crate::commands::tts::ensure_tts_install();
    println!();

    // --- Sealed local brain ---
    bootstrap_sealed_brain(&mut cfg, already_installed);

    println!();
    // --- Runtime + network ---
    if already_installed || cfg.runtime().is_host() {
        println!(
            "  {} runtime = {}, network = {}",
            BULLET_GREEN,
            cfg.runtime().as_str(),
            cfg.network().as_str()
        );
    } else {
        println!("  Runtime (where the brain process runs):");
        println!("    container — smith jails the brain in Docker (default).");
        println!(
            "    host      — native control plane; you contain this machine. Workers stay in Docker."
        );
        match prompt_yn_skip("Switch to host runtime? (default: keep container)") {
            Some(true) => {
                cfg.set_runtime_network(RuntimeMode::Host, NetworkMode::Public);
                if let Err(e) = save_config(&cfg) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("  {} runtime = host, network = public", BULLET_YELLOW);
                println!("     {}", HOST_RUNTIME_WARNING);
            }
            _ => {
                println!(
                    "  {} runtime = container, network = {}",
                    BULLET_GREEN,
                    cfg.network().as_str()
                );
            }
        }
    }

    println!();
    if let Err(e) = run_install_finish() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    println!(
        "  {} Supervisor unit unchanged; install and manage it explicitly when ready.",
        BULLET_BLUE
    );

    println!("  {} Install complete", BULLET_GREEN);
    println!("     Runtime:       smith settings runtime get|set host|container");
    println!("     Network:       smith settings network get|set public|private");
    println!(
        "     Service unit:  smith settings service install|status|start|stop|restart|uninstall"
    );
    println!("     Autostart:     smith settings service autostart on|off");
    println!("     Sealed brain:  smith chat | smith think \"…\" | smith settings brain status");
    println!("     Coding:        smith settings sandbox status|stop | agent start coding");
    println!("     Ubuntu client: cd client && flutter run -d linux (separate application)");
    println!("     SearXNG:       http://127.0.0.1:4792");
    println!("     Projects:      smith settings project add <name> --repo <url-or-path>");
    println!("     Workspaces:    smith settings workspace add <name> --path <dir>");
    println!("     Workers:       smith settings agent add|start <name> (coding = agents.coding)");
}

/// Bootstrap sealed brain: memory, local model profile, network, Ollama, brain, validate.
fn bootstrap_sealed_brain(cfg: &mut SmithConfig, already_installed: bool) {
    println!("  Sealed local brain:");

    // 1. Memory layout
    match brain::ensure_memory_layout() {
        Ok(path) => println!("  {} memory - {}", BULLET_GREEN, path.display()),
        Err(e) => {
            eprintln!("  {} memory - {}", BULLET_RED, e);
            return;
        }
    }

    if docker::check_docker_available().is_err() {
        eprintln!(
            "  {} docker unavailable — skipping brain network/containers",
            BULLET_YELLOW
        );
        println!("     Re-run `smith install` after Docker is available.");
        return;
    }

    // 2. Ensure coding agent exists as the default local (Ollama) worker profile
    let has_local = cfg
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .any(|a| a.agent_type.as_deref() == Some("local"));

    if !has_local {
        let create = if already_installed {
            true
        } else {
            prompt_yn(
                &format!(
                    "Ensure coding worker agents.coding (ollama / {}) for demos?",
                    DEFAULT_LOCAL_MODEL
                ),
                false,
            )
        };
        if create {
            match docker::ensure_coding_config() {
                Ok(()) => {
                    // Reload — ensure_coding_config saves its own copy.
                    if let Ok(updated) = load_config() {
                        *cfg = updated;
                    }
                    if cfg.current_agent.is_none() {
                        cfg.current_agent = Some(docker::CODING_AGENT_NAME.to_string());
                        if let Err(e) = save_config(cfg) {
                            eprintln!("  {} failed to save current agent: {}", BULLET_RED, e);
                        }
                    }
                    println!(
                        "  {} coding worker ({}) ready — service agents.coding",
                        BULLET_GREEN,
                        docker::CODING_AGENT_NAME
                    );
                }
                Err(e) => eprintln!("  {} coding agent profile: {}", BULLET_RED, e),
            }
        } else {
            println!(
                "  {} no coding worker — add later with `smith settings agent add coding --type local --model {}`",
                BULLET_YELLOW, DEFAULT_LOCAL_MODEL
            );
        }
    } else {
        println!("  {} local Ollama worker profile present", BULLET_GREEN);
        // Prefer coding as current when an old 'local' profile still exists.
        if cfg.current_agent.as_deref() == Some("local")
            && cfg
                .agents
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .any(|a| a.name == docker::CODING_AGENT_NAME)
        {
            cfg.current_agent = Some(docker::CODING_AGENT_NAME.to_string());
            let _ = save_config(cfg);
        }
    }

    // 3. Internal network
    match docker::ensure_brain_network() {
        Ok(()) => println!(
            "  {} network {} (internal, no egress)",
            BULLET_GREEN,
            docker::BRAIN_NETWORK
        ),
        Err(e) => {
            eprintln!("  {} network - {}", BULLET_RED, e);
            return;
        }
    }

    // 4. Resolve local model name
    let local_model = cfg
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find(|a| a.agent_type.as_deref() == Some("local"))
        .and_then(|a| a.model.clone())
        .unwrap_or_else(|| DEFAULT_LOCAL_MODEL.to_string());

    // 5. Start Ollama (heal without prompt)
    if docker::is_ollama_running() {
        println!(
            "  {} ollama - already running ({:?})",
            BULLET_GREEN,
            docker::active_ollama_backend().unwrap_or(docker::OllamaBackend::Docker)
        );
    } else {
        println!("  Starting Ollama with model '{}'...", local_model);
        match docker::start_ollama(&local_model) {
            Ok((_, backend)) => println!("  {} ollama - started ({:?})", BULLET_GREEN, backend),
            Err(e) => {
                eprintln!("  {} ollama - {}", BULLET_RED, e);
                return;
            }
        }
    }

    // 6. Ensure brain image present
    match docker::image_exists(docker::BRAIN_IMAGE) {
        Ok(true) => println!("  {} image {}", BULLET_GREEN, docker::BRAIN_IMAGE),
        Ok(false) => {
            println!("  Pulling {} ...", docker::BRAIN_IMAGE);
            let pull = Command::new("docker")
                .args(["pull", docker::BRAIN_IMAGE])
                .status();
            match pull {
                Ok(s) if s.success() => {
                    println!("  {} image pulled", BULLET_GREEN);
                }
                _ => {
                    eprintln!(
                        "  {} failed to pull {} — chat may fail until available",
                        BULLET_YELLOW,
                        docker::BRAIN_IMAGE
                    );
                }
            }
        }
        Err(e) => eprintln!("  {} image check: {}", BULLET_YELLOW, e),
    }

    // 7. Start sealed brain
    let mem = match brain::ensure_memory_layout() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  {} {}", BULLET_RED, e);
            return;
        }
    };
    match docker::ensure_brain_ready(&mem, Some(&local_model)) {
        Ok(()) => println!(
            "  {} container {} - running",
            BULLET_GREEN,
            docker::BRAIN_CONTAINER_NAME
        ),
        Err(e) => {
            eprintln!("  {} brain start - {}", BULLET_RED, e);
            return;
        }
    }

    // 8. Validate
    let st = docker::brain_status(&mem);
    let ok_net = st.network_exists && st.ollama_on_network && st.brain_running && st.ollama_running;
    if ok_net {
        println!(
            "  {} connectivity - brain ↔ ollama on internal net",
            BULLET_GREEN
        );
    } else {
        eprintln!(
            "  {} connectivity incomplete (brain={} ollama={} ollama_net={})",
            BULLET_YELLOW, st.brain_running, st.ollama_running, st.ollama_on_network
        );
    }

    match docker::verify_no_egress() {
        Ok(true) => {
            if cfg.network().is_private() {
                println!("  {} egress - blocked (network=private)", BULLET_GREEN);
            } else {
                println!(
                    "  {} egress - blocked (unexpected for network=public)",
                    BULLET_YELLOW
                );
            }
        }
        Ok(false) => {
            if !cfg.network().is_private() {
                println!("  {} egress - open (network=public)", BULLET_GREEN);
            } else if docker::active_ollama_backend() == Some(docker::OllamaBackend::Host) {
                eprintln!(
                    "  {} egress - soft-seal (private intended; host Ollama path may leak)",
                    BULLET_YELLOW
                );
            } else {
                eprintln!(
                    "  {} egress - UNEXPECTED reachability (network=private)",
                    BULLET_RED
                );
            }
        }
        Err(e) => eprintln!("  {} egress check - {}", BULLET_YELLOW, e),
    }

    // 9. Always-on coding agent + scratch workspace
    match docker::ensure_coding_config() {
        Ok(()) => println!(
            "  {} coding config - scratch + agent 'coding'",
            BULLET_GREEN
        ),
        Err(e) => eprintln!("  {} coding config - {}", BULLET_YELLOW, e),
    }
    match docker::ensure_coding_agent_ready() {
        Ok(()) => println!(
            "  {} coding agent - running ({})",
            BULLET_GREEN,
            docker::CODING_AGENT_NAME
        ),
        Err(e) => eprintln!("  {} coding agent - {}", BULLET_YELLOW, e),
    }

    // 9b. Default public smith project (self-iteration bootstrap)
    match ensure_default_smith_project() {
        Ok(true) => println!(
            "  {} project '{}' → {} @{}",
            BULLET_GREEN,
            DEFAULT_SMITH_PROJECT_NAME,
            DEFAULT_SMITH_PROJECT_REPO,
            DEFAULT_SMITH_PROJECT_BASE_BRANCH
        ),
        Ok(false) => println!(
            "  {} project '{}' already registered",
            BULLET_GREEN, DEFAULT_SMITH_PROJECT_NAME
        ),
        Err(e) => eprintln!("  {} default smith project - {}", BULLET_YELLOW, e),
    }

    // 10. Local SearXNG (metasearch UI + API on loopback)
    match docker::ensure_searxng() {
        Ok(()) => println!(
            "  {} searxng - http://127.0.0.1:{} (browser + smith search)",
            BULLET_GREEN,
            docker::SEARXNG_PORT
        ),
        Err(e) => eprintln!("  {} searxng - {}", BULLET_YELLOW, e),
    }

    println!("  {} sealed brain ready — try `smith chat`", BULLET_GREEN);
}

pub async fn handle_uninstall(
    force: bool,
    remove_config: bool,
    remove_project_data: bool,
    remove_images: bool,
) {
    let prompt =
        "This will stop all agent containers and Ollama. Continue? Type 'yes' to confirm: ";
    if !confirm_reset(prompt, force) {
        eprintln!("Uninstall cancelled.");
        std::process::exit(1);
    }

    println!("{} smith uninstall", BULLET_YELLOW);
    println!();

    println!("  Stopping managed supervisor user unit...");
    if let Err(e) = service::os_register::stop() {
        eprintln!("    Warning: {}", e);
    } else {
        println!("    stop manager action completed");
    }

    println!("  Removing supervisor user-unit registration...");
    if let Err(e) = service::uninstall_supervisor() {
        eprintln!("    Warning: {}", e);
    } else {
        println!("    login autostart disabled and user unit removed");
    }

    if let Err(e) = docker::check_docker_available() {
        eprintln!(
            "Warning: Docker not available - skipping container cleanup: {}",
            e
        );
    } else {
        println!("  Stopping sealed brain...");
        match docker::remove_brain_container() {
            Ok(()) => println!("    {}: removed", docker::BRAIN_CONTAINER_NAME),
            Err(e) => eprintln!("    Warning: {}", e),
        }

        println!("  Stopping agent containers...");
        match docker::stop_all_agent_containers() {
            Ok(stopped) => {
                if stopped.is_empty() {
                    println!("    (no running agent containers)");
                } else {
                    for name in &stopped {
                        println!("    {}: stopped", name);
                    }
                    println!("    Stopped {} container(s).", stopped.len());
                }
            }
            Err(e) => eprintln!("    Warning: {}", e),
        }

        if docker::is_docker_ollama_running() || docker::is_host_ollama_api_up() {
            println!("  Stopping Ollama...");
            if let Err(e) = docker::stop_ollama() {
                eprintln!("    Warning: {}", e);
            }
        }

        println!("  Stopping SearXNG...");
        if let Err(e) = docker::stop_searxng() {
            eprintln!("    Warning: {}", e);
        } else {
            println!("    {}: removed", docker::SEARXNG_CONTAINER_NAME);
        }

        // Drop internal brain network if present
        let _ = Command::new("docker")
            .args(["network", "rm", docker::BRAIN_NETWORK])
            .output();

        if remove_images {
            println!("  Removing Docker images...");
            let _ = Command::new("docker")
                .args(["rmi", "-f", "smith/unnamed:latest"])
                .output();
            let cfg = load_config().unwrap_or_default();
            if let Some(agents) = cfg.agents {
                for agent in &agents {
                    let tag = docker::agent_built_image_tag(&agent.name);
                    let _ = Command::new("docker").args(["rmi", "-f", &tag]).output();
                    println!("    {}: removed", tag);
                }
            }
            if docker::image_exists("ollama/ollama").unwrap_or(false) {
                let _ = Command::new("docker")
                    .args(["rmi", "-f", "ollama/ollama"])
                    .output();
                println!("    ollama/ollama: removed");
            }
            if docker::image_exists(docker::BRAIN_IMAGE).unwrap_or(false) {
                let _ = Command::new("docker")
                    .args(["rmi", "-f", docker::BRAIN_IMAGE])
                    .output();
                println!("    {}: removed", docker::BRAIN_IMAGE);
            }
        }
    }

    let remove_config = if remove_config {
        true
    } else {
        let prompt =
            "Remove all config and profile data (~/.config/smith)? Type 'yes' to confirm: ";
        confirm_reset(prompt, false)
    };

    let remove_project_data = if remove_project_data {
        true
    } else {
        let prompt = "Remove cloned project checkouts (smith project cache)? This does not delete host workspace directories. Type 'yes' to confirm: ";
        confirm_reset(prompt, false)
    };

    if remove_project_data {
        println!("  Removing cloned project data...");
        match project_cache_dir() {
            Ok(dir) => {
                if dir.exists() {
                    if let Err(e) = fs::remove_dir_all(&dir) {
                        eprintln!("    Failed to remove project cache: {}", e);
                    } else {
                        println!("    {}: removed", dir.display());
                    }
                } else {
                    println!("    (no project cache)");
                }
            }
            Err(e) => eprintln!("    Warning: {}", e),
        }
    }

    if remove_config {
        println!("  Removing config directory...");
        match config_dir() {
            Ok(dir) => {
                if dir.exists() {
                    if let Err(e) = fs::remove_dir_all(&dir) {
                        eprintln!("    Failed to remove config: {}", e);
                    } else {
                        println!("    {}: removed", dir.display());
                    }
                } else {
                    println!("    (config directory does not exist)");
                }
            }
            Err(e) => eprintln!("    Warning: {}", e),
        }
    }

    println!();
    println!("  {} Uninstalled successfully", BULLET_GREEN);
    if !remove_config {
        println!("     (config preserved - run with --remove-config to delete)");
    }
    if !remove_project_data {
        println!("     (project cache preserved - run with --remove-project-data to delete)");
    }
    if !remove_images {
        println!("     (images preserved - run with --remove-images to delete)");
    }

    let prompt = "Remove the smith binary? Type 'yes' to run 'cargo uninstall smith': ";
    if confirm_reset(prompt, force) {
        println!("  Running cargo uninstall smith...");
        let status = Command::new("cargo").args(["uninstall", "smith"]).status();
        match status {
            Ok(s) if s.success() => {
                println!("    smith binary removed");
            }
            _ => {
                eprintln!("    Warning: cargo uninstall failed");
                println!("    To remove manually, run: which smith");
            }
        }
    }
}
