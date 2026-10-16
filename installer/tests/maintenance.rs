//! Private synthetic old-voice installation and fake-manager effects only.
use smith_installer::manifest::APP;
use smith_installer::{
    artifact,
    maintenance::{Native, Operation, Outcome, Prompt, Runner},
    Result,
};
use smith_service::{MaintenanceManagerFacts, Manager, ManagerFacts, Roots};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
#[path = "fixtures/publisher.rs"]
mod fixture;
use fixture::{chmod, write, Fixture};

#[derive(Default)]
struct Captured {
    child: Option<std::process::Child>,
}
impl Drop for Captured {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[derive(Clone, Default)]
struct Fake {
    calls: Rc<RefCell<Vec<String>>>,
    foreign: Rc<RefCell<Option<String>>>,
    fail_reload: Rc<RefCell<bool>>,
    busy_group: Rc<RefCell<bool>>,
    child: Rc<RefCell<Captured>>,
    fail_stop: Rc<RefCell<bool>>,
    child_after_stop: Rc<RefCell<bool>>,
}
impl Manager for Fake {
    fn inspect(&self, r: &Roots) -> Result<ManagerFacts> {
        let present = r.unit().exists();
        Ok(ManagerFacts {
            load: if present { "loaded" } else { "not-found" }.into(),
            active: if self.child.borrow().child.is_some() {
                "active"
            } else {
                "inactive"
            }
            .into(),
            sub: if self.child.borrow().child.is_some() {
                "running"
            } else {
                "dead"
            }
            .into(),
            pid: self
                .child
                .borrow()
                .child
                .as_ref()
                .map_or(0, |child| child.id()),
            fragment: if present {
                r.unit().display().to_string()
            } else {
                String::new()
            },
            drop_ins: if self.foreign.borrow().as_deref() == Some("dropins") {
                "foreign.conf".into()
            } else {
                String::new()
            },
            need_reload: false,
            transient: false,
            unit_file_state: if !present {
                ""
            } else if fs::read(r.unit()).ok().as_deref()
                == Some(r.legacy_static_unit_text().as_bytes())
            {
                "static"
            } else if r.enablement_link().exists() {
                "enabled"
            } else {
                "disabled"
            }
            .into(),
        })
    }
    fn reload(&self, _: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("reload".into());
        if *self.fail_reload.borrow() {
            Err("fake reload failed".into())
        } else {
            Ok(())
        }
    }
    fn start(&self, _: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("start".into());
        Err("fake Start unavailable".into())
    }
    fn enable(&self, r: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("enable".into());
        fs::create_dir_all(r.enablement_link().parent().unwrap()).map_err(|e| e.to_string())?;
        chmod(&r.config.join("systemd/user/default.target.wants"), 0o700);
        std::os::unix::fs::symlink(r.unit(), r.enablement_link()).map_err(|e| e.to_string())
    }
    fn disable(&self, r: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("disable".into());
        fs::remove_file(r.enablement_link()).map_err(|e| e.to_string())
    }
    fn maintenance_inspect(&self, r: &Roots) -> Result<MaintenanceManagerFacts> {
        let service = self.inspect(r)?;
        let unit_file_state = if service.load == "loaded" {
            self.foreign
                .borrow()
                .clone()
                .filter(|v| v != "dropins")
                .unwrap_or_else(|| service.unit_file_state.clone())
        } else {
            String::new()
        };
        let mut service = service;
        service.unit_file_state = unit_file_state.clone();
        Ok(MaintenanceManagerFacts {
            service,
            unit_file_state,
            control_group: match self.child.borrow().child.as_ref() {
                Some(child) => fs::read_to_string(format!("/proc/{}/cgroup", child.id()))
                    .unwrap()
                    .strip_prefix("0::")
                    .unwrap()
                    .trim_end()
                    .to_owned(),
                None => String::new(),
            },
        })
    }
    fn stop(&self, _: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("stop".into());
        if *self.fail_stop.borrow() {
            return Err("fake Stop failed".into());
        }
        if let Some(mut child) = self.child.borrow_mut().child.take() {
            child.kill().map_err(|e| e.to_string())?;
            child.wait().map_err(|e| e.to_string())?;
        }
        if *self.child_after_stop.borrow() {
            *self.busy_group.borrow_mut() = true;
        }
        Ok(())
    }
    fn quiescence(&self, _: &Roots, _: &str) -> Result<()> {
        if *self.busy_group.borrow() {
            Err("fake remaining child".into())
        } else {
            Ok(())
        }
    }
}
struct Answers {
    titles: Vec<String>,
    decline: Option<String>,
    change: Option<Box<dyn FnOnce()>>,
}
impl Answers {
    fn yes() -> Self {
        Self {
            titles: Vec::new(),
            decline: None,
            change: None,
        }
    }
}
impl Prompt for Answers {
    fn confirm(&mut self, title: &str, effects: &[String], deadline: Instant) -> Result<bool> {
        assert!(Instant::now() < deadline);
        assert!(!effects.is_empty());
        self.titles.push(title.into());
        if let Some(change) = self.change.take() {
            change();
        }
        Ok(!self
            .decline
            .as_ref()
            .is_some_and(|part| title.contains(part)))
    }
    fn progress(&mut self, _: &str) -> Result<()> {
        Ok(())
    }
}
fn roots(f: &Fixture) -> Roots {
    let config = f.root.join("config");
    let runtime = f.root.join("runtime");
    for p in [&config, &runtime] {
        fs::create_dir(p).unwrap();
        chmod(p, 0o700);
    }
    Roots {
        data: f.data.clone(),
        config,
        runtime,
    }
}
fn old(f: &Fixture, roots: &Roots, registered: bool) {
    // A retained voice-era release is valid without today's payload allowlist.
    write(
        &f.bundle.join("smith_backend/voice-reference"),
        b"old voice bytes",
        0o644,
    );
    f.install(true, true).unwrap();
    if registered {
        fs::create_dir_all(roots.unit().parent().unwrap()).unwrap();
        chmod(&roots.config.join("systemd"), 0o700);
        chmod(&roots.config.join("systemd/user"), 0o700);
        write(&roots.unit(), roots.unit_text().as_bytes(), 0o600);
    }
    write(
        &f.data.join("smith-next/app-state.json"),
        b"corrupt app data preserved",
        0o600,
    );
    write(&f.data.join("smith-next/state.json"), b"{}", 0o600);
    fs::create_dir(f.data.join("smith-next/models")).unwrap();
    chmod(&f.data.join("smith-next/models"), 0o700);
    write(
        &f.data.join("smith-next/models/keep.gguf"),
        b"fake weights sentinel",
        0o600,
    );
}
fn enable(r: &Roots) {
    let link = r.enablement_link();
    let wants = link.parent().unwrap();
    fs::create_dir_all(wants).unwrap();
    chmod(&r.config.join("systemd/user/default.target.wants"), 0o700);
    std::os::unix::fs::symlink(r.unit(), r.enablement_link()).unwrap();
}
fn candidate(f: &Fixture) -> PathBuf {
    let package = f.root.join("package");
    fs::create_dir_all(package.join("payload/service")).unwrap();
    for path in [
        &package,
        &package.join("payload"),
        &package.join("payload/service"),
    ] {
        chmod(path, 0o700);
    }
    fs::rename(&f.bundle, package.join("payload/client")).unwrap();
    write(
        &package.join("payload/client/smith_backend/smith-service"),
        b"service-v2",
        0o755,
    );
    write(
        &package.join("payload/service/smith-service"),
        b"service-v2",
        0o755,
    );
    package
}
fn run(
    f: &Fixture,
    roots: Roots,
    manager: Fake,
    bundle: PathBuf,
    op: Operation,
    answers: &mut Answers,
) -> Outcome {
    Native::with_roots(bundle, f.home.clone(), roots, manager)
        .unwrap()
        .execute(op, answers)
        .unwrap()
}
#[test]
fn oldvoice_upgrade_preserves_old_binary_release_and_data_start_decline() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let old_selector = fs::read_link(f.current()).unwrap();
    let package = candidate(&f);
    // Core preparation observes a valid Core state, independently of corrupt app data.
    write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
    let manager = Fake::default();
    let mut answers = Answers::yes();
    answers.decline = Some("Start".into());
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        package,
        Operation::Upgrade,
        &mut answers,
    );
    assert!(!result.complete, "{}", result.message);
    assert!(result.message.contains("Start declined"));
    assert_eq!(fs::read(f.binary()).unwrap(), b"service-v2");
    assert_ne!(fs::read_link(f.current()).unwrap(), old_selector);
    assert!(f
        .data
        .join("smith-next/client")
        .join(old_selector)
        .join("smith_backend/voice-reference")
        .exists());
    let old_binary = f.named(f.binary().parent().unwrap(), ".smith-next-upgrade-");
    assert_eq!(old_binary.len(), 1);
    assert_eq!(fs::read(&old_binary[0]).unwrap(), b"service-v1");
    assert_eq!(
        fs::read(r.data.join("smith-next/app-state.json")).unwrap(),
        b"corrupt app data preserved"
    );
    assert!(r.data.join("smith-next/models/keep.gguf").exists());
    assert_eq!(*manager.calls.borrow(), ["enable"]);
    assert_eq!(
        answers.titles,
        ["Upgrade Smith artifacts", "Enable", "Start"]
    );
}

