//! Native (host isolation) control-plane runtime: OpenCode on the metal + host Ollama.
//!
//! Coding / pipeline / search workers stay in Docker. This module is chat/think only.

use crate::docker::{
    is_host_ollama_api_up, BrainPromptOpts, BrainPromptResult, BRAIN_CHAT_AGENT, BRAIN_THINK_AGENT,
    OLLAMA_PORT,
};
use serde_json::Value;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const HOST_CONFIG_NAME: &str = "opencode.json";
const OPENCODE_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const OPENCODE_PROBE_REAP_TIMEOUT: Duration = Duration::from_secs(1);
const OPENCODE_VERSION_OUTPUT_LIMIT: u64 = 4096;
const OPENCODE_HELP_OUTPUT_LIMIT: u64 = 16 * 1024;
static NEXT_PROBE_CAPTURE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostPreflightError {
    PermanentConfiguration(String),
    Transient(String),
}

impl HostPreflightError {
    fn with_context(self, context: impl AsRef<str>) -> Self {
        let message = format!("{}: {}", context.as_ref(), self);
        match self {
            Self::PermanentConfiguration(_) => Self::PermanentConfiguration(message),
            Self::Transient(_) => Self::Transient(message),
        }
    }
}

impl std::fmt::Display for HostPreflightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermanentConfiguration(message) | Self::Transient(message) => {
                formatter.write_str(message)
            }
        }
    }
}

/// Verify the static host-runtime dependency before startup performs warm-service work.
pub(crate) fn preflight_host_brain() -> Result<(), HostPreflightError> {
    resolve_opencode_bin().map(|_| ())
}

pub fn ensure_host_brain_ready(memory_host_path: &Path, model: Option<&str>) -> Result<(), String> {
    preflight_host_brain().map_err(|error| error.to_string())?;
    if !is_host_ollama_api_up() {
        return Err(
            "Host Ollama is not running at 127.0.0.1:11434. Start it (`smith settings model start` or native Ollama), then retry."
                .to_string(),
        );
    }
    let model_name = model.unwrap_or(crate::DEFAULT_LOCAL_MODEL);
    write_host_opencode_config(memory_host_path, model_name)?;
    Ok(())
}

/// Resolve a runnable OpenCode binary for host runtime.
///
/// User services do not inherit interactive-shell initialization. Resolve an absolute executable
/// from explicit configuration, the actual service `PATH`, or documented stable user locations.
fn resolve_opencode_bin() -> Result<PathBuf, HostPreflightError> {
    let explicit = std::env::var_os("OPENCODE_BIN");
    let path = std::env::var_os("PATH");
    let home = dirs::home_dir();
    resolve_opencode_bin_with(
        explicit.as_deref(),
        path.as_deref(),
        home.as_deref(),
        probe_opencode_cmd,
    )
}

fn resolve_opencode_bin_with(
    explicit: Option<&OsStr>,
    path: Option<&OsStr>,
    home: Option<&Path>,
    mut probe: impl FnMut(&Path) -> Result<(), HostPreflightError>,
) -> Result<PathBuf, HostPreflightError> {
    if let Some(explicit) = explicit {
        let candidate = PathBuf::from(explicit);
        if candidate.as_os_str().is_empty() || !candidate.is_absolute() {
            return Err(HostPreflightError::PermanentConfiguration(
                "OPENCODE_BIN must be a non-empty absolute path to a runnable OpenCode executable"
                    .into(),
            ));
        }
        return validate_opencode_candidate(&candidate, &mut probe).map_err(|error| {
            error.with_context(format!(
                "invalid OPENCODE_BIN for Smith host runtime; {}",
                opencode_remediation()
            ))
        });
    }

    let mut first_permanent = None;
    let mut first_transient = None;
    let mut candidates = Vec::new();
    if let Some(path) = path {
        candidates.extend(
            std::env::split_paths(path)
                .filter(|entry| entry.is_absolute())
                .map(|directory| directory.join(opencode_executable_name())),
        );
    }
    if let Some(home) = home.filter(|path| path.is_absolute()) {
        candidates.extend([
            home.join(".opencode/bin").join(opencode_executable_name()),
            home.join(".local/bin").join(opencode_executable_name()),
        ]);
    }

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        match validate_opencode_candidate(&candidate, &mut probe) {
            Ok(candidate) => return Ok(candidate),
            Err(error @ HostPreflightError::PermanentConfiguration(_)) => {
                first_permanent.get_or_insert(error);
            }
            Err(error @ HostPreflightError::Transient(_)) => {
                first_transient.get_or_insert(error);
            }
        }
    }

    if let Some(error) = first_transient {
        return Err(error.with_context(
            "OpenCode discovery encountered a retryable probe failure in Smith's non-interactive service environment",
        ));
    }
    if let Some(error) = first_permanent {
        return Err(error.with_context(format!(
            "OpenCode candidates were invalid in Smith's non-interactive service environment; {}",
            opencode_remediation()
        )));
    }

    Err(HostPreflightError::PermanentConfiguration(format!(
        "OpenCode is not runnable in Smith's non-interactive service environment. {}",
        opencode_remediation()
    )))
}

