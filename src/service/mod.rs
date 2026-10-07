//! First-class Smith services: warm/on-demand policy and Linux user-service management.

pub mod os_register;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Hierarchical service ids (config / API / CLI).
pub const ID_BRIDGE: &str = "bridge";
pub const ID_BRAIN: &str = "brain";
pub const ID_SANDBOX_DEFAULT: &str = "sandbox.default";
pub const ID_AGENTS_CODING: &str = "agents.coding";
pub const ID_SERVICES_SEARXNG: &str = "services.searxng";
pub const ID_SERVICES_OLLAMA: &str = "services.ollama";
pub const ID_SERVICES_WHISPER: &str = "services.whisper";
pub const ID_SERVICES_TTS: &str = "services.tts";

/// Compose `com.docker.compose.service` labels (flat, prefix-sorted).
pub const LABEL_BRAIN: &str = "brain";
pub const LABEL_SANDBOX_DEFAULT: &str = "sandbox-default";
pub const LABEL_AGENTS_CODING: &str = "agents-coding";
pub const LABEL_SERVICES_SEARXNG: &str = "services-searxng";
pub const LABEL_SERVICES_OLLAMA: &str = "services-ollama";

const TEST_EXTERNAL_PROBE_SUPPRESSION_ENV: &str = "SMITH_TEST_SUPPRESS_EXTERNAL_STATUS_PROBES";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceMode {
    Warm,
    #[default]
    OnDemand,
}

