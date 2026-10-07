//! Always-on coding agent + long-lived demo sandbox with published app ports.
//!
//! Scratch files live under `<data_dir>/sandboxes/scratch`. The coding agent
//! (`smith-agent-coding`) edits them; `smith-sandbox` serves them on a host port.

use super::compose::push_compose_service_labels;
use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

pub const CODING_AGENT_NAME: &str = "coding";
pub const CODING_AGENT_PORT: u16 = 4098;
pub const SANDBOX_CONTAINER_NAME: &str = "smith-sandbox";
pub const SANDBOX_IMAGE: &str = "node:22-bookworm-slim";
pub const DEFAULT_SANDBOX_PORT: u16 = 8080;
const SCRATCH_WORKSPACE_NAME: &str = "scratch";

#[derive(Debug, Clone)]
pub struct SandboxStatus {
    pub scratch_path: String,
    pub coding_configured: bool,
    pub coding_running: bool,
    pub sandbox_running: bool,
    pub published_port: Option<u16>,
}

/// Host path for the shared scratch workspace.
pub fn scratch_dir() -> Result<PathBuf, String> {
    let dirs = directories::ProjectDirs::from("com", "agent", "smith")
        .ok_or_else(|| "Could not determine data directory for smith".to_string())?;
    Ok(dirs.data_dir().join("sandboxes").join("scratch"))
}

pub fn ensure_scratch_dir() -> Result<PathBuf, String> {
    let dir = scratch_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create scratch dir: {}", e))?;
    Ok(dir)
}

/// Ensure `scratch` workspace + local `coding` agent exist in config (does not start containers).
pub fn ensure_coding_config() -> Result<(), String> {
    let scratch = ensure_scratch_dir()?;
    let scratch_str = scratch.to_string_lossy().to_string();
    let mut cfg = crate::load_config()?;
    let mut dirty = false;

    if !cfg
        .workspaces
        .iter()
        .any(|w| w.name == SCRATCH_WORKSPACE_NAME)
    {
        cfg.workspaces.push(crate::WorkspaceConfig {
            name: SCRATCH_WORKSPACE_NAME.to_string(),
            path: scratch_str.clone(),
            access: Some("readwrite".to_string()),
        });
        dirty = true;
    } else if let Some(ws) = cfg
        .workspaces
        .iter_mut()
        .find(|w| w.name == SCRATCH_WORKSPACE_NAME)
    {
        if ws.path != scratch_str {
            ws.path = scratch_str.clone();
            dirty = true;
        }
    }

    let has_coding = cfg
        .agents
        .as_ref()
        .is_some_and(|a| a.iter().any(|e| e.name == CODING_AGENT_NAME));
    if !has_coding {
        let model = resolve_local_model_name();
        crate::add_agent_to_config(
            &mut cfg,
            CODING_AGENT_NAME.to_string(),
            Some(crate::DEFAULT_AGENT_IMAGE.to_string()),
            Some("local".to_string()),
            Some(model),
            None,
            Some("ollama".to_string()),
            Some(format!("http://host.docker.internal:{}", OLLAMA_PORT)),
            Some(CODING_AGENT_PORT),
            Some(true),
            None,
            None,
            None,
            Some(
                "You are smith's local coding worker. Prefer editing /workspaces/scratch for ad-hoc demos. Keep apps listening on 0.0.0.0."
                    .to_string(),
            ),
            None,
        )?;
        if let Some(entry) = cfg
            .agents
            .as_mut()
            .and_then(|a| a.iter_mut().find(|e| e.name == CODING_AGENT_NAME))
        {
            entry.workspaces = Some(vec![SCRATCH_WORKSPACE_NAME.to_string()]);
        }
        dirty = true;
    } else if let Some(entry) = cfg
        .agents
        .as_mut()
        .and_then(|a| a.iter_mut().find(|e| e.name == CODING_AGENT_NAME))
    {
        let mut ws = entry.workspaces.clone().unwrap_or_default();
        if !ws.iter().any(|n| n == SCRATCH_WORKSPACE_NAME) {
            ws.push(SCRATCH_WORKSPACE_NAME.to_string());
            entry.workspaces = Some(ws);
            dirty = true;
        }
        if entry.agent_type.as_deref() != Some("local") {
            entry.agent_type = Some("local".to_string());
            entry.provider = Some("ollama".to_string());
            entry.base_url = Some(format!("http://host.docker.internal:{}", OLLAMA_PORT));
            dirty = true;
        }
    }

    if dirty {
        crate::save_config(&cfg)?;
    }
    Ok(())
}

