//! Native (host isolation) control-plane runtime: OpenCode on the metal + host Ollama.
//!
//! Coding / pipeline / search workers stay in Docker. This module is chat/think only.

use crate::docker::{
    is_host_ollama_api_up, BrainPromptOpts, BrainPromptResult, BRAIN_CHAT_AGENT, BRAIN_THINK_AGENT,
    OLLAMA_PORT,
};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const HOST_CONFIG_NAME: &str = "opencode.json";

pub fn ensure_host_brain_ready(memory_host_path: &Path, model: Option<&str>) -> Result<(), String> {
    if !is_host_ollama_api_up() {
        return Err(
            "Host Ollama is not running at 127.0.0.1:11434. Start it (`smith settings model start` or native Ollama), then retry."
                .to_string(),
        );
    }
    let _ = resolve_opencode_bin()?;
    let model_name = model.unwrap_or(crate::DEFAULT_LOCAL_MODEL);
    write_host_opencode_config(memory_host_path, model_name)?;
    Ok(())
}

/// Resolve a runnable OpenCode binary for host runtime.
///
/// On Windows, Rust `Command::new("opencode")` does not execute npm `.cmd` shims, so we
/// prefer a real `.exe` (npm package bin, LocalAppData install, or `OPENCODE_BIN`).
fn resolve_opencode_bin() -> Result<PathBuf, String> {
    if let Ok(explicit) = std::env::var("OPENCODE_BIN") {
        let p = PathBuf::from(explicit.trim());
        if p.is_file() {
            return Ok(p);
        }
        return Err(format!(
            "OPENCODE_BIN is set but not a file: {}",
            p.display()
        ));
    }

    // Direct name — works on Unix and when a real opencode.exe is on PATH.
    if probe_opencode_cmd(Path::new("opencode")) {
        return Ok(PathBuf::from("opencode"));
    }

    for candidate in candidate_opencode_bins() {
        if candidate.is_file() && probe_opencode_cmd(&candidate) {
            return Ok(candidate);
        }
    }

    Err(
        "opencode is not runnable from smith (Windows often needs the real .exe, not an npm .cmd shim).\n\
         Fix one of:\n\
         - set OPENCODE_BIN to the full path of opencode.exe\n\
         - npm i -g opencode-ai  (then retry)\n\
         - or switch back: smith settings runtime set container\n\
         Note: `smith install` starts the Docker brain; it does not install host OpenCode."
            .to_string(),
    )
}

fn probe_opencode_cmd(bin: &Path) -> bool {
    let mut cmd = Command::new(bin);
    cmd.arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.status()
        .map(|st| st.success())
        .unwrap_or(false)
}

fn candidate_opencode_bins() -> Vec<PathBuf> {
    let mut out = Vec::new();

    // `where opencode` / PATH entries that are .exe
    let mut where_cmd = Command::new("where");
    where_cmd.arg("opencode")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        where_cmd.creation_flags(CREATE_NO_WINDOW);
    }

    if let Ok(output) = where_cmd.output() {
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let p = PathBuf::from(line.trim());
                if p.extension().and_then(|e| e.to_str()) == Some("exe") {
                    out.push(p);
                }
            }
        }
    }

    if let Some(npm) = std::env::var_os("APPDATA").map(PathBuf::from) {
        out.push(
            npm.join("npm")
                .join("node_modules")
                .join("opencode-ai")
                .join("bin")
                .join("opencode.exe"),
        );
    }
    if let Ok(prefix) = std::env::var("npm_config_prefix") {
        out.push(
            PathBuf::from(prefix)
                .join("node_modules")
                .join("opencode-ai")
                .join("bin")
                .join("opencode.exe"),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        out.push(local.join("opencode").join("opencode-cli.exe"));
        out.push(local.join("opencode").join("opencode.exe"));
    }
    if let Some(home) = dirs::home_dir() {
        out.push(
            home.join("AppData")
                .join("Roaming")
                .join("npm")
                .join("node_modules")
                .join("opencode-ai")
                .join("bin")
                .join("opencode.exe"),
        );
    }

    out
}

fn write_host_opencode_config(memory_host_path: &Path, model: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(memory_host_path)
        .map_err(|e| format!("Failed to create memory dir: {}", e))?;
    let path = memory_host_path.join(HOST_CONFIG_NAME);
    let body = format!(
        r#"{{
  "$schema": "https://opencode.ai/config.json",
  "model": "ollama/{model}",
  "default_agent": "chat",
  "share": "disabled",
  "autoupdate": false,
  "agent": {{
    "chat": {{
      "description": "Host native conversational brain",
      "mode": "primary",
      "prompt": "You are smith — skeptical, precise, reality-checking. Host runtime: the user contains this machine. Lead with substance — do not open by echoing the user. Close with a natural Review check-in when useful. Prefer :::smith intents for files/search/code/pipeline; never claim async work finished early. No Chat|Dev mode. You cannot browse or touch the disk yourself; emit :::smith intents for files (and search when live facts are needed — oneshots under network=private). For files, query is a filename/glob like portrait* or *.jpg — never a Windows absolute path with backslashes. Speak only to the user outside the block. No tools.",
      "permission": {{
        "*": "deny"
      }}
    }},
    "think": {{
      "description": "Host native solve loop with local memory writes",
      "mode": "primary",
      "prompt": "You are smith running a solve loop on the host. Advance with substance; do not open by restating the goal. Update files under the memory directory using local tools only. Respect network policy; do not treat cloud APIs as the default.",
      "permission": {{
        "*": "deny",
        "bash": "allow",
        "edit": "allow",
        "read": "allow",
        "glob": "allow",
        "grep": "allow",
        "list": "allow"
      }}
    }}
  }},
  "provider": {{
    "ollama": {{
      "npm": "@ai-sdk/openai-compatible",
      "name": "Ollama (host)",
      "options": {{
        "baseURL": "http://127.0.0.1:{port}/v1",
        "apiKey": "ollama"
      }},
      "models": {{
        "{model}": {{
          "name": "{model}"
        }}
      }}
    }}
  }}
}}
"#,
        model = model,
        port = OLLAMA_PORT
    );
    fs::write(&path, body).map_err(|e| format!("Failed to write host opencode config: {}", e))?;
    Ok(path)
}