impl ServiceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ServiceMode::Warm => "warm",
            ServiceMode::OnDemand => "on_demand",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "warm" | "always" | "on" => Ok(ServiceMode::Warm),
            "on_demand" | "ondemand" | "demand" | "off" | "cold" => Ok(ServiceMode::OnDemand),
            other => Err(format!("unknown mode '{}'; use warm or on_demand", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceGroup {
    Bridge,
    Brain,
    Sandbox,
    Agents,
    Services,
}

impl ServiceGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            ServiceGroup::Bridge => "bridge",
            ServiceGroup::Brain => "brain",
            ServiceGroup::Sandbox => "sandbox",
            ServiceGroup::Agents => "agents",
            ServiceGroup::Services => "services",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ServiceEntryConfig {
    #[serde(default)]
    pub mode: ServiceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceRootConfig {
    /// Requested login autostart policy. Observed manager state is reported separately.
    #[serde(default)]
    pub autostart: bool,
    /// Per-service policy keyed by hierarchical id.
    #[serde(default)]
    pub services: BTreeMap<String, ServiceEntryConfig>,
}

impl Default for ServiceRootConfig {
    fn default() -> Self {
        let mut root = Self {
            autostart: false,
            services: BTreeMap::new(),
        };
        root.ensure_defaults();
        root
    }
}

impl ServiceRootConfig {
    pub fn ensure_defaults(&mut self) {
        for (id, mode) in default_modes() {
            self.services
                .entry(id.to_string())
                .or_insert(ServiceEntryConfig { mode });
        }
    }

    pub fn mode_of(&self, id: &str) -> ServiceMode {
        self.services
            .get(id)
            .map(|e| e.mode)
            .unwrap_or_else(|| default_mode_for(id))
    }

    pub fn set_mode(&mut self, id: &str, mode: ServiceMode) -> Result<(), String> {
        let canonical = resolve_service_id(id)?;
        self.services
            .insert(canonical.to_string(), ServiceEntryConfig { mode });
        Ok(())
    }

    pub fn is_warm(&self, id: &str) -> bool {
        self.mode_of(id) == ServiceMode::Warm
    }
}

fn default_modes() -> Vec<(&'static str, ServiceMode)> {
    vec![
        (ID_BRIDGE, ServiceMode::Warm),
        (ID_BRAIN, ServiceMode::Warm),
        (ID_SANDBOX_DEFAULT, ServiceMode::OnDemand),
        (ID_AGENTS_CODING, ServiceMode::OnDemand),
        (ID_SERVICES_SEARXNG, ServiceMode::Warm),
        (ID_SERVICES_OLLAMA, ServiceMode::Warm),
        (ID_SERVICES_WHISPER, ServiceMode::OnDemand),
        (ID_SERVICES_TTS, ServiceMode::OnDemand),
    ]
}

fn default_mode_for(id: &str) -> ServiceMode {
    default_modes()
        .into_iter()
        .find(|(i, _)| *i == id)
        .map(|(_, m)| m)
        .unwrap_or(ServiceMode::OnDemand)
}

/// All known hierarchical ids in display order.
pub fn known_service_ids() -> &'static [&'static str] {
    &[
        ID_BRIDGE,
        ID_BRAIN,
        ID_SANDBOX_DEFAULT,
        ID_AGENTS_CODING,
        ID_SERVICES_SEARXNG,
        ID_SERVICES_OLLAMA,
        ID_SERVICES_WHISPER,
        ID_SERVICES_TTS,
    ]
}

pub fn service_group(id: &str) -> ServiceGroup {
    match id {
        ID_BRIDGE => ServiceGroup::Bridge,
        ID_BRAIN => ServiceGroup::Brain,
        ID_SANDBOX_DEFAULT => ServiceGroup::Sandbox,
        ID_AGENTS_CODING => ServiceGroup::Agents,
        _ if id.starts_with("sandbox.") => ServiceGroup::Sandbox,
        _ if id.starts_with("agents.") => ServiceGroup::Agents,
        _ if id.starts_with("services.") => ServiceGroup::Services,
        _ => ServiceGroup::Services,
    }
}

/// Resolve aliases (`searxng` → `services.searxng`, `coding` → `agents.coding`).
pub fn resolve_service_id(raw: &str) -> Result<&'static str, String> {
    let s = raw.trim().to_ascii_lowercase();
    let hit = known_service_ids().iter().copied().find(|id| *id == s);
    if let Some(id) = hit {
        return Ok(id);
    }
    let alias = match s.as_str() {
        "searx" | "searxng" | "search" => Some(ID_SERVICES_SEARXNG),
        "ollama" => Some(ID_SERVICES_OLLAMA),
        "whisper" | "stt" => Some(ID_SERVICES_WHISPER),
        "tts" | "kokoro" => Some(ID_SERVICES_TTS),
        "coding" | "code" => Some(ID_AGENTS_CODING),
        "sandbox" => Some(ID_SANDBOX_DEFAULT),
        "serve" | "supervisor" => Some(ID_BRIDGE),
        _ => None,
    };
    alias.ok_or_else(|| {
        format!(
            "unknown service '{}'; known: {}",
            raw,
            known_service_ids().join(", ")
        )
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatusRow {
    pub id: String,
    pub group: String,
    pub mode: String,
    pub running: bool,
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesSnapshot {
    /// Configured preference retained for compatibility; compare with observed enablement.
    pub autostart: bool,
    /// Observed user-unit file registration retained for API compatibility.
    pub os_registered: bool,
    /// Additive supervisor lifecycle and bridge identity observations.
    #[serde(default)]
    pub supervisor: os_register::ObservedSupervisorStatus,
    pub services: Vec<ServiceStatusRow>,
}

/// Load config, fill defaults, return service root.
pub fn load_service_config() -> Result<ServiceRootConfig, String> {
    let mut cfg = crate::load_config()?;
    cfg.service.ensure_defaults();
    Ok(cfg.service.clone())
}

pub fn save_service_config(service: &ServiceRootConfig) -> Result<(), String> {
    let mut cfg = crate::load_config()?;
    cfg.service = service.clone();
    cfg.service.ensure_defaults();
    crate::save_config(&cfg)
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    let mut svc = load_service_config()?;
    if on {
        os_register::enable()?;
    } else {
        os_register::disable()?;
    }
    svc.autostart = on;
    save_service_config(&svc)
}

pub fn uninstall_supervisor() -> Result<(), String> {
    let mut svc = load_service_config()?;
    os_register::uninstall()?;
    svc.autostart = false;
    save_service_config(&svc)
}

pub fn set_service_mode(id: &str, mode: ServiceMode) -> Result<(), String> {
    let mut svc = load_service_config()?;
    svc.set_mode(id, mode)?;
    save_service_config(&svc)
}

/// Probe running/healthy for known services (best-effort).
pub fn collect_status() -> Result<ServicesSnapshot, String> {
    let svc = load_service_config()?;
    let supervisor = os_register::observed_status();
    let os_registered = supervisor.registered;
    let runtime = crate::current_runtime();
    let external_probes_suppressed = external_status_probes_suppressed();
    let mut rows = Vec::new();
    for id in known_service_ids() {
        let (running, healthy, url, detail) =
            probe_service(id, &supervisor, runtime, external_probes_suppressed);
        rows.push(ServiceStatusRow {
            id: id.to_string(),
            group: service_group(id).as_str().to_string(),
            mode: svc.mode_of(id).as_str().to_string(),
            running,
            healthy,
            url,
            detail,
        });
    }
    Ok(ServicesSnapshot {
        autostart: svc.autostart,
        os_registered,
        supervisor,
        services: rows,
    })
}

fn external_status_probes_suppressed() -> bool {
    std::env::var_os(TEST_EXTERNAL_PROBE_SUPPRESSION_ENV).is_some_and(|value| value == "1")
}

fn probe_service(
    id: &str,
    supervisor: &os_register::ObservedSupervisorStatus,
    runtime: crate::RuntimeMode,
    external_probes_suppressed: bool,
) -> (bool, bool, Option<String>, Option<String>) {
    if id == ID_BRIDGE {
        let port = crate::commands::serve::BRIDGE_DEFAULT_PORT;
        return (
            supervisor.bridge_identity == "smith",
            supervisor.bridge_healthy,
            Some(format!("http://127.0.0.1:{port}")),
            (!supervisor.bridge_healthy)
                .then(|| format!("bridge identity: {}", supervisor.bridge_identity)),
        );
    }

    if id == ID_BRAIN && runtime.is_host() {
        let smith_bridge = supervisor.bridge_identity == "smith";
        return (
            smith_bridge,
            smith_bridge && supervisor.bridge_healthy,
            None,
            Some(format!(
                "native host runtime; supervisor bridge identity: {}",
                supervisor.bridge_identity
            )),
        );
    }

    if external_probes_suppressed {
        return (
            false,
            false,
            None,
            Some("external status probes suppressed for test".into()),
        );
    }

    match id {
        ID_BRAIN => {
            let running = docker_name_running(crate::docker::BRAIN_CONTAINER_NAME);
            (running, running, None, None)
        }
        ID_SANDBOX_DEFAULT => {
            let running = match crate::docker::sandbox_status() {
                Ok(st) => st.sandbox_running,
                Err(_) => false,
            };
            (running, running, None, None)
        }
        ID_AGENTS_CODING => {
            let running = match crate::docker::sandbox_status() {
                Ok(st) => st.coding_running,
                Err(_) => false,
            };
            (running, running, None, None)
        }
        ID_SERVICES_SEARXNG => {
            let (running, healthy) = crate::docker::searxng_status();
            (
                running,
                healthy,
                Some(format!("http://127.0.0.1:{}", crate::docker::SEARXNG_PORT)),
                None,
            )
        }
        ID_SERVICES_OLLAMA => {
            let running = crate::docker::is_ollama_running();
            (
                running,
                running,
                Some(format!("http://127.0.0.1:{}", crate::docker::OLLAMA_PORT)),
                None,
            )
        }
        ID_SERVICES_WHISPER => {
            let ok = port_open(4790);
            (ok, ok, Some("http://127.0.0.1:4790".into()), None)
        }
        ID_SERVICES_TTS => {
            let ok = port_open(4791);
            (ok, ok, Some("http://127.0.0.1:4791".into()), None)
        }
        _ => (false, false, None, Some("unknown".into())),
    }
}

fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(400),
    )
    .is_ok()
}

fn docker_name_running(name: &str) -> bool {
    std::process::Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", name])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Ensure all services marked `warm` are up (best-effort; collect notes).
pub fn apply_warm_set() -> Result<Vec<String>, String> {
    let svc = load_service_config()?;
    let mut notes = Vec::new();
    let runtime = crate::current_runtime();

    if svc.is_warm(ID_SERVICES_OLLAMA) {
        match crate::docker::start_ollama(crate::DEFAULT_LOCAL_MODEL) {
            Ok(_) => notes.push("services.ollama: warm".into()),
            Err(e) => notes.push(format!("services.ollama: {e}")),
        }
    }
    if svc.is_warm(ID_BRAIN) && !runtime.is_host() {
        match ensure_brain_warm() {
            Ok(()) => notes.push("brain: warm".into()),
            Err(e) => notes.push(format!("brain: {e}")),
        }
    }
    if svc.is_warm(ID_SERVICES_SEARXNG) {
        match crate::docker::ensure_searxng() {
            Ok(()) => notes.push("services.searxng: warm".into()),
            Err(e) => notes.push(format!("services.searxng: {e}")),
        }
    }
    if svc.is_warm(ID_AGENTS_CODING) {
        match crate::docker::ensure_coding_agent_ready() {
            Ok(()) => notes.push("agents.coding: warm".into()),
            Err(e) => notes.push(format!("agents.coding: {e}")),
        }
    }
    if svc.is_warm(ID_SERVICES_WHISPER) {
        if port_open(4790) {
            notes.push("services.whisper: warm".into());
        } else if os_register::bridge_is_listening() {
            notes.push(
                "services.whisper: warm (bridge up — ensure via POST /v1/sidecars/ensure)".into(),
            );
        } else {
            notes.push("services.whisper: warm (start `smith serve` to launch sidecar)".into());
        }
    }
    if svc.is_warm(ID_SERVICES_TTS) {
        if port_open(4791) {
            notes.push("services.tts: warm".into());
        } else if os_register::bridge_is_listening() {
            notes.push(
                "services.tts: warm (bridge up — ensure via POST /v1/sidecars/ensure)".into(),
            );
        } else {
            notes.push("services.tts: warm (start `smith serve` to launch sidecar)".into());
        }
    }
    Ok(notes)
}

fn ensure_brain_warm() -> Result<(), String> {
    let mem = crate::brain::ensure_memory_layout()?;
    let model = crate::DEFAULT_LOCAL_MODEL;
    crate::docker::ensure_brain_ready(&mem, Some(model))?;
    Ok(())
}

/// Whether coding agent should auto-start on chat/serve ensure.
pub fn coding_should_autostart() -> bool {
    load_service_config()
        .map(|s| s.is_warm(ID_AGENTS_CODING))
        .unwrap_or(false)
}

pub fn whisper_is_warm() -> bool {
    load_service_config()
        .map(|s| s.is_warm(ID_SERVICES_WHISPER))
        .unwrap_or(false)
}

pub fn tts_is_warm() -> bool {
    load_service_config()
        .map(|s| s.is_warm(ID_SERVICES_TTS))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_searxng_warm() {
        let mut root = ServiceRootConfig {
            autostart: false,
            services: BTreeMap::new(),
        };
        root.ensure_defaults();
        assert_eq!(root.mode_of(ID_SERVICES_SEARXNG), ServiceMode::Warm);
        assert_eq!(root.mode_of(ID_AGENTS_CODING), ServiceMode::OnDemand);
        assert_eq!(root.mode_of(ID_SERVICES_WHISPER), ServiceMode::OnDemand);
    }

    #[test]
    fn autostart_defaults_off_and_old_values_remain_compatible() {
        let missing: ServiceRootConfig = toml::from_str("").unwrap();
        assert!(!missing.autostart);

        let enabled: ServiceRootConfig = toml::from_str("autostart = true").unwrap();
        assert!(enabled.autostart);
    }

    #[test]
    fn old_snapshot_json_deserializes_with_unknown_supervisor_state() {
        let raw = r#"{"autostart":false,"os_registered":false,"services":[]}"#;
        let snapshot: ServicesSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(snapshot.supervisor.load_state, "unknown");
        assert_eq!(snapshot.supervisor.enabled_at_login, None);
    }

    #[test]
    fn host_brain_status_reuses_verified_supervisor_bridge_observation() {
        let unrelated_listener = os_register::ObservedSupervisorStatus {
            bridge_listening: true,
            bridge_identity: "other_listener".into(),
            bridge_healthy: false,
            ..os_register::ObservedSupervisorStatus::default()
        };
        let (running, healthy, _, detail) = probe_service(
            ID_BRAIN,
            &unrelated_listener,
            crate::RuntimeMode::Host,
            false,
        );
        assert!(!running);
        assert!(!healthy);
        assert!(detail.unwrap().contains("other_listener"));

        let smith_bridge = os_register::ObservedSupervisorStatus {
            bridge_listening: true,
            bridge_identity: "smith".into(),
            bridge_healthy: true,
            ..os_register::ObservedSupervisorStatus::default()
        };
        let (running, healthy, _, _) =
            probe_service(ID_BRAIN, &smith_bridge, crate::RuntimeMode::Host, false);
        assert!(running);
        assert!(healthy);
    }

    #[test]
    fn test_probe_suppression_avoids_container_and_loopback_status_checks() {
        let supervisor = os_register::ObservedSupervisorStatus::default();
        for id in [
            ID_BRAIN,
            ID_SANDBOX_DEFAULT,
            ID_AGENTS_CODING,
            ID_SERVICES_SEARXNG,
            ID_SERVICES_OLLAMA,
            ID_SERVICES_WHISPER,
            ID_SERVICES_TTS,
        ] {
            let (running, healthy, _, detail) =
                probe_service(id, &supervisor, crate::RuntimeMode::Container, true);
            assert!(!running, "{id}");
            assert!(!healthy, "{id}");
            assert_eq!(
                detail.as_deref(),
                Some("external status probes suppressed for test"),
                "{id}"
            );
        }
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(resolve_service_id("searxng").unwrap(), ID_SERVICES_SEARXNG);
        assert_eq!(resolve_service_id("coding").unwrap(), ID_AGENTS_CODING);
        assert!(resolve_service_id("nope").is_err());
    }

    #[test]
    fn groups() {
        assert_eq!(service_group(ID_SERVICES_SEARXNG), ServiceGroup::Services);
        assert_eq!(service_group(ID_AGENTS_CODING), ServiceGroup::Agents);
        assert_eq!(service_group(ID_SANDBOX_DEFAULT), ServiceGroup::Sandbox);
    }
}