fn validate_opencode_candidate(
    candidate: &Path,
    probe: &mut dyn FnMut(&Path) -> Result<(), HostPreflightError>,
) -> Result<PathBuf, HostPreflightError> {
    if !candidate.is_absolute() {
        return Err(HostPreflightError::PermanentConfiguration(
            "candidate path is not absolute".into(),
        ));
    }
    let canonical = candidate.canonicalize().map_err(|error| {
        HostPreflightError::PermanentConfiguration(format!(
            "cannot resolve '{}': {error}",
            candidate.display()
        ))
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        HostPreflightError::PermanentConfiguration(format!(
            "cannot inspect '{}': {error}",
            canonical.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(HostPreflightError::PermanentConfiguration(format!(
            "'{}' is not a regular file",
            canonical.display()
        )));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(HostPreflightError::PermanentConfiguration(format!(
            "'{}' is not executable",
            canonical.display()
        )));
    }
    probe(&canonical)?;
    Ok(canonical)
}

fn opencode_executable_name() -> &'static str {
    #[cfg(windows)]
    {
        "opencode.exe"
    }
    #[cfg(not(windows))]
    {
        "opencode"
    }
}

fn opencode_remediation() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "On Ubuntu, install OpenCode at ~/.opencode/bin/opencode or ~/.local/bin/opencode, or set OPENCODE_BIN to a stable absolute executable path in the service environment; verify it with `opencode --version`, then explicitly retry. To avoid host OpenCode, select `smith settings runtime set container`."
    }
    #[cfg(not(target_os = "linux"))]
    {
        "Configure OPENCODE_BIN as a stable absolute executable path, verify it with `opencode --version`, then explicitly retry."
    }
}

struct ProbeCapture {
    path: PathBuf,
}

impl ProbeCapture {
    fn create(label: &str) -> Result<(Self, File), HostPreflightError> {
        let sequence = NEXT_PROBE_CAPTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "smith-opencode-probe-{}-{sequence}-{label}.log",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&path).map_err(|error| {
            HostPreflightError::Transient(format!(
                "cannot create bounded OpenCode {label} capture: {error}"
            ))
        })?;
        Ok((Self { path }, file))
    }

    fn read_bounded(
        &self,
        command: &str,
        label: &str,
        limit: u64,
    ) -> Result<Vec<u8>, HostPreflightError> {
        let mut bytes = Vec::new();
        File::open(&self.path)
            .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
            .map_err(|error| {
                HostPreflightError::Transient(format!(
                    "cannot read bounded OpenCode {command} {label} capture: {error}"
                ))
            })?;
        if bytes.len() as u64 >= limit {
            return Err(HostPreflightError::PermanentConfiguration(format!(
                "OpenCode {command} {label} reached the {limit}-byte identity limit"
            )));
        }
        Ok(bytes)
    }
}

impl Drop for ProbeCapture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn probe_opencode_cmd(bin: &Path) -> Result<(), HostPreflightError> {
    validate_native_opencode_executable(bin)?;
    probe_opencode_behavior_with_timeout(bin, OPENCODE_PROBE_TIMEOUT)
}

