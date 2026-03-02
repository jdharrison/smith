use crate::*;

pub async fn handle(cmd: ModelCommands) {
    match cmd {
        ModelCommands::Add {
            name,
            image,
            agent_type,
            model,
            small_model,
            provider,
            base_url,
            port,
            enabled,
        } => {
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            if let Err(e) = add_agent_to_config(
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
            ) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            save_config(&cfg).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!("Agent '{}' added successfully", name);
        }
        ModelCommands::Status => {
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let running = docker::list_running_agent_containers().unwrap_or_default();
            let _current = cfg.current_agent.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
            let base = docker::OPENCODE_SERVER_PORT;
            let list = cfg.agents.as_deref().unwrap_or(&[]);
            #[allow(clippy::type_complexity)]
            let agents: Vec<(
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<bool>,
                Option<HashMap<String, AgentRole>>,
                u16,
            )> = if list.is_empty() {
                vec![(
                    DEFAULT_AGENT_NAME.to_string(),
                    DEFAULT_AGENT_IMAGE.to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(true),
                    None,
                    base,
                )]
            } else {
                list.iter()
                    .enumerate()
                    .map(|(i, e)| {
                        (
                            e.name.clone(),
                            e.image.clone(),
                            e.agent_type.clone(),
                            e.model.clone(),
                            e.small_model.clone(),
                            e.provider.clone(),
                            e.base_url.clone(),
                            e.enabled,
                            e.roles.clone(),
                            agent_port(e, i),
                        )
                    })
                    .collect()
            };
            for (
                name,
                image,
                agent_type,
                model,
                small_model,
                provider,
                _base_url,
                enabled,
                roles,
                port,
            ) in &agents
            {
                let active = running.contains(name);
                let reachable = if active {
                    Some(docker::check_agent_reachable(*port))
                } else {
                    None
                };
                let built =
                    docker::image_exists(&docker::agent_built_image_tag(name)).unwrap_or(false);
                let is_cloud = agent_type.as_deref() != Some("local");
                let circle = status_circle(active, reachable, built, is_cloud);
                println!("\n  {} {}", circle, name);
                if active {
                    if reachable == Some(false) {
                        println!("      Active:   active (running)");
                        println!(
                            "      Port:     {} {} (warning: unreachable)",
                            port,
                            clickable_agent_url(*port)
                        );
                    } else {
                        println!("      Active:   active (running)");
                        println!("      Port:     {} {}", port, clickable_agent_url(*port));
                    }
                } else if is_cloud {
                    println!("      Active:   cloud (config only)");
                } else {
                    println!(
                        "      Active:   {}",
                        if enabled.unwrap_or(true) {
                            "inactive"
                        } else {
                            "disabled"
                        }
                    );
                }
                if !is_cloud {
                    let built_tag = docker::agent_built_image_tag(name);
                    let image_line = if built {
                        built_tag
                    } else {
                        format!("not built (uses {})", image)
                    };
                    println!("      Image:    {}", image_line);
                }
                let model_str = model.as_deref().unwrap_or("default");
                let small_model_str = small_model.as_deref().unwrap_or("-");
                let is_local = agent_type.as_deref() == Some("local");
                let provider_str = provider.as_deref().unwrap_or("-");
                let mode_str = if is_local { "local" } else { "cloud" };
                println!(
                    "      Model:    {}  Small: {}  Provider: {}  Type: {}",
                    model_str, small_model_str, provider_str, mode_str
                );
                let roles_str = match roles.as_ref() {
                    Some(r) if !r.is_empty() => r.keys().cloned().collect::<Vec<_>>().join(", "),
                    _ => "-".to_string(),
                };
                println!("      Roles:    {}", roles_str);
            }
            if agents.is_empty() {
                println!("\n  (no cloud agents configured)");
            } else {
                println!();
            }
        }
        ModelCommands::Update {
            name,
            image,
            agent_type,
            model,
            small_model,
            provider,
            base_url,
            port,
            enabled,
        } => {
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let agents = cfg.agents.get_or_insert_with(Vec::new);
            match agents.iter_mut().find(|a| a.name == name) {
                Some(entry) => {
                    let is_wizard = image.is_none()
                        && agent_type.is_none()
                        && model.is_none()
                        && small_model.is_none()
                        && provider.is_none()
                        && base_url.is_none()
                        && port.is_none()
                        && enabled.is_none();
                    if is_wizard {
                        println!("  Updating agent '{}'", entry.name);
                        let image_in = prompt_line(&format!("  Image [{}]: ", entry.image));
                        if !image_in.is_empty() {
                            entry.image = image_in;
                        }
                        let type_in = prompt_line(&format!(
                            "  Type (local/cloud) [{}]: ",
                            entry.agent_type.as_deref().unwrap_or("cloud")
                        ));
                        if !type_in.is_empty() {
                            entry.agent_type = Some(type_in);
                        } else if entry.agent_type.is_some() && type_in.is_empty() {
                            entry.agent_type = None;
                        }
                        let model_in = prompt_line(&format!(
                            "  Model [{}]: ",
                            entry.model.as_deref().unwrap_or("(none)")
                        ));
                        if !model_in.is_empty() {
                            entry.model = Some(model_in);
                        } else if entry.model.is_some() && model_in.is_empty() {
                            entry.model = None;
                        }
                        let small_model_in = prompt_line(&format!(
                            "  Small model [{}]: ",
                            entry.small_model.as_deref().unwrap_or("(none)")
                        ));
                        if !small_model_in.is_empty() {
                            entry.small_model = Some(small_model_in);
                        } else if entry.small_model.is_some() && small_model_in.is_empty() {
                            entry.small_model = None;
                        }
                        let provider_in = prompt_line(&format!(
                            "  Provider [{}]: ",
                            entry.provider.as_deref().unwrap_or("(none)")
                        ));
                        if !provider_in.is_empty() {
                            entry.provider = Some(provider_in);
                        } else if entry.provider.is_some() && provider_in.is_empty() {
                            entry.provider = None;
                        }
                        let base_url_in = prompt_line(&format!(
                            "  Base URL [{}]: ",
                            entry.base_url.as_deref().unwrap_or("(none)")
                        ));
                        if !base_url_in.is_empty() {
                            entry.base_url = Some(base_url_in);
                        } else if entry.base_url.is_some() && base_url_in.is_empty() {
                            entry.base_url = None;
                        }
                        let port_in = prompt_line(&format!(
                            "  Port [{}]: ",
                            entry
                                .port
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "4096".to_string())
                        ));
                        if !port_in.is_empty() {
                            if let Ok(p) = port_in.parse() {
                                entry.port = Some(p);
                            }
                        } else if entry.port.is_some() && port_in.is_empty() {
                            entry.port = None;
                        }
                        let enabled_in = prompt_line(&format!(
                            "  Enabled (true/false) [{}]: ",
                            entry.enabled.unwrap_or(true)
                        ));
                        if !enabled_in.is_empty() {
                            entry.enabled = Some(enabled_in == "true");
                        }
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("  {} Agent '{}' updated", BULLET_GREEN, name);
                    } else {
                        if let Some(ref s) = image {
                            entry.image = if s.is_empty() {
                                DEFAULT_AGENT_IMAGE.to_string()
                            } else {
                                s.clone()
                            };
                        }
                        if let Some(ref s) = agent_type {
                            entry.agent_type = if s.is_empty() { None } else { Some(s.clone()) };
                        }
                        if let Some(ref s) = model {
                            entry.model = if s.is_empty() { None } else { Some(s.clone()) };
                        }
                        if let Some(ref s) = small_model {
                            entry.small_model = if s.is_empty() { None } else { Some(s.clone()) };
                        }
                        if let Some(ref s) = provider {
                            entry.provider = if s.is_empty() { None } else { Some(s.clone()) };
                        }
                        if let Some(ref s) = base_url {
                            entry.base_url = if s.is_empty() { None } else { Some(s.clone()) };
                        }
                        if let Some(p) = port {
                            entry.port = Some(p);
                        }
                        if let Some(e) = enabled {
                            entry.enabled = Some(e);
                        }
                        save_config(&cfg).unwrap_or_else(|e| {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        });
                        println!("Agent '{}' updated successfully", name);
                    }
                }
                None => {
                    eprintln!("Error: Agent '{}' not found", name);
                    std::process::exit(1);
                }
            }
        }
        ModelCommands::Remove { name } => {
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let agents = cfg.agents.get_or_insert_with(Vec::new);
            let initial_len = agents.len();
            agents.retain(|a| a.name != name);
            if agents.len() == initial_len {
                eprintln!("Error: Agent '{}' not found", name);
                std::process::exit(1);
            }
            if cfg.current_agent.as_deref() == Some(name.as_str()) {
                cfg.current_agent = Some(
                    agents
                        .first()
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| DEFAULT_AGENT_NAME.to_string()),
                );
            }
            save_config(&cfg).unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!("Agent '{}' removed successfully", name);
        }
        ModelCommands::Sync => {
            use serde_json::{json, Map, Value};

            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });

            let agents = cfg.agents.as_deref().unwrap_or(&[]);
            let enabled_agents: Vec<_> = agents
                .iter()
                .filter(|e| e.enabled.unwrap_or(true))
                .collect();

            if enabled_agents.is_empty() {
                eprintln!("No enabled agents to sync.");
                std::process::exit(1);
            }

            // Build provider config for each agent
            let mut providers: Map<String, Value> = Map::new();
            let mut default_model: Option<String> = None;
            let mut default_small_model: Option<String> = None;

            for agent in &enabled_agents {
                let is_local = agent.agent_type.as_deref() == Some("local");

                // Build provider options
                let mut options: Map<String, Value> = Map::new();

                if is_local {
                    // Local agent: set baseURL to localhost
                    options.insert("baseURL".to_string(), json!("http://localhost:11434"));
                }
                // Cloud agent: no baseURL (uses provider default)

                // Use agent name as provider identifier
                let provider_name = agent.name.clone();
                providers.insert(provider_name.clone(), json!({ "options": options }));

                // Set default model from first agent
                // For cloud agents: only set if model is explicitly configured (provider is needed)
                // For local agents: use agent-name/model format
                if default_model.is_none() {
                    if let Some(ref model) = agent.model {
                        if is_local {
                            default_model = Some(format!("{}/{}", agent.name, model));
                        } else {
                            default_model = Some(model.clone());
                        }
                    }
                    // Don't default cloud agents - they need explicit provider
                }
                if default_small_model.is_none() {
                    if let Some(ref small_model) = agent.small_model {
                        if is_local {
                            default_small_model = Some(format!("{}/{}", agent.name, small_model));
                        } else {
                            default_small_model = Some(small_model.clone());
                        }
                    }
                }
            }

            // Build opencode config JSON
            let mut opencode_config: Map<String, Value> = Map::new();
            opencode_config.insert(
                "$schema".to_string(),
                json!("https://opencode.ai/config.json"),
            );

            if let Some(ref model) = default_model {
                opencode_config.insert("model".to_string(), json!(model));
            }
            if let Some(ref small_model) = default_small_model {
                opencode_config.insert("small_model".to_string(), json!(small_model));
            }

            if !providers.is_empty() {
                opencode_config.insert("provider".to_string(), json!(providers));
            }

            let json_str = serde_json::to_string_pretty(&opencode_config)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));

            // Ensure directory exists
            let config_dir = dirs::config_dir()
                .map(|p| p.join("opencode"))
                .expect("Could not find config directory");

            std::fs::create_dir_all(&config_dir).unwrap_or_else(|e| {
                eprintln!("Error creating config directory: {}", e);
                std::process::exit(1);
            });

            let config_path = config_dir.join("opencode.json");
            std::fs::write(&config_path, &json_str).unwrap_or_else(|e| {
                eprintln!("Error writing config: {}", e);
                std::process::exit(1);
            });

            println!(
                "  {} Synced {} agent(s) to {}",
                BULLET_GREEN,
                enabled_agents.len(),
                config_path.display()
            );
            println!("  Edit ~/.config/opencode/opencode.json to select model with \"model\": \"agent-name/model\"");
        }
        ModelCommands::Build {
            name,
            all,
            force,
            verbose,
        } => {
            if let Err(e) = docker::check_docker_available() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            #[allow(clippy::type_complexity)]
            let agents: Vec<(
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                u16,
            )> = if all || name.is_none() {
                match cfg.agents.as_deref() {
                    Some(a) if !a.is_empty() => a
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            (
                                e.name.clone(),
                                e.image.clone(),
                                e.agent_type.clone(),
                                e.model.clone(),
                                e.small_model.clone(),
                                e.provider.clone(),
                                agent_port(e, i),
                            )
                        })
                        .collect(),
                    _ => vec![(
                        DEFAULT_AGENT_NAME.to_string(),
                        DEFAULT_AGENT_IMAGE.to_string(),
                        Some("cloud".to_string()),
                        None,
                        None,
                        None,
                        docker::OPENCODE_SERVER_PORT,
                    )],
                }
            } else {
                let n = name.as_deref().unwrap_or(DEFAULT_AGENT_NAME);
                let (base_image, agent_type, model, small_model, provider, port) = cfg
                    .agents
                    .as_deref()
                    .and_then(|a| {
                        a.iter().position(|e| e.name == n).map(|idx| {
                            let e = &a[idx];
                            (
                                e.image.clone(),
                                e.agent_type.clone(),
                                e.model.clone(),
                                e.small_model.clone(),
                                e.provider.clone(),
                                agent_port(e, idx),
                            )
                        })
                    })
                    .unwrap_or_else(|| {
                        if n == DEFAULT_AGENT_NAME {
                            (
                                DEFAULT_AGENT_IMAGE.to_string(),
                                Some("cloud".to_string()),
                                None,
                                None,
                                None,
                                docker::OPENCODE_SERVER_PORT,
                            )
                        } else {
                            eprintln!("Error: Agent '{}' not found", n);
                            std::process::exit(1);
                        }
                    });
                vec![(
                    n.to_string(),
                    base_image,
                    agent_type,
                    model,
                    small_model,
                    provider,
                    port,
                )]
            };
            let dir = config_dir().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let mut ok = 0usize;
            let mut failed = Vec::new();
            for (agent_name, base_image, agent_type, model, small_model, provider, port) in &agents
            {
                let is_cloud = agent_type.as_deref() != Some("local");
                if is_cloud {
                    println!("  {} Skipping cloud agent '{}'", BULLET_BLUE, agent_name);
                    continue;
                }
                if verbose {
                    let agent_dir = dir.join("agents").join(agent_name);
                    let dockerfile = agent_dir.join("Dockerfile");
                    let tag = docker::agent_built_image_tag(agent_name);
                    println!(
                        "  {}: agent_dir={} Dockerfile={} image={} port={}",
                        agent_name,
                        agent_dir.display(),
                        dockerfile.display(),
                        tag,
                        port
                    );
                }
                match build_agent_image(
                    dir.as_path(),
                    agent_name,
                    base_image,
                    *port,
                    model.as_deref(),
                    small_model.as_deref(),
                    provider.as_deref(),
                    force,
                ) {
                    Ok(()) => {
                        let tag = docker::agent_built_image_tag(agent_name);
                        if verbose {
                            println!(
                                "  {}: docker build -t {} {}",
                                agent_name,
                                tag,
                                dir.join("agents").join(agent_name).display()
                            );
                        }
                        println!("  {}: built {}", agent_name, tag);
                        ok += 1;
                    }
                    Err(e) => {
                        eprintln!("  {}: build failed - {}", agent_name, e);
                        failed.push((agent_name.clone(), e));
                    }
                }
            }
            if failed.is_empty() {
                println!("Built {} agent image(s) successfully.", ok);
            } else {
                println!("Built {}; {} failed.", ok, failed.len());
                std::process::exit(1);
            }
        }
        ModelCommands::Start { verbose } => {
            if let Err(e) = docker::check_docker_available() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            let agents = cfg.agents.as_deref().unwrap_or(&[]);

            // Start Ollama for each local agent (each gets its own container with its model)
            let local_agents: Vec<_> = agents
                .iter()
                .filter(|e| e.agent_type.as_deref() == Some("local"))
                .collect();
            for local in &local_agents {
                let local_model = local
                    .model
                    .clone()
                    .unwrap_or_else(|| "qwen3:8b".to_string());
                if docker::is_ollama_running() {
                    println!("  Ollama already running");
                } else {
                    match docker::start_ollama_container(&local_model, true) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error starting Ollama: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }

            // Build agent list (1 agent : 1 container) - only enabled local agents (skip cloud)
            let enabled_agents: Vec<_> = if agents.is_empty() {
                // Default agent is cloud, so no containers to start
                vec![]
            } else {
                agents
                    .iter()
                    .filter(|e| {
                        let is_enabled = e.enabled.unwrap_or(true);
                        let is_local = e.agent_type.as_deref() == Some("local");
                        is_enabled && is_local
                    })
                    .enumerate()
                    .map(|(i, e)| {
                        let image = if docker::image_exists(&docker::agent_built_image_tag(&e.name))
                            .unwrap_or(false)
                        {
                            docker::agent_built_image_tag(&e.name)
                        } else {
                            e.image.clone()
                        };
                        let is_local = e.agent_type.as_deref() == Some("local");
                        let provider = if is_local {
                            Some("ollama".to_string())
                        } else {
                            e.provider.clone()
                        };
                        let base_url = if is_local {
                            Some(format!(
                                "http://host.docker.internal:{}",
                                docker::OLLAMA_PORT
                            ))
                        } else {
                            e.base_url.clone()
                        };
                        (
                            e.name.clone(),
                            image,
                            provider,
                            base_url,
                            e.enabled.unwrap_or(true),
                            agent_port(e, i),
                        )
                    })
                    .collect()
            };
            // Print skipping messages for cloud agents
            if !agents.is_empty() {
                for agent in agents.iter() {
                    let is_local = agent.agent_type.as_deref() == Some("local");
                    if !is_local {
                        println!("  {} Skipping cloud agent '{}'", BULLET_BLUE, agent.name);
                    }
                }
            }
            let running = docker::list_running_agent_containers().unwrap_or_default();
            if verbose {
                println!("Agents: {}", enabled_agents.len());
                for (name, image, provider, base_url, _enabled, port) in &enabled_agents {
                    let status = if running.contains(name) {
                        "already running"
                    } else {
                        "will start"
                    };
                    let provider_str = provider.as_deref().unwrap_or("-");
                    let mode = if base_url.is_some() { "local" } else { "cloud" };
                    println!(
                        "  {} -> {} port={} provider={} mode={} {} [{}]",
                        name,
                        image,
                        port,
                        provider_str,
                        mode,
                        clickable_agent_url(*port),
                        status
                    );
                }
                if !running.is_empty() {
                    println!("Already running: {}", running.join(", "));
                }
            }
            let mut ok = 0usize;
            let mut failed = Vec::new();
            for (name, image, provider, base_url, _enabled, port) in &enabled_agents {
                if running.contains(name) {
                    println!(
                        "  {}: already running (port {} {})",
                        name,
                        port,
                        clickable_agent_url(*port)
                    );
                    ok += 1;
                    continue;
                }
                if verbose {
                    let container_name = docker::agent_container_name(name);
                    let env_vars = if let Some(ref p) = provider {
                        let mut vars = format!(" -e {}_API_KEY={}", p.to_uppercase(), "dummy");
                        if let Some(ref url) = base_url {
                            vars.push_str(&format!(" -e OPENCODE_BASE_URL={}", url));
                        }
                        vars
                    } else {
                        String::new()
                    };
                    println!(
                                "  {}: docker run -d --name {} -p {}:{}{} --entrypoint opencode {} serve --hostname 0.0.0.0 --port {}",
                                name, container_name, port, port, env_vars, image, port
                            );
                }
                match docker::start_agent_container(
                    name,
                    image,
                    *port,
                    provider.as_deref(),
                    base_url.as_deref(),
                ) {
                    Ok(()) => {
                        println!(
                            "  {}: started (port {} {})",
                            name,
                            port,
                            clickable_agent_url(*port)
                        );
                        if verbose {
                            println!("  {}: waiting 3s before health check...", name);
                        }
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        if verbose {
                            println!("  {}: GET {}", name, clickable_agent_url(*port));
                        }
                        match docker::test_agent_server(*port) {
                            Ok(()) => {
                                println!("  {}: health check OK", name);
                                ok += 1;
                            }
                            Err(e) => {
                                eprintln!("  {}: health check failed - {}", name, e);
                                failed.push((name.clone(), e));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  {}: failed to start - {}", name, e);
                        failed.push((name.clone(), e));
                    }
                }
            }
            if failed.is_empty() {
                if ok == 0 {
                    println!("No local agents to start.");
                } else {
                    println!("All {} local agent(s) started and tested successfully.", ok);
                }
            } else {
                println!("Started {}; {} failed.", ok, failed.len());
                std::process::exit(1);
            }
        }
        ModelCommands::Stop => {
            if let Err(e) = docker::check_docker_available() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            // Print skipping messages for cloud agents
            let all_agents = cfg.agents.as_deref().unwrap_or(&[]);
            for agent in all_agents.iter() {
                let is_local = agent.agent_type.as_deref() == Some("local");
                if !is_local {
                    println!("  {} Skipping cloud agent '{}'", BULLET_BLUE, agent.name);
                }
            }
            // Only stop local agent containers (skip cloud)
            let running = docker::list_running_agent_containers().unwrap_or_default();
            let agents = cfg.agents.as_deref().unwrap_or(&[]);
            let local_agent_names: std::collections::HashSet<_> = agents
                .iter()
                .filter(|e| e.agent_type.as_deref() == Some("local"))
                .map(|e| e.name.clone())
                .collect();
            let to_stop: Vec<_> = running
                .into_iter()
                .filter(|name| local_agent_names.contains(name))
                .collect();
            let mut stopped = Vec::new();
            for name in &to_stop {
                if docker::stop_agent_container(name).is_ok() {
                    stopped.push(name.clone());
                }
            }
            if stopped.is_empty() {
                println!("No running local agent containers.");
            } else {
                for name in &stopped {
                    println!("  {}: stopped", name);
                }
                println!("Stopped {} container(s).", stopped.len());
            }
            // Also stop Ollama if it's running
            if docker::is_ollama_running() {
                if let Err(e) = docker::stop_ollama_container() {
                    eprintln!("Warning: failed to stop Ollama: {}", e);
                } else {
                    println!("  Ollama: stopped");
                }
            }
        }
        ModelCommands::Logs { name } => {
            if let Err(e) = docker::check_docker_available() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            let container_name = docker::agent_container_name(&name);
            if !docker::container_exists(&container_name).unwrap_or(false) {
                eprintln!(
                    "Error: Container '{}' not found. Start the agent with 'smith agent start'.",
                    container_name
                );
                std::process::exit(1);
            }
            let status = Command::new("docker")
                .args(["logs", "-f", &container_name])
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("Error: Failed to run docker logs: {}", e);
                    std::process::exit(1);
                });
            if let Some(code) = status.code() {
                std::process::exit(code);
            }
            std::process::exit(1);
        }
    }
}
