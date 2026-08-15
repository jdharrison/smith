use super::compose::{ensure_ollama_volume, push_compose_service_labels, OLLAMA_VOLUME};
use super::*;
use std::path::PathBuf;
use std::process::Stdio;

/// Container name for the Ollama service.
pub const OLLAMA_CONTAINER_NAME: &str = "smith-ollama";

/// Default port for Ollama API.
pub const OLLAMA_PORT: u16 = 11434;

/// Where Ollama is running for smith.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaBackend {
    /// Native host process (preferred on Windows+AMD).
    Host,
    /// `smith-ollama` container.
    Docker,
}

/// Hostname sealed brain uses to reach Ollama on the Docker network.
pub const OLLAMA_DOCKER_HOSTNAME: &str = OLLAMA_CONTAINER_NAME;

/// Hostname containers use to reach host Ollama (Docker Desktop / Linux host-gateway).
pub const OLLAMA_HOST_GATEWAY: &str = "host.docker.internal";

/// Probe whether anything is serving the Ollama API on localhost.
pub fn is_host_ollama_api_up() -> bool {
    ollama_http_ok("127.0.0.1")
}

fn ollama_http_ok(host: &str) -> bool {
    #[cfg(windows)]
    let null_out = "NUL";
    #[cfg(not(windows))]
    let null_out = "/dev/null";
    Command::new("curl")
        .args([
            "-s",
            "-o",
            null_out,
            "-w",
            "%{http_code}",
            "--max-time",
            "2",
            &format!("http://{}:{}/api/tags", host, OLLAMA_PORT),
        ])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().starts_with('2'))
        .unwrap_or(false)
}

/// True if the smith-ollama container is running.
pub fn is_docker_ollama_running() -> bool {
    let output = Command::new("docker")
        .args(["inspect", OLLAMA_CONTAINER_NAME, "-f", "{{.State.Running}}"])
        .output();

    output
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("true"))
        .unwrap_or(false)
}

/// True if host API or docker container is available.
pub fn is_ollama_running() -> bool {
    is_host_ollama_api_up() || is_docker_ollama_running()
}

/// Which backend is currently serving (host preferred if API is up).
pub fn active_ollama_backend() -> Option<OllamaBackend> {
    if is_host_ollama_api_up() {
        // Host process wins even if an old container also exists.
        Some(OllamaBackend::Host)
    } else if is_docker_ollama_running() {
        Some(OllamaBackend::Docker)
    } else {
        None
    }
}

/// Resolve ollama binary on the host (Windows install paths + PATH).
pub fn find_host_ollama_bin() -> Option<PathBuf> {
    if let Ok(output) = Command::new("where").arg("ollama").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let p = PathBuf::from(line.trim());
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    // Unix `which`
    if let Ok(output) = Command::new("which").arg("ollama").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let p = PathBuf::from(line.trim());
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }

    let candidates = [
        dirs::home_dir().map(|h| h.join("AppData/Local/Programs/Ollama/ollama.exe")),
        std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Programs/Ollama/ollama.exe")),
        std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("Ollama/ollama.exe")),
        Some(PathBuf::from("/usr/local/bin/ollama")),
        Some(PathBuf::from("/usr/bin/ollama")),
    ];
    candidates.into_iter().flatten().find(|c| c.is_file())
}

/// Docker `--gpus` only works with the NVIDIA Container Toolkit — not AMD/ROCm on Desktop.
pub fn docker_nvidia_runtime_available() -> bool {
    let info = Command::new("docker").args(["info"]).output();
    let Ok(out) = info else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let lower = text.to_lowercase();
    // Runtimes / nvidia fields from `docker info`
    lower.contains("nvidia")
        && (lower.contains("runtimes:")
            || lower.contains("nvidia-container")
            || lower.contains("gpu"))
}

/// Best-effort host GPU kind for messaging / backend choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostGpuKind {
    Nvidia,
    Amd,
    Unknown,
}

pub fn detect_host_gpu_kind() -> HostGpuKind {
    #[cfg(windows)]
    {
        let wmi = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_VideoController).Name -join ';'",
            ])
            .output();
        if let Ok(out) = wmi {
            let names = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if names.contains("nvidia") {
                return HostGpuKind::Nvidia;
            }
            if names.contains("amd") || names.contains("radeon") {
                return HostGpuKind::Amd;
            }
        }
    }
    #[cfg(not(windows))]
    {
        use std::path::Path;
        if Path::new("/dev/nvidia0").exists() {
            return HostGpuKind::Nvidia;
        }
        if Path::new("/dev/kfd").exists() || Path::new("/dev/dri").exists() {
            // Could be AMD; treat as AMD-ish if lspci says so
            let lspci = Command::new("sh")
                .args(["-c", "lspci 2>/dev/null | tr '[:upper:]' '[:lower:]'"])
                .output();
            if let Ok(out) = lspci {
                let t = String::from_utf8_lossy(&out.stdout);
                if t.contains("nvidia") {
                    return HostGpuKind::Nvidia;
                }
                if t.contains("amd") || t.contains("advanced micro devices") {
                    return HostGpuKind::Amd;
                }
            }
        }
    }
    HostGpuKind::Unknown
}