fn resolve_local_model_name() -> String {
    if let Ok(cfg) = crate::load_config() {
        if let Some(agents) = cfg.agents.as_ref() {
            // Prefer the coding agent when it is local-typed.
            if let Some(coding) = agents.iter().find(|a| a.name == CODING_AGENT_NAME) {
                let is_local = coding
                    .agent_type
                    .as_deref()
                    .map(|t| t == "local")
                    .unwrap_or(false);
                if is_local {
                    if let Some(ref m) = coding.model {
                        if !m.trim().is_empty() {
                            return m.clone();
                        }
                    }
                }
            }
            for a in agents {
                let is_local = a
                    .agent_type
                    .as_deref()
                    .map(|t| t == "local")
                    .unwrap_or(false);
                if is_local {
                    if let Some(ref m) = a.model {
                        if !m.trim().is_empty() {
                            return m.clone();
                        }
                    }
                }
            }
        }
    }
    crate::DEFAULT_LOCAL_MODEL.to_string()
}

/// Ensure config + start coding agent when Docker/Ollama allow it (best-effort).
pub fn ensure_coding_agent_ready() -> Result<(), String> {
    ensure_coding_config()?;
    check_docker_available()?;

    let name = agent_container_name(CODING_AGENT_NAME);
    if container_running(&name) {
        return Ok(());
    }

    if !is_ollama_running() {
        return Err(
            "Ollama is not running. Start with `smith settings model start`, then retry.".into(),
        );
    }

    let cfg = crate::load_config()?;
    let entry = cfg
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find(|e| e.name == CODING_AGENT_NAME)
        .cloned()
        .ok_or_else(|| "coding agent missing from config after ensure".to_string())?;

    let port = entry.port.unwrap_or(CODING_AGENT_PORT);
    let image = if image_exists(&agent_built_image_tag(&entry.name)).unwrap_or(false) {
        agent_built_image_tag(&entry.name)
    } else {
        entry.image.clone()
    };

    let mounts = crate::resolve_container_mounts(
        &cfg,
        entry.projects.as_deref(),
        entry.workspaces.as_deref(),
        None,
    )?;
    let config_dir = crate::write_named_agent_runtime_config(&entry, &mounts)?;

    start_agent_container(
        CODING_AGENT_NAME,
        &image,
        port,
        Some("ollama"),
        Some(&format!("http://host.docker.internal:{}", OLLAMA_PORT)),
        Some(&config_dir),
        &mounts,
    )?;
    println!(
        "  coding agent started (port {} http://127.0.0.1:{})",
        port, port
    );
    Ok(())
}

fn container_running(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name, "-f", "{{.State.Running}}"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("true"))
        .unwrap_or(false)
}

fn container_exists(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn sandbox_published_port() -> Option<u16> {
    let output = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} {{end}}",
            SANDBOX_CONTAINER_NAME,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // e.g. "8080/tcp "
    text.split_whitespace()
        .find_map(|tok| tok.strip_suffix("/tcp").and_then(|p| p.parse::<u16>().ok()))
}

