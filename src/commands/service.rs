//! `smith settings service` — explicit supervisor lifecycle and warm/on-demand policy.

use crate::*;

pub async fn handle(cmd: ServiceCommands) {
    match cmd {
        ServiceCommands::Status => match service::collect_status() {
            Ok(snapshot) => print_status(&snapshot),
            Err(error) => fail(error),
        },
        ServiceCommands::Install => match service::os_register::install() {
            Ok(_) => {
                println!("{} supervisor unit installed", BULLET_GREEN);
                println!("  manager reload: complete");
                println!("  enabled at login: unchanged");
                println!("  runtime state: unchanged");
                print_observed_supervisor();
            }
            Err(error) => fail(error),
        },
        ServiceCommands::Autostart { state } => {
            let on = match state.trim().to_ascii_lowercase().as_str() {
                "on" | "true" | "1" | "enable" | "enabled" => true,
                "off" | "false" | "0" | "disable" | "disabled" => false,
                other => fail(format!("expected on|off, got '{other}'")),
            };
            match service::set_autostart(on) {
                Ok(()) => {
                    println!(
                        "{} login autostart {}",
                        BULLET_GREEN,
                        if on { "enabled" } else { "disabled" }
                    );
                    println!("  runtime state: unchanged");
                    println!("  unit registration: unchanged");
                    print_observed_supervisor();
                }
                Err(error) => fail(error),
            }
        }
        ServiceCommands::Set { id, mode } => {
            let mode = match service::ServiceMode::parse(&mode) {
                Ok(mode) => mode,
                Err(error) => fail(error),
            };
            match service::set_service_mode(&id, mode) {
                Ok(()) => {
                    let canonical = service::resolve_service_id(&id).unwrap_or("?");
                    println!("{} {} → {}", BULLET_GREEN, canonical, mode.as_str());
                }
                Err(error) => fail(error),
            }
        }
        ServiceCommands::Apply => {
            match service::apply_warm_set() {
                Ok(notes) => {
                    println!("{} apply warm set", BULLET_GREEN);
                    for note in notes {
                        println!("  {note}");
                    }
                }
                Err(error) => fail(error),
            }
            // Voice sidecars are owned by a running serve — ask it to ensure when up.
            let svc = service::load_service_config().unwrap_or_default();
            if service::os_register::bridge_is_listening()
                && (svc.is_warm(service::ID_SERVICES_WHISPER)
                    || svc.is_warm(service::ID_SERVICES_TTS))
            {
                match service::os_register::request_sidecars_ensure() {
                    Ok(()) => println!("  voice sidecars: ensure requested"),
                    Err(error) => eprintln!("  Warning: sidecars ensure: {error}"),
                }
            }
        }
        ServiceCommands::Start => match service::os_register::start() {
            Ok(()) => {
                println!(
                    "{} supervisor start requested through systemd",
                    BULLET_GREEN
                );
                println!("  enabled at login: unchanged");
                println!("  unit registration: unchanged");
                print_observed_supervisor();
            }
            Err(error) => fail(error),
        },
        ServiceCommands::Stop => match service::os_register::stop() {
            Ok(()) => {
                println!("{} supervisor stop requested through systemd", BULLET_GREEN);
                println!("  enabled at login: unchanged");
                println!("  unit registration: unchanged");
                print_observed_supervisor();
            }
            Err(error) => fail(error),
        },
        ServiceCommands::Restart => match service::os_register::restart() {
            Ok(()) => {
                println!(
                    "{} supervisor restart requested through systemd",
                    BULLET_GREEN
                );
                println!("  enabled at login: unchanged");
                println!("  unit registration: unchanged");
                print_observed_supervisor();
            }
            Err(error) => fail(error),
        },
        ServiceCommands::Uninstall => match service::uninstall_supervisor() {
            Ok(()) => {
                println!("{} supervisor unit uninstalled", BULLET_GREEN);
                println!("  runtime state: stopped");
                println!("  enabled at login: disabled");
                println!("  unit registration: removed");
                print_observed_supervisor();
            }
            Err(error) => fail(error),
        },
    }
}

fn fail<T>(error: String) -> T {
    eprintln!("Error: {error}");
    std::process::exit(1);
}

fn print_observed_supervisor() {
    print_supervisor_status(&service::os_register::observed_status(), "  ");
}

fn print_status(snapshot: &service::ServicesSnapshot) {
    println!("{} smith services", BULLET_GREEN);
    println!(
        "  configured autostart policy: {}",
        if snapshot.autostart { "on" } else { "off" }
    );
    print_supervisor_status(&snapshot.supervisor, "  ");
    println!("  compose project: {}", docker::COMPOSE_PROJECT);
    println!();

    let mut last_group = String::new();
    for row in &snapshot.services {
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

pub(crate) fn print_supervisor_status(
    status: &service::os_register::ObservedSupervisorStatus,
    indent: &str,
) {
    let availability = if status.bridge_healthy && status.bridge_identity == "smith" {
        "healthy"
    } else if status.sub_state == "auto-restart" {
        "unavailable (automatic restart pending)"
    } else if status.active_state == "failed" || status.service_result == "exit-code" {
        "unavailable (startup failed)"
    } else if status.active_state == "inactive" {
        "stopped"
    } else {
        "unknown"
    };
    println!("{indent}supervisor availability: {availability}");
    println!("{indent}unit registered: {}", yes_no(status.registered));
    println!("{indent}unit load state: {}", status.load_state);
    println!(
        "{indent}enabled at login: {}",
        optional_yes_no(status.enabled_at_login)
    );
    println!("{indent}unit enablement state: {}", status.enabled_state);
    println!("{indent}active state: {}", status.active_state);
    println!("{indent}substate: {}", status.sub_state);
    println!("{indent}service result: {}", status.service_result);
    println!(
        "{indent}restart attempts: {}",
        status
            .restart_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "unknown".into())
    );
    println!("{indent}main process code: {}", status.exec_main_code);
    println!("{indent}main process status: {}", status.exec_main_status);
    println!("{indent}linger: {}", optional_yes_no(status.linger));
    println!(
        "{indent}bridge listening: {}",
        yes_no(status.bridge_listening)
    );
    println!("{indent}bridge identity: {}", status.bridge_identity);
    println!("{indent}bridge healthy: {}", yes_no(status.bridge_healthy));
    if status.sub_state == "auto-restart" {
        println!(
            "{indent}recovery: stop the retry loop with `smith settings service stop`; correct the startup failure before explicitly starting Smith"
        );
    }
    if let Some(detail) = &status.detail {
        println!("{indent}observation detail: {detail}");
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn optional_yes_no(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unknown",
    }
}