#[test]
fn captured_fake_oldvoice_runtime_stop_then_reload_and_stop_failure_fence() {
    for scenario in ["uninstall", "stop-fails", "remaining-child", "upgrade"] {
        let f = Fixture::new();
        let r = roots(&f);
        // Private captured sleep ELF stands in for old voice service bytes. No host manager.
        let fake_elf = fs::read("/usr/bin/sleep").unwrap();
        write(&f.service, &fake_elf, 0o755);
        write(
            &f.bundle.join("smith_backend/smith-service"),
            &fake_elf,
            0o755,
        );
        old(&f, &r, true);
        let manager = Fake::default();
        manager.child.borrow_mut().child = Some(
            std::process::Command::new(f.binary())
                .arg("30")
                .spawn()
                .unwrap(),
        );
        *manager.fail_stop.borrow_mut() = scenario == "stop-fails";
        *manager.child_after_stop.borrow_mut() = scenario == "remaining-child";
        let upgrade = scenario == "upgrade";
        let package = if upgrade {
            candidate(&f)
        } else {
            f.root.join("absent")
        };
        if upgrade {
            write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
        }
        let mut answers = Answers::yes();
        if upgrade {
            answers.decline = Some("Start".into());
        }
        let result = run(
            &f,
            r.clone(),
            manager.clone(),
            package,
            if upgrade {
                Operation::Upgrade
            } else {
                Operation::Uninstall
            },
            &mut answers,
        );
        match scenario {
            "uninstall" => {
                assert!(result.complete, "{}", result.message);
                assert_eq!(*manager.calls.borrow(), ["stop", "reload"]);
                assert!(!f.binary().exists());
            }
            "upgrade" => {
                assert!(!result.complete, "{}", result.message);
                assert_eq!(*manager.calls.borrow(), ["stop", "enable"]);
                assert_eq!(fs::read(f.binary()).unwrap(), b"service-v2");
                assert!(result.message.contains("Start declined"));
            }
            _ => {
                assert!(!result.complete, "{scenario}");
                assert_eq!(*manager.calls.borrow(), ["stop"]);
                assert_eq!(fs::read(f.binary()).unwrap(), fake_elf);
                assert!(r.unit().exists());
                assert!(f.current().exists());
            }
        }
        assert!(answers
            .titles
            .iter()
            .any(|title| title == "Stop existing Smith service"));
    }
}
#[test]
fn uninstall_without_candidate_preserves_exact_objects_and_corrupt_data() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let binary_inode = std::os::unix::fs::MetadataExt::ino(&fs::metadata(f.binary()).unwrap());
    let manager = Fake::default();
    let mut answers = Answers::yes();
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        f.root.join("absent-package"),
        Operation::Uninstall,
        &mut answers,
    );
    assert!(result.complete, "{}", result.message);
    assert_eq!(*manager.calls.borrow(), ["reload"]);
    assert!(!r.unit().exists());
    assert!(!f.binary().exists());
    assert!(!f.current().exists());
    assert!(!f.home.join(".local/bin/smith-next").exists());
    assert!(!f.data.join(format!("applications/{APP}.desktop")).exists());
    let retained = f.named(f.binary().parent().unwrap(), ".smith-next-maintained-");
    assert_eq!(retained.len(), 1);
    assert_eq!(
        std::os::unix::fs::MetadataExt::ino(&fs::metadata(&retained[0]).unwrap()),
        binary_inode
    );
    assert_eq!(fs::read(&retained[0]).unwrap(), b"service-v1");
    assert_eq!(
        fs::read(r.data.join("smith-next/app-state.json")).unwrap(),
        b"corrupt app data preserved"
    );
    assert!(r.state().exists());
    assert!(r.data.join("smith-next/models/keep.gguf").exists());
    assert!(!f
        .named(&f.data.join("smith-next"), ".smith-next-maintenance-")
        .is_empty());
    assert!(!f
        .named(r.unit().parent().unwrap(), ".smith-next.service-retained-")
        .is_empty());
}
#[test]
fn initial_decline_is_read_only() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let before = f.snapshot();
    let mut answers = Answers::yes();
    answers.decline = Some("artifacts".into());
    let result = run(
        &f,
        r,
        Fake::default(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut answers,
    );
    assert!(!result.complete);
    assert_eq!(f.snapshot(), before);
}
#[test]
fn modified_shim_binary_manifest_and_selector_refuse_before_consent() {
    for kind in ["shim", "binary", "manifest", "selector"] {
        let f = Fixture::new();
        let r = roots(&f);
        old(&f, &r, true);
        match kind {
            "shim" => write(&f.home.join(".local/bin/smith-next"), b"foreign", 0o755),
            "binary" => write(&f.binary(), b"foreign", 0o755),
            "manifest" => {
                let release = f
                    .data
                    .join("smith-next/client")
                    .join(fs::read_link(f.current()).unwrap());
                write(&release.join(".smith-next-manifest.json"), b"{}\n", 0o600);
            }
            _ => {
                fs::remove_file(f.current()).unwrap();
                std::os::unix::fs::symlink("releases/foreign", f.current()).unwrap();
            }
        }
        let before = f.snapshot();
        let mut answers = Answers::yes();
        let result = run(
            &f,
            r,
            Fake::default(),
            f.root.join("absent"),
            Operation::Uninstall,
            &mut answers,
        );
        assert!(!result.complete, "{kind}");
        assert!(answers.titles.is_empty());
        assert_eq!(f.snapshot(), before);
    }
}
#[test]
fn foreign_dropins_enablement_and_remaining_child_refuse_early() {
    for foreign in ["dropins", "enabled", "enabled-runtime", "unknown", "child"] {
        let f = Fixture::new();
        let r = roots(&f);
        old(&f, &r, true);
        let manager = Fake::default();
        if foreign == "child" {
            *manager.busy_group.borrow_mut() = true;
        } else {
            *manager.foreign.borrow_mut() = Some(foreign.into());
        }
        let before = f.snapshot();
        let mut answers = Answers::yes();
        let result = run(
            &f,
            r,
            manager.clone(),
            f.root.join("absent"),
            Operation::Uninstall,
            &mut answers,
        );
        assert!(!result.complete);
        assert!(answers.titles.is_empty());
        assert!(manager.calls.borrow().is_empty());
        assert_eq!(f.snapshot(), before);
    }
}
#[test]
fn consent_drift_blocks_without_reprepare() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let binary = f.binary();
    let mut answers = Answers::yes();
    answers.change = Some(Box::new(move || {
        write(&binary, b"new foreign bytes", 0o755)
    }));
    let manager = Fake::default();
    let result = run(
        &f,
        r,
        manager.clone(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut answers,
    );
    assert!(!result.complete);
    assert_eq!(answers.titles.len(), 1);
    assert!(manager.calls.borrow().is_empty());
    assert_eq!(fs::read(f.binary()).unwrap(), b"new foreign bytes");
    assert!(f.current().exists());
}
#[test]
fn unregister_decline_preserves_active_artifacts() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let mut answers = Answers::yes();
    answers.decline = Some("Unregister".into());
    let manager = Fake::default();
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut answers,
    );
    assert!(!result.complete);
    assert!(result.message.contains("declined"));
    assert!(r.unit().exists());
    assert!(f.binary().exists());
    assert!(f.current().exists());
    assert!(manager.calls.borrow().is_empty());
}
#[test]
fn reload_failure_retains_unit_and_all_active_artifacts() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let manager = Fake::default();
    *manager.fail_reload.borrow_mut() = true;
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut Answers::yes(),
    );
    assert!(!result.complete);
    assert!(result.message.contains("fake reload failed"));
    assert!(!r.unit().exists());
    assert!(f.binary().exists());
    assert!(f.current().exists());
    assert_eq!(*manager.calls.borrow(), ["reload"]);
}
#[test]
fn missing_launchers_are_conservatively_skipped_not_recreated_by_uninstall() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, false);
    fs::remove_file(f.home.join(".local/bin/smith-next")).unwrap();
    let result = run(
        &f,
        r,
        Fake::default(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut Answers::yes(),
    );
    assert!(result.complete, "{}", result.message);
    assert!(!f.home.join(".local/bin/smith-next").exists());
}
#[test]
fn normal_install_still_rejects_differing_old_service() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, false);
    let package = candidate(&f);
    let item = artifact::Installer::with_roots(
        Some(package.join("payload/client")),
        Some(package.join("payload/service/smith-service")),
        Some(f.home.clone()),
        f.data.clone(),
    )
    .unwrap();
    assert!(item
        .prepare_review()
        .err()
        .unwrap()
        .contains("upgrades require separate approval"));
}

