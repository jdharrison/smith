//! Sealed brain ops: status / stop.

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

            println!("{} sealed brain", brain_b);
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
                docker::BRAIN_NETWORK,
                if st.network_exists {
                    "internal"
                } else {
                    "missing"
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

            if st.brain_running {
                match docker::verify_no_egress() {
                    Ok(true) => {
                        println!("     {} egress — blocked (sealed)", crate::BULLET_GREEN)
                    }
                    Ok(false) => {
                        if docker::active_ollama_backend() == Some(docker::OllamaBackend::Host) {
                            println!(
                                "     {} egress — soft-seal (host Ollama; no cloud keys mounted)",
                                crate::BULLET_YELLOW
                            );
                        } else {
                            println!(
                                "     {} egress — UNEXPECTED reachability",
                                crate::BULLET_RED
                            );
                        }
                    }
                    Err(e) => println!("     egress check: {}", e),
                }
            }
        }
        BrainCmd::Stop => match docker::stop_brain_container() {
            Ok(()) => println!("Sealed brain stopped (host memory preserved)."),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    }
}
