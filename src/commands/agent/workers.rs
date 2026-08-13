//! Named specialized agents (`smith-agent-<name>`) — prompt/MCP stubs, manual start/stop.

use crate::*;

#[allow(clippy::too_many_arguments)]
pub fn add_named(
    name: String,
    image: Option<String>,
    agent_type: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    base_url: Option<String>,
    port: Option<u16>,
    prompt: Option<String>,
    mcp_specs: Vec<String>,
) {
    let mut cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    let mcp = match parse_agent_mcp_specs(&mcp_specs) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) => None,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    // Local agents talk to host Ollama unless base_url is set.
    let (provider, base_url) = if agent_type.as_deref() == Some("local") {
        let provider = provider.or_else(|| Some("ollama".to_string()));
        let base_url = base_url.or_else(|| {
            Some(format!(
                "http://host.docker.internal:{}",
                docker::OLLAMA_PORT
            ))
        });
        (provider, base_url)
    } else {
        (provider, base_url)
    };
    if let Err(e) = add_agent_to_config(
        &mut cfg,
        name.clone(),
        image,
        agent_type,
        model,
        None,
        provider,
        base_url,
        port,
        Some(true),
        None,
        None,
        None,
        prompt,
        mcp,
    ) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    save_config(&cfg).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    if let Some(entry) = cfg
        .agents
        .as_ref()
        .and_then(|a| a.iter().find(|e| e.name == name))
    {
        let _ = write_named_agent_runtime_config(entry);
    }
    println!(
        "Agent '{}' added. Start with: smith agent start {}",
        name, name
    );
}

pub fn remove_named(name: String) {
    let mut cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    let agents = cfg.agents.get_or_insert_with(Vec::new);
    let before = agents.len();
    agents.retain(|e| e.name != name);
    if agents.len() == before {
        eprintln!("Error: Agent '{}' not found", name);
        std::process::exit(1);
    }
    if cfg.current_agent.as_deref() == Some(name.as_str()) {
        cfg.current_agent = agents.first().map(|e| e.name.clone());
    }
    save_config(&cfg).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    println!(
        "Agent '{}' removed from config (container left untouched; use `smith agent stop {}`).",
        name, name
    );
}

pub fn start_named(name: String, port_override: Option<u16>) {
    if let Err(e) = docker::check_docker_available() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    let cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    let (index, entry) = cfg
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .find(|(_, e)| e.name == name)
        .map(|(i, e)| (i, e.clone()))
        .unwrap_or_else(|| {
            eprintln!(
                "Error: Agent '{}' not found. Add with `smith agent add {}`.",
                name, name
            );
            std::process::exit(1);
        });

    let port = port_override.unwrap_or_else(|| agent_port(&entry, index));
    let image =
        if docker::image_exists(&docker::agent_built_image_tag(&entry.name)).unwrap_or(false) {
            docker::agent_built_image_tag(&entry.name)
        } else {
            entry.image.clone()
        };

    let is_local = entry.agent_type.as_deref() == Some("local");
    let provider = if is_local {
        Some("ollama".to_string())
    } else {
        entry.provider.clone()
    };
    let base_url = if is_local {
        Some(format!(
            "http://host.docker.internal:{}",
            docker::OLLAMA_PORT
        ))
    } else {
        entry.base_url.clone()
    };

    if is_local && !docker::is_ollama_running() {
        eprintln!(
            "Warning: Ollama is not running. Start it with `smith model start` (ollama only), then retry."
        );
    }

    let config_dir = match write_named_agent_runtime_config(&entry) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    match docker::start_agent_container(
        &entry.name,
        &image,
        port,
        provider.as_deref(),
        base_url.as_deref(),
        Some(&config_dir),
    ) {
        Ok(()) => {
            println!(
                "  {}: started (port {} {})",
                entry.name,
                port,
                clickable_agent_url(port)
            );
            if let Some(ref prompt) = entry.prompt {
                let preview: String = prompt.chars().take(80).collect();
                println!(
                    "  prompt: {}{}",
                    preview,
                    if prompt.len() > 80 { "…" } else { "" }
                );
            }
            if let Some(ref mcp) = entry.mcp {
                if !mcp.is_empty() {
                    let names: Vec<_> = mcp.iter().map(|m| m.name.as_str()).collect();
                    println!("  mcp: {}", names.join(", "));
                }
            }
            println!("  config: {}", config_dir.display());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn stop_named(name: String) {
    if let Err(e) = docker::check_docker_available() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    match docker::stop_agent_container(&name) {
        Ok(()) => println!("  {}: stopped", name),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn stop_all_named() {
    if let Err(e) = docker::check_docker_available() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    let running = docker::list_running_agent_containers().unwrap_or_default();
    if running.is_empty() {
        println!("No running named agents.");
        return;
    }
    let mut stopped = 0usize;
    for name in &running {
        if docker::stop_agent_container(name).is_ok() {
            println!("  {}: stopped", name);
            stopped += 1;
        }
    }
    println!("Stopped {} named agent(s).", stopped);
}

pub fn list_named_section() {
    let cfg = load_config().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });
    let running = docker::list_running_agent_containers().unwrap_or_default();
    let list = cfg.agents.as_deref().unwrap_or(&[]);
    println!("Named agents (smith-agent-*):");
    if list.is_empty() {
        println!("  (none — `smith agent add <name>`)");
        return;
    }
    for (i, e) in list.iter().enumerate() {
        let port = agent_port(e, i);
        let status = if running.iter().any(|n| n == &e.name) {
            "running"
        } else {
            "stopped"
        };
        let kind = e.agent_type.as_deref().unwrap_or("cloud");
        let mcp_n = e.mcp.as_ref().map(|m| m.len()).unwrap_or(0);
        let prompt_mark = if e.prompt.as_ref().is_some_and(|p| !p.is_empty()) {
            "prompt"
        } else {
            "-"
        };
        println!(
            "  {} [{}] type={} port={} mcp={} {} {}",
            e.name,
            status,
            kind,
            port,
            mcp_n,
            prompt_mark,
            if status == "running" {
                clickable_agent_url(port)
            } else {
                String::new()
            }
        );
    }
}
