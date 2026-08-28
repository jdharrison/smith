//! `smith settings service` — supervisor autostart + warm/on-demand policy.

use crate::*;

pub async fn handle(cmd: ServiceCommands) {
    match cmd {
        ServiceCommands::Status => match service::collect_status() {
            Ok(snap) => print_status(&snap),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        ServiceCommands::Autostart { state } => {
            let on = match state.trim().to_ascii_lowercase().as_str() {
                "on" | "true" | "1" | "enable" | "enabled" => true,
                "off" | "false" | "0" | "disable" | "disabled" => false,
                other => {
                    eprintln!("Error: expected on|off, got '{other}'");
                    std::process::exit(1);
                }
            };
            match service::set_autostart(on) {
                Ok(()) => {
                    println!(
                        "{} autostart {}",
                        BULLET_GREEN,
                        if on { "enabled" } else { "disabled" }
                    );
                    if on {
                        println!("  OS registration: smith serve at login");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        ServiceCommands::Set { id, mode } => {
            let mode = match service::ServiceMode::parse(&mode) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            match service::set_service_mode(&id, mode) {
                Ok(()) => {
                    let canonical = service::resolve_service_id(&id).unwrap_or("?");
                    println!(
                        "{} {} → {}",
                        BULLET_GREEN,
                        canonical,
                        mode.as_str()
                    );
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        ServiceCommands::Apply => {
            match service::apply_warm_set() {
                Ok(notes) => {
                    println!("{} apply warm set", BULLET_GREEN);
                    for n in notes {
                        println!("  {n}");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            // Voice sidecars are owned by a running serve — ask it to ensure when up.
            let svc = service::load_service_config().unwrap_or_default();
            if service::os_register::bridge_is_listening()
                && (svc.is_warm(service::ID_SERVICES_WHISPER)
                    || svc.is_warm(service::ID_SERVICES_TTS))
            {
                match service::os_register::request_sidecars_ensure() {
                    Ok(()) => println!("  voice sidecars: ensure requested"),
                    Err(e) => eprintln!("  Warning: sidecars ensure: {e}"),
                }
            }
        }
        ServiceCommands::Start => {
            if service::os_register::bridge_is_listening() {
                println!(
                    "{} supervisor bridge is already running on http://127.0.0.1:{}",
                    BULLET_GREEN,
                    crate::commands::serve::BRIDGE_DEFAULT_PORT
                );
                return;
            }
            // Persist autostart + register once (register starts serve if port is free).
            match service::set_autostart(true) {
                Ok(()) => println!(
                    "{} supervisor start / autostart enabled (`smith serve`)",
                    BULLET_GREEN
                ),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            // Wait briefly for bridge to come up.
            for _ in 0..20 {
                if service::os_register::bridge_is_listening() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            let svc = service::load_service_config().unwrap_or_default();
            if service::os_register::bridge_is_listening()
                && (svc.is_warm(service::ID_SERVICES_WHISPER)
                    || svc.is_warm(service::ID_SERVICES_TTS))
            {
                if let Err(e) = service::os_register::request_sidecars_ensure() {
                    eprintln!("  Warning: sidecars ensure: {e}");
                }
            }
        }
        ServiceCommands::Stop => {
            if let Err(e) = service::os_register::request_bridge_shutdown() {
                eprintln!("Warning: bridge shutdown: {e}");
            }
            match service::set_autostart(false) {
                Ok(()) => println!(
                    "{} supervisor stopped (autostart off; OS unit removed)",
                    BULLET_GREEN
                ),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        },
    }
}

fn print_status(snap: &service::ServicesSnapshot) {
    println!("{} smith services", BULLET_GREEN);
    println!(
        "  autostart: {}  |  os registered: {}",
        if snap.autostart { "on" } else { "off" },
        if snap.os_registered { "yes" } else { "no" }
    );
    println!("  compose project: {}", docker::COMPOSE_PROJECT);
    println!();

    let mut last_group = String::new();
    for row in &snap.services {
        if row.group != last_group {
            last_group = row.group.clone();
            let title = match row.group.as_str() {
                "bridge" => "Bridge",
                "brain" => "Brain",
                "sandbox" => "Sandbox",
                "agents" => "Agents",
                "services" => "Services",
                other => other,
            };
            println!("  {title}");
        }
        let bullet = if row.healthy {
            BULLET_GREEN
        } else if row.running {
            BULLET_YELLOW
        } else {
            BULLET_BLUE
        };
        let url = row.url.as_deref().unwrap_or("");
        let url_bit = if url.is_empty() {
            String::new()
        } else {
            format!("  {url}")
        };
        let detail = row.detail.as_deref().unwrap_or("");
        let detail_bit = if detail.is_empty() {
            String::new()
        } else {
            format!("  ({detail})")
        };
        println!(
            "    {} {}  [{}]  {}{}{}",
            bullet,
            row.id,
            row.mode,
            if row.running { "running" } else { "stopped" },
            url_bit,
            detail_bit
        );
    }
}