/// Prefer host Ollama on Windows/AMD (or whenever NVIDIA Docker GPU is unavailable).
pub fn prefer_host_ollama() -> bool {
    if is_host_ollama_api_up() {
        return true;
    }
    if find_host_ollama_bin().is_none() {
        return false;
    }
    if docker_nvidia_runtime_available() && detect_host_gpu_kind() == HostGpuKind::Nvidia {
        return false;
    }
    // AMD / unknown / no NVIDIA docker runtime → host path when binary exists
    true
}

/// Base URL path hostname for sealed brain → Ollama.
pub fn ollama_hostname_for_brain(backend: OllamaBackend) -> &'static str {
    match backend {
        OllamaBackend::Host => OLLAMA_HOST_GATEWAY,
        OllamaBackend::Docker => OLLAMA_DOCKER_HOSTNAME,
    }
}

/// Smart start: host Ollama (AMD/Windows) when sensible, else Docker (NVIDIA GPUs only if available).
pub fn start_ollama(model: &str) -> Result<(u16, OllamaBackend), String> {
    if let Some(backend) = active_ollama_backend() {
        println!("  Ollama already running ({:?})", backend);
        ensure_ollama_model(model, backend)?;
        return Ok((OLLAMA_PORT, backend));
    }

    if prefer_host_ollama() {
        match start_host_ollama(model) {
            Ok(port) => return Ok((port, OllamaBackend::Host)),
            Err(e) => {
                println!("  Host Ollama start failed ({}); trying Docker...", e);
            }
        }
    } else if detect_host_gpu_kind() == HostGpuKind::Amd {
        println!(
            "  AMD GPU detected: Docker `--gpus` is NVIDIA-only and ROCm passthrough is unreliable on Docker Desktop."
        );
        println!(
            "  Install native Ollama for Windows (https://ollama.com/download) for GPU acceleration, then re-run `smith settings model start`."
        );
    }

    // Docker path: only pass --gpus when NVIDIA runtime exists
    let use_nvidia = docker_nvidia_runtime_available();
    if use_nvidia {
        match start_ollama_container(model, true) {
            Ok(port) => return Ok((port, OllamaBackend::Docker)),
            Err(e) => {
                println!("  NVIDIA GPU start failed, retrying CPU container...");
                println!("  ({})", e.trim());
            }
        }
    } else {
        println!("  Starting Ollama container in CPU mode (no NVIDIA Docker runtime)");
    }

    start_ollama_container(model, false).map(|port| (port, OllamaBackend::Docker))
}

