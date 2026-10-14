//! Old-owned service maintenance. The caller proves release pairing before supplying its digest.
use crate::control::{enablement_link, EnablementLink};
use crate::{filesystem, MaintenanceManagerFacts, Manager, Result, Roots};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum Action {
    Stop,
    Unregister,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnitKind {
    Missing,
    LegacyStatic,
    Current,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Facts {
    pub unit_present: bool,
    pub running: bool,
    pub registered: bool,
    pub manager: MaintenanceManagerFacts,
}

/// Retains the same data-directory flock used by participating Core operations.
/// Not cloneable and not constructible from executable parameters.
pub struct MaintenanceLease {
    directory: File,
    roots: Roots,
    identities: Vec<RootIdentity>,
}
impl MaintenanceLease {
    pub fn acquire(roots: &Roots) -> Result<Self> {
        roots.validate()?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&roots.data)
            .map_err(|_| "Cannot open Core lock directory")?;
        // SAFETY: flock operates on our held directory descriptor.
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Another operation owns this namespace".into());
        }
        let lease = Self {
            directory,
            roots: roots.clone(),
            identities: root_identities(roots)?,
        };
        lease.validate(roots)?;
        Ok(lease)
    }
    pub(crate) fn validate(&self, roots: &Roots) -> Result<()> {
        roots.validate()?;
        if roots.data != self.roots.data
            || roots.config != self.roots.config
            || roots.runtime != self.roots.runtime
            || root_identities(roots)? != self.identities
        {
            return Err("Maintenance lease roots or path identities changed".into());
        }
        let held = self
            .directory
            .metadata()
            .map_err(|_| "Cannot inspect held Core lock")?;
        let current = fs::symlink_metadata(&roots.data).map_err(|_| "Core lock path missing")?;
        if held.dev() != current.dev() || held.ino() != current.ino() {
            return Err("Core lock directory replaced".into());
        }
        Ok(())
    }
}
// Closing the sole owned descriptor releases flock, including all error paths.

type RootIdentity = (PathBuf, u64, u64, u32, u32);
fn root_identities(roots: &Roots) -> Result<Vec<RootIdentity>> {
    let mut paths = std::collections::BTreeSet::new();
    for root in [&roots.data, &roots.config, &roots.runtime] {
        for path in root.ancestors() {
            paths.insert(path.to_path_buf());
        }
    }
    paths
        .into_iter()
        .map(|path| {
            let m = fs::symlink_metadata(&path).map_err(|_| "Cannot bind root identity")?;
            Ok((path, m.dev(), m.ino(), m.mode(), m.uid()))
        })
        .collect()
}

type PathIdentity = (PathBuf, u64, u64, u32, u32, u64, i64, i64);
#[derive(Serialize)]
struct Snapshot {
    facts: Facts,
    digest: String,
    identities: Vec<PathIdentity>,
}

/// A single-use, original 60-second consent request. Pass by value to apply.
pub struct Prepared {
    action: Action,
    token: String,
    description: String,
    expires: Instant,
    binding: Vec<u8>,
}
impl Prepared {
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn expires(&self) -> Instant {
        self.expires
    }
    fn check(&self, approval: &str) -> Result<()> {
        if approval != self.token {
            return Err("Approval does not match original maintenance request".into());
        }
        if Instant::now() >= self.expires {
            return Err("Original maintenance approval expired".into());
        }
        crate::manager::check_cancellation()
    }
}

