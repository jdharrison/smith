//! Runtime / network settings (and deprecated `isolation` alias).

use crate::*;

pub async fn handle_runtime(cmd: RuntimeCommands) {
    match cmd {
        RuntimeCommands::Get => {
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!("runtime: {}", cfg.runtime().as_str());
            match cfg.runtime() {
                RuntimeMode::Container => {
                    println!("  brain process runs in Docker (`smith-brain`).");
                    println!("  Coding / pipeline / search workers stay containerized.");
                }
                RuntimeMode::Host => {
                    println!("  {}", HOST_RUNTIME_WARNING);
                    println!("  Chat/think/files: native (no Docker). Workers still need Docker.");
                    let roots = host_file_roots(&cfg);
                    if roots.is_empty() {
                        println!("  file roots: (none — add workspaces or host.roots)");
                    } else {
                        println!("  file roots:");
                        for r in roots {
                            println!("    {}", r.display());
                        }
                    }
                }
            }
            println!("  network: {} (orthogonal — `smith settings network`)", cfg.network().as_str());
        }
        RuntimeCommands::Set { mode } => {
            let parsed = match RuntimeMode::parse_mode(&mode) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            cfg.set_runtime(parsed);
            if let Err(e) = save_config(&cfg) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!("runtime set to {}", parsed.as_str());
            if parsed.is_host() {
                eprintln!("Warning: {}", HOST_RUNTIME_WARNING);
            }
            println!(
                "  network remains {} — use `smith settings network set …` to change",
                cfg.network().as_str()
            );
        }
    }
}

pub async fn handle_network(cmd: NetworkCommands) {
    match cmd {
        NetworkCommands::Get => {
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!("network: {}", cfg.network().as_str());
            match cfg.network() {
                NetworkMode::Private => {
                    println!("  brain has no general internet.");
                    println!("  Live search/weather: smith runs declassified oneshot workers on the bridge.");
                }
                NetworkMode::Public => {
                    println!("  brain may reach the internet (subject to runtime).");
                    println!("  Oneshot search workers remain available.");
                }
            }
            println!("  runtime: {} (orthogonal — `smith settings runtime`)", cfg.runtime().as_str());
        }
        NetworkCommands::Set { mode } => {
            let parsed = match NetworkMode::parse_mode(&mode) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            cfg.set_network(parsed);
            if let Err(e) = save_config(&cfg) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!("network set to {}", parsed.as_str());
            if cfg.runtime().is_host() && parsed.is_private() {
                println!(
                    "  host+private: brain stays offline; net tasks use oneshot workers (not silent host egress)."
                );
            }
            if !cfg.runtime().is_host() {
                println!("  recreate the brain container on next chat so Docker networking applies.");
            }
        }
    }
}

/// Deprecated: `sealed` → container+private, `host` → host+public.
pub async fn handle(cmd: IsolationCommands) {
    match cmd {
        IsolationCommands::Get => {
            let cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            println!(
                "isolation: {} (deprecated alias)",
                cfg.isolation.as_str()
            );
            println!(
                "  runtime: {}  |  network: {}",
                cfg.runtime().as_str(),
                cfg.network().as_str()
            );
            println!("  prefer: `smith settings runtime get` / `smith settings network get`");
            match cfg.runtime() {
                RuntimeMode::Container => {
                    println!("  brain process: Docker. Network policy: {}.", cfg.network().as_str());
                }
                RuntimeMode::Host => {
                    println!("  {}", HOST_RUNTIME_WARNING);
                    let roots = host_file_roots(&cfg);
                    if roots.is_empty() {
                        println!("  file roots: (none — add workspaces or host.roots)");
                    } else {
                        println!("  file roots:");
                        for r in roots {
                            println!("    {}", r.display());
                        }
                    }
                }
            }
        }
        IsolationCommands::Set { mode } => {
            let parsed = match IsolationMode::parse_mode(&mode) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            if parsed == IsolationMode::Child {
                eprintln!(
                    "Error: isolation 'child' is reserved (relay of a primary host). Not implemented."
                );
                std::process::exit(1);
            }
            let (runtime, network) = parsed.to_runtime_network();
            let mut cfg = load_config().unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            });
            cfg.set_runtime_network(runtime, network);
            if let Err(e) = save_config(&cfg) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!(
                "isolation set to {} → runtime={} network={}",
                parsed.as_str(),
                runtime.as_str(),
                network.as_str()
            );
            if runtime.is_host() {
                eprintln!("Warning: {}", HOST_RUNTIME_WARNING);
            }
        }
    }
}