/// Start host `ollama serve` (detached) and ensure model is pulled.
pub fn start_host_ollama(model: &str) -> Result<u16, String> {
    if is_host_ollama_api_up() {
        ensure_ollama_model(model, OllamaBackend::Host)?;
        return Ok(OLLAMA_PORT);
    }

    let bin = find_host_ollama_bin().ok_or_else(|| {
        "Host Ollama not found. Install from https://ollama.com/download (recommended on Windows+AMD)".to_string()
    })?;

    println!("  Starting host Ollama ({})...", bin.display());
    // On Windows, the app often auto-serves; `ollama serve` is still the portable path.
    let mut child = Command::new(&bin);
    child
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        child.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    child
        .spawn()
        .map_err(|e| format!("Failed to spawn host ollama: {}", e))?;

    println!("  Waiting for host Ollama API...");
    for attempt in 1..=45 {
        if is_host_ollama_api_up() {
            println!("  Host Ollama is ready!");
            ensure_ollama_model(model, OllamaBackend::Host)?;
            return Ok(OLLAMA_PORT);
        }
        if attempt < 45 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    Err("Host Ollama started but API not responding on :11434".to_string())
}

/// Start an Ollama container for local model serving.
/// Runs `ollama serve` (detached), waits for the API, then pulls the requested model.
/// Returns the port the Ollama API is available on.
///
/// `gpu` enables Docker `--gpus all` (NVIDIA only). Prefer [`start_ollama`] which detects platform.
pub fn start_ollama_container(model: &str, gpu: bool) -> Result<u16, String> {
    let container_name = OLLAMA_CONTAINER_NAME;

    if is_docker_ollama_running() {
        println!("  Ollama container already running");
        ensure_ollama_model(model, OllamaBackend::Docker)?;
        return Ok(OLLAMA_PORT);
    }

    // Host already serving — do not also bind :11434 in Docker
    if is_host_ollama_api_up() {
        println!(
            "  Host Ollama already on :{}; skipping container",
            OLLAMA_PORT
        );
        ensure_ollama_model(model, OllamaBackend::Host)?;
        return Ok(OLLAMA_PORT);
    }

    // Remove leftover Created/Exited containers so retries can recreate
    let _ = Command::new("docker")
        .args(["rm", "-f", container_name])
        .output();

    ensure_ollama_volume()?;

    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--restart".to_string(),
        "no".to_string(),
        "--name".to_string(),
        container_name.to_string(),
        "-p".to_string(),
        format!("{}:{}", OLLAMA_PORT, OLLAMA_PORT),
        "-v".to_string(),
        format!("{}:/root/.ollama", OLLAMA_VOLUME),
    ];
    push_compose_service_labels(&mut args, "ollama");

    if gpu {
        if !docker_nvidia_runtime_available() {
            return Err(
                "Refusing --gpus all: NVIDIA Container Toolkit not detected (AMD/ROCm is not supported via Docker Desktop --gpus)".to_string(),
            );
        }
        args.push("--gpus".to_string());
        args.push("all".to_string());
    }

    // Serve only — `ollama run` is interactive and fails under `docker run -d`
    args.push("ollama/ollama".to_string());
    args.push("serve".to_string());

    println!("  Starting Ollama container (serve)...");
    if gpu {
        println!("  (with NVIDIA GPU passthrough)");
    } else {
        println!("  (CPU mode)");
    }

    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start Ollama container: {}", e))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let _ = Command::new("docker")
            .args(["rm", "-f", container_name])
            .output();
        return Err(format!("Failed to start Ollama container: {}", error));
    }

    println!("  Waiting for Ollama API...");
    let max_attempts = 45;
    for attempt in 1..=max_attempts {
        if is_host_ollama_api_up() {
            println!("  Ollama is ready!");
            ensure_ollama_model(model, OllamaBackend::Docker)?;
            return Ok(OLLAMA_PORT);
        }

        // Bail early if container died
        if !is_docker_ollama_running() {
            let logs = Command::new("docker")
                .args(["logs", container_name])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stderr).to_string()
                        + &String::from_utf8_lossy(&o.stdout)
                })
                .unwrap_or_default();
            let _ = Command::new("docker")
                .args(["rm", "-f", container_name])
                .output();
            return Err(format!(
                "Ollama container exited while starting. Logs:\n{}",
                logs.trim()
            ));
        }

        if attempt < max_attempts {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    Err("Ollama container started but API not responding".to_string())
}

/// Pull the model if not already present (host CLI or docker exec).
fn ensure_ollama_model(model: &str, backend: OllamaBackend) -> Result<(), String> {
    if model.trim().is_empty() {
        return Ok(());
    }

    println!("  Ensuring model '{}' is available...", model);
    let status = match backend {
        OllamaBackend::Host => {
            let bin = find_host_ollama_bin().ok_or_else(|| {
                "ollama CLI not found on host (API is up but cannot pull)".to_string()
            })?;
            Command::new(bin)
                .args(["pull", model])
                .status()
                .map_err(|e| format!("Failed to pull model '{}': {}", model, e))?
        }
        OllamaBackend::Docker => Command::new("docker")
            .args(["exec", OLLAMA_CONTAINER_NAME, "ollama", "pull", model])
            .status()
            .map_err(|e| format!("Failed to pull model '{}': {}", model, e))?,
    };

    if status.success() {
        println!("  Model '{}' ready", model);
        Ok(())
    } else {
        Err(format!(
            "Failed to pull model '{}' (exit {}). Check network / disk space.",
            model,
            status.code().unwrap_or(-1)
        ))
    }
}

/// Stop the Ollama container (does not kill host Ollama).
pub fn stop_ollama_container() -> Result<(), String> {
    let container_name = OLLAMA_CONTAINER_NAME;

    let inspect = Command::new("docker")
        .args(["inspect", container_name])
        .output();

    if let Ok(output) = inspect {
        if !output.status.success() {
            return Ok(());
        }
    } else {
        return Ok(());
    }

    println!("  Stopping Ollama container...");
    stop_container(container_name)
}

/// Stop docker ollama if present; host Ollama is left running (user-managed).
pub fn stop_ollama() -> Result<(), String> {
    if is_docker_ollama_running() || container_exists_name(OLLAMA_CONTAINER_NAME) {
        stop_ollama_container()?;
    }
    if is_host_ollama_api_up() {
        println!(
            "  Host Ollama still listening on :{} (stop via system tray / `ollama stop` or quit the app)",
            OLLAMA_PORT
        );
    }
    Ok(())
}

fn container_exists_name(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
