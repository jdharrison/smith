//! Explicit existing-installation maintenance. Never executes an installed helper.
use crate::artifact::{Installer, Plan};
use crate::fs::{self, require, Effects, Fact, FileData, Filesystem, Trace};
use crate::manifest::{digest, APP};
use crate::{consent, Result};
use smith_service::maintenance::{Action, ExistingController, MaintenanceLease};
use smith_service::{
    Controller, MaintenanceManagerFacts, Manager, ManagerFacts, Roots, SystemManager,
};
use std::cell::RefCell;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Upgrade,
    Uninstall,
}
pub trait Prompt {
    fn confirm(&mut self, title: &str, effects: &[String], deadline: Instant) -> Result<bool>;
    fn progress(&mut self, message: &str) -> Result<()>;
}
#[derive(Debug)]
pub struct Outcome {
    pub complete: bool,
    pub message: String,
    pub details: Vec<String>,
}
pub trait Runner {
    fn execute(&mut self, op: Operation, prompt: &mut dyn Prompt) -> Result<Outcome>;
}
pub struct Native<M = SystemManager> {
    bundle: PathBuf,
    home: PathBuf,
    roots: Roots,
    manager: M,
    effects: Effects,
}
impl Native<SystemManager> {
    pub fn new(bundle: PathBuf) -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME required")?;
        Self::with_roots(bundle, home, Roots::environment()?, SystemManager)
    }
}