fn probe_opencode_behavior_with_timeout(
    bin: &Path,
    timeout: Duration,
) -> Result<(), HostPreflightError> {
    let (version_stdout, version_stderr) =
        execute_bounded_probe(bin, "--version", timeout, OPENCODE_VERSION_OUTPUT_LIMIT)?;
    if !version_stderr.iter().all(u8::is_ascii_whitespace)
        || !is_opencode_version_output(&version_stdout)
    {
        return Err(HostPreflightError::PermanentConfiguration(format!(
            "'{} --version' did not return the expected bare semantic version",
            bin.display()
        )));
    }

    let (help_stdout, help_stderr) =
        execute_bounded_probe(bin, "--help", timeout, OPENCODE_HELP_OUTPUT_LIMIT)?;
    if !is_opencode_help_output(&help_stdout, &help_stderr) {
        return Err(HostPreflightError::PermanentConfiguration(format!(
            "'{} --help' did not return the expected OpenCode command signature",
            bin.display()
        )));
    }
    Ok(())
}

fn execute_bounded_probe(
    bin: &Path,
    argument: &str,
    timeout: Duration,
    output_limit: u64,
) -> Result<(Vec<u8>, Vec<u8>), HostPreflightError> {
    let (stdout_capture, stdout_file) = ProbeCapture::create("stdout")?;
    let (stderr_capture, stderr_file) = ProbeCapture::create("stderr")?;
    let mut cmd = Command::new(bin);
    cmd.arg(argument)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        // SAFETY: pre_exec runs only async-signal-safe setrlimit with a fully initialized value.
        // The limit applies to the probe child only and bounds each redirected capture file.
        unsafe {
            cmd.pre_exec(move || {
                let limit = libc::rlimit {
                    rlim_cur: output_limit as libc::rlim_t,
                    rlim_max: output_limit as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|error| {
        HostPreflightError::Transient(format!(
            "failed to start '{} {argument}': {error}",
            bin.display()
        ))
    })?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                terminate_probe_tree(&mut child).map_err(HostPreflightError::Transient)?;
                return Err(HostPreflightError::Transient(format!(
                    "'{} {argument}' timed out after {} milliseconds",
                    bin.display(),
                    timeout.as_millis()
                )));
            }
            Err(error) => {
                let cleanup = terminate_probe_tree(&mut child);
                return Err(HostPreflightError::Transient(match cleanup {
                    Ok(()) => format!(
                        "waiting for '{} {argument}' failed: {error}",
                        bin.display()
                    ),
                    Err(cleanup) => format!(
                        "waiting for '{} {argument}' failed: {error}; cleanup also failed: {cleanup}",
                        bin.display()
                    ),
                }));
            }
        }
    };

    terminate_probe_tree(&mut child).map_err(HostPreflightError::Transient)?;
    let stdout = stdout_capture.read_bounded(argument, "stdout", output_limit)?;
    let stderr = stderr_capture.read_bounded(argument, "stderr", output_limit)?;
    if !status.success() {
        return Err(HostPreflightError::Transient(format!(
            "'{} {argument}' exited unsuccessfully ({status})",
            bin.display()
        )));
    }
    Ok((stdout, stderr))
}

fn terminate_probe_tree(child: &mut Child) -> Result<(), String> {
    #[cfg(unix)]
    let process_group = -(child.id() as i32);
    #[cfg(unix)]
    {
        // SAFETY: kill is called with the negative child process-group id created above and a
        // constant signal; no pointers are involved.
        let result = unsafe { libc::kill(process_group, libc::SIGKILL) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(format!(
                    "failed to kill OpenCode probe process group: {error}"
                ));
            }
        }
    }
    #[cfg(not(unix))]
    if child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        child
            .kill()
            .map_err(|error| format!("failed to kill OpenCode probe: {error}"))?;
    }

    let deadline = Instant::now() + OPENCODE_PROBE_REAP_TIMEOUT;
    let mut child_reaped = false;
    loop {
        if !child_reaped {
            child_reaped = match child.try_wait() {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(error) => return Err(format!("failed to reap the OpenCode probe: {error}")),
            };
        }
        #[cfg(unix)]
        let group_gone = !process_group_exists(process_group)?;
        #[cfg(not(unix))]
        let group_gone = true;
        if child_reaped && group_gone {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out reaping the OpenCode probe process group".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn process_group_exists(process_group: i32) -> Result<bool, String> {
    // SAFETY: signal 0 performs an existence/permission check and dereferences no pointers.
    let result = unsafe { libc::kill(process_group, 0) };
    if result == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(format!(
            "failed to confirm OpenCode probe process-group cleanup: {error}"
        )),
    }
}

