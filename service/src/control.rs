use crate::filesystem::{self, MAX_FILE};
use crate::{request, Manager, ManagerFacts, Result, Roots};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    InstallBinary,
    InstallUnit,
    Enable,
    Start,
    CompleteSetup,
}
impl Action {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "install-binary" => Ok(Self::InstallBinary),
            "install-unit" => Ok(Self::InstallUnit),
            "enable" => Ok(Self::Enable),
            "start" => Ok(Self::Start),
            "complete-setup" => Ok(Self::CompleteSetup),
            _ => Err("Unsupported action".into()),
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::InstallBinary => "Create private smith-next/bin directories and publish the exact helper executable; no unit registration or start.",
            Self::InstallUnit => "Publish the exact smith-next.service unit if missing, then reload the user manager; no start, enable, autostart or lingering.",
            Self::Enable => "Enable only the exact smith-next.service for persistent default-target startup at user login; do not start it now or change lingering.",
            Self::Start => "Start only smith-next.service through the user manager; create its private runtime namespace/socket. Service survives desktop detach; no enable or detached fallback.",
            Self::CompleteSetup => "Ask the verified managed service to atomically sync setup_complete=true in its private state file; no provider or task actions.",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub binary_path: PathBuf,
    pub unit_path: PathBuf,
    pub state_path: PathBuf,
    pub socket_path: PathBuf,
    pub description: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub schema_version: u32,
    pub service: String,
    pub ok: bool,
    pub binary_state: String,
    pub unit_state: String,
    pub enablement_state: String,
    pub runtime_state: String,
    pub bridge_state: String,
    pub setup_complete: Option<bool>,
    pub message: String,
    pub error: Option<String>,
    pub effects: Effects,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub approval_token: Option<String>,
}
type PathFact = (PathBuf, u64, u64, u32, u32, u64, i64, i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnablementLink {
    Missing,
    Exact,
    Conflict,
}

pub(crate) fn enablement_link(roots: &Roots) -> Result<EnablementLink> {
    let path = roots.enablement_link();
    filesystem::check_ancestors(path.parent().ok_or("Enablement link parent missing")?)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EnablementLink::Missing)
        }
        Err(_) => return Err("Cannot inspect default-target enablement link".into()),
    };
    if !metadata.file_type().is_symlink()
        || metadata.uid() != filesystem::uid()
        || metadata.nlink() != 1
    {
        return Ok(EnablementLink::Conflict);
    }
    match fs::read_link(path) {
        Ok(target) if target == roots.unit() => Ok(EnablementLink::Exact),
        Ok(_) => Ok(EnablementLink::Conflict),
        Err(_) => Err("Cannot read default-target enablement link".into()),
    }
}

#[derive(Serialize)]
struct Snapshot {
    roots: Roots,
    source: PathBuf,
    source_digest: String,
    unit_digest: String,
    observation: Observation,
    manager: Option<ManagerFacts>,
    metadata: Vec<PathFact>,
    state_digest: Option<String>,
}