// Avoid requiring Clone on injected managers or widening the service public API.
struct Borrowed<'a, M>(&'a M);
impl<M: Manager> Manager for Borrowed<'_, M> {
    fn inspect(&self, r: &Roots) -> Result<ManagerFacts> {
        self.0.inspect(r)
    }
    fn reload(&self, r: &Roots) -> Result<()> {
        self.0.reload(r)
    }
    fn start(&self, r: &Roots) -> Result<()> {
        self.0.start(r)
    }
    fn enable(&self, r: &Roots) -> Result<()> {
        self.0.enable(r)
    }
    fn disable(&self, r: &Roots) -> Result<()> {
        self.0.disable(r)
    }
    fn maintenance_inspect(&self, r: &Roots) -> Result<MaintenanceManagerFacts> {
        self.0.maintenance_inspect(r)
    }
    fn stop(&self, r: &Roots) -> Result<()> {
        self.0.stop(r)
    }
    fn quiescence(&self, r: &Roots, g: &str) -> Result<()> {
        self.0.quiescence(r, g)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Installed {
    sha: String,
    trace: Trace,
    objects: Vec<(PathBuf, Fact, Option<FileData>)>,
    missing: Vec<PathBuf>,
    unit: Option<(Fact, FileData)>,
    legacy_static_unit: bool,
}
fn confirm(
    prompt: &mut dyn Prompt,
    title: &str,
    effects: &[String],
    deadline: Instant,
) -> Result<bool> {
    consent::check()?;
    require(Instant::now() < deadline, "Maintenance consent expired")?;
    let accepted = prompt.confirm(title, effects, deadline)?;
    consent::check()?;
    require(
        Instant::now() < deadline,
        "Maintenance consent expired; no automatic reprepare",
    )?;
    Ok(accepted)
}
impl<M: Manager> Native<M> {
    /// Library-only injection; never an executable manager/root override.
    pub fn with_roots(bundle: PathBuf, home: PathBuf, roots: Roots, manager: M) -> Result<Self> {
        fs::path_parts(&home)?;
        let bundle =
            std::path::absolute(bundle).map_err(|_| "Cannot resolve maintenance package path")?;
        roots.validate()?;
        require(
            fs::effective_uid() != 0,
            "Maintenance requires an unprivileged Linux user",
        )?;
        Ok(Self {
            bundle,
            home,
            roots,
            manager,
            effects: Rc::new(RefCell::new(Vec::new())),
        })
    }
    fn filesystem(&self) -> Filesystem {
        Filesystem::new(self.effects.clone())
    }
    fn publisher(&self, candidate: bool) -> Result<Installer> {
        let mut publisher = Installer::with_roots(
            candidate.then(|| self.bundle.join("payload/client")),
            Some(if candidate {
                self.bundle.join("payload/service/smith-service")
            } else {
                self.roots.binary()
            }),
            Some(self.home.clone()),
            self.roots.data.clone(),
        )?;
        publisher.maintenance_effects(self.effects.clone());
        Ok(publisher)
    }
    fn existing(&self, old: &Installed) -> ExistingController<Borrowed<'_, M>> {
        ExistingController {
            roots: self.roots.clone(),
            manager: Borrowed(&self.manager),
            binary_sha256: old.sha.clone(),
        }
    }
    fn inspect(&self) -> Result<Installed> {
        consent::check()?;
        self.roots.validate()?;
        let mut fs = self.filesystem();
        require(
            fs.directory(&self.home, false, false)?.is_some(),
            "Existing HOME required",
        )?;
        for path in [
            self.roots.data.join("smith-next"),
            self.roots.data.join("smith-next/bin"),
            self.roots.data.join("smith-next/client"),
            self.roots.data.join("smith-next/client/releases"),
        ] {
            require(
                fs.directory(&path, false, true)?.is_some(),
                "Incomplete installed namespaces",
            )?;
        }
        let publisher = self.publisher(false)?;
        let selector = publisher
            .current_state(&mut fs)?
            .ok_or("Missing selected release; ownership ambiguous")?;
        let current = self.roots.data.join("smith-next/client/current");
        let release = self
            .roots
            .data
            .join("smith-next/client")
            .join(&selector.target);
        for (name, mode) in [
            ("smith_client".to_owned(), 0o755),
            (format!("share/pixmaps/{APP}.png"), 0o644),
        ] {
            require(
                fs.file(&release.join(name), Some(mode))?
                    .is_some_and(|file| !file.bytes.is_empty()),
                "Selected release lacks a safe client/icon",
            )?;
        }
        let helper = fs
            .file(
                &self
                    .roots
                    .data
                    .join("smith-next/client")
                    .join(&selector.target)
                    .join("smith_backend/smith-service"),
                Some(0o755),
            )?
            .ok_or("Selected helper missing")?;
        let binary = fs
            .file(&self.roots.binary(), Some(0o755))?
            .ok_or("Installed binary missing")?;
        require(
            !helper.bytes.is_empty() && helper == binary,
            "Installed binary differs from manifest-verified selected helper",
        )?;
        let executable = current.join("smith_client");
        let icon = current.join(format!("share/pixmaps/{APP}.png"));
        let desktop = format!("[Desktop Entry]\nType=Application\nName=smith\nComment=Smith desktop client\nExec={}\nIcon={}\nStartupWMClass={APP}\nTerminal=false\nCategories=Utility;\n", executable.display(), icon.display());
        let shim = format!(
            "#!/bin/sh\n# Managed by Smith next artifact installer.\nexec \"{}\" \"$@\"\n",
            executable.display()
        );
        let mut objects = Vec::new();
        let mut missing = Vec::new();
        for (path, expected) in [
            (
                self.roots.data.join(format!("applications/{APP}.desktop")),
                FileData {
                    bytes: desktop.into_bytes(),
                    mode: 0o644,
                },
            ),
            (
                self.home.join(".local/bin/smith-next"),
                FileData {
                    bytes: shim.into_bytes(),
                    mode: 0o755,
                },
            ),
        ] {
            let found = fs.file(&path, Some(expected.mode))?;
            require(
                found.as_ref().is_none_or(|v| v == &expected),
                "Foreign/modified desktop or command shim",
            )?;
            if let Some(found) = found {
                objects.push((
                    path.clone(),
                    fs.trace
                        .get(&path)
                        .ok_or("Missing object identity")?
                        .clone(),
                    Some(found),
                ));
            } else {
                missing.push(path);
            }
        }
        objects.push((current, Fact::Selector(selector), None));
        objects.push((
            self.roots.binary(),
            fs.trace
                .get(&self.roots.binary())
                .ok_or("Binary identity missing")?
                .clone(),
            Some(binary.clone()),
        ));
        let unit_data = fs.file(&self.roots.unit(), Some(0o600))?;
        let legacy_static_unit = unit_data
            .as_ref()
            .is_some_and(|unit| unit.bytes == self.roots.legacy_static_unit_text().as_bytes());
        require(
            unit_data.as_ref().is_none_or(|unit| {
                legacy_static_unit || unit.bytes == self.roots.unit_text().as_bytes()
            }),
            "Unit is not an exact owned legacy or current unit",
        )?;
        let unit = unit_data.map(|data| {
            let fact = fs
                .trace
                .get(&self.roots.unit())
                .expect("inspected unit identity")
                .clone();
            (fact, data)
        });
        Ok(Installed {
            sha: digest(&binary.bytes),
            trace: fs.trace,
            objects,
            missing,
            unit,
            legacy_static_unit,
        })
    }
    fn revalidate(&self, old: &Installed) -> Result<()> {
        require(
            self.inspect()? == *old,
            "Installed objects changed after review; no retry",
        )
    }
    fn backup(&self, old: &Installed, op: Operation) -> Result<PathBuf> {
        let mut fs = self.filesystem();
        let parent = self.roots.data.join("smith-next");
        let fd = fs
            .directory(&parent, false, true)?
            .ok_or("Backup parent missing")?;
        let name = fs::stage_name(".smith-next-maintenance-")?;
        let root = parent.join(&name);
        fs::mkdir_at(fd.as_raw_fd(), &name, &root)?;
        fs.effect(format!(
            "created private maintenance backup {}",
            root.display()
        ));
        fs::sync(fd.as_raw_fd(), &parent)?;
        let mut record = format!("Operation: {op:?}\nOld helper SHA256: {}\nNo crash replay, rollback or global transaction. All states/models/releases retained.\n", old.sha);
        for (index, (path, fact, data)) in old.objects.iter().enumerate() {
            record.push_str(&format!("Object {index}: {} {fact:?}\n", path.display()));
            if let Some(data) = data {
                fs.publish(&root.join(format!("object-{index}")), &data.bytes, 0o600)?;
            }
        }
        if let Some((_, unit)) = &old.unit {
            require(
                unit.bytes == self.roots.unit_text().as_bytes()
                    || unit.bytes == self.roots.legacy_static_unit_text().as_bytes(),
                "Unit changed before backup",
            )?;
            fs.publish(&root.join("unit"), &unit.bytes, 0o600)?;
            record.push_str("Exact old legacy/current unit preserved as unit.\n");
        }
        fs.publish(&root.join("effects.txt"), record.as_bytes(), 0o600)?;
        Ok(root)
    }
    fn migrate_legacy_unit(&self, old: &Installed) -> Result<bool> {
        if !old.legacy_static_unit {
            return Ok(false);
        }
        let (fact, unit) = old.unit.as_ref().ok_or("Legacy unit missing")?;
        require(
            unit.bytes == self.roots.legacy_static_unit_text().as_bytes(),
            "Legacy unit changed before migration",
        )?;
        self.filesystem().exchange_file(
            &self.roots.unit(),
            self.roots.unit_text().as_bytes(),
            0o600,
            fact,
            ".smith-next-unit-upgrade-",
        )?;
        self.manager.reload(&self.roots)?;
        Ok(true)
    }

    fn runner_lease(&self) -> Result<OwnedFd> {
        let mut fs = self.filesystem();
        let parent = self.roots.runtime.join("smith-next");
        let directory = fs
            .directory(&parent, true, true)?
            .ok_or("Runner parent missing")?;
        let path = parent.join("runner.lock");
        let fd = fs::open_at(
            directory.as_raw_fd(),
            "runner.lock",
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            0o600,
        )
        .map_err(|_| "Cannot open runner lease")?;
        let identity = fs::verify_lock(fd.as_raw_fd())?;
        // Same flock as the service loader/remover. Never wait/retry behind a runner.
        require(
            unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "Runner lease busy; no maintenance publication",
        )?;
        fs::verify_named_identity(directory.as_raw_fd(), "runner.lock", &identity)?;
        fs::sync(fd.as_raw_fd(), &path)?;
        fs::sync(directory.as_raw_fd(), &parent)?;
        Ok(fd)
    }
    fn validate_runner_lease(&self, lease: &OwnedFd) -> Result<()> {
        let mut fs = self.filesystem();
        let parent = fs
            .directory(&self.roots.runtime.join("smith-next"), false, true)?
            .ok_or("Runner namespace missing")?;
        let identity = fs::verify_lock(lease.as_raw_fd())?;
        fs::verify_named_identity(parent.as_raw_fd(), "runner.lock", &identity)
    }

    fn core_action(
        &self,
        candidate: &Controller<Borrowed<'_, M>>,
        action: smith_service::Action,
        lease: &MaintenanceLease,
        prompt: &mut dyn Prompt,
        installation: &OwnedFd,
        backup: &std::path::Path,
    ) -> Result<bool> {
        let deadline = Instant::now() + Duration::from_secs(60);
        let prepared = candidate.prepare(action);
        require(
            prepared.ok,
            prepared
                .error
                .clone()
                .unwrap_or_else(|| "Candidate preparation failed".into()),
        )?;
        let token = prepared
            .approval_token
            .ok_or("Missing original Core token")?;
        if !confirm(
            prompt,
            &format!("{action:?}"),
            &[prepared.effects.description],
            deadline,
        )? {
            return Ok(false);
        }
        self.publisher(false)?
            .validate_maintenance_lock(installation)?;
        self.filesystem().publish(
            &backup.join(format!("{action:?}-accepted.txt")),
            b"Original Core consent accepted; outcome uncertain until fresh verification.\n",
            0o600,
        )?;
        self.effects.borrow_mut().push(format!(
            "{action:?} accepted; outcome uncertain until verified"
        ));
        let result = candidate.apply_with_lease(action, &token, lease);
        require(
            result.ok,
            result
                .error
                .unwrap_or_else(|| "Candidate lifecycle failed; effects preserved".into()),
        )?;
        Ok(true)
    }
    fn verify_upgraded(&self, plan: &Plan) -> Result<()> {
        let installed = self.inspect()?;
        require(
            installed.sha == digest(&plan.helper),
            "Upgraded helper changed",
        )?;
        require(installed.objects.iter().any(|(_, fact, _)| matches!(fact, Fact::Selector(selector) if selector.target == format!("releases/{}", plan.release))), "Upgraded selector changed")?;
        let facts = self.existing(&installed).inspect()?;
        require(
            !facts.running,
            "Service changed before separately consented Start",
        )
    }

    fn run(&self, op: Operation, prompt: &mut dyn Prompt) -> Result<Outcome> {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut old = self.inspect()?;
        let existing = self.existing(&old);
        let facts = existing.inspect()?;
        let publisher = self.publisher(op == Operation::Upgrade)?;
        let mut candidate: Option<(Plan, Trace)> = if op == Operation::Upgrade {
            Some(publisher.maintenance_candidate(&old.sha)?)
        } else {
            None
        };
        let mut review = vec![format!("{op:?} exact manifest-owned installation; old helper SHA256 {}", old.sha), "Keep ALL Core/app state, history, preferences, models, releases, backups, stages, lockfiles and unrelated files. Retained data continues to use disk space. No purge, automatic retry/rollback, model download or old-helper execution.".into()];
        review.extend(
            old.objects
                .iter()
                .map(|(path, _, _)| format!("Active object: {}", path.display())),
        );
        review.extend(
            old.missing
                .iter()
                .map(|path| format!("Already absent, left untouched: {}", path.display())),
        );
        review.push(format!(
            "Unit: {}; running={}, registered={}; lifecycle confirmations are separate",
            self.roots.unit().display(),
            facts.running,
            facts.registered
        ));
        if let Some((plan, _)) = &candidate {
            review.push(format!(
                "Candidate client manifest SHA256 {}; helper SHA256 {}; sources {}/payload/client and {}/payload/service/smith-service; publish paired client via existing guarded publisher",
                plan.release, digest(&plan.helper), self.bundle.display(), self.bundle.display()
            ));
        }
        if !confirm(
            prompt,
            &format!("{op:?} Smith artifacts"),
            &review,
            deadline,
        )? {
            return Ok(self.outcome(false, "Declined; no maintenance effects"));
        }
        self.revalidate(&old)?;
        let installation = publisher.lock(&mut publisher.filesystem(), Duration::from_secs(5))?;
        let lease = MaintenanceLease::acquire(&self.roots)?;
        self.revalidate(&old)?;
        require(
            existing.inspect()? == facts,
            "Manager facts changed after review",
        )?;
        if let Some((plan, trace)) = &candidate {
            publisher.maintenance_revalidate(plan, trace, &old.sha)?;
        }
        require(
            Instant::now() < deadline,
            "Artifact consent expired before backup",
        )?;
        let backup = self.backup(&old, op)?;
        let backup_parent = self.roots.data.join("smith-next");
        old.trace.directory_added(&backup_parent)?;
        if let Some((_, trace)) = &mut candidate {
            trace.directory_added(&backup_parent)?;
        }
        self.revalidate(&old)?;
        prompt.progress("Old artifacts/unit preserved; lifecycle remains separately consented")?;
        let mut stopped = facts;
        if stopped.running {
            let prepared = existing.prepare(Action::Stop)?;
            let token = prepared.token().to_owned();
            if !confirm(
                prompt,
                "Stop existing Smith service",
                &[prepared.description().into()],
                prepared.expires(),
            )? {
                return Ok(self.outcome(false, "Stop declined; old installation retained"));
            }
            self.effects
                .borrow_mut()
                .push("Stop accepted; outcome uncertain until verified".into());
            publisher.validate_maintenance_lock(&installation)?;
            self.revalidate(&old)?;
            self.filesystem().publish(
                &backup.join("stop-accepted.txt"),
                b"Stop accepted; outcome uncertain until verified.\n",
                0o600,
            )?;
            stopped = existing.apply_locked(prepared, &token, &lease)?;
        }
        let runner = self.runner_lease()?;
        self.revalidate(&old)?;
        publisher.validate_maintenance_lock(&installation)?;
        if op == Operation::Uninstall {
            if stopped.registered {
                let prepared = existing.prepare(Action::Unregister)?;
                let token = prepared.token().to_owned();
                if !confirm(
                    prompt,
                    "Unregister Smith and reload",
                    &[prepared.description().into()],
                    prepared.expires(),
                )? {
                    return Ok(self.outcome(
                        false,
                        "Unregister declined; artifacts retained, service stopped",
                    ));
                }
                self.effects.borrow_mut().push(
                    "Unregister/reload accepted; retained unit, outcome uncertain until verified"
                        .into(),
                );
                publisher.validate_maintenance_lock(&installation)?;
                self.revalidate(&old)?;
                self.validate_runner_lease(&runner)?;
                self.filesystem().publish(
                    &backup.join("unregister-accepted.txt"),
                    b"Unregister/reload accepted; outcome uncertain until verified.\n",
                    0o600,
                )?;
                let absent = existing.apply_locked(prepared, &token, &lease)?;
                require(
                    !absent.registered && !absent.unit_present,
                    "Registration removal unconfirmed",
                )?;
            }
            require(
                !existing.inspect()?.registered,
                "Registration must be absent before artifact removal",
            )?;
            let active = self.inspect()?;
            require(
                active.sha == old.sha
                    && active.objects == old.objects
                    && active.missing == old.missing,
                "Installed artifacts changed after registration removal; no retry",
            )?;
            publisher.validate_maintenance_lock(&installation)?;
            let mut fs = self.filesystem();
            require(
                Instant::now() < deadline,
                "Uninstall artifact consent expired; lifecycle effects retained",
            )?;
            self.validate_runner_lease(&runner)?;
            for (path, fact, _) in &old.objects {
                fs.preserve(path, fact)?;
            }
            for (path, _, _) in &old.objects {
                fs.directory(path.parent().ok_or("Missing active parent")?, false, false)?
                    .ok_or("Active parent disappeared")?;
                require(
                    std::fs::symlink_metadata(path)
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                    "Active object reappeared after preservation; completion unconfirmed",
                )?;
            }
            fs.publish(
                &backup.join("complete.txt"),
                b"Uninstall: active objects preserved, registration absent. All data retained.\n",
                0o600,
            )?;
            return Ok(self.outcome(
                true,
                "Uninstalled active Smith artifacts; all data and preserved objects retained",
            ));
        }
        if self.migrate_legacy_unit(&old)? {
            self.effects.borrow_mut().push(
                "Migrated exact legacy static unit to the current installable unit and reloaded the user manager; old unit retained".into(),
            );
            old = self.inspect()?;
            stopped = self.existing(&old).inspect()?;
            require(!stopped.running, "Service changed during unit migration")?;
        }
        let (plan, trace) = candidate.ok_or("Candidate review missing")?;
        publisher.maintenance_revalidate(&plan, &trace, &old.sha)?;
        let binary_fact = old
            .objects
            .iter()
            .find(|(path, _, _)| *path == self.roots.binary())
            .ok_or("Old binary fact missing")?;
        require(
            Instant::now() < deadline,
            "Upgrade artifact consent expired; service remains stopped",
        )?;
        require(
            existing.inspect()? == stopped,
            "Stopped registration facts changed before exchange",
        )?;
        publisher.validate_maintenance_lock(&installation)?;
        self.validate_runner_lease(&runner)?;
        self.filesystem().publish(
            &backup.join("exchange-accepted.txt"),
            b"Candidate binary exchange/publication accepted; may be partial until verified.\n",
            0o600,
        )?;
        self.filesystem()
            .exchange_binary(&self.roots.binary(), &plan.helper, &binary_fact.1)?;
        self.effects.borrow_mut().push(
            "Candidate client publication accepted; partial selector effects may be retained"
                .into(),
        );
        let upgraded_control = ExistingController {
            roots: self.roots.clone(),
            manager: Borrowed(&self.manager),
            binary_sha256: digest(&plan.helper),
        };
        require(
            upgraded_control.inspect()? == stopped,
            "Service changed after binary exchange; selector not published",
        )?;
        publisher.maintenance_publish(&plan)?;
        self.verify_upgraded(&plan)?;
        self.filesystem().publish(
            &backup.join("artifacts-upgraded.txt"),
            b"Candidate pair verified; service stopped. Start requires separate consent.\n",
            0o600,
        )?;
        prompt.progress("Artifacts upgraded; service stopped; no models provisioned")?;
        drop(runner); // A newly started candidate may acquire the runner lease itself.
        let controller = Controller {
            roots: self.roots.clone(),
            source: self.bundle.join("payload/service/smith-service"),
            manager: Borrowed(&self.manager),
        };
        if !stopped.registered
            && !self.core_action(
                &controller,
                smith_service::Action::InstallUnit,
                &lease,
                prompt,
                &installation,
                &backup,
            )?
        {
            self.verify_upgraded(&plan)?;
            return Ok(self.outcome(
                false,
                "Artifacts upgraded; registration declined; service stopped",
            ));
        }
        let fresh = controller.inspect();
        require(
            fresh.ok && fresh.unit_state == "installed",
            "Current unit registration was not freshly verified",
        )?;
        if fresh.enablement_state == "disabled"
            && !self.core_action(
                &controller,
                smith_service::Action::Enable,
                &lease,
                prompt,
                &installation,
                &backup,
            )?
        {
            self.verify_upgraded(&plan)?;
            return Ok(self.outcome(
                false,
                "Artifacts upgraded; Enable declined; service stopped",
            ));
        }
        if !self.core_action(
            &controller,
            smith_service::Action::Start,
            &lease,
            prompt,
            &installation,
            &backup,
        )? {
            self.verify_upgraded(&plan)?;
            return Ok(self.outcome(false, "Artifacts upgraded; Start declined; service stopped"));
        }
        let fresh = controller.inspect();
        require(
            fresh.ok
                && fresh.binary_state == "installed"
                && fresh.unit_state == "installed"
                && fresh.enablement_state == "enabled"
                && fresh.runtime_state == "running"
                && fresh.bridge_state == "ready",
            "Start accepted but fresh Core readiness unconfirmed; artifacts retained",
        )?;
        let reply = controller.app(smith_service::app::AppRequest {
            schema_version: 1,
            service: "smith-next".into(),
            protocol: "app".into(),
            call_id: smith_service::app::new_id()?,
            op: "state".into(),
            args: serde_json::json!({}),
        })?;
        require(
            reply.ok,
            "Fresh candidate app readiness unconfirmed; no model readiness claimed",
        )?;
        Ok(self.outcome(
            true,
            "Artifacts upgraded; candidate Core/app running; model readiness not verified",
        ))
    }
    fn outcome(&self, complete: bool, message: &str) -> Outcome {
        Outcome {
            complete,
            message: message.into(),
            details: self.effects.borrow().clone(),
        }
    }
}
impl<M: Manager> Runner for Native<M> {
    fn execute(&mut self, op: Operation, prompt: &mut dyn Prompt) -> Result<Outcome> {
        self.effects.borrow_mut().clear();
        match self.run(op, prompt) {
            Ok(outcome) => Ok(outcome),
            Err(error) => Ok(self.outcome(false, &format!("Maintenance incomplete: {error}. Accepted/uncertain effects preserved; no rollback or automatic retry."))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::fixture::{write, Fixture};

    struct Never;
    impl Prompt for Never {
        fn confirm(&mut self, _: &str, _: &[String], _: Instant) -> Result<bool> {
            panic!("Expired consent must not be offered again")
        }
        fn progress(&mut self, _: &str) -> Result<()> {
            Ok(())
        }
    }
    #[test]
    fn expired_original_consent_is_not_reprepared() {
        assert!(confirm(
            &mut Never,
            "Upgrade",
            &[],
            Instant::now() - Duration::from_secs(1)
        )
        .unwrap_err()
        .contains("expired"));
    }
    #[test]
    fn late_binary_conflict_is_retained_without_selector_change_or_rollback() {
        let fixture = Fixture::new();
        fixture.install(true, true).unwrap();
        let old_selector = std::fs::read_link(fixture.current()).unwrap();
        let mut filesystem = Filesystem::new(Rc::new(RefCell::new(Vec::new())));
        filesystem.file(&fixture.binary(), Some(0o755)).unwrap();
        let fact = filesystem.trace.get(&fixture.binary()).unwrap().clone();
        let path = fixture.binary();
        let replace = path.clone();
        let result = fs::testing::with_hook(
            Box::new(move |event: &fs::testing::Event| {
                if matches!(event, fs::testing::Event::BeforeRename { path, flags, .. } if *path == replace && *flags == libc::RENAME_EXCHANGE)
                {
                    std::fs::remove_file(&replace).unwrap();
                    write(&replace, b"foreign late bytes", 0o755);
                }
                Ok(())
            }),
            || filesystem.exchange_binary(&path, b"candidate", &fact),
        );
        assert!(result.unwrap_err().contains("identity conflict"));
        assert_eq!(std::fs::read(&path).unwrap(), b"candidate");
        let retained = fixture.named(path.parent().unwrap(), ".smith-next-upgrade-");
        assert_eq!(retained.len(), 1);
        assert_eq!(std::fs::read(&retained[0]).unwrap(), b"foreign late bytes");
        assert_eq!(std::fs::read_link(fixture.current()).unwrap(), old_selector);
    }
}