#[test]
fn core_and_runner_lock_contention_preserve_active_objects() {
    use std::os::fd::AsRawFd;
    for kind in ["core", "runner"] {
        let f = Fixture::new();
        let r = roots(&f);
        old(&f, &r, true);
        let lock = if kind == "core" {
            fs::File::open(&r.data).unwrap()
        } else {
            fs::create_dir(r.runtime.join("smith-next")).unwrap();
            chmod(&r.runtime.join("smith-next"), 0o700);
            write(&r.runtime.join("smith-next/runner.lock"), b"", 0o600);
            fs::File::open(r.runtime.join("smith-next/runner.lock")).unwrap()
        };
        assert_eq!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        let manager = Fake::default();
        let result = run(
            &f,
            r.clone(),
            manager.clone(),
            f.root.join("absent"),
            Operation::Uninstall,
            &mut Answers::yes(),
        );
        assert!(!result.complete, "{kind}");
        assert!(manager.calls.borrow().is_empty());
        assert!(f.binary().exists());
        assert!(f.current().exists());
        assert!(r.unit().exists());
    }
}
#[test]
fn symlinked_root_and_on_disk_override_are_refused() {
    let f = Fixture::new();
    let mut r = roots(&f);
    old(&f, &r, true);
    let link = f.root.join("config-link");
    std::os::unix::fs::symlink(&r.config, &link).unwrap();
    let real = r.config.clone();
    r.config = link;
    assert!(Native::with_roots(
        f.root.join("absent"),
        f.home.clone(),
        r.clone(),
        Fake::default()
    )
    .is_err());
    r.config = real;
    let dropins = r.config.join("systemd/user/smith-next.service.d");
    fs::create_dir(&dropins).unwrap();
    chmod(&dropins, 0o700);
    write(&dropins.join("override.conf"), b"foreign override", 0o600);
    let before = f.snapshot();
    let mut answers = Answers::yes();
    let result = run(
        &f,
        r,
        Fake::default(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut answers,
    );
    assert!(!result.complete);
    assert!(answers.titles.is_empty());
    assert_eq!(f.snapshot(), before);
}
#[test]
fn installed_old_helper_is_never_executed() {
    let f = Fixture::new();
    let r = roots(&f);
    let sentinel = f.root.join("old-helper-executed");
    let script = format!("#!/bin/sh\ntouch {}\nexit 1\n", sentinel.display());
    write(&f.service, script.as_bytes(), 0o755);
    write(
        &f.bundle.join("smith_backend/smith-service"),
        script.as_bytes(),
        0o755,
    );
    old(&f, &r, true);
    let result = run(
        &f,
        r,
        Fake::default(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut Answers::yes(),
    );
    assert!(result.complete, "{}", result.message);
    assert!(!sentinel.exists());
}
#[test]
fn candidate_mismatch_is_read_only_and_start_failure_is_partial() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let package = candidate(&f);
    write(
        &package.join("payload/service/smith-service"),
        b"mismatch",
        0o755,
    );
    let before = f.snapshot();
    let mut answers = Answers::yes();
    let result = run(
        &f,
        r.clone(),
        Fake::default(),
        package.clone(),
        Operation::Upgrade,
        &mut answers,
    );
    assert!(!result.complete);
    assert!(answers.titles.is_empty());
    assert_eq!(f.snapshot(), before);
    write(
        &package.join("payload/service/smith-service"),
        b"service-v2",
        0o755,
    );
    write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
    let manager = Fake::default();
    let result = run(
        &f,
        r,
        manager.clone(),
        package,
        Operation::Upgrade,
        &mut Answers::yes(),
    );
    assert!(!result.complete);
    assert!(result.message.contains("fake Start unavailable"));
    assert_eq!(fs::read(f.binary()).unwrap(), b"service-v2");
    assert_eq!(*manager.calls.borrow(), ["enable", "start"]);
}
#[test]
fn legacy_static_upgrade_migrates_unit_then_reloads_and_enables_before_start() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    write(&r.unit(), r.legacy_static_unit_text().as_bytes(), 0o600);
    let package = candidate(&f);
    write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
    let manager = Fake::default();
    let mut answers = Answers::yes();
    answers.decline = Some("Start".into());
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        package,
        Operation::Upgrade,
        &mut answers,
    );
    assert!(!result.complete, "{}", result.message);
    assert_eq!(
        fs::read(r.unit()).unwrap(),
        r.unit_text().as_bytes(),
        "{}",
        result.message
    );
    assert_eq!(*manager.calls.borrow(), ["reload", "enable"]);
    assert_eq!(
        answers.titles,
        ["Upgrade Smith artifacts", "Enable", "Start"]
    );
    let retained = f.named(r.unit().parent().unwrap(), ".smith-next-unit-upgrade-");
    assert_eq!(retained.len(), 1);
    assert_eq!(
        fs::read(&retained[0]).unwrap(),
        r.legacy_static_unit_text().as_bytes()
    );
}

