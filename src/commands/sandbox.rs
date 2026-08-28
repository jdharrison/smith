//! `smith settings sandbox` — ad-hoc demo host status/stop.

use crate::*;

#[derive(Debug, Clone)]
pub enum SandboxCmd {
    Status,
    Stop,
}

pub async fn handle(cmd: SandboxCmd) {
    match cmd {
        SandboxCmd::Status => match docker::sandbox_status() {
            Ok(st) => {
                let coding_b = if st.coding_running {
                    BULLET_GREEN
                } else if st.coding_configured {
                    BULLET_YELLOW
                } else {
                    BULLET_BLUE
                };
                let sand_b = if st.sandbox_running {
                    BULLET_GREEN
                } else {
                    BULLET_BLUE
                };
                println!("{} coding agent ({})", coding_b, docker::CODING_AGENT_NAME);
                println!(
                    "     {}",
                    if st.coding_running {
                        "running"
                    } else if st.coding_configured {
                        "configured — start with `smith settings agent start coding` or `smith chat`"
                    } else {
                        "not configured — will be created on next chat/install"
                    }
                );
                println!("{} sandbox ({})", sand_b, docker::SANDBOX_CONTAINER_NAME);
                println!(
                    "     {}",
                    if st.sandbox_running {
                        match st.published_port {
                            Some(p) => format!("running — http://127.0.0.1:{p}"),
                            None => "running".to_string(),
                        }
                    } else {
                        "stopped".to_string()
                    }
                );
                println!("     scratch: {}", st.scratch_path);
                println!("     stop: smith settings sandbox stop");
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        SandboxCmd::Stop => {
            if let Err(e) = docker::stop_sandbox() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
