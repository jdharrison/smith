//! Brain ops: status / stop.

use crate::brain::ensure_memory_layout;
use crate::docker;

#[derive(Debug, Clone)]
pub enum BrainCmd {
    Status,
    Stop,
}

pub async fn handle(cmd: BrainCmd) {
    match cmd {
        BrainCmd::Status => {
            let mem = match ensure_memory_layout() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let st = docker::brain_status(&mem);
            let _ = docker::ensure_compose_project_file();
            let net_mode = crate::current_network();
            let runtime = crate::current_runtime();
            let net_b = if st.network_exists {
                crate::BULLET_GREEN
            } else {
                crate::BULLET_RED
            };
            let brain_b = if st.brain_running {
                crate::BULLET_GREEN
            } else {
                crate::BULLET_BLUE
            };
            let ollama_b = if st.ollama_running {
                crate::BULLET_GREEN
            } else {
                crate::BULLET_RED
            };
            let onet_b = if st.ollama_on_network {
                crate::BULLET_GREEN
            } else {
                crate::BULLET_YELLOW
            };

            println!("{} brain", brain_b);
            println!(
                "     runtime: {}  |  network: {}",
                runtime.as_str(),
                net_mode.as_str()
            );
            println!(
                "     compose project: {} ({})",
                docker::COMPOSE_PROJECT,
                docker::compose_file_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(no compose file)".to_string())
            );
            println!(
                "     {} network {} ({})",
                net_b,
                st.network_name,
                if !st.network_exists {
                    "missing"
                } else if st.network_internal {
                    "internal"
                } else {
                    "public/egress"
                }
            );
            println!(
                "     {} container {} — {}",
                brain_b,
                docker::BRAIN_CONTAINER_NAME,
                if st.brain_running {
                    "running"
                } else {
                    "stopped"
                }
            );
            println!(
                "     {} ollama — {}",
                ollama_b,
                if st.ollama_running {
                    match docker::active_ollama_backend() {
                        Some(docker::OllamaBackend::Host) => "running (host)",
                        Some(docker::OllamaBackend::Docker) => "running (docker)",
                        None => "running",
                    }
                } else {
                    "stopped"
                }
            );
            println!(
                "     {} ollama on brain net — {}",
                onet_b,
                if st.ollama_on_network {
                    match docker::active_ollama_backend() {
                        Some(docker::OllamaBackend::Host) => "host.docker.internal",
                        _ => "yes",
                    }
                } else {
                    "no"
                }
            );
            println!("     memory: {}", st.memory_path);

            if st.brain_running && !runtime.is_host() {
                match docker::verify_no_egress() {
                    Ok(true) => {
                        if net_mode.is_private() {
                            println!("     {} egress — blocked (private)", crate::BULLET_GREEN);
                        } else {
                            println!(
                                "     {} egress — blocked (unexpected for network=public)",
                                crate::BULLET_YELLOW
                            );
                        }
                    }
                    Ok(false) => {
                        if !net_mode.is_private() {
                            println!(
                                "     {} egress — open (network=public)",
                                crate::BULLET_GREEN
                            );
                        } else if docker::active_ollama_backend()
                            == Some(docker::OllamaBackend::Host)
                        {
                            println!(
                                "     {} egress — soft-seal (private intended; host Ollama path may leak). Prefer Docker Ollama or accept public.",
                                crate::BULLET_YELLOW
                            );
                        } else {
                            println!(
                                "     {} egress — UNEXPECTED reachability (network=private)",
                                crate::BULLET_RED
                            );
                        }
                    }
                    Err(e) => println!("     egress check: {}", e),
                }
            }
        }
        BrainCmd::Stop => match docker::stop_brain_container() {
            Ok(()) => println!("Brain container stopped (host memory preserved)."),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    }
}