#[test]
fn current_enabled_upgrade_preserves_enablement_without_churn() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    enable(&r);
    let package = candidate(&f);
    write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
    let manager = Fake::default();
    let mut answers = Answers::yes();
    answers.decline = Some("Start".into());
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        package,
        Operation::Upgrade,
        &mut answers,
    );
    assert!(!result.complete, "{}", result.message);
    assert!(r.enablement_link().exists());
    assert!(manager.calls.borrow().is_empty());
    assert_eq!(answers.titles, ["Upgrade Smith artifacts", "Start"]);
}

#[test]
fn current_disabled_upgrade_enables_before_separately_confirmed_start() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    let package = candidate(&f);
    write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
    let manager = Fake::default();
    let mut answers = Answers::yes();
    answers.decline = Some("Start".into());
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        package,
        Operation::Upgrade,
        &mut answers,
    );
    assert!(!result.complete, "{}", result.message);
    assert_eq!(*manager.calls.borrow(), ["enable"]);
    assert!(r.enablement_link().exists());
    assert_eq!(
        answers.titles,
        ["Upgrade Smith artifacts", "Enable", "Start"]
    );
}

#[test]
fn enabled_uninstall_disables_before_unregister_reload_and_artifact_preservation() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, true);
    enable(&r);
    let manager = Fake::default();
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        f.root.join("absent"),
        Operation::Uninstall,
        &mut Answers::yes(),
    );
    assert!(result.complete, "{}", result.message);
    assert_eq!(*manager.calls.borrow(), ["disable", "reload"]);
    assert!(!r.enablement_link().exists());
    assert!(!r.unit().exists());
    assert!(!f.binary().exists());
}

#[test]
fn absent_unit_upgrade_registers_then_separately_offers_start() {
    let f = Fixture::new();
    let r = roots(&f);
    old(&f, &r, false);
    let package = candidate(&f);
    write(&r.state(), b"{\"setup_complete\":false}\n", 0o600);
    let manager = Fake::default();
    let mut answers = Answers::yes();
    answers.decline = Some("Start".into());
    let result = run(
        &f,
        r.clone(),
        manager.clone(),
        package,
        Operation::Upgrade,
        &mut answers,
    );
    assert!(!result.complete, "{}", result.message);
    assert!(r.unit().exists());
    assert_eq!(*manager.calls.borrow(), ["reload", "enable"]);
    assert_eq!(
        answers.titles,
        ["Upgrade Smith artifacts", "InstallUnit", "Enable", "Start"]
    );
}
