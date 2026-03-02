use super::*;

/// Container name for the Ollama service.
pub const OLLAMA_CONTAINER_NAME: &str = "smith-ollama";

/// Default port for Ollama API.
pub const OLLAMA_PORT: u16 = 11434;

/// Start an Ollama container for local model serving.
/// Returns the port the Ollama API is available on.
pub fn start_ollama_container(model: &str, gpu: bool) -> Result<u16, String> {
    let container_name = OLLAMA_CONTAINER_NAME;

    // Check if already running
    let inspect = Command::new("docker")
        .args(["inspect", container_name])
        .output();

    if let Ok(output) = inspect {
        if output.status.success() {
            println!("  Ollama container already running");
            return Ok(OLLAMA_PORT);
        }
    }

    // Build docker run command
    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--name".to_string(),
        container_name.to_string(),
        "-p".to_string(),
        format!("{}:{}", OLLAMA_PORT, OLLAMA_PORT),
        "-v".to_string(),
        "smith-ollama:/root/.ollama".to_string(),
    ];

    if gpu {
        args.push("--gpus".to_string());
        args.push("all".to_string());
    }

    args.push("ollama/ollama".to_string());
    args.push("run".to_string());
    args.push(model.to_string());

    println!("  Starting Ollama container with model '{}'...", model);
    if gpu {
        print!("  (with GPU passthrough)");
    }
    println!();

    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start Ollama container: {}", e))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to start Ollama container: {}", error));
    }

    // Wait for Ollama to be ready
    println!("  Waiting for Ollama to be ready...");
    let max_attempts = 30;
    for attempt in 1..=max_attempts {
        let curl = Command::new("curl")
            .args(["-s", &format!("http://localhost:{}/api/tags", OLLAMA_PORT)])
            .output();

        if let Ok(out) = curl {
            if out.status.success() {
                println!("  Ollama is ready!");
                return Ok(OLLAMA_PORT);
            }
        }

        if attempt < max_attempts {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    Err("Ollama container started but API not responding".to_string())
}

/// Stop the Ollama container.
pub fn stop_ollama_container() -> Result<(), String> {
    let container_name = OLLAMA_CONTAINER_NAME;

    // Check if exists
    let inspect = Command::new("docker")
        .args(["inspect", container_name])
        .output();

    if let Ok(output) = inspect {
        if !output.status.success() {
            // Container doesn't exist, nothing to stop
            return Ok(());
        }
    } else {
        return Ok(());
    }

    println!("  Stopping Ollama container...");
    stop_container(container_name)
}

/// Check if Ollama container is running.
pub fn is_ollama_running() -> bool {
    let output = Command::new("docker")
        .args(["inspect", OLLAMA_CONTAINER_NAME, "-f", "{{.State.Running}}"])
        .output();

    output
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("true"))
        .unwrap_or(false)
}