#[cfg(target_os = "linux")]
fn validate_native_opencode_executable(bin: &Path) -> Result<(), HostPreflightError> {
    let mut header = [0_u8; 20];
    File::open(bin)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| {
            HostPreflightError::PermanentConfiguration(format!(
                "cannot inspect native OpenCode executable '{}': {error}",
                bin.display()
            ))
        })?;
    let is_x86_64_elf = header[0..4] == *b"\x7fELF"
        && header[4] == 2
        && header[5] == 1
        && header[18] == 0x3e
        && header[19] == 0;
    if !is_x86_64_elf {
        return Err(HostPreflightError::PermanentConfiguration(format!(
            "'{}' is not a native Ubuntu x86_64 OpenCode executable",
            bin.display()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn validate_native_opencode_executable(_bin: &Path) -> Result<(), HostPreflightError> {
    Ok(())
}

fn is_opencode_help_output(stdout: &[u8], stderr: &[u8]) -> bool {
    let mut combined = Vec::with_capacity(stdout.len() + stderr.len());
    combined.extend_from_slice(stdout);
    combined.extend_from_slice(stderr);
    let Ok(help) = std::str::from_utf8(&combined) else {
        return false;
    };
    [
        "Commands:",
        "opencode run [message..]",
        "opencode serve",
        "--version",
    ]
    .into_iter()
    .all(|marker| help.contains(marker))
}

fn is_opencode_version_output(raw: &[u8]) -> bool {
    let Ok(output) = std::str::from_utf8(raw) else {
        return false;
    };
    let version = output.trim();
    !version.is_empty()
        && !version.chars().any(char::is_whitespace)
        && semantic_version_is_valid(version)
}

fn semantic_version_is_valid(version: &str) -> bool {
    let (without_build, build) = match version.split_once('+') {
        Some((base, build)) if !build.contains('+') => (base, Some(build)),
        Some(_) => return false,
        None => (version, None),
    };
    if build.is_some_and(|value| !semver_identifiers_are_valid(value, false)) {
        return false;
    }
    let (core, prerelease) = match without_build.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (without_build, None),
    };
    if prerelease.is_some_and(|value| !semver_identifiers_are_valid(value, true)) {
        return false;
    }
    let components = core.split('.').collect::<Vec<_>>();
    components.len() == 3 && components.into_iter().all(valid_numeric_identifier)
}

fn semver_identifiers_are_valid(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && (!reject_numeric_leading_zero
                    || !identifier.bytes().all(|byte| byte.is_ascii_digit())
                    || valid_numeric_identifier(identifier))
        })
}

fn valid_numeric_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
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

    let opencode_bin = resolve_opencode_bin().map_err(|error| error.to_string())?;
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

    let mut child = cmd.spawn().map_err(|e| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "smith-host-runtime-{label}-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, path: impl AsRef<Path>) -> PathBuf {
            self.0.join(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn executable(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn explicit_opencode_override_is_absolute_verified_and_fail_closed() {
        let dir = TestDir::new("explicit");
        let explicit = dir.join("custom/opencode");
        executable(&explicit, "printf '1.18.30\\n'");
        let resolved =
            resolve_opencode_bin_with(Some(explicit.as_os_str()), None, None, |_| Ok(())).unwrap();
        assert_eq!(resolved, explicit.canonicalize().unwrap());

        let fallback = dir.join(".opencode/bin/opencode");
        executable(&fallback, "printf '1.18.30\\n'");
        let error = resolve_opencode_bin_with(
            Some(OsStr::new("relative/opencode")),
            None,
            Some(&dir.0),
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            HostPreflightError::PermanentConfiguration(message) if message.contains("OPENCODE_BIN")
        ));
    }

    #[test]
    fn service_path_and_stable_ubuntu_user_locations_are_supported() {
        let dir = TestDir::new("search");
        let path_candidate = dir.join("service-bin/opencode");
        executable(&path_candidate, "printf '1.18.30\\n'");
        let service_path = std::env::join_paths([path_candidate.parent().unwrap()]).unwrap();
        let resolved =
            resolve_opencode_bin_with(None, Some(service_path.as_os_str()), None, |_| Ok(()))
                .unwrap();
        assert_eq!(resolved, path_candidate.canonicalize().unwrap());

        let home_candidate = dir.join(".opencode/bin/opencode");
        executable(&home_candidate, "printf '1.18.30\\n'");
        let resolved = resolve_opencode_bin_with(None, None, Some(&dir.0), |_| Ok(())).unwrap();
        assert_eq!(resolved, home_candidate.canonicalize().unwrap());
    }

    #[test]
    fn non_executable_candidates_are_rejected_with_ubuntu_guidance() {
        let dir = TestDir::new("non-executable");
        let candidate = dir.join(".local/bin/opencode");
        executable(&candidate, "printf '1.18.30\\n'");
        let mut permissions = fs::metadata(&candidate).unwrap().permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&candidate, permissions).unwrap();

        let error = resolve_opencode_bin_with(None, None, Some(&dir.0), |_| Ok(())).unwrap_err();
        let HostPreflightError::PermanentConfiguration(message) = error else {
            panic!("non-executable candidate must be a permanent configuration failure");
        };
        assert!(message.contains("non-interactive service environment"));
        #[cfg(target_os = "linux")]
        {
            assert!(message.contains("On Ubuntu"));
            assert!(!message.contains(".exe"));
            assert!(!message.contains("APPDATA"));
        }
    }

    #[test]
    fn behavioral_probe_validates_opencode_signature_and_classifies_failures() {
        let dir = TestDir::new("probe");
        let good = dir.join("good-opencode");
        let semver_only_impostor = dir.join("semver-only-impostor");
        let wrong_identity = dir.join("wrong-identity");
        let empty = dir.join("empty");
        let nonzero = dir.join("nonzero");
        let missing = dir.join("missing");
        executable(
            &good,
            "case \"$1\" in\n  --version) printf '1.18.30\\n' ;;\n  --help) printf 'Commands:\\nopencode run [message..]\\nopencode serve\\n--version\\n' >&2 ;;\nesac",
        );
        executable(&semver_only_impostor, "printf '1.18.30\\n'");
        executable(&wrong_identity, "printf 'true (GNU coreutils) 9.4\\n'");
        executable(&empty, "exit 0");
        executable(&nonzero, "exit 7");

        assert_eq!(
            probe_opencode_behavior_with_timeout(&good, Duration::from_secs(1)),
            Ok(())
        );
        for candidate in [&semver_only_impostor, &wrong_identity, &empty] {
            assert!(matches!(
                probe_opencode_behavior_with_timeout(candidate, Duration::from_secs(1)),
                Err(HostPreflightError::PermanentConfiguration(_))
            ));
        }
        for candidate in [&nonzero, &missing] {
            assert!(matches!(
                execute_bounded_probe(
                    candidate,
                    "--version",
                    Duration::from_secs(1),
                    OPENCODE_VERSION_OUTPUT_LIMIT
                ),
                Err(HostPreflightError::Transient(_))
            ));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_probe_rejects_script_impostors_before_behavioral_checks() {
        let dir = TestDir::new("native-identity");
        let impostor = dir.join("opencode");
        executable(
            &impostor,
            "case \"$1\" in\n  --version) printf '1.18.30\\n' ;;\n  --help) printf 'Commands:\\nopencode run [message..]\\nopencode serve\\n--version\\n' >&2 ;;\nesac",
        );
        assert!(matches!(
            probe_opencode_cmd(&impostor),
            Err(HostPreflightError::PermanentConfiguration(_))
        ));
        validate_native_opencode_executable(&std::env::current_exe().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn excessive_probe_output_is_permanent_and_bounded() {
        let dir = TestDir::new("excessive-output");
        let probe = dir.join("noisy-opencode");
        // Avoid SIGXFSZ core-reporting latency: only this fixture ignores the signal and
        // attempts a finite oversized write, leaving the production file-size cap intact.
        let output = "x".repeat(OPENCODE_VERSION_OUTPUT_LIMIT as usize + 1);
        executable(&probe, &format!("trap '' XFSZ\nprintf '%s' '{output}'"));
        let error = execute_bounded_probe(
            &probe,
            "--version",
            Duration::from_secs(1),
            OPENCODE_VERSION_OUTPUT_LIMIT,
        )
        .unwrap_err();
        assert!(
            matches!(
                &error,
                HostPreflightError::PermanentConfiguration(message) if message.contains("identity limit")
            ),
            "expected permanent output-limit failure, got {error:?}"
        );
    }

    #[test]
    fn resolver_preserves_transient_probe_classification() {
        let dir = TestDir::new("transient-probe");
        let candidate = dir.join(".opencode/bin/opencode");
        executable(&candidate, "printf '1.18.30\\n'");
        let error = resolve_opencode_bin_with(None, None, Some(&dir.0), |_| {
            Err(HostPreflightError::Transient("resource pressure".into()))
        })
        .unwrap_err();
        assert!(matches!(error, HostPreflightError::Transient(_)));
    }

    #[test]
    fn semver_identity_is_strict_and_bounded() {
        for accepted in ["0.1.0", "1.18.30", "2.0.0-beta.1", "2.0.0+build.7"] {
            assert!(
                is_opencode_version_output(accepted.as_bytes()),
                "{accepted}"
            );
        }
        for rejected in [
            "",
            "v1.18.30",
            "1.18",
            "01.18.30",
            "1.18.30 other",
            "true (GNU coreutils) 9.4",
            "1.18.30\\nextra",
        ] {
            assert!(
                !is_opencode_version_output(rejected.as_bytes()),
                "{rejected}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn timed_out_probe_kills_its_process_group_and_reaps_the_child() {
        let dir = TestDir::new("probe-process-group");
        let probe = dir.join("forking-opencode");
        let descendant_pid = dir.join("descendant.pid");
        executable(
            &probe,
            &format!(
                "sleep 30 &\nprintf '%s\\n' \"$!\" > '{}'\nwait",
                descendant_pid.display()
            ),
        );

        let error =
            probe_opencode_behavior_with_timeout(&probe, Duration::from_millis(200)).unwrap_err();
        assert!(matches!(error, HostPreflightError::Transient(_)));
        let pid = fs::read_to_string(&descendant_pid)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while linux_process_is_live(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !linux_process_is_live(pid),
            "probe descendant {pid} survived"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn completed_and_nonzero_probes_kill_forked_descendants() {
        for (label, exit_line, expect_success) in
            [("successful", "exit 0", true), ("nonzero", "exit 7", false)]
        {
            let dir = TestDir::new(label);
            let probe = dir.join("forking-opencode");
            let descendant_pid = dir.join("descendant.pid");
            executable(
                &probe,
                &format!(
                    "if [ \"$1\" = \"--version\" ]; then\n  sleep 30 &\n  printf '%s\\n' \"$!\" > '{}'\n  printf '1.18.30\\n'\n  {exit_line}\nfi\nprintf 'Commands:\\nopencode run [message..]\\nopencode serve\\n--version\\n' >&2",
                    descendant_pid.display()
                ),
            );

            let result = probe_opencode_behavior_with_timeout(&probe, Duration::from_secs(1));
            assert_eq!(result.is_ok(), expect_success, "{label}: {result:?}");
            let pid = fs::read_to_string(&descendant_pid)
                .unwrap()
                .trim()
                .parse::<u32>()
                .unwrap();
            assert_process_exits(pid);
        }
    }

    #[cfg(target_os = "linux")]
    fn assert_process_exits(pid: u32) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while linux_process_is_live(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !linux_process_is_live(pid),
            "probe descendant {pid} survived"
        );
    }

    #[cfg(target_os = "linux")]
    fn linux_process_is_live(pid: u32) -> bool {
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        stat.rsplit_once(") ")
            .and_then(|(_, fields)| fields.chars().next())
            .is_some_and(|state| state != 'Z')
    }
}