pub struct Controller<M: Manager> {
    pub roots: Roots,
    pub source: PathBuf,
    pub manager: M,
}
impl<M: Manager> Controller<M> {
    fn empty(&self, action: Option<Action>) -> Observation {
        Observation {
            schema_version: 2,
            service: "smith-next".into(),
            ok: true,
            binary_state: "unknown".into(),
            unit_state: "unknown".into(),
            enablement_state: "unknown".into(),
            runtime_state: "unknown".into(),
            bridge_state: "unknown".into(),
            setup_complete: None,
            message: "Read-only debug service observation".into(),
            error: None,
            effects: Effects {
                binary_path: self.roots.binary(),
                unit_path: self.roots.unit(),
                state_path: self.roots.state(),
                socket_path: self.roots.socket(),
                description: action
                    .map(Action::description)
                    .unwrap_or(
                        "Read only; no installation, manager mutation, start or setup completion.",
                    )
                    .into(),
            },
            approval_token: None,
        }
    }
    pub fn failure(&self, error: String) -> Observation {
        let mut o = self.empty(None);
        o.ok = false;
        o.message = "Action failed; no automatic retry or rollback".into();
        o.error = Some(error);
        o
    }
    pub fn inspect(&self) -> Observation {
        self.observe().0
    }
    fn observe(&self) -> (Observation, Option<ManagerFacts>) {
        let mut o = self.empty(None);
        if let Err(e) = self.roots.validate() {
            o.ok = false;
            o.error = Some(e);
            return (o, None);
        }
        let source = filesystem::read_file(&self.source, 0o755, MAX_FILE);
        let installed = filesystem::read_file(&self.roots.binary(), 0o755, MAX_FILE);
        o.binary_state = match (&source, &installed) {
            (Ok(Some(a)), Ok(Some(b))) if a == b => "installed",
            (Ok(Some(_)), Ok(None)) => "missing",
            _ => "conflict",
        }
        .into();
        o.unit_state = match filesystem::read_file(&self.roots.unit(), 0o600, 16384) {
            Ok(None) => "missing",
            Ok(Some(v)) if v == self.roots.unit_text().as_bytes() => "installed",
            _ => "conflict",
        }
        .into();
        let link = enablement_link(&self.roots);
        if matches!(link, Ok(EnablementLink::Conflict)) {
            o.enablement_state = "conflict".into();
        }
        match filesystem::state(&self.roots) {
            Ok(v) => o.setup_complete = Some(v),
            Err(e) => {
                o.ok = false;
                o.error = Some(e);
            }
        }
        let facts = match self.manager.inspect(&self.roots) {
            Ok(f) => f,
            Err(_) => {
                o.runtime_state = "unavailable".into();
                o.bridge_state = "unavailable".into();
                return (o, None);
            }
        };
        if self.reload_only_repair(&o, &facts) {
            o.message = "Exact installed unit requires a separately approved reload-only repair before Start".into();
        }
        if (facts.load == "loaded" && facts.fragment != self.roots.unit().to_string_lossy())
            || !["loaded", "not-found"].contains(&facts.load.as_str())
            || !facts.drop_ins.is_empty()
            || (facts.need_reload && !self.reload_only_repair(&o, &facts))
            || facts.transient
        {
            o.unit_state = "conflict".into();
        }
        o.enablement_state = match (
            o.unit_state.as_str(),
            facts.load.as_str(),
            facts.unit_file_state.as_str(),
            link,
        ) {
            ("missing", "not-found", "", Ok(EnablementLink::Missing)) => "missing",
            ("installed", "loaded", "disabled", Ok(EnablementLink::Missing))
            | ("installed", "not-found", "", Ok(EnablementLink::Missing)) => "disabled",
            ("installed", "loaded", "enabled", Ok(EnablementLink::Exact)) => "enabled",
            (_, _, _, Err(_)) => "unknown",
            _ => "conflict",
        }
        .into();
        let socket_exists = fs::symlink_metadata(self.roots.socket()).is_ok();
        let runtime_path_exists =
            socket_exists || fs::symlink_metadata(self.roots.app_socket()).is_ok();
        o.runtime_state = if facts.pid == 0
            && ["inactive", "failed"].contains(&facts.active.as_str())
            && ["dead", "failed"].contains(&facts.sub.as_str())
        {
            if runtime_path_exists {
                "conflict"
            } else {
                "stopped"
            }
        } else if facts.active == "active" && facts.sub == "running" && facts.pid > 0 {
            if o.binary_state == "installed"
                && o.unit_state == "installed"
                && facts.load == "loaded"
                && facts.fragment == self.roots.unit().to_string_lossy()
                && self.runtime_binary(facts.pid).is_ok()
            {
                "running"
            } else {
                "conflict"
            }
        } else {
            "unknown"
        }
        .into();
        o.bridge_state = "unavailable".into();
        if o.runtime_state == "conflict" {
            o.bridge_state = "conflict".into();
        }
        if o.runtime_state == "running" {
            match request(&self.roots, facts.pid, "get_state") {
                Ok(reply) if reply.setup_complete == o.setup_complete && o.ok => {
                    o.bridge_state = "ready".into();
                }
                Ok(_) => {
                    o.bridge_state = "conflict".into();
                    o.setup_complete = None;
                    o.ok = false;
                    o.error = Some("Bridge and durable state disagree".into());
                }
                Err(e) => {
                    o.bridge_state = if socket_exists {
                        "conflict"
                    } else {
                        "unavailable"
                    }
                    .into();
                    o.ok = false;
                    o.error = Some(e);
                }
            }
        }
        (o, Some(facts))
    }
    /// Verify the managed runtime using the same current facts as lifecycle control.
    pub fn app(&self, request: crate::app::AppRequest) -> Result<crate::app::AppReply> {
        request.validate().map_err(|e| e.message)?;
        let (observation, facts) = self.observe();
        if !observation.ok
            || observation.binary_state != "installed"
            || observation.unit_state != "installed"
            || observation.runtime_state != "running"
            || observation.bridge_state != "ready"
        {
            return Err("Application requires a verified installed managed service".into());
        }
        let facts = facts.ok_or("Managed runtime facts unavailable")?;
        crate::app_transport::request(&self.roots, facts.pid, &request)
    }
    fn runtime_binary(&self, pid: u32) -> Result<()> {
        let proc = PathBuf::from(format!("/proc/{pid}/exe"));
        if fs::read_link(&proc).map_err(|_| "Cannot inspect managed executable")?
            != self.roots.binary()
        {
            return Err("Managed executable path conflict".into());
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        fs::File::open(proc)
            .map_err(|_| "Cannot verify runtime executable")?
            .take(MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read runtime identity")?;
        let source =
            filesystem::read_file(&self.source, 0o755, MAX_FILE)?.ok_or("Helper missing")?;
        if bytes != source {
            return Err("Managed executable digest conflict".into());
        }
        Ok(())
    }
    fn snapshot(&self, action: Action) -> Result<Snapshot> {
        self.roots.validate()?;
        let source =
            filesystem::read_file(&self.source, 0o755, MAX_FILE)?.ok_or("Helper missing")?;
        let (mut observation, manager) = self.observe();
        observation.effects.description = action_description(action, &observation);
        self.allowed(action, &observation, manager.as_ref())?;
        let mut paths = std::collections::BTreeSet::new();
        for path in [
            &self.roots.data,
            &self.roots.config,
            &self.roots.runtime,
            &self.source,
            &self.roots.binary(),
            &self.roots.unit(),
            &self.roots.enablement_link(),
            &self.roots.state(),
            &self.roots.socket(),
        ] {
            for p in path.ancestors() {
                paths.insert(p.to_path_buf());
            }
        }
        let mut metadata = Vec::new();
        for p in paths {
            match fs::symlink_metadata(&p) {
                Ok(m) => metadata.push((
                    p,
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.uid(),
                    if m.is_dir() { 0 } else { m.len() },
                    if m.is_dir() { 0 } else { m.mtime() },
                    if m.is_dir() { 0 } else { m.mtime_nsec() },
                )),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Cannot bind filesystem facts".into()),
            }
        }
        Ok(Snapshot {
            roots: self.roots.clone(),
            source: self.source.clone(),
            source_digest: filesystem::digest(&source),
            unit_digest: filesystem::digest(self.roots.unit_text().as_bytes()),
            observation,
            manager,
            metadata,
            state_digest: filesystem::read_file(&self.roots.state(), 0o600, 4096)?
                .map(|v| filesystem::digest(&v)),
        })
    }
    fn reload_only_repair(&self, o: &Observation, facts: &ManagerFacts) -> bool {
        o.binary_state == "installed"
            && o.unit_state == "installed"
            && facts.load == "loaded"
            && facts.fragment == self.roots.unit().to_string_lossy()
            && facts.active == "inactive"
            && facts.sub == "dead"
            && facts.pid == 0
            && facts.need_reload
            && facts.drop_ins.is_empty()
            && !facts.transient
            && fs::symlink_metadata(self.roots.socket())
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    }
    fn allowed(&self, action: Action, o: &Observation, facts: Option<&ManagerFacts>) -> Result<()> {
        if !o.ok
            || o.binary_state == "conflict"
            || o.unit_state == "conflict"
            || ["conflict", "unknown"].contains(&o.enablement_state.as_str())
            || ["conflict", "unknown"].contains(&o.runtime_state.as_str())
        {
            return Err("Unsafe, corrupt or uncertain observations block approval".into());
        }
        let allowed = match action {
            Action::InstallBinary => {
                o.binary_state == "missing"
                    && o.unit_state == "missing"
                    && !self.roots.socket().exists()
            }
            Action::InstallUnit => {
                o.binary_state == "installed"
                    && o.runtime_state == "stopped"
                    && facts.is_some_and(|f| {
                        (f.load == "not-found" && f.fragment.is_empty() && !f.need_reload)
                            || self.reload_only_repair(o, f)
                    })
            }
            Action::Enable => {
                o.binary_state == "installed"
                    && o.unit_state == "installed"
                    && o.enablement_state == "disabled"
                    && o.runtime_state == "stopped"
                    && facts.is_some_and(|f| {
                        f.load == "loaded"
                            && f.fragment == self.roots.unit().to_string_lossy()
                            && !f.need_reload
                    })
            }
            Action::Start => {
                o.binary_state == "installed"
                    && o.unit_state == "installed"
                    && o.enablement_state == "enabled"
                    && o.runtime_state == "stopped"
                    && facts.is_some_and(|f| {
                        f.load == "loaded"
                            && f.fragment == self.roots.unit().to_string_lossy()
                            && !f.need_reload
                    })
            }
            Action::CompleteSetup => o.bridge_state == "ready" && o.setup_complete == Some(false),
        };
        if allowed {
            Ok(())
        } else {
            Err("Action is not applicable to freshly verified state".into())
        }
    }
    fn binding(&self, action: Action, issued: u64, snapshot: &Snapshot) -> Result<String> {
        let bytes = serde_json::to_vec(&(2, "smith-next", action, issued, snapshot))
            .map_err(|_| "Cannot bind approval")?;
        Ok(filesystem::digest(&bytes))
    }
    pub fn prepare(&self, action: Action) -> Observation {
        match self.snapshot(action).and_then(|s| {
            let issued = now()?;
            let token = format!("{issued}.{}", self.binding(action, issued, &s)?);
            Ok((s, token))
        }) {
            Ok((s, token)) => {
                let mut o = s.observation;
                o.approval_token = Some(token);
                o.message =
                    "Confirm these exact effects within 60 seconds; preparation made no changes"
                        .into();
                o
            }
            Err(e) => {
                let mut o = self.inspect();
                o.ok = false;
                o.error = Some(e);
                o.effects.description = action.description().into();
                o
            }
        }
    }
    pub fn apply(&self, action: Action, approval: &str) -> Observation {
        match crate::MaintenanceLease::acquire(&self.roots) {
            Ok(lease) => self.apply_with_lease(action, approval, &lease),
            Err(error) => {
                let mut observation = self.inspect();
                observation.effects.description = action.description().to_owned();
                observation.ok = false;
                observation.message =
                    "Action failed; intermediate state preserved, no automatic retry".into();
                observation.error = Some(error);
                observation
            }
        }
    }
    /// Candidate lifecycle under the installer's held Core lease; never recursively flock.
    pub fn apply_with_lease(
        &self,
        action: Action,
        approval: &str,
        lease: &crate::MaintenanceLease,
    ) -> Observation {
        let mut description = action.description().to_owned();
        let result = (|| {
            lease.validate(&self.roots)?;
            let (issued, hash) = approval.split_once('.').ok_or("Malformed approval token")?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || issued.len() > 20
            {
                return Err("Malformed approval token".into());
            }
            let issued: u64 = issued.parse().map_err(|_| "Malformed approval time")?;
            check_time(issued)?;
            let snapshot = self.snapshot(action)?;
            description = snapshot.observation.effects.description.clone();
            if hash != self.binding(action, issued, &snapshot)? {
                return Err("Approval action, paths, source or current facts changed".into());
            }
            // Inspection can consume the approval window; recheck before any effects.
            check_time(issued)?;
            lease.validate(&self.roots)?;
            crate::manager::check_cancellation()?;
            match action {
                Action::InstallBinary => {
                    filesystem::mkdir(&self.roots.data.join("smith-next/bin"))?;
                    let source = filesystem::read_file(&self.source, 0o755, MAX_FILE)?
                        .ok_or("Helper missing")?;
                    if filesystem::digest(&source) != snapshot.source_digest {
                        return Err("Helper changed before publication".into());
                    }
                    filesystem::publish(&self.roots.binary(), &source, 0o755, false)?;
                }
                Action::InstallUnit => {
                    if snapshot.observation.unit_state == "missing" {
                        filesystem::mkdir(&self.roots.config.join("systemd/user"))?;
                        filesystem::publish(
                            &self.roots.unit(),
                            self.roots.unit_text().as_bytes(),
                            0o600,
                            false,
                        )?;
                    }
                    self.manager.reload(&self.roots)?;
                }
                Action::Enable => self.manager.enable(&self.roots)?,
                Action::Start => self.manager.start(&self.roots)?,
                Action::CompleteSetup => {
                    let facts = snapshot.manager.ok_or("Manager unavailable")?;
                    self.runtime_binary(facts.pid)?;
                    check_time(issued)?;
                    request(&self.roots, facts.pid, "complete_setup")?;
                }
            }
            Ok(())
        })();
        let mut o = self.inspect();
        o.effects.description = description;
        match result {
            Ok(()) => {
                o.message =
                    "Approved action applied; readiness is a separate fresh observation".into();
            }
            Err(e) => {
                o.ok = false;
                o.message =
                    "Action failed; intermediate state preserved, no automatic retry".into();
                o.error = Some(e);
            }
        }
        o
    }
}
fn action_description(action: Action, o: &Observation) -> String {
    if action == Action::InstallUnit && o.unit_state == "installed" {
        "Reload only the user manager for the exact installed smith-next.service; no unit overwrite, start, enable, autostart or lingering.".into()
    } else {
        action.description().into()
    }
}

fn check_time(issued: u64) -> Result<()> {
    let age = now()?
        .checked_sub(issued)
        .ok_or("Approval is from the future")?;
    if age >= 60 {
        Err("Approval expired; prepare again".into())
    } else {
        Ok(())
    }
}

fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| "Clock unavailable".into())
}