pub struct ExistingController<M> {
    pub roots: Roots,
    pub manager: M,
    pub binary_sha256: String,
}
impl<M: Manager> ExistingController<M> {
    pub fn inspect(&self) -> Result<Facts> {
        Ok(self.snapshot()?.facts)
    }
    fn snapshot(&self) -> Result<Snapshot> {
        self.roots.validate()?;
        let binary = filesystem::read_file(&self.roots.binary(), 0o755, filesystem::MAX_FILE)?
            .ok_or("Installed old binary missing")?;
        if self.binary_sha256.len() != 64 || filesystem::digest(&binary) != self.binary_sha256 {
            return Err("Installed old binary differs from captured release identity".into());
        }
        let unit = filesystem::read_file(&self.roots.unit(), 0o600, 16384)?;
        let unit_kind = match unit.as_deref() {
            None => UnitKind::Missing,
            Some(bytes) if bytes == self.roots.legacy_static_unit_text().as_bytes() => {
                UnitKind::LegacyStatic
            }
            Some(bytes) if bytes == self.roots.unit_text().as_bytes() => UnitKind::Current,
            Some(_) => return Err("Unit is not an exact owned legacy or current unit".into()),
        };
        reject_overrides(&self.roots)?;
        let manager = self.manager.maintenance_inspect(&self.roots)?;
        let f = &manager.service;
        let link = enablement_link(&self.roots)?;
        let exact_enablement = match unit_kind {
            UnitKind::Missing => {
                f.load == "not-found"
                    && f.fragment.is_empty()
                    && f.unit_file_state.is_empty()
                    && link == EnablementLink::Missing
            }
            UnitKind::LegacyStatic => {
                f.load == "loaded"
                    && f.fragment == self.roots.unit().to_string_lossy()
                    && f.unit_file_state == "static"
                    && link == EnablementLink::Missing
            }
            UnitKind::Current => {
                f.load == "loaded"
                    && f.fragment == self.roots.unit().to_string_lossy()
                    && ((f.unit_file_state == "disabled" && link == EnablementLink::Missing)
                        || (f.unit_file_state == "enabled" && link == EnablementLink::Exact))
            }
        };
        if !f.drop_ins.is_empty()
            || f.transient
            || f.need_reload
            || !["loaded", "not-found"].contains(&f.load.as_str())
            || manager.unit_file_state != f.unit_file_state
            || !exact_enablement
        {
            return Err("Foreign, malformed, conflicting or unknown service registration".into());
        }
        let running = f.load == "loaded" && f.active == "active" && f.sub == "running" && f.pid > 0;
        let stopped = f.pid == 0
            && ((f.active == "inactive" && f.sub == "dead")
                || (f.active == "failed" && f.sub == "failed"));
        if running {
            self.runtime_identity(f.pid, &manager.control_group)?;
        } else if stopped {
            self.quiet(&manager)?;
        } else {
            return Err("Unknown or transitional runtime blocks maintenance".into());
        }
        let mut paths = std::collections::BTreeSet::new();
        for path in [
            self.roots.binary(),
            self.roots.unit(),
            self.roots.socket(),
            self.roots.app_socket(),
        ] {
            for p in path.ancestors() {
                paths.insert(p.to_path_buf());
            }
        }
        let mut identities = Vec::new();
        for path in paths {
            match fs::symlink_metadata(&path) {
                Ok(m) => identities.push((
                    path,
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.uid(),
                    if m.is_dir() { 0 } else { m.len() },
                    if m.is_dir() { 0 } else { m.mtime() },
                    if m.is_dir() { 0 } else { m.mtime_nsec() },
                )),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Cannot bind maintenance paths".into()),
            }
        }
        Ok(Snapshot {
            facts: Facts {
                unit_present: unit_kind != UnitKind::Missing,
                running,
                registered: f.load == "loaded",
                manager,
            },
            digest: self.binary_sha256.clone(),
            identities,
        })
    }
    fn runtime_identity(&self, pid: u32, group: &str) -> Result<()> {
        if pid > i32::MAX as u32 || group.is_empty() {
            return Err("Running service has no safe cgroup identity".into());
        }
        let proc = PathBuf::from(format!("/proc/{pid}/exe"));
        if fs::read_link(&proc).map_err(|_| "Cannot inspect old runtime executable")?
            != self.roots.binary()
        {
            return Err("Old runtime executable path conflict".into());
        }
        let mut bytes = Vec::new();
        File::open(proc)
            .map_err(|_| "Cannot open old runtime executable")?
            .take(filesystem::MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read old runtime executable")?;
        if filesystem::digest(&bytes) != self.binary_sha256 {
            return Err("Old runtime executable bytes conflict".into());
        }
        let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup"))
            .map_err(|_| "Cannot bind runtime cgroup")?;
        if cgroup != format!("0::{group}\n") {
            return Err("Runtime cgroup identity conflict".into());
        }
        Ok(())
    }
    fn quiet(&self, facts: &MaintenanceManagerFacts) -> Result<()> {
        if facts.service.pid != 0
            || !["inactive", "failed"].contains(&facts.service.active.as_str())
            || !["dead", "failed"].contains(&facts.service.sub.as_str())
        {
            return Err("Service is not verified stopped".into());
        }
        self.manager.quiescence(&self.roots, &facts.control_group)?;
        for path in [self.roots.socket(), self.roots.app_socket()] {
            absent(&path)?;
        }
        Ok(())
    }
    fn binding(&self, action: Action, snapshot: &Snapshot) -> Result<Vec<u8>> {
        serde_json::to_vec(&(action, &self.roots, snapshot))
            .map_err(|_| "Cannot bind maintenance approval".into())
    }
    pub fn prepare(&self, action: Action) -> Result<Prepared> {
        let expires = Instant::now() + Duration::from_secs(60);
        let snapshot = self.snapshot()?;
        if action == Action::Unregister && (!snapshot.facts.registered || snapshot.facts.running) {
            return Err("Unregister requires an owned registered stopped service".into());
        }
        if action == Action::Stop && !snapshot.facts.running {
            return Err("Stop requires an owned running service".into());
        }
        let binding = self.binding(action, &snapshot)?;
        let mut nonce = [0; 32];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut nonce))
            .map_err(|_| "Cannot create original consent identity")?;
        let token = filesystem::digest(&[binding.as_slice(), &nonce].concat());
        let description = match action {
            Action::Stop => "Stop only the verified old-owned smith-next.service; verify PID0, empty cgroup and both sockets absent. No enablement, data deletion or retry.",
            Action::Unregister => "If enabled, disable and verify the exact stopped unit first; then preserve it under a fresh no-clobber name, sync the directory, reload and verify registration absent. Retain all runtime data; no retry or rollback.",
        }.to_owned();
        let prepared = Prepared {
            action,
            token,
            description,
            expires,
            binding,
        };
        prepared.check(prepared.token())?;
        Ok(prepared)
    }
    pub fn apply(&self, prepared: Prepared, approval: &str) -> Result<Facts> {
        let lease = MaintenanceLease::acquire(&self.roots)?;
        self.apply_locked(prepared, approval, &lease)
    }
    pub fn apply_locked(
        &self,
        prepared: Prepared,
        approval: &str,
        lease: &MaintenanceLease,
    ) -> Result<Facts> {
        lease.validate(&self.roots)?;
        prepared.check(approval)?;
        let before = self.snapshot()?;
        if self.binding(prepared.action, &before)? != prepared.binding {
            return Err("Original maintenance action, identity, paths or facts changed".into());
        }
        lease.validate(&self.roots)?;
        prepared.check(approval)?;
        match prepared.action {
            Action::Stop => {
                self.manager.stop(&self.roots)?;
                let after = self.snapshot()?;
                self.quiet(&after.facts.manager)?;
                // The manager can clear ControlGroup after Stop; also prove the captured group empty.
                self.manager
                    .quiescence(&self.roots, &before.facts.manager.control_group)?;
                lease.validate(&self.roots)?;
                Ok(after.facts)
            }
            Action::Unregister => {
                if before.facts.running || !before.facts.registered {
                    return Err("Unregister requires stopped registration".into());
                }
                let mut unregister = before;
                if unregister.facts.manager.service.unit_file_state == "enabled" {
                    self.manager.disable(&self.roots)?;
                    unregister = self.snapshot()?;
                    if unregister.facts.manager.service.unit_file_state != "disabled" {
                        return Err(
                            "Persistent service disablement unconfirmed; unit preserved".into()
                        );
                    }
                }
                self.quiet(&unregister.facts.manager)?;
                lease.validate(&self.roots)?;
                prepared.check(approval)?;
                preserve_unit(&self.roots, &prepared.token)?;
                lease.validate(&self.roots)?;
                prepared.check(approval)?;
                self.manager.reload(&self.roots)?;
                let after = self.snapshot()?;
                self.quiet(&after.facts.manager)?;
                self.manager
                    .quiescence(&self.roots, &unregister.facts.manager.control_group)?;
                if after.facts.registered || after.facts.unit_present {
                    return Err("Registration removal unconfirmed; retained unit preserved".into());
                }
                lease.validate(&self.roots)?;
                Ok(after.facts)
            }
        }
    }
}
fn absent(path: &Path) -> Result<()> {
    filesystem::check_ancestors(path)?;
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("Runtime socket or conflicting object remains; preserved".into()),
    }
}
fn reject_overrides(roots: &Roots) -> Result<()> {
    for base in [
        roots.config.join("systemd/user.control"),
        roots.runtime.join("systemd/user.control"),
    ] {
        absent(&base.join("smith-next.service"))?;
        absent(&base.join("smith-next.service.d"))?;
    }
    for base in [
        roots.config.join("systemd/user"),
        roots.runtime.join("systemd/user"),
        PathBuf::from("/etc/systemd/user"),
        PathBuf::from("/run/systemd/user"),
        PathBuf::from("/usr/local/lib/systemd/user"),
        PathBuf::from("/usr/lib/systemd/user"),
    ] {
        for name in ["smith-next.service.d", "smith-.service.d", "service.d"] {
            absent(&base.join(name)).map_err(|_| "On-disk service overrides block maintenance")?;
        }
        let unit = base.join("smith-next.service");
        if unit != roots.unit() {
            absent(&unit).map_err(|_| "Alternate on-disk unit blocks maintenance")?;
        }
    }
    Ok(())
}
fn preserve_unit(roots: &Roots, token: &str) -> Result<()> {
    use std::ffi::CString;
    let path = roots.unit();
    filesystem::check_ancestors(&path)?;
    let parent = path.parent().ok_or("Unit parent missing")?;
    let dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(parent)
        .map_err(|_| "Cannot hold unit directory")?;
    let held_parent = dir
        .metadata()
        .map_err(|_| "Cannot inspect held unit directory")?;
    let current_parent = fs::symlink_metadata(parent).map_err(|_| "Unit parent disappeared")?;
    if held_parent.dev() != current_parent.dev() || held_parent.ino() != current_parent.ino() {
        return Err("Unit directory identity changed".into());
    }
    let old = fs::symlink_metadata(&path).map_err(|_| "Unit disappeared")?;
    let bytes = filesystem::read_file(&path, 0o600, 16384)?.ok_or("Unit disappeared")?;
    if bytes != roots.unit_text().as_bytes() && bytes != roots.legacy_static_unit_text().as_bytes()
    {
        return Err("Unit changed before preservation".into());
    }
    let destination = format!(".smith-next.service-retained-{token}");
    let name = CString::new(destination.clone()).map_err(|_| "Invalid retained unit name")?;
    // SAFETY: held directory, fixed source and validated fresh destination; NOREPLACE never clobbers.
    if unsafe {
        libc::renameat2(
            dir.as_raw_fd(),
            c"smith-next.service".as_ptr(),
            dir.as_raw_fd(),
            name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err("Cannot preserve unit without clobber; no reload attempted".into());
    }
    dir.sync_all()
        .map_err(|_| "Retained unit directory sync failed; no rollback")?;
    let retained = parent.join(destination);
    let moved = fs::symlink_metadata(&retained).map_err(|_| "Cannot verify retained unit")?;
    if old.dev() != moved.dev()
        || old.ino() != moved.ino()
        || old.mode() != moved.mode()
        || old.nlink() != moved.nlink()
        || old.uid() != moved.uid()
        || filesystem::read_file(&retained, 0o600, 16384)?.as_deref() != Some(bytes.as_slice())
        || fs::symlink_metadata(parent)
            .map_err(|_| "Unit directory disappeared")?
            .ino()
            != held_parent.ino()
    {
        return Err("Moved unit identity changed; no reload or rollback".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_consent_is_monotonic_expiring_and_cannot_be_substituted() {
        let mut prepared = Prepared {
            action: Action::Stop,
            token: "original".into(),
            description: "fixed effects".into(),
            expires: Instant::now() + Duration::from_secs(60),
            binding: vec![1, 2, 3],
        };
        assert_eq!(prepared.token(), "original");
        assert_eq!(prepared.description(), "fixed effects");
        assert!(prepared.check("substitute").is_err());
        assert!(prepared.check("original").is_ok());
        prepared.expires = Instant::now();
        assert!(prepared.check("original").unwrap_err().contains("expired"));
    }
}
