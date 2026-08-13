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
        BULLET_RED
    };
    println!("{} smith", smith_bullet);
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
    if list.is_empty() {
        println!("  {} agents", BULLET_BLUE);
        println!("       (none)");
    } else {
        // First pass: determine aggregate status
        let mut agents_bullet = BULLET_BLUE;
        let mut has_cloud_or_running = false;
        for agent_entry in list.iter() {
            let active = running.contains(&agent_entry.name);
            let built = docker::image_exists(&docker::agent_built_image_tag(&agent_entry.name))
                .unwrap_or(false);
            let is_cloud = agent_entry
                .agent_type
                .as_deref()
                .map(|t| t != "local")
                .unwrap_or(true);
            if is_cloud || active {
                has_cloud_or_running = true;
            } else if !built {
                agents_bullet = BULLET_RED;
            }
        }
        // Print header with aggregate status
        if agents_bullet == BULLET_RED {
            println!("  {} agents", BULLET_RED);
        } else if has_cloud_or_running {
            println!("  {} agents", BULLET_GREEN);
        } else {
            println!("  {} agents", BULLET_BLUE);
        }
        // Second pass: print each agent
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
            } else if built {
                BULLET_BLUE
            } else {
                BULLET_RED
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
            if proj.repo.starts_with("https://") {
                (
                    proj.name.clone(),
                    false,
                    "unsupported repo URL (use SSH)".to_string(),
                )
            } else {
                (proj.name.clone(), true, "configured".to_string())
            }
        })
        .collect();
    let project_ok_count = project_results.iter().filter(|(_, ok, _)| *ok).count();
    let project_total = project_results.len();
    let projects_bullet = if project_total == 0 {
        BULLET_RED
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

pub async fn handle_install() {
    println!("{} smith install", BULLET_GREEN);
    println!();
    // --- Dependencies ---
    println!("  Dependencies:");
    try_install_docker();
    println!();
    // --- Docker always run (Linux) ---
    #[cfg(target_os = "linux")]
    {
        if docker::check_docker_available().is_ok() {
            println!("  Always run Docker at boot so agents stay available after restart?");
            println!("  (Requires sudo / password to run systemctl enable docker)");
            if prompt_yn("Enable Docker at boot?", true) {
                ensure_docker_started_and_enabled();
                println!("  {} Docker - enabled at boot", BULLET_GREEN);
            }
            println!();
        }
    }
    // --- Config and agents ---
    let mut cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    let agents_list = cfg.agents.as_deref().unwrap_or(&[]);
    if agents_list.is_empty() {
        println!("  Current agents: (none)");
        println!();
        match prompt_yn("Create default opencode agent?", true) {
            false => {
                println!("  No agent created. Use 'smith agent add' later.");
            }
            true => {
                let new_agent = AgentEntry {
                    name: DEFAULT_AGENT_NAME.to_string(),
                    image: DEFAULT_AGENT_IMAGE.to_string(),
                    agent_type: Some("cloud".to_string()),
                    model: None,
                    small_model: None,
                    provider: None,
                    base_url: None,
                    port: None,
                    enabled: Some(true),
                    default_role: None,
                    roles: None,
                    env: None,
                    prompt: None,
                    mcp: None,
                };
                cfg.agents = Some(vec![new_agent]);
                if let Err(e) = save_config(&cfg) {
                    eprintln!("Error saving config: {}", e);
                } else {
                    println!(
                        "  {} Created default agent: {}",
                        BULLET_GREEN, DEFAULT_AGENT_NAME
                    );
                }
            }
        }
    } else {
        println!("  Current agents:");
        for a in agents_list {
            println!("    - {} (image: {})", a.name, a.image);
        }
    }
    println!();
    match prompt_yn_skip("Add more agents?") {
        None => println!("  Skipping agents."),
        Some(false) => {}
        Some(true) => loop {
            let name = prompt_line("  Agent name: ");
            if name.is_empty() {
                println!("  Agent name cannot be empty.");
                continue;
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
            let base_url_in = prompt_line("  Base URL for provider (optional, Enter to skip): ");
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
                Ok(()) => println!("  {} Added agent '{}'", BULLET_GREEN, name),
                Err(e) => eprintln!("  {} {}", BULLET_RED, e),
            }
            save_config(&cfg).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            if !prompt_yn("Add another agent?", false) {
                break;
            }
        },
    }
    println!();
    // --- Projects ---
    if cfg.projects.is_empty() {
        println!("  Current projects: (none)");
    } else {
        println!("  Current projects:");
        for p in &cfg.projects {
            println!("    - {} ({})", p.name, p.repo);
        }
    }
    println!();
    match prompt_yn_skip("Add any projects?") {
        None => println!("  Skipping projects."),
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
    // --- Sealed local brain ---
    bootstrap_sealed_brain(&mut cfg);

    println!();
    if let Err(e) = run_install_finish() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    println!("  {} Install complete", BULLET_GREEN);
    println!("     Sealed brain:  smith chat | smith think \"…\" | smith brain status");
    println!("     Pipelines:     smith run plan | develop | review | release");
}

/// Bootstrap sealed brain: memory, local model profile, network, Ollama, brain, validate.
fn bootstrap_sealed_brain(cfg: &mut SmithConfig) {
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

    // 2. Ensure a local model profile exists
    let has_local = cfg
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .any(|a| a.agent_type.as_deref() == Some("local"));

    if !has_local {
        let create = prompt_yn(
            &format!(
                "Create default local model profile (ollama / {}) for sealed brain?",
                DEFAULT_LOCAL_MODEL
            ),
            false,
        );
        if create {
            match add_agent_to_config(
                cfg,
                "local".to_string(),
                Some(DEFAULT_AGENT_IMAGE.to_string()),
                Some("local".to_string()),
                Some(DEFAULT_LOCAL_MODEL.to_string()),
                None,
                Some("ollama".to_string()),
                Some(format!("http://localhost:{}", docker::OLLAMA_PORT)),
                None,
                Some(true),
                None,
                None,
                None,
                None,
                None,
            ) {
                Ok(()) => {
                    if cfg.current_agent.is_none() {
                        cfg.current_agent = Some("local".to_string());
                    }
                    if let Err(e) = save_config(cfg) {
                        eprintln!("  {} failed to save local profile: {}", BULLET_RED, e);
                    } else {
                        println!(
                            "  {} local model profile 'local' ({})",
                            BULLET_GREEN, DEFAULT_LOCAL_MODEL
                        );
                    }
                }
                Err(e) => eprintln!("  {} {}", BULLET_RED, e),
            }
        } else {
            println!(
                "  {} no local profile — add later with `smith model add local --agent-type local --model {}`",
                BULLET_YELLOW, DEFAULT_LOCAL_MODEL
            );
        }
    } else {
        println!("  {} local model profile present", BULLET_GREEN);
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

    let start_stack = prompt_yn(
        "Start Ollama + sealed brain now and validate isolation?",
        false,
    );
    if !start_stack {
        println!("  {} skipped runtime start", BULLET_BLUE);
        println!("     Later: `smith model start` then `smith chat`");
        return;
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

    // 5. Start Ollama (host preferred on AMD/Windows; Docker NVIDIA only when available)
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
        Ok(true) => println!("  {} egress - blocked (sealed)", BULLET_GREEN),
        Ok(false) => eprintln!(
            "  {} egress - UNEXPECTED public reachability from brain",
            BULLET_RED
        ),
        Err(e) => eprintln!("  {} egress check - {}", BULLET_YELLOW, e),
    }

    println!("  {} sealed brain ready — try `smith chat`", BULLET_GREEN);
}

pub async fn handle_uninstall(force: bool, remove_config: bool, remove_images: bool) {
    let prompt =
        "This will stop all agent containers and Ollama. Continue? Type 'yes' to confirm: ";
    if !confirm_reset(prompt, force) {
        eprintln!("Uninstall cancelled.");
        std::process::exit(1);
    }

    println!("{} smith uninstall", BULLET_YELLOW);
    println!();

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