fn host_model_ref(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("ollama/{}", model)
    }
}

pub fn run_prompt_on_host(
    memory_host_path: &Path,
    prompt: &str,
    opts: BrainPromptOpts<'_>,
) -> Result<BrainPromptResult, String> {
    ensure_host_brain_ready(memory_host_path, opts.model)?;

    let mem = memory_host_path
        .canonicalize()
        .unwrap_or_else(|_| memory_host_path.to_path_buf());
    let config_path = mem.join(HOST_CONFIG_NAME);

    let agent_name = if opts.agent.trim().is_empty() {
        BRAIN_CHAT_AGENT
    } else {
        opts.agent.trim()
    };
    let title = if agent_name == BRAIN_THINK_AGENT {
        "smith-think"
    } else {
        "smith-chat"
    };

    let mut args = vec![
        "run".to_string(),
        "--dir".to_string(),
        mem.to_string_lossy().to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--agent".to_string(),
        agent_name.to_string(),
    ];

    let session_id = opts
        .session_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.starts_with("ses_"));
    if let Some(id) = session_id {
        args.push("--session".to_string());
        args.push(id.to_string());
    } else if opts.continue_last {
        args.push("--continue".to_string());
    } else {
        args.push("--title".to_string());
        args.push(title.to_string());
    }

    if let Some(m) = opts.model {
        if !m.trim().is_empty() {
            args.push("-m".to_string());
            args.push(host_model_ref(m));
        }
    }
    args.push(prompt.to_string());

    let opencode_bin = resolve_opencode_bin()?;
    let mut cmd = Command::new(&opencode_bin);
    cmd.args(&args)
        .current_dir(&mem)
        .env("OPENCODE_CONFIG", config_path.as_os_str())
        .env(
            "OPENCODE_MODEL",
            host_model_ref(opts.model.unwrap_or(crate::DEFAULT_LOCAL_MODEL)),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to start host opencode ({}): {}",
                opencode_bin.display(),
                e
            )
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture host opencode stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture host opencode stderr".to_string())?;

    let mut rendered_answer = String::new();
    let mut fallback_stdout = String::new();
    let mut error_context = String::new();
    let mut discovered_session: Option<String> = session_id.map(str::to_string);

    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if discovered_session.is_none() {
                discovered_session = extract_session_id(&value);
            }
            let mut parts = Vec::new();
            crate::docker::collect_text_parts(&value, &mut parts);
            for part in parts {
                rendered_answer.push_str(&part);
            }
        } else {
            // Ignore non-JSON noise (protocol crumbs); only keep as last-resort fallback
            // if we never get typed text events.
            if looks_like_opencode_protocol_noise(&line) {
                continue;
            }
            if !fallback_stdout.is_empty() {
                fallback_stdout.push('\n');
            }
            fallback_stdout.push_str(&line);
        }
    }

    let err_reader = BufReader::new(stderr);
    for line in err_reader.lines().map_while(Result::ok) {
        if !error_context.is_empty() {
            error_context.push('\n');
        }
        error_context.push_str(&line);
    }

    let status = child
        .wait()
        .map_err(|e| format!("wait host opencode failed: {}", e))?;

    let answer = if !rendered_answer.trim().is_empty() {
        rendered_answer
    } else {
        fallback_stdout
    };

    if !status.success() && answer.trim().is_empty() {
        return Err(if !error_context.is_empty() {
            error_context
        } else {
            format!("host opencode exited with {}", status)
        });
    }
    if answer.trim().is_empty() {
        return Err(if !error_context.is_empty() {
            error_context
        } else {
            "Host brain returned empty response".to_string()
        });
    }
    Ok(BrainPromptResult {
        answer,
        session_id: discovered_session,
    })
}

fn extract_session_id(value: &Value) -> Option<String> {
    let keys = ["sessionID", "sessionId", "session_id", "id"];
    if let Some(obj) = value.as_object() {
        for k in keys {
            if let Some(s) = obj.get(k).and_then(Value::as_str) {
                if s.starts_with("ses_") {
                    return Some(s.to_string());
                }
            }
        }
        if let Some(session) = obj.get("session") {
            return extract_session_id(session);
        }
    }
    None
}

fn looks_like_opencode_protocol_noise(line: &str) -> bool {
    let t = line.trim();
    t.contains("step-start")
        || t.contains("step-finish")
        || t.contains("step_start")
        || t.contains("step_finish")
        || t.starts_with("prt_")
        || t.starts_with("msg_")
        || (t.contains("prt_") && t.contains("ses_"))
}