/// Ensure sandbox container is running with the requested host port published.
pub fn ensure_sandbox_container(port: u16) -> Result<(), String> {
    check_docker_available()?;
    let scratch = ensure_scratch_dir()?;
    let scratch_mount = docker_host_path(&scratch);

    if container_running(SANDBOX_CONTAINER_NAME) {
        if sandbox_published_port() == Some(port) {
            return Ok(());
        }
        println!(
            "  Recreating '{}' for port {}…",
            SANDBOX_CONTAINER_NAME, port
        );
        let _ = Command::new("docker")
            .args(["rm", "-f", SANDBOX_CONTAINER_NAME])
            .output();
    } else if container_exists(SANDBOX_CONTAINER_NAME) {
        let _ = Command::new("docker")
            .args(["rm", "-f", SANDBOX_CONTAINER_NAME])
            .output();
    }

    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--restart".to_string(),
        "no".to_string(),
        "--name".to_string(),
        SANDBOX_CONTAINER_NAME.to_string(),
        "-p".to_string(),
        format!("{}:{}", port, port),
        "-v".to_string(),
        format!("{}:/app:rw", scratch_mount),
        "-w".to_string(),
        "/app".to_string(),
    ];
    push_compose_service_labels(&mut args, crate::service::LABEL_SANDBOX_DEFAULT);
    args.extend([
        SANDBOX_IMAGE.to_string(),
        "sleep".to_string(),
        "infinity".to_string(),
    ]);

    println!(
        "  Starting sandbox '{}' (port {}, image {})…",
        SANDBOX_CONTAINER_NAME, port, SANDBOX_IMAGE
    );
    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start sandbox: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "Failed to start sandbox: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub fn stop_sandbox() -> Result<(), String> {
    if !container_exists(SANDBOX_CONTAINER_NAME) {
        println!("Sandbox is not running.");
        return Ok(());
    }
    let output = Command::new("docker")
        .args(["rm", "-f", SANDBOX_CONTAINER_NAME])
        .output()
        .map_err(|e| format!("Failed to stop sandbox: {}", e))?;
    if output.status.success() {
        println!("Sandbox stopped (scratch files kept on host).");
        Ok(())
    } else {
        Err(format!(
            "Failed to stop sandbox: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub fn sandbox_status() -> Result<SandboxStatus, String> {
    let scratch = ensure_scratch_dir()?;
    let cfg = crate::load_config().unwrap_or_default();
    let coding_configured = cfg
        .agents
        .as_ref()
        .is_some_and(|a| a.iter().any(|e| e.name == CODING_AGENT_NAME));
    let coding_running = container_running(&agent_container_name(CODING_AGENT_NAME));
    let sandbox_running = container_running(SANDBOX_CONTAINER_NAME);
    Ok(SandboxStatus {
        scratch_path: scratch.display().to_string(),
        coding_configured,
        coding_running,
        sandbox_running,
        published_port: if sandbox_running {
            sandbox_published_port()
        } else {
            None
        },
    })
}

/// Build (via coding agent) + host on `port`. Returns declassified summary for the brain.
pub fn run_code_and_host(brief: &str, port: u16) -> Result<String, String> {
    let brief = brief.trim();
    if brief.is_empty() {
        return Err("Code brief is empty".to_string());
    }
    let port = if port == 0 {
        DEFAULT_SANDBOX_PORT
    } else {
        port
    };

    ensure_coding_config()?;
    let scratch = ensure_scratch_dir()?;

    // Best-effort coding agent; coding can still run via one-shot OpenCode image.
    match ensure_coding_agent_ready() {
        Ok(()) => {}
        Err(e) => println!("  Warning: coding agent not started ({e}); using one-shot coder."),
    }

    ensure_sandbox_container(port)?;

    println!("  Coding in scratch…");
    let code_log = run_coder_on_scratch(brief, port)?;

    if !scratch_has_servable_content(&scratch) {
        write_fallback_hello(&scratch, port, brief)?;
    }

    println!("  Starting server on port {port}…");
    start_serve_process(port)?;

    let url = format!("http://127.0.0.1:{port}");
    let reachable = probe_url(&url, 12);
    let files = list_scratch_files(&scratch);

    Ok(format!(
        "Scratch: {scratch}\nURL: {url}\nReachable: {reachable}\nFiles:\n{files}\nCoder log (truncated):\n{code_log}\nStop with: smith settings sandbox stop",
        scratch = scratch.display(),
        url = url,
        reachable = if reachable { "yes" } else { "not yet (may still be starting)" },
        files = files,
        code_log = truncate(&code_log, 2500),
    ))
}

fn scratch_has_servable_content(scratch: &Path) -> bool {
    scratch.join("index.html").is_file()
        || scratch.join("server.js").is_file()
        || scratch.join("index.js").is_file()
        || scratch.join("package.json").is_file()
}

fn write_fallback_hello(scratch: &Path, port: u16, brief: &str) -> Result<(), String> {
    let _ = port;
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>smith scratch</title>
</head>
<body>
  <h1>Hello World</h1>
  <p>Generated by smith for: {}</p>
</body>
</html>
"#,
        html_escape(brief)
    );
    fs::write(scratch.join("index.html"), html)
        .map_err(|e| format!("Failed to write fallback index.html: {}", e))?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

fn run_coder_on_scratch(brief: &str, port: u16) -> Result<String, String> {
    let model = resolve_local_model_name();
    let model_ref = if model.contains('/') {
        model.clone()
    } else {
        format!("ollama/{}", model)
    };
    let prompt = format!(
        "Implement this under the current directory only (already /workspaces/scratch or /app).\n\
         Task: {brief}\n\
         Requirements:\n\
         - Create an index.html with Hello World (or equivalent) if this is a simple web demo.\n\
         - Prefer a static index.html that can be served as-is.\n\
         - If you write a Node server, listen on 0.0.0.0:{port} (not localhost only).\n\
         - Do not leave the process running; just write files.\n\
         - No cloud APIs. Keep it minimal.",
        brief = brief,
        port = port
    );

    let coding_name = agent_container_name(CODING_AGENT_NAME);
    if container_running(&coding_name) {
        let args = [
            "exec",
            coding_name.as_str(),
            "opencode",
            "run",
            "--dir",
            "/workspaces/scratch",
            "--format",
            "json",
            "-m",
            model_ref.as_str(),
            "--title",
            "smith-code",
            prompt.as_str(),
        ];
        let output = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| format!("Failed to exec coding agent: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.success() || scratch_has_servable_content(&scratch_dir()?) {
            return Ok(if stdout.trim().is_empty() {
                stderr
            } else {
                stdout
            });
        }
        println!("  coding agent run failed; trying one-shot OpenCode…");
    }

    let scratch = ensure_scratch_dir()?;
    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--add-host".to_string(),
        "host.docker.internal:host-gateway".to_string(),
        "-e".to_string(),
        format!(
            "OPENCODE_BASE_URL=http://host.docker.internal:{}",
            OLLAMA_PORT
        ),
        "-v".to_string(),
        format!("{}:/workspaces/scratch:rw", docker_host_path(&scratch)),
    ];
    push_compose_service_labels(&mut args, "agents-code-oneshot");
    args.extend([
        "--entrypoint".to_string(),
        "opencode".to_string(),
        crate::DEFAULT_AGENT_IMAGE.to_string(),
        "run".to_string(),
        "--dir".to_string(),
        "/workspaces/scratch".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "-m".to_string(),
        model_ref,
        "--title".to_string(),
        "smith-code".to_string(),
        prompt,
    ]);

    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run one-shot coder: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(if stdout.trim().is_empty() {
        stderr
    } else {
        format!("{}\n{}", stdout, stderr)
    })
}

fn start_serve_process(port: u16) -> Result<(), String> {
    let _ = Command::new("docker")
        .args([
            "exec",
            SANDBOX_CONTAINER_NAME,
            "sh",
            "-c",
            "pkill -f 'npx|serve|node server|node index' 2>/dev/null || true",
        ])
        .output();

    let scratch = ensure_scratch_dir()?;
    let inner = if scratch.join("server.js").is_file() {
        "node server.js".to_string()
    } else if scratch.join("index.js").is_file() && !scratch.join("index.html").is_file() {
        "node index.js".to_string()
    } else if scratch.join("package.json").is_file() {
        format!(
            "npm install --silent >/tmp/npm.log 2>&1; npx --yes serve -l {} .",
            port
        )
    } else {
        format!("npx --yes serve -l {} .", port)
    };

    let script = format!("nohup {inner} > /tmp/smith-serve.log 2>&1 &");
    let output = Command::new("docker")
        .args(["exec", "-d", SANDBOX_CONTAINER_NAME, "sh", "-c", &script])
        .output()
        .map_err(|e| format!("Failed to start serve: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "Failed to start serve: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn probe_url(url: &str, attempts: u32) -> bool {
    for _ in 0..attempts {
        let ok = Command::new("curl")
            .args(["-sf", "--max-time", "2", url])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
        // Fallback: PowerShell on Windows if curl missing/failing
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!("try {{ (Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 '{url}').StatusCode -eq 200 }} catch {{ $false }}"),
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            ps_cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let ps = ps_cmd
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("True"))
            .unwrap_or(false);
        if ps {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

fn list_scratch_files(scratch: &Path) -> String {
    let mut names: Vec<String> = fs::read_dir(scratch)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            if p.is_file() {
                Some(format!("- {}", name))
            } else if p.is_dir() {
                Some(format!("- {}/", name))
            } else {
                None
            }
        })
        .collect();
    names.sort();
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join("\n")
    }
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let cut: String = t.chars().take(max).collect();
        format!("{cut}…")
    }
}
