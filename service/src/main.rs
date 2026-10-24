use smith_service::{Action, Controller, Roots, SystemManager, IDENTITY};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "dev-local-worker") {
        if args.len() != 2 || smith_service::dev_worker_local::run(&args[1]).is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.first().is_some_and(|arg| arg == "dev-source-export") {
        if args.len() != 2 || smith_service::dev_worker_local::run_export(&args[1]).is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.first().is_some_and(|arg| arg == "dev-worker") {
        // Never mix normal control envelopes or raw diagnostics into worker stdout.
        if args.len() != 2 || smith_service::dev_worker::run(&args[1]).is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.is_empty() {
        println!("Hello, world! Smith next debug service — use inspect for read-only state.");
        return;
    }
    if args == ["--version"] {
        println!("{IDENTITY}");
        return;
    }
    if args == ["--help"] {
        println!("smith-service app (one JSON line on stdin) | inspect | prepare --action <install-binary|install-unit|enable|start|complete-setup> | apply --action <action> --approval <token> | serve [--data-root <path> --config-root <path> --runtime-root <path>]");
        return;
    }
    if args.first().is_some_and(|a| a == "app") {
        run_app(&args);
        return;
    }
    let result = run(&args);
    if let Err(error) = result {
        // Even invalid input/root discovery produces the control envelope, not raw paths/errors.
        println!(
            "{}",
            serde_json::json!({
                "schema_version":2,"service":"smith-next","ok":false,
                "binary_state":"unknown","unit_state":"unknown","enablement_state":"unknown","runtime_state":"unknown","bridge_state":"unknown",
                "setup_complete":null,"message":"Control preflight failed; no action performed","error":error,
                "effects":{"binary_path":"","unit_path":"","state_path":"","socket_path":"","description":"No effects; valid roots unavailable or arguments rejected."}
            })
        );
        std::process::exit(1);
    }
}
fn run_app(args: &[String]) {
    let mut call_id = String::new();
    let mut version = 1;
    let result = (|| -> smith_service::Result<smith_service::app::AppReply> {
        if args.len() != 1 {
            return Err("App accepts no arguments".into());
        }
        let request = smith_service::app_stdin_request_correlated(&mut version, &mut call_id)?;
        call_id = request.call_id.clone();
        version = request.schema_version;
        production_preflight(unsafe { libc::geteuid() })?;
        smith_service::install_control_cancellation()?;
        let controller = Controller {
            roots: Roots::environment()?,
            source: std::env::current_exe().map_err(|_| "Cannot resolve helper identity")?,
            manager: SystemManager,
        };
        controller.app(request)
    })();
    let reply = result.unwrap_or_else(|_| {
        smith_service::app::AppReply::from_versioned_result(
            version,
            "00000000000000000000000000000000",
            &call_id,
            Err(smith_service::app::AppError::new(
                "APP_UNAVAILABLE",
                "Application preflight, identity or exchange failed; synchronize before retry",
            )),
        )
    });
    if smith_service::app_stdout_reply(&reply).is_err() || !reply.ok {
        std::process::exit(1);
    }
}
fn production_preflight(euid: u32) -> smith_service::Result<()> {
    if euid == 0 {
        Err("Root execution is forbidden for control and serve".into())
    } else {
        Ok(())
    }
}

fn run(args: &[String]) -> smith_service::Result<()> {
    // SAFETY: geteuid has no arguments or memory access. This must precede
    // root discovery/validation, filesystem access, and any manager command.
    production_preflight(unsafe { libc::geteuid() })?;
    if args[0] == "serve" {
        let roots = if args.len() == 1 {
            Roots::environment()?
        } else if args.len() == 7
            && args[1] == "--data-root"
            && args[3] == "--config-root"
            && args[5] == "--runtime-root"
        {
            Roots {
                data: PathBuf::from(&args[2]),
                config: PathBuf::from(&args[4]),
                runtime: PathBuf::from(&args[6]),
            }
        } else {
            return Err("Invalid foreground service arguments".into());
        };
        return smith_service::serve(&roots);
    }
    let operation = match args {
        [command] if command == "inspect" => ("inspect", None, None),
        [command, flag, action] if command == "prepare" && flag == "--action" => {
            ("prepare", Some(Action::parse(action)?), None)
        }
        [command, flag, action, approval_flag, token]
            if command == "apply"
                && flag == "--action"
                && approval_flag == "--approval"
                && token.len() <= 128 =>
        {
            ("apply", Some(Action::parse(action)?), Some(token.as_str()))
        }
        _ => return Err("Invalid control arguments".into()),
    };
    smith_service::install_control_cancellation()?;
    let controller = Controller {
        roots: Roots::environment()?,
        source: std::env::current_exe().map_err(|_| "Cannot resolve helper identity")?,
        manager: SystemManager,
    };
    let observation = match operation {
        ("inspect", _, _) => controller.inspect(),
        ("prepare", Some(action), _) => controller.prepare(action),
        ("apply", Some(action), Some(token)) => controller.apply(action, token),
        _ => return Err("Invalid control operation".into()),
    };
    println!(
        "{}",
        serde_json::to_string(&observation).map_err(|_| "Cannot encode observation")?
    );
    if !observation.ok {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_preflight_rejects_root_and_accepts_unprivileged_identity() {
        assert!(production_preflight(0)
            .unwrap_err()
            .contains("Root execution is forbidden"));
        assert!(production_preflight(1000).is_ok());
    }
}
