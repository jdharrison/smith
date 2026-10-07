//! Linux user-service registration and observed supervisor status.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(target_os = "linux")]
use std::ffi::OsString;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
#[cfg(target_os = "linux")]
use std::process::{Command, Output};

const SYSTEMD_UNIT: &str = "smith.service";
const INSTALL_ROOT_MARKER: &str = ".smith-install-root";
const INSTALL_ROOT_MARKER_CONTENT: &str = "smith-install-root-v1\n";
const UNKNOWN: &str = "unknown";
const TEST_EXTERNAL_PROBE_SUPPRESSION_ENV: &str = "SMITH_TEST_SUPPRESS_EXTERNAL_STATUS_PROBES";

fn unknown_value() -> String {
    UNKNOWN.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedSupervisorStatus {
    pub registered: bool,
    pub load_state: String,
    pub enabled_state: String,
    pub enabled_at_login: Option<bool>,
    pub active_state: String,
    pub sub_state: String,
    #[serde(default = "unknown_value")]
    pub service_result: String,
    #[serde(default)]
    pub restart_count: Option<u64>,
    #[serde(default = "unknown_value")]
    pub exec_main_code: String,
    #[serde(default = "unknown_value")]
    pub exec_main_status: String,
    pub linger: Option<bool>,
    pub bridge_listening: bool,
    pub bridge_identity: String,
    pub bridge_healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Default for ObservedSupervisorStatus {
    fn default() -> Self {
        Self {
            registered: false,
            load_state: UNKNOWN.to_string(),
            enabled_state: UNKNOWN.to_string(),
            enabled_at_login: None,
            active_state: UNKNOWN.to_string(),
            sub_state: UNKNOWN.to_string(),
            service_result: UNKNOWN.to_string(),
            restart_count: None,
            exec_main_code: UNKNOWN.to_string(),
            exec_main_status: UNKNOWN.to_string(),
            linger: None,
            bridge_listening: false,
            bridge_identity: UNKNOWN.to_string(),
            bridge_healthy: false,
            detail: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BridgeObservation {
    listening: bool,
    identity: &'static str,
    healthy: bool,
    detail: Option<String>,
}

/// True only when the configured listener proves it is a healthy Smith bridge.
pub fn bridge_is_listening() -> bool {
    let observation = observe_bridge();
    observation.identity == "smith" && observation.healthy
}

/// Observe registration, manager state, linger policy, and bridge identity without mutation.
pub fn observed_status() -> ObservedSupervisorStatus {
    #[cfg(target_os = "linux")]
    {
        let unit_path = match systemd_unit_path() {
            Ok(path) => path,
            Err(error) => {
                let mut status = ObservedSupervisorStatus {
                    detail: Some(error),
                    ..ObservedSupervisorStatus::default()
                };
                apply_bridge_observation(&mut status, observe_bridge());
                return status;
            }
        };
        // SAFETY: geteuid has no preconditions and does not dereference pointers.
        let uid = unsafe { libc::geteuid() }.to_string();
        observed_status_with(
            Path::new("systemctl"),
            Path::new("loginctl"),
            &unit_path,
            &uid,
            observe_bridge(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut status = ObservedSupervisorStatus {
            detail: Some("Smith user-service management is supported only on Linux".into()),
            ..ObservedSupervisorStatus::default()
        };
        apply_bridge_observation(&mut status, observe_bridge());
        status
    }
}

/// Stage the deterministic user unit and reload the user manager. Does not enable or start it.
pub fn install() -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    {
        let executable =
            std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
        install_linux_with(&executable, &systemd_unit_path()?, Path::new("systemctl"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("Smith user-service installation is supported only on Linux".into())
    }
}

/// Enable the installed unit at login. Does not start it.
pub fn enable() -> Result<(), String> {
    systemctl_action("enable")
}

/// Disable the unit at login. Does not stop or remove it.
pub fn disable() -> Result<(), String> {
    systemctl_action("disable")
}

/// Start the installed unit now. Does not enable it.
pub fn start() -> Result<(), String> {
    systemctl_action("start")
}

/// Stop the managed unit now. Does not disable or remove it.
pub fn stop() -> Result<(), String> {
    systemctl_action("stop")
}

/// Restart the managed unit now. Does not change enablement.
pub fn restart() -> Result<(), String> {
    systemctl_action("restart")
}

/// Explicitly stop, disable, and remove the user unit, then reload the manager.
pub fn uninstall() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        uninstall_linux_with(&systemd_unit_path()?, Path::new("systemctl"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("Smith user-service uninstallation is supported only on Linux".into())
    }
}

/// Ask a running bridge to ensure Whisper/TTS sidecars (no-op if bridge is down).
pub fn request_sidecars_ensure() -> Result<(), String> {
    let observation = observe_bridge();
    if !observation.listening {
        return Ok(());
    }
    if observation.identity != "smith" || !observation.healthy {
        return Err(
            "refusing sidecar request: configured bridge port is not a healthy Smith bridge".into(),
        );
    }
    let port = crate::commands::serve::BRIDGE_DEFAULT_PORT;
    let mut stream = TcpStream::connect_timeout(&bridge_addr(), Duration::from_secs(2))
        .map_err(|error| format!("connect bridge for sidecars: {error}"))?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
    let request = format!(
        "POST /v1/sidecars/ensure HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("sidecars ensure request: {error}"))?;
    let mut response = Vec::new();
    stream
        .take(8192)
        .read_to_end(&mut response)
        .map_err(|error| format!("read sidecars ensure response: {error}"))?;
    if response_status_is_success(&response) {
        Ok(())
    } else {
        Err("sidecars ensure returned a non-success HTTP response".into())
    }
}

fn bridge_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], crate::commands::serve::BRIDGE_DEFAULT_PORT))
}

fn external_status_probes_suppressed() -> bool {
    std::env::var_os(TEST_EXTERNAL_PROBE_SUPPRESSION_ENV).is_some_and(|value| value == "1")
}

fn observe_bridge() -> BridgeObservation {
    if external_status_probes_suppressed() {
        return BridgeObservation {
            listening: false,
            identity: "not_listening",
            healthy: false,
            detail: Some("external status probes suppressed for test".into()),
        };
    }

    let mut stream = match TcpStream::connect_timeout(&bridge_addr(), Duration::from_millis(400)) {
        Ok(stream) => stream,
        Err(_) => {
            return BridgeObservation {
                listening: false,
                identity: "not_listening",
                healthy: false,
                detail: None,
            };
        }
    };

    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let port = crate::commands::serve::BRIDGE_DEFAULT_PORT;
    let request =
        format!("GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return unverified_listener();
    }

    let mut response = Vec::new();
    if stream.take(8192).read_to_end(&mut response).is_err() {
        return unverified_listener();
    }
    if response_is_healthy_smith_bridge(&response) {
        BridgeObservation {
            listening: true,
            identity: "smith",
            healthy: true,
            detail: None,
        }
    } else {
        unverified_listener()
    }
}

fn response_status_is_success(response: &[u8]) -> bool {
    let response = String::from_utf8_lossy(response);
    response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .is_some_and(|status| (200..300).contains(&status))
}

fn response_is_healthy_smith_bridge(response: &[u8]) -> bool {
    let response = String::from_utf8_lossy(response);
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return false;
    };
    let status_ok = headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 "));
    status_ok
        && serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .is_some_and(|value| {
                value.get("ok").and_then(|entry| entry.as_bool()) == Some(true)
                    && value.get("service").and_then(|entry| entry.as_str()) == Some("smith-bridge")
            })
}

fn unverified_listener() -> BridgeObservation {
    BridgeObservation {
        listening: true,
        identity: "other_listener",
        healthy: false,
        detail: Some("listener did not return a valid Smith health response".into()),
    }
}

fn apply_bridge_observation(status: &mut ObservedSupervisorStatus, bridge: BridgeObservation) {
    status.bridge_listening = bridge.listening;
    status.bridge_identity = bridge.identity.to_string();
    status.bridge_healthy = bridge.healthy;
    if let Some(detail) = bridge.detail {
        append_detail(&mut status.detail, detail);
    }
}

fn append_detail(target: &mut Option<String>, detail: String) {
    match target {
        Some(existing) => {
            existing.push_str("; ");
            existing.push_str(&detail);
        }
        None => *target = Some(detail),
    }
}

#[cfg(target_os = "linux")]
fn systemd_unit_path() -> Result<PathBuf, String> {
    systemd_unit_path_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

#[cfg(target_os = "linux")]
fn systemd_unit_path_from(
    xdg_config_home: Option<OsString>,
    home: Option<PathBuf>,
) -> Result<PathBuf, String> {
    let config_home = match xdg_config_home.filter(|value| !value.is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err("XDG_CONFIG_HOME must be an absolute path".into());
            }
            path
        }
        None => {
            let home = home.ok_or_else(|| "HOME is not set".to_string())?;
            if !home.is_absolute() {
                return Err("HOME must be an absolute path".into());
            }
            home.join(".config")
        }
    };
    Ok(config_home.join("systemd").join("user").join(SYSTEMD_UNIT))
}

#[cfg(target_os = "linux")]
fn canonical_stable_executable_with(
    executable: &Path,
    effective_uid: u32,
    test_unstable_root: Option<&Path>,
) -> Result<PathBuf, String> {
    if effective_uid == 0 {
        return Err(
            "refusing to register Smith as effective root; use the intended unprivileged user"
                .into(),
        );
    }
    if !executable.is_absolute() {
        return Err(format!(
            "service executable must be an absolute path: {}",
            executable.display()
        ));
    }
    let canonical = executable.canonicalize().map_err(|error| {
        format!(
            "cannot canonicalize service executable '{}': {error}",
            executable.display()
        )
    })?;
    validate_stable_location(&canonical, test_unstable_root)?;
    validate_install_provenance(&canonical, effective_uid)?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        format!(
            "cannot inspect service executable '{}': {error}",
            canonical.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "service executable is not a regular file: {}",
            canonical.display()
        ));
    }
    if metadata.uid() != effective_uid {
        return Err(format!(
            "service executable must be owned by effective user {effective_uid}: {}",
            canonical.display()
        ));
    }
    let mode = metadata.permissions().mode();
    if mode & 0o111 == 0 {
        return Err(format!(
            "service executable is not executable: {}",
            canonical.display()
        ));
    }
    if mode & 0o022 != 0 {
        return Err(format!(
            "service executable must not be group- or world-writable: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

#[cfg(target_os = "linux")]
fn validate_install_provenance(executable: &Path, effective_uid: u32) -> Result<(), String> {
    let bin_dir = executable
        .parent()
        .ok_or_else(|| "service executable has no bin directory".to_string())?;
    if bin_dir.file_name().and_then(|value| value.to_str()) != Some("bin")
        || executable.file_name().and_then(|value| value.to_str()) != Some("smith")
    {
        return Err(format!(
            "service executable must be installed as <install-root>/bin/smith: {}",
            executable.display()
        ));
    }
    let install_root = bin_dir
        .parent()
        .ok_or_else(|| "service executable has no install root".to_string())?;
    let root_metadata = fs::metadata(install_root).map_err(|error| {
        format!(
            "cannot inspect service install root '{}': {error}",
            install_root.display()
        )
    })?;
    if !root_metadata.is_dir()
        || root_metadata.uid() != effective_uid
        || root_metadata.permissions().mode() & 0o022 != 0
    {
        return Err(format!(
            "service install root must be securely owned by effective user {effective_uid}: {}",
            install_root.display()
        ));
    }

    let marker = install_root.join(INSTALL_ROOT_MARKER);
    let marker_metadata = fs::metadata(&marker).map_err(|error| {
        format!(
            "missing or unreadable Smith install marker '{}': {error}; run scripts/install.sh first",
            marker.display()
        )
    })?;
    if !marker_metadata.is_file()
        || marker_metadata.uid() != effective_uid
        || marker_metadata.permissions().mode() & 0o077 != 0
    {
        return Err(format!(
            "Smith install marker must be a private file owned by effective user {effective_uid}: {}",
            marker.display()
        ));
    }
    let marker_content = fs::read_to_string(&marker)
        .map_err(|error| format!("read Smith install marker '{}': {error}", marker.display()))?;
    if marker_content != INSTALL_ROOT_MARKER_CONTENT {
        return Err(format!(
            "invalid Smith install marker '{}'; run scripts/install.sh again",
            marker.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_stable_location(path: &Path, test_unstable_root: Option<&Path>) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "service executable must be an absolute path: {}",
            path.display()
        ));
    }
    if test_unstable_root.is_some_and(|root| path.starts_with(root)) {
        return Ok(());
    }
    for transient_root in ["/tmp", "/var/tmp", "/dev", "/run", "/proc", "/sys"] {
        if path.starts_with(transient_root) {
            return Err(format!(
                "refusing transient service executable path '{}'; install Smith to a stable user-owned root first",
                path.display()
            ));
        }
    }

    let named_components = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str().map(str::to_ascii_lowercase),
            _ => None,
        })
        .collect::<Vec<_>>();
    if named_components.iter().enumerate().any(|(index, value)| {
        let is_home_username = index == 1 && named_components.first().is_some_and(|v| v == "home");
        !is_home_username
            && (matches!(
                value.as_str(),
                "tmp"
                    | "temp"
                    | "dev"
                    | "develop"
                    | "development"
                    | "build"
                    | "target"
                    | "debug"
                    | "release"
            ))
    }) {
        return Err(format!(
            "refusing development/build service executable path '{}'; install Smith to a stable user-owned root first",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn render_linux_unit(executable: &Path) -> Result<String, String> {
    if !executable.is_absolute() {
        return Err("systemd ExecStart requires an absolute executable path".into());
    }
    let raw = executable
        .to_str()
        .ok_or_else(|| "service executable path must be valid UTF-8".to_string())?;
    if raw.chars().any(char::is_control) {
        return Err("service executable path must not contain control characters".into());
    }
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    let configuration_exit = crate::commands::serve::CONFIGURATION_EXIT_CODE;
    Ok(format!(
        "[Unit]\n\
Description=Smith local supervisor\n\
StartLimitIntervalSec=300\n\
StartLimitBurst=5\n\
\n\
[Service]\n\
Type=simple\n\
ExecStart=\"{escaped}\" serve\n\
Restart=on-failure\n\
RestartPreventExitStatus={configuration_exit}\n\
RestartSec=15\n\
TimeoutStopSec=30\n\
KillMode=control-group\n\
UMask=0077\n\
NoNewPrivileges=true\n\
\n\
[Install]\n\
WantedBy=default.target\n"
    ))
}

#[cfg(target_os = "linux")]
fn install_linux_with(
    executable: &Path,
    unit_path: &Path,
    systemctl: &Path,
) -> Result<PathBuf, String> {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    let effective_uid = unsafe { libc::geteuid() };
    install_linux_with_validation(executable, unit_path, systemctl, effective_uid, None)
}

#[cfg(target_os = "linux")]
fn install_linux_with_validation(
    executable: &Path,
    unit_path: &Path,
    systemctl: &Path,
    effective_uid: u32,
    test_unstable_root: Option<&Path>,
) -> Result<PathBuf, String> {
    let executable =
        canonical_stable_executable_with(executable, effective_uid, test_unstable_root)?;
    let body = render_linux_unit(&executable)?;
    let parent = unit_path
        .parent()
        .ok_or_else(|| format!("unit path has no parent: {}", unit_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create systemd user directory '{}': {error}",
            parent.display()
        )
    })?;
    let staging = unit_path.with_extension(format!("service.tmp.{}", std::process::id()));
    fs::write(&staging, body)
        .map_err(|error| format!("stage user unit '{}': {error}", staging.display()))?;
    if let Err(error) = fs::rename(&staging, unit_path) {
        let _ = fs::remove_file(&staging);
        return Err(format!(
            "install user unit '{}': {error}",
            unit_path.display()
        ));
    }
    checked_command(systemctl, &["--user", "daemon-reload"], "systemctl")?;
    Ok(unit_path.to_path_buf())
}

#[cfg(target_os = "linux")]
fn uninstall_linux_with(unit_path: &Path, systemctl: &Path) -> Result<(), String> {
    checked_command(systemctl, &["--user", "stop", SYSTEMD_UNIT], "systemctl")?;
    checked_command(systemctl, &["--user", "disable", SYSTEMD_UNIT], "systemctl")?;
    match fs::remove_file(unit_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "remove user unit '{}': {error}",
                unit_path.display()
            ));
        }
    }
    checked_command(systemctl, &["--user", "daemon-reload"], "systemctl")
}

fn systemctl_action(action: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        checked_command(
            Path::new("systemctl"),
            &["--user", action, SYSTEMD_UNIT],
            "systemctl",
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = action;
        Err("Smith user-service management is supported only on Linux".into())
    }
}

#[cfg(target_os = "linux")]
fn checked_command(program: &Path, args: &[&str], label: &str) -> Result<(), String> {
    let output = Command::new(program).args(args).output().map_err(|error| {
        format!(
            "{} failed to execute: {error}",
            command_context(label, args)
        )
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(command_failure(label, args, &output))
}

#[cfg(target_os = "linux")]
fn command_failure(label: &str, args: &[&str], output: &Output) -> String {
    let exit = output
        .status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_string());
    let stderr = sanitize_output(&output.stderr);
    format!(
        "{} failed (exit {exit}): {}",
        command_context(label, args),
        if stderr.is_empty() {
            "no stderr"
        } else {
            &stderr
        }
    )
}

#[cfg(target_os = "linux")]
fn command_context(label: &str, args: &[&str]) -> String {
    std::iter::once(label)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(target_os = "linux")]
fn sanitize_output(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    let clean = text
        .chars()
        .filter_map(|character| {
            if character.is_control() {
                character.is_whitespace().then_some(' ')
            } else {
                Some(character)
            }
        })
        .collect::<String>();
    let mut normalized = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() > 512 {
        normalized = normalized.chars().take(509).collect::<String>();
        normalized.push_str("...");
    }
    normalized
}

#[cfg(target_os = "linux")]
fn query_command(program: &Path, args: &[&str], label: &str) -> Result<Output, String> {
    Command::new(program).args(args).output().map_err(|error| {
        format!(
            "{} failed to execute: {error}",
            command_context(label, args)
        )
    })
}

#[cfg(target_os = "linux")]
fn observed_status_with(
    systemctl: &Path,
    loginctl: &Path,
    unit_path: &Path,
    uid: &str,
    bridge: BridgeObservation,
) -> ObservedSupervisorStatus {
    let mut status = ObservedSupervisorStatus {
        registered: unit_path.is_file(),
        ..ObservedSupervisorStatus::default()
    };

    let show_args = [
        "--user",
        "show",
        SYSTEMD_UNIT,
        "--property=LoadState",
        "--property=ActiveState",
        "--property=SubState",
        "--property=Result",
        "--property=NRestarts",
        "--property=ExecMainCode",
        "--property=ExecMainStatus",
    ];
    match query_command(systemctl, &show_args, "systemctl") {
        Ok(output) if output.status.success() => {
            let values = parse_properties(&output.stdout);
            status.load_state = validated_property(&values, "LoadState", valid_load_state);
            status.active_state = validated_property(&values, "ActiveState", valid_active_state);
            status.sub_state = validated_property(&values, "SubState", valid_service_substate);
            status.service_result = validated_property(&values, "Result", valid_service_result);
            status.restart_count =
                unique_property(&values, "NRestarts").and_then(|value| value.parse::<u64>().ok());
            status.exec_main_code =
                validated_property(&values, "ExecMainCode", valid_exec_main_code);
            status.exec_main_status =
                validated_property(&values, "ExecMainStatus", valid_exec_main_status);
            if values.malformed
                || status.load_state == UNKNOWN
                || status.active_state == UNKNOWN
                || status.sub_state == UNKNOWN
                || status.service_result == UNKNOWN
                || status.restart_count.is_none()
                || status.exec_main_code == UNKNOWN
                || status.exec_main_status == UNKNOWN
            {
                append_detail(
                    &mut status.detail,
                    "systemctl returned missing, duplicate, or malformed service properties; untrusted fields are unknown"
                        .into(),
                );
            }
        }
        Ok(output) => append_detail(
            &mut status.detail,
            command_failure("systemctl", &show_args, &output),
        ),
        Err(error) => append_detail(&mut status.detail, error),
    }

    let enabled_args = ["--user", "is-enabled", SYSTEMD_UNIT];
    match query_command(systemctl, &enabled_args, "systemctl") {
        Ok(output) => {
            let state = single_output_line(&output.stdout).map(str::to_string);
            let enabled = state.as_deref().and_then(enabled_at_login);
            let expected_result = (output.status.success() && enabled.is_some())
                || (output.status.code() == Some(1) && enabled == Some(false));
            if expected_result {
                status.enabled_at_login = enabled;
                status.enabled_state = state.expect("validated enablement state");
            } else if output.status.success() {
                append_detail(
                    &mut status.detail,
                    "systemctl --user is-enabled returned malformed or unknown output; enablement is unknown"
                        .into(),
                );
            } else {
                append_detail(
                    &mut status.detail,
                    command_failure("systemctl", &enabled_args, &output),
                );
            }
        }
        Err(error) => append_detail(&mut status.detail, error),
    }

    let linger_args = ["show-user", uid, "--property=Linger", "--value"];
    match query_command(loginctl, &linger_args, "loginctl") {
        Ok(output) => {
            if output.status.success() {
                status.linger = match single_output_line(&output.stdout) {
                    Some("yes") => Some(true),
                    Some("no") => Some(false),
                    _ => None,
                };
            }
            if output.status.success() && status.linger.is_none() {
                append_detail(
                    &mut status.detail,
                    "loginctl show-user returned malformed or unknown Linger output; linger is unknown"
                        .into(),
                );
            } else if !output.status.success() {
                append_detail(
                    &mut status.detail,
                    command_failure("loginctl", &linger_args, &output),
                );
            }
        }
        Err(error) => append_detail(&mut status.detail, error),
    }

    apply_bridge_observation(&mut status, bridge);
    status
}

#[cfg(target_os = "linux")]
#[derive(Debug, Default)]
struct ParsedProperties {
    values: std::collections::BTreeMap<String, Option<String>>,
    malformed: bool,
}

#[cfg(target_os = "linux")]
fn parse_properties(raw: &[u8]) -> ParsedProperties {
    const EXPECTED: [&str; 7] = [
        "LoadState",
        "ActiveState",
        "SubState",
        "Result",
        "NRestarts",
        "ExecMainCode",
        "ExecMainStatus",
    ];
    let mut parsed = ParsedProperties::default();
    for line in String::from_utf8_lossy(raw).lines() {
        let Some((key, value)) = line.split_once('=') else {
            parsed.malformed |= !line.trim().is_empty();
            continue;
        };
        let key = key.trim();
        if !EXPECTED.contains(&key) {
            parsed.malformed = true;
            continue;
        }
        let value = value.trim();
        if value.is_empty() || parsed.values.contains_key(key) {
            parsed.malformed = true;
            parsed.values.insert(key.to_string(), None);
        } else {
            parsed
                .values
                .insert(key.to_string(), Some(value.to_string()));
        }
    }
    parsed
}

#[cfg(target_os = "linux")]
fn unique_property<'a>(values: &'a ParsedProperties, key: &str) -> Option<&'a str> {
    values.values.get(key).and_then(Option::as_deref)
}

#[cfg(target_os = "linux")]
fn validated_property(
    values: &ParsedProperties,
    key: &str,
    validate: impl FnOnce(&str) -> bool,
) -> String {
    unique_property(values, key)
        .filter(|value| validate(value))
        .unwrap_or(UNKNOWN)
        .to_string()
}

#[cfg(target_os = "linux")]
fn valid_load_state(value: &str) -> bool {
    matches!(
        value,
        "stub" | "loaded" | "not-found" | "bad-setting" | "error" | "merged" | "masked"
    )
}

#[cfg(target_os = "linux")]
fn valid_active_state(value: &str) -> bool {
    matches!(
        value,
        "active"
            | "reloading"
            | "inactive"
            | "failed"
            | "activating"
            | "deactivating"
            | "maintenance"
            | "refreshing"
    )
}

#[cfg(target_os = "linux")]
fn valid_service_substate(value: &str) -> bool {
    matches!(
        value,
        "dead"
            | "condition"
            | "start-pre"
            | "start"
            | "start-post"
            | "running"
            | "exited"
            | "reload"
            | "reload-signal"
            | "reload-notify"
            | "stop"
            | "stop-watchdog"
            | "stop-sigterm"
            | "stop-sigkill"
            | "stop-post"
            | "final-watchdog"
            | "final-sigterm"
            | "final-sigkill"
            | "failed"
            | "auto-restart"
            | "auto-restart-queued"
            | "cleaning"
    )
}

#[cfg(target_os = "linux")]
fn valid_service_result(value: &str) -> bool {
    matches!(
        value,
        "success"
            | "protocol"
            | "timeout"
            | "exit-code"
            | "signal"
            | "core-dump"
            | "watchdog"
            | "start-limit-hit"
            | "resources"
            | "oom-kill"
            | "exec-condition"
            | "clean"
            | "skipped"
    )
}

#[cfg(target_os = "linux")]
fn valid_exec_main_code(value: &str) -> bool {
    value.parse::<u8>().is_ok_and(|code| code <= 6)
}

#[cfg(target_os = "linux")]
fn valid_exec_main_status(value: &str) -> bool {
    value.parse::<u8>().is_ok()
}

#[cfg(target_os = "linux")]
fn single_output_line(raw: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut lines = text.lines();
    let value = lines.next()?.trim();
    if value.is_empty() || lines.next().is_some() {
        return None;
    }
    Some(value)
}

#[cfg(target_os = "linux")]
fn enabled_at_login(state: &str) -> Option<bool> {
    match state {
        "enabled" => Some(true),
        "enabled-runtime" | "linked" | "linked-runtime" | "alias" | "disabled" | "static"
        | "indirect" | "generated" | "transient" | "masked" | "masked-runtime" | "not-found" => {
            Some(false)
        }
        _ => None,
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "smith-os-register-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, path: &str) -> PathBuf {
            self.0.join(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fake_command(dir: &TestDir, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        path
    }

    fn write_install_marker(install_root: &Path) {
        let mut root_permissions = fs::metadata(install_root).unwrap().permissions();
        root_permissions.set_mode(0o700);
        fs::set_permissions(install_root, root_permissions).unwrap();
        let marker = install_root.join(INSTALL_ROOT_MARKER);
        fs::write(&marker, INSTALL_ROOT_MARKER_CONTENT).unwrap();
        let mut marker_permissions = fs::metadata(&marker).unwrap().permissions();
        marker_permissions.set_mode(0o600);
        fs::set_permissions(marker, marker_permissions).unwrap();
    }

    fn no_bridge() -> BridgeObservation {
        BridgeObservation {
            listening: false,
            identity: "not_listening",
            healthy: false,
            detail: None,
        }
    }

    #[test]
    fn unit_rendering_is_deterministic_hardened_and_escaped() {
        let path = Path::new("/home/smith stable/bin/smith%worker");
        let first = render_linux_unit(path).unwrap();
        let second = render_linux_unit(path).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("ExecStart=\"/home/smith stable/bin/smith%%worker\" serve"));
        assert!(first.contains("StartLimitIntervalSec=300"));
        assert!(first.contains("StartLimitBurst=5"));
        assert!(first.contains("Restart=on-failure"));
        assert!(first.contains("RestartPreventExitStatus=78"));
        assert!(first.contains("RestartSec=15"));
        assert!(first.contains("TimeoutStopSec=30"));
        assert!(first.contains("KillMode=control-group"));
        assert!(first.contains("UMask=0077"));
        assert!(first.contains("NoNewPrivileges=true"));
        let service_section = first.find("[Service]").unwrap();
        assert!(first.find("StartLimitIntervalSec=300").unwrap() < service_section);
        assert!(first.find("StartLimitBurst=5").unwrap() < service_section);
        assert!(first.find("RestartPreventExitStatus=78").unwrap() > service_section);
        assert!(!first.contains("StartLimitAction="));
        assert!(!first.contains("After=default.target"));
        assert!(!first.contains("PrivateTmp=true"));
        assert!(!first.contains("enable --now"));
    }

    #[test]
    fn renderer_rejects_relative_and_control_character_paths() {
        assert!(render_linux_unit(Path::new("bin/smith")).is_err());
        assert!(render_linux_unit(Path::new("/home/smith/bin/smith\ninvalid")).is_err());
    }

    #[test]
    fn stable_path_validation_rejects_transient_development_and_build_outputs() {
        for path in [
            "/tmp/smith",
            "/var/tmp/smith",
            "/dev/shm/smith",
            "/run/user/1000/smith",
            "/home/user/develop/smith/bin/smith",
            "/home/user/smith/build/smith",
            "/home/user/smith/target/debug/smith",
            "/home/user/smith/target/release/smith",
        ] {
            assert!(
                validate_stable_location(Path::new(path), None).is_err(),
                "{path}"
            );
        }
        for path in [
            "/home/user/.local/bin/smith",
            "/home/dev/.local/bin/smith",
            "/opt/custom-smith/v1/bin/smith",
            "/home/user/releases/v1/bin/smith",
        ] {
            assert!(
                validate_stable_location(Path::new(path), None).is_ok(),
                "{path}"
            );
        }
    }

    #[test]
    fn executable_validation_enforces_effective_owner_mode_and_narrow_test_root() {
        let dir = TestDir::new("executable-validation");
        let smoke_root = dir.join(".smith-user-service-smoke.fixture");
        let install_root = smoke_root.join("install");
        let executable = install_root.join("bin/smith");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        write_install_marker(&install_root);
        fs::write(&executable, "#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();
        let owner = fs::metadata(&executable).unwrap().uid();

        let canonical =
            canonical_stable_executable_with(&executable, owner, Some(&install_root)).unwrap();
        assert!(canonical.is_absolute());
        assert_eq!(canonical, executable.canonicalize().unwrap());
        assert!(
            canonical_stable_executable_with(&executable, owner + 1, Some(&install_root)).is_err()
        );
        assert!(canonical_stable_executable_with(&executable, 0, Some(&install_root)).is_err());

        fs::remove_file(install_root.join(INSTALL_ROOT_MARKER)).unwrap();
        assert!(canonical_stable_executable_with(&executable, owner, Some(&install_root)).is_err());
        write_install_marker(&install_root);

        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o722);
        fs::set_permissions(&executable, permissions).unwrap();
        assert!(canonical_stable_executable_with(&executable, owner, Some(&install_root)).is_err());
        assert!(canonical_stable_executable_with(Path::new("bin/smith"), owner, None).is_err());
    }

    #[test]
    fn unit_path_respects_absolute_xdg_config_home() {
        let path = systemd_unit_path_from(
            Some(OsString::from("/home/operator/config")),
            Some(PathBuf::from("/home/operator")),
        )
        .unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/operator/config/systemd/user/smith.service")
        );
        assert!(systemd_unit_path_from(
            Some(OsString::from("relative/config")),
            Some(PathBuf::from("/home/operator"))
        )
        .is_err());
    }

    #[test]
    fn install_only_stages_and_reloads() {
        let dir = TestDir::new("install");
        let log = dir.join("calls.log");
        let manager = fake_command(
            &dir,
            "systemctl",
            &format!("printf '%s\\n' \"$*\" >> '{}'", log.display()),
        );
        let unit = dir.join("config/systemd/user/smith.service");
        let smoke_root = dir.join(".smith-user-service-smoke.fixture/install");
        let executable = smoke_root.join("bin/smith");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        write_install_marker(&smoke_root);
        fs::write(&executable, "#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();
        let owner = fs::metadata(&executable).unwrap().uid();
        let installed =
            install_linux_with_validation(&executable, &unit, &manager, owner, Some(&smoke_root))
                .unwrap();
        assert_eq!(installed, unit);
        let calls = fs::read_to_string(log).unwrap();
        assert_eq!(calls, "--user daemon-reload\n");
        let body = fs::read_to_string(unit).unwrap();
        assert!(body.contains(&format!("ExecStart=\"{}\"", executable.display())));
    }

    #[test]
    fn lifecycle_actions_are_separate_and_uninstall_stops_then_disables() {
        let dir = TestDir::new("lifecycle");
        let log = dir.join("calls.log");
        let manager = fake_command(
            &dir,
            "systemctl",
            &format!("printf '%s\\n' \"$*\" >> '{}'", log.display()),
        );
        for action in ["enable", "disable", "start", "stop", "restart"] {
            checked_command(&manager, &["--user", action, SYSTEMD_UNIT], "systemctl").unwrap();
        }
        let unit = dir.join("smith.service");
        fs::write(&unit, "unit").unwrap();
        uninstall_linux_with(&unit, &manager).unwrap();

        let calls = fs::read_to_string(log).unwrap();
        assert!(calls.contains("--user enable smith.service\n"));
        assert!(calls.contains("--user disable smith.service\n"));
        assert!(calls.contains("--user start smith.service\n"));
        assert!(calls.contains("--user stop smith.service\n"));
        assert!(calls.contains("--user restart smith.service\n"));
        assert!(calls.ends_with(
            "--user stop smith.service\n--user disable smith.service\n--user daemon-reload\n"
        ));
        assert!(!unit.exists());
    }

    #[test]
    fn checked_commands_report_exit_and_sanitized_stderr() {
        let dir = TestDir::new("failure");
        let manager = fake_command(
            &dir,
            "systemctl",
            "printf 'permission denied\\nsecond line\\033[31m' >&2\nexit 23",
        );
        let error =
            checked_command(&manager, &["--user", "start", SYSTEMD_UNIT], "systemctl").unwrap_err();
        assert!(error.contains("systemctl --user start smith.service failed (exit 23)"));
        assert!(error.contains("permission denied second line[31m"));
        assert!(!error.contains('\n'));
        assert!(!error.contains('\u{1b}'));
    }

    #[test]
    fn observed_status_uses_read_only_fake_managers() {
        let dir = TestDir::new("observed-status");
        let systemctl_log = dir.join("systemctl.log");
        let systemctl = fake_command(
            &dir,
            "systemctl",
            &format!(
                "printf '%s\\n' \"$*\" >> '{}'\nif [ \"$2\" = \"show\" ]; then\n  printf 'LoadState=loaded\\nActiveState=activating\\nSubState=auto-restart\\nResult=exit-code\\nNRestarts=7971\\nExecMainCode=1\\nExecMainStatus=1\\n'\nelif [ \"$2\" = \"is-enabled\" ]; then\n  printf 'enabled\\n'\nfi",
                systemctl_log.display()
            ),
        );
        let loginctl_log = dir.join("loginctl.log");
        let loginctl = fake_command(
            &dir,
            "loginctl",
            &format!(
                "printf '%s\\n' \"$*\" >> '{}'\nprintf 'no\\n'",
                loginctl_log.display()
            ),
        );
        let unit = dir.join("smith.service");
        fs::write(&unit, "unit").unwrap();

        let status = observed_status_with(&systemctl, &loginctl, &unit, "1000", no_bridge());
        assert!(status.registered);
        assert_eq!(status.load_state, "loaded");
        assert_eq!(status.enabled_at_login, Some(true));
        assert_eq!(status.active_state, "activating");
        assert_eq!(status.sub_state, "auto-restart");
        assert_eq!(status.service_result, "exit-code");
        assert_eq!(status.restart_count, Some(7971));
        assert_eq!(status.exec_main_code, "1");
        assert_eq!(status.exec_main_status, "1");
        assert_eq!(status.linger, Some(false));
        assert_eq!(
            fs::read_to_string(systemctl_log).unwrap(),
            "--user show smith.service --property=LoadState --property=ActiveState --property=SubState --property=Result --property=NRestarts --property=ExecMainCode --property=ExecMainStatus\n--user is-enabled smith.service\n"
        );
        assert_eq!(
            fs::read_to_string(loginctl_log).unwrap(),
            "show-user 1000 --property=Linger --value\n"
        );
    }

    #[test]
    fn observed_status_preserves_raw_nonpersistent_enablement_state() {
        let dir = TestDir::new("runtime-enablement");
        let systemctl = fake_command(
            &dir,
            "systemctl",
            "if [ \"$2\" = \"show\" ]; then\n  printf 'LoadState=loaded\\nActiveState=inactive\\nSubState=dead\\n'\nelse\n  printf 'enabled-runtime\\n'\nfi",
        );
        let loginctl = fake_command(&dir, "loginctl", "printf 'no\\n'");
        let unit = dir.join("smith.service");
        fs::write(&unit, "unit").unwrap();

        let status = observed_status_with(&systemctl, &loginctl, &unit, "1000", no_bridge());
        assert_eq!(status.enabled_state, "enabled-runtime");
        assert_eq!(status.enabled_at_login, Some(false));
    }

    #[test]
    fn failed_systemctl_queries_never_trust_stdout_state() {
        let dir = TestDir::new("failed-systemctl-status");
        let systemctl = fake_command(
            &dir,
            "systemctl",
            "if [ \"$2\" = \"show\" ]; then\n  printf 'LoadState=loaded\\nActiveState=active\\nSubState=running\\n'\nelse\n  printf 'enabled\\n'\nfi\nprintf 'manager query failed' >&2\nexit 9",
        );
        let loginctl = fake_command(&dir, "loginctl", "printf 'no\\n'");
        let unit = dir.join("smith.service");
        fs::write(&unit, "unit").unwrap();

        let status = observed_status_with(&systemctl, &loginctl, &unit, "1000", no_bridge());
        assert_eq!(status.load_state, UNKNOWN);
        assert_eq!(status.active_state, UNKNOWN);
        assert_eq!(status.sub_state, UNKNOWN);
        assert_eq!(status.service_result, UNKNOWN);
        assert_eq!(status.restart_count, None);
        assert_eq!(status.exec_main_code, UNKNOWN);
        assert_eq!(status.exec_main_status, UNKNOWN);
        assert_eq!(status.enabled_state, UNKNOWN);
        assert_eq!(status.enabled_at_login, None);
        let detail = status.detail.unwrap();
        assert!(detail.contains("systemctl --user show smith.service"));
        assert!(detail.contains("systemctl --user is-enabled smith.service"));
        assert_eq!(detail.matches("failed (exit 9)").count(), 2);
    }

    #[test]
    fn failed_loginctl_query_never_trusts_yes_or_no_stdout() {
        for value in ["yes", "no"] {
            let dir = TestDir::new(value);
            let systemctl = fake_command(
                &dir,
                "systemctl",
                "if [ \"$2\" = \"show\" ]; then\n  printf 'LoadState=loaded\\nActiveState=inactive\\nSubState=dead\\n'\nelse\n  printf 'disabled\\n'\nfi",
            );
            let loginctl = fake_command(
                &dir,
                "loginctl",
                &format!("printf '{value}\\n'\nprintf 'query failed' >&2\nexit 9"),
            );
            let unit = dir.join("smith.service");
            fs::write(&unit, "unit").unwrap();

            let status = observed_status_with(&systemctl, &loginctl, &unit, "1000", no_bridge());
            assert_eq!(status.linger, None, "failed stdout was {value}");
            let detail = status.detail.unwrap();
            assert!(detail.contains("loginctl show-user 1000"));
            assert!(detail.contains("failed (exit 9): query failed"));
        }
    }

    #[test]
    fn contradictory_enablement_and_linger_output_remains_unknown() {
        let dir = TestDir::new("contradictory-single-value-output");
        let systemctl = fake_command(
            &dir,
            "systemctl",
            "if [ \"$2\" = \"show\" ]; then\n  printf 'LoadState=loaded\\nActiveState=inactive\\nSubState=dead\\nResult=success\\nNRestarts=0\\nExecMainCode=1\\nExecMainStatus=0\\n'\nelse\n  printf 'enabled\\ndisabled\\n'\nfi",
        );
        let loginctl = fake_command(&dir, "loginctl", "printf 'yes\\nno\\n'");
        let unit = dir.join("smith.service");
        fs::write(&unit, "unit").unwrap();

        let status = observed_status_with(&systemctl, &loginctl, &unit, "1000", no_bridge());
        assert_eq!(status.enabled_state, UNKNOWN);
        assert_eq!(status.enabled_at_login, None);
        assert_eq!(status.linger, None);
        let detail = status.detail.unwrap();
        assert!(detail.contains("is-enabled returned malformed"));
        assert!(detail.contains("malformed or unknown Linger"));
    }

    #[test]
    fn bridge_identity_requires_successful_smith_health_payload() {
        assert!(response_is_healthy_smith_bridge(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true,\"service\":\"smith-bridge\"}"
        ));
        assert!(!response_is_healthy_smith_bridge(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true,\"service\":\"other\"}"
        ));
        assert!(!response_is_healthy_smith_bridge(
            b"HTTP/1.1 503 Service Unavailable\r\n\r\n{\"ok\":true,\"service\":\"smith-bridge\"}"
        ));
        assert!(response_status_is_success(
            b"HTTP/1.1 204 No Content\r\n\r\n"
        ));
        assert!(!response_status_is_success(
            b"HTTP/1.1 500 Internal Server Error\r\n\r\n"
        ));
    }

    #[test]
    fn malformed_successful_manager_properties_remain_unknown() {
        let dir = TestDir::new("malformed-successful-status");
        let systemctl = fake_command(
            &dir,
            "systemctl",
            "if [ \"$2\" = \"show\" ]; then\n  printf 'LoadState=loaded\\nLoadState=masked\\nActiveState=garbage\\nSubState=running\\nResult=garbage\\nNRestarts=NaN\\nExecMainCode=exited\\nExecMainStatus=999\\n'\nelse\n  printf 'disabled\\n'\nfi",
        );
        let loginctl = fake_command(&dir, "loginctl", "printf 'no\\n'");
        let unit = dir.join("smith.service");
        fs::write(&unit, "unit").unwrap();

        let status = observed_status_with(&systemctl, &loginctl, &unit, "1000", no_bridge());
        assert_eq!(status.load_state, UNKNOWN, "duplicate property was trusted");
        assert_eq!(status.active_state, UNKNOWN);
        assert_eq!(status.sub_state, "running");
        assert_eq!(status.service_result, UNKNOWN);
        assert_eq!(status.restart_count, None);
        assert_eq!(status.exec_main_code, UNKNOWN);
        assert_eq!(status.exec_main_status, UNKNOWN);
        assert!(status
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("missing, duplicate, or malformed")));
    }

    #[test]
    fn manager_state_parsers_preserve_distinct_states() {
        let properties = parse_properties(
            b"LoadState=loaded\nActiveState=active\nSubState=running\nResult=success\nNRestarts=0\nExecMainCode=1\nExecMainStatus=0\n",
        );
        assert_eq!(
            validated_property(&properties, "LoadState", valid_load_state),
            "loaded"
        );
        assert_eq!(
            validated_property(&properties, "ActiveState", valid_active_state),
            "active"
        );
        assert_eq!(
            validated_property(&properties, "SubState", valid_service_substate),
            "running"
        );
        assert_eq!(enabled_at_login("enabled"), Some(true));
        for state in [
            "enabled-runtime",
            "linked",
            "linked-runtime",
            "alias",
            "disabled",
        ] {
            assert_eq!(enabled_at_login(state), Some(false), "{state}");
        }
        assert_eq!(enabled_at_login("unexpected"), None);
        assert_eq!(single_output_line(b"enabled\n"), Some("enabled"));
        assert_eq!(single_output_line(b"enabled\ndisabled\n"), None);
        assert_eq!(single_output_line(b"enabled\n\n"), None);
        assert_eq!(single_output_line(b"\n"), None);
    }
}
