use sha2::{Digest, Sha256};
use smith_service::maintenance::{Action, ExistingController};
use smith_service::{
    Controller, MaintenanceLease, MaintenanceManagerFacts, Manager, ManagerFacts, Result, Roots,
};
use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    roots: Roots,
}
fn dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn file(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "smith-maintenance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        dir(&base);
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("runtime"),
        };
        for p in [
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
            &roots.data.join("smith-next/bin"),
            &roots.config.join("systemd"),
            &roots.config.join("systemd/user"),
            &roots.runtime.join("smith-next"),
        ] {
            dir(p);
        }
        // Deliberately not executable code: maintenance must never invoke old bytes.
        file(&roots.binary(), b"old-voice-release-not-candidate", 0o755);
        file(&roots.unit(), roots.unit_text().as_bytes(), 0o600);
        file(&roots.state(), b"corrupt Core state", 0o600);
        file(
            &roots.data.join("smith-next/app-state.json"),
            b"corrupt app state",
            0o600,
        );
        Self { base, roots }
    }
    fn controller(&self) -> ExistingController<Fake> {
        ExistingController {
            roots: self.roots.clone(),
            manager: Fake::new(&self.roots),
            binary_sha256: format!("{:x}", Sha256::digest(b"old-voice-release-not-candidate")),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
struct Fake {
    facts: RefCell<MaintenanceManagerFacts>,
    calls: RefCell<Vec<&'static str>>,
    children: Cell<bool>,
    fail_reload: Cell<bool>,
    remain_registered: Cell<bool>,
    child: RefCell<Option<std::process::Child>>,
    fail_stop: Cell<bool>,
    fail_disable: Cell<bool>,
}
impl Fake {
    fn new(roots: &Roots) -> Self {
        Self {
            facts: RefCell::new(MaintenanceManagerFacts {
                service: ManagerFacts {
                    load: "loaded".into(),
                    active: "inactive".into(),
                    sub: "dead".into(),
                    pid: 0,
                    fragment: roots.unit().to_string_lossy().into_owned(),
                    drop_ins: String::new(),
                    need_reload: false,
                    transient: false,
                    unit_file_state: "disabled".into(),
                },
                unit_file_state: "disabled".into(),
                control_group: String::new(),
            }),
            calls: RefCell::new(Vec::new()),
            children: Cell::new(false),
            fail_reload: Cell::new(false),
            remain_registered: Cell::new(false),
            child: RefCell::new(None),
            fail_stop: Cell::new(false),
            fail_disable: Cell::new(false),
        }
    }
}
impl Manager for Fake {
    fn inspect(&self, _: &Roots) -> Result<ManagerFacts> {
        Ok(self.facts.borrow().service.clone())
    }
    fn maintenance_inspect(&self, _: &Roots) -> Result<MaintenanceManagerFacts> {
        Ok(self.facts.borrow().clone())
    }
    fn reload(&self, _: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("reload");
        if self.fail_reload.get() {
            return Err("fake reload failure".into());
        }
        if !self.remain_registered.get() {
            let mut f = self.facts.borrow_mut();
            f.service.load = "not-found".into();
            f.service.fragment.clear();
            f.service.unit_file_state.clear();
            f.unit_file_state.clear();
        }
        Ok(())
    }
    fn start(&self, _: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("start");
        Err("not supported".into())
    }
    fn disable(&self, roots: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("disable");
        if self.fail_disable.get() {
            return Err("fake disable failure".into());
        }
        fs::remove_file(roots.enablement_link()).map_err(|_| "fake disable failure")?;
        let mut facts = self.facts.borrow_mut();
        facts.service.unit_file_state = "disabled".into();
        facts.unit_file_state = "disabled".into();
        Ok(())
    }
    fn stop(&self, _: &Roots) -> Result<()> {
        self.calls.borrow_mut().push("stop");
        if self.fail_stop.get() {
            return Err("fake stop failure".into());
        }
        if let Some(mut child) = self.child.borrow_mut().take() {
            child.kill().map_err(|_| "fixture Stop failed")?;
            child.wait().map_err(|_| "fixture Stop wait failed")?;
        }
        let mut f = self.facts.borrow_mut();
        f.service.pid = 0;
        f.service.active = "inactive".into();
        f.service.sub = "dead".into();
        f.control_group.clear();
        Ok(())
    }
    fn quiescence(&self, _: &Roots, _: &str) -> Result<()> {
        if self.children.get() {
            Err("remaining child".into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn unregister_preserves_old_unit_and_all_corrupt_state_under_held_lease() {
    let f = Fixture::new();
    let c = f.controller();
    let before = fs::metadata(c.roots.unit()).unwrap();
    let prepared = c.prepare(Action::Unregister).unwrap();
    let token = prepared.token().to_owned();
    assert!(prepared.description().contains("Retain all runtime data"));
    let lease = MaintenanceLease::acquire(&f.roots).unwrap();
    assert!(MaintenanceLease::acquire(&f.roots).is_err());
    let facts = c.apply_locked(prepared, &token, &lease).unwrap();
    assert!(!facts.unit_present && !facts.registered && !facts.running);
    let retained = f
        .roots
        .unit()
        .parent()
        .unwrap()
        .join(format!(".smith-next.service-retained-{token}"));
    let after = fs::metadata(&retained).unwrap();
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
    assert_eq!(fs::read(retained).unwrap(), f.roots.unit_text().as_bytes());
    assert_eq!(*c.manager.calls.borrow(), ["reload"]);
    assert_eq!(fs::read(f.roots.state()).unwrap(), b"corrupt Core state");
    assert_eq!(
        fs::read(f.roots.data.join("smith-next/app-state.json")).unwrap(),
        b"corrupt app state"
    );
    assert_eq!(
        fs::read(f.roots.binary()).unwrap(),
        b"old-voice-release-not-candidate"
    );
    assert!(c.prepare(Action::Unregister).is_err());
    drop(lease);
    assert!(MaintenanceLease::acquire(&f.roots).is_ok());
}
#[test]
fn exact_legacy_static_unit_is_recognized_and_preserved() {
    let f = Fixture::new();
    file(
        &f.roots.unit(),
        f.roots.legacy_static_unit_text().as_bytes(),
        0o600,
    );
    let c = f.controller();
    {
        let mut facts = c.manager.facts.borrow_mut();
        facts.service.unit_file_state = "static".into();
        facts.unit_file_state = "static".into();
    }
    assert!(c.inspect().unwrap().registered);
    let prepared = c.prepare(Action::Unregister).unwrap();
    let token = prepared.token().to_owned();
    c.apply(prepared, &token).unwrap();
    let retained = f
        .roots
        .unit()
        .parent()
        .unwrap()
        .join(format!(".smith-next.service-retained-{token}"));
    assert_eq!(
        fs::read(retained).unwrap(),
        f.roots.legacy_static_unit_text().as_bytes()
    );
    assert_eq!(*c.manager.calls.borrow(), ["reload"]);
}

#[test]
fn enabled_current_unit_is_disabled_before_unregister_and_failure_preserves_it() {
    for fail in [false, true] {
        let f = Fixture::new();
        let c = f.controller();
        let link = f.roots.enablement_link();
        let parent = link.parent().unwrap();
        dir(parent);
        symlink(f.roots.unit(), &link).unwrap();
        {
            let mut facts = c.manager.facts.borrow_mut();
            facts.service.unit_file_state = "enabled".into();
            facts.unit_file_state = "enabled".into();
        }
        let prepared = c.prepare(Action::Unregister).unwrap();
        let token = prepared.token().to_owned();
        c.manager.fail_disable.set(fail);
        let result = c.apply(prepared, &token);
        if fail {
            assert!(result.is_err());
            assert!(f.roots.unit().exists());
            assert!(f.roots.enablement_link().exists());
            assert_eq!(*c.manager.calls.borrow(), ["disable"]);
        } else {
            assert!(result.is_ok());
            assert_eq!(*c.manager.calls.borrow(), ["disable", "reload"]);
            assert!(!f.roots.enablement_link().exists());
        }
    }
}

#[test]
fn consent_drift_wrong_token_and_backup_collision_have_no_effects() {
    for kind in 0..5 {
        let f = Fixture::new();
        let c = f.controller();
        let prepared = c.prepare(Action::Unregister).unwrap();
        let token = prepared.token().to_owned();
        match kind {
            0 => c.manager.facts.borrow_mut().service.sub = "failed".into(),
            1 => file(&f.roots.binary(), b"different old bytes", 0o755),
            2 => {
                fs::rename(f.roots.unit(), f.base.join("old-unit")).unwrap();
                file(&f.roots.unit(), f.roots.unit_text().as_bytes(), 0o600);
            }
            3 => file(
                &f.roots
                    .unit()
                    .parent()
                    .unwrap()
                    .join(format!(".smith-next.service-retained-{token}")),
                b"keep me",
                0o600,
            ),
            _ => {}
        }
        assert!(c
            .apply(prepared, if kind == 4 { "wrong" } else { &token })
            .is_err());
        assert!(f.roots.unit().exists());
        assert!(c.manager.calls.borrow().is_empty());
    }
}
#[test]
fn remaining_children_and_either_socket_block_preparation_or_apply() {
    for kind in 0..3 {
        let f = Fixture::new();
        let c = f.controller();
        let prepared = c.prepare(Action::Unregister).unwrap();
        let token = prepared.token().to_owned();
        match kind {
            0 => c.manager.children.set(true),
            1 => file(&f.roots.socket(), b"not a socket", 0o600),
            _ => {
                symlink(f.base.join("missing"), f.roots.app_socket()).unwrap();
            }
        }
        assert!(c.prepare(Action::Unregister).is_err());
        assert!(c.apply(prepared, &token).is_err());
        assert!(f.roots.unit().exists());
        assert!(c.manager.calls.borrow().is_empty());
    }
}
#[test]
fn foreign_enabled_unknown_transient_and_pending_overrides_refused() {
    for kind in 0..10 {
        let f = Fixture::new();
        let c = f.controller();
        match kind {
            0 => {
                let mut facts = c.manager.facts.borrow_mut();
                facts.unit_file_state = "enabled".into();
                facts.service.unit_file_state = "enabled".into();
            }
            1 => {
                let mut facts = c.manager.facts.borrow_mut();
                facts.unit_file_state = "unknown".into();
                facts.service.unit_file_state = "unknown".into();
            }
            2 => c.manager.facts.borrow_mut().service.transient = true,
            3 => c.manager.facts.borrow_mut().service.drop_ins = "foreign.conf".into(),
            4 => c.manager.facts.borrow_mut().service.fragment = "/foreign/unit".into(),
            5 => c.manager.facts.borrow_mut().service.need_reload = true,
            6 => dir(&f.roots.unit().with_file_name("smith-next.service.d")),
            7 => fs::set_permissions(f.roots.unit(), fs::Permissions::from_mode(0o644)).unwrap(),
            8 => fs::hard_link(f.roots.unit(), f.base.join("unit-link")).unwrap(),
            _ => file(&f.roots.unit(), b"foreign unit", 0o600),
        }
        assert!(c.inspect().is_err());
        assert!(c.prepare(Action::Unregister).is_err());
        assert!(c.manager.calls.borrow().is_empty());
    }
}
#[test]
fn reload_failure_or_unconfirmed_removal_keeps_retained_evidence_without_retry() {
    for unconfirmed in [false, true] {
        let f = Fixture::new();
        let c = f.controller();
        let prepared = c.prepare(Action::Unregister).unwrap();
        let token = prepared.token().to_owned();
        c.manager.fail_reload.set(!unconfirmed);
        c.manager.remain_registered.set(unconfirmed);
        assert!(c.apply(prepared, &token).is_err());
        assert!(!f.roots.unit().exists());
        assert!(f
            .roots
            .unit()
            .parent()
            .unwrap()
            .join(format!(".smith-next.service-retained-{token}"))
            .exists());
        assert_eq!(*c.manager.calls.borrow(), ["reload"]);
    }
}
#[test]
fn lease_rejects_other_roots_and_replaced_data_directory() {
    let f = Fixture::new();
    let c = f.controller();
    let other = Fixture::new();
    let prepared = c.prepare(Action::Unregister).unwrap();
    let token = prepared.token().to_owned();
    let wrong = MaintenanceLease::acquire(&other.roots).unwrap();
    assert!(c.apply_locked(prepared, &token, &wrong).is_err());
    let prepared = c.prepare(Action::Unregister).unwrap();
    let token = prepared.token().to_owned();
    let lease = MaintenanceLease::acquire(&f.roots).unwrap();
    fs::rename(&f.roots.data, f.base.join("retained-data")).unwrap();
    dir(&f.roots.data);
    assert!(c.apply_locked(prepared, &token, &lease).is_err());
    assert!(c.manager.calls.borrow().is_empty());
}
#[test]
fn candidate_locked_apply_uses_held_lease_and_original_token() {
    let f = Fixture::new();
    // Candidate path retains its existing state validation (unlike old maintenance).
    file(&f.roots.state(), b"{\"setup_complete\":false}", 0o600);
    fs::remove_file(f.roots.unit()).unwrap();
    let manager = Fake::new(&f.roots);
    {
        let mut facts = manager.facts.borrow_mut();
        facts.service.load = "not-found".into();
        facts.service.fragment.clear();
        facts.service.unit_file_state.clear();
        facts.unit_file_state.clear();
    }
    let controller = Controller {
        roots: f.roots.clone(),
        source: f.roots.binary(),
        manager,
    };
    let prepared = controller.prepare(smith_service::Action::InstallUnit);
    assert!(prepared.ok, "{:?}", prepared.error);
    let lease = MaintenanceLease::acquire(&f.roots).unwrap();
    let result = controller.apply_with_lease(
        smith_service::Action::InstallUnit,
        prepared.approval_token.as_deref().unwrap(),
        &lease,
    );
    assert!(result.ok, "{:?}", result.error);
    assert!(f.roots.unit().exists());
    assert_eq!(*controller.manager.calls.borrow(), ["reload"]);
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.get_mut().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[test]
fn stop_verifies_old_proc_bytes_and_refuses_remaining_group_sockets_or_failure() {
    for kind in 0..5 {
        let f = Fixture::new();
        // Only a private captured synthetic process; never a real service/helper invocation.
        fs::copy("/usr/bin/sleep", f.roots.binary()).unwrap();
        fs::set_permissions(f.roots.binary(), fs::Permissions::from_mode(0o755)).unwrap();
        let mut c = f.controller();
        c.binary_sha256 = format!("{:x}", Sha256::digest(fs::read(f.roots.binary()).unwrap()));
        let child = std::process::Command::new(f.roots.binary())
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        let group_text = fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
        let group = group_text
            .trim_end()
            .strip_prefix("0::")
            .expect("unified cgroup2 test host")
            .to_owned();
        *c.manager.child.borrow_mut() = Some(child);
        {
            let mut facts = c.manager.facts.borrow_mut();
            facts.service.active = "active".into();
            facts.service.sub = "running".into();
            facts.service.pid = pid;
            facts.control_group = group;
        }
        // The process may not have exec'd yet; wait only for the captured fixture's exec.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while fs::read_link(format!("/proc/{pid}/exe")).unwrap() != f.roots.binary() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        match kind {
            2 => file(&f.roots.socket(), b"remaining socket", 0o600),
            3 => file(&f.roots.app_socket(), b"remaining app socket", 0o600),
            _ => {}
        }
        assert!(c.prepare(Action::Unregister).is_err());
        let prepared = c.prepare(Action::Stop).unwrap();
        let token = prepared.token().to_owned();
        c.manager.fail_stop.set(kind == 1);
        c.manager.children.set(kind == 4);
        let lease = MaintenanceLease::acquire(&f.roots).unwrap();
        let result = c.apply_locked(prepared, &token, &lease);
        if kind == 0 {
            let facts = result.unwrap();
            assert!(!facts.running && facts.registered && facts.unit_present);
            assert!(c.prepare(Action::Unregister).is_ok());
        } else {
            assert!(result.is_err());
        }
        assert_eq!(*c.manager.calls.borrow(), ["stop"]);
        assert!(f.roots.unit().exists());
    }
}

struct Legacy;
impl Manager for Legacy {
    fn inspect(&self, _: &Roots) -> Result<ManagerFacts> {
        Err("unused".into())
    }
    fn reload(&self, _: &Roots) -> Result<()> {
        panic!("must not reload")
    }
    fn start(&self, _: &Roots) -> Result<()> {
        panic!("must not start")
    }
}
#[test]
fn old_manager_implementations_default_to_refusal_and_foreign_pid_never_stopped() {
    let f = Fixture::new();
    let c = f.controller();
    let legacy = ExistingController {
        roots: f.roots.clone(),
        manager: Legacy,
        binary_sha256: c.binary_sha256.clone(),
    };
    assert!(legacy.inspect().is_err());
    assert!(legacy.manager.stop(&f.roots).is_err());
    assert!(legacy.manager.quiescence(&f.roots, "").is_err());
    {
        let mut facts = c.manager.facts.borrow_mut();
        facts.service.active = "active".into();
        facts.service.sub = "running".into();
        facts.service.pid = std::process::id();
        facts.control_group = "/foreign/smith-next.service".into();
    }
    assert!(c.prepare(Action::Stop).is_err());
    assert!(c.manager.calls.borrow().is_empty());
}
