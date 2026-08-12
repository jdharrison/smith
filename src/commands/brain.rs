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
                    "running"
                } else {
                    "stopped"
                }
            );
            println!(
                "     {} ollama on brain net — {}",
                onet_b,
                if st.ollama_on_network { "yes" } else { "no" }
            );
            println!("     memory: {}", st.memory_path);

            if st.brain_running {
                match docker::verify_no_egress() {
                    Ok(true) => {
                        println!("     {} egress — blocked (sealed)", crate::BULLET_GREEN)
                    }
                    Ok(false) => println!(
                        "     {} egress — UNEXPECTED reachability",
                        crate::BULLET_RED
                    ),
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
