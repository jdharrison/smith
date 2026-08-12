//! Sealed brain ops: status / stop.

use crate::brain::ensure_memory_layout;
use crate::docker;

#[derive(Debug, Clone)]
pub enum BrainCmd {
    Status { verify_egress: bool },
    Stop { remove: bool },
}

pub async fn handle(cmd: BrainCmd) {
    match cmd {
        BrainCmd::Status { verify_egress } => {
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

            if verify_egress {
                if !st.brain_running {
                    println!("     egress check: skipped (brain not running)");
                } else {
                    match docker::verify_no_egress() {
                        Ok(true) => {
                            println!("     {} egress — blocked (good)", crate::BULLET_GREEN)
                        }
                        Ok(false) => {
                            println!(
                                "     {} egress — UNEXPECTED reachability",
                                crate::BULLET_RED
                            );
                        }
                        Err(e) => println!("     egress check error: {}", e),
                    }
                }
            }
        }
        BrainCmd::Stop { remove } => {
            let result = if remove {
                docker::remove_brain_container()
            } else {
                docker::stop_brain_container()
            };
            match result {
                Ok(()) => {
                    if remove {
                        println!("Sealed brain container removed (host memory preserved).");
                    } else {
                        println!("Sealed brain stopped (host memory preserved).");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
