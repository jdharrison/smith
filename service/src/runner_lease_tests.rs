use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

struct LeaseFixture {
    root: PathBuf,
    path: PathBuf,
}

impl LeaseFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "smith-runner-lease-{}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .expect("create private lease-test root");
        let fixture = Self {
            path: root.join("lease"),
            root,
        };
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&fixture.path)
            .expect("create owned regular lease file");
        fixture
    }

    fn locked_file(&self) -> File {
        let file = self.open();
        try_lock(&file).expect("acquire initial lease");
        file
    }

    fn open(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .expect("independently open lease file")
    }

    fn model(&self, file: File) -> VerifiedModel {
        VerifiedModel {
            // A synthetic held regular-file descriptor, not verified model weights.
            file: self.open(),
            path: self.path.clone(),
            sha256: "0".repeat(64),
            lease: Some((
                self.path.clone(),
                Arc::new(Mutex::new(ParkedLease(Some(file)))),
            )),
        }
    }

    fn probe(&self) -> std::io::Result<()> {
        let contender = self.open();
        try_lock(&contender)?;
        // A parallel fork could inherit even this probe before exec. Do not
        // depend on close alone to release a successful probe's OFD lock.
        unlock(&contender);
        Ok(())
    }

    fn assert_held(&self) {
        let error = self
            .probe()
            .expect_err("independent contender must be busy");
        assert_eq!(error.kind(), ErrorKind::WouldBlock);
        assert_eq!(error.raw_os_error(), Some(libc::EWOULDBLOCK));
    }

    fn assert_released(&self) {
        self.probe()
            .expect("independent contender must acquire before duplicate closes");
    }
}

impl Drop for LeaseFixture {
    fn drop(&mut self) {
        // Remove only the file and directory this fixture created.
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.root);
    }
}

fn try_lock(file: &File) -> std::io::Result<()> {
    // SAFETY: file owns a valid descriptor throughout this nonblocking call.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn unlock(file: &File) {
    // SAFETY: file owns the descriptor and this test controls its OFD lease.
    assert_eq!(unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) }, 0);
}

fn cleaned_runner(model: &VerifiedModel) -> Runner {
    let (_, slot) = model.lease.as_ref().expect("model has a lease slot");
    let lock = slot
        .lock()
        .expect("unpoisoned slot")
        .0
        .take()
        .expect("parked lease available for consumption");
    // No child or directory remains: exercise actual terminal handback without
    // spawning or making any assertion about process cleanup or timeout paths.
    Runner {
        child: None,
        pid: 0,
        dir: None,
        socket: PathBuf::new(),
        key: String::new(),
        alias: String::new(),
        context: CONTEXT,
        lock: Some(lock),
        lease_return: Some(Arc::clone(slot)),
        stderr: Arc::new(Mutex::new(Vec::new())),
        evidence_stderr: Arc::new(Mutex::new(StartupEvidence::default())),
        cancelled: Arc::new(|| false),
        runtime_profile: RuntimeProfile::Unknown,
        inspection_template_verified: false,
    }
}

fn poison(slot: &Mutex<ParkedLease>) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = slot.lock().expect("slot initially unpoisoned");
        panic!("intentional lease-slot poison");
    }));
    assert!(outcome.is_err());
    assert!(slot.is_poisoned());
}

#[test]
fn model_dropped_then_stop_releases_final_parked_lease_with_duplicate_alive() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    let runner = cleaned_runner(&model);

    drop(model);
    fixture.assert_held();
    runner.stop().expect("cleaned runner stop hands back lease");
    fixture.assert_released();
    drop(duplicate);
}

#[test]
fn model_dropped_then_runner_drop_releases_final_parked_lease_with_duplicate_alive() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    let runner = cleaned_runner(&model);

    drop(model);
    fixture.assert_held();
    drop(runner);
    fixture.assert_released();
    drop(duplicate);
}

#[test]
fn dropping_model_does_not_unlock_raw_active_runner_lease() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    let runner = cleaned_runner(&model);
    assert!(model.lease.as_ref().unwrap().1.lock().unwrap().0.is_none());

    drop(model);
    fixture.assert_held();
    assert!(runner.lock.is_some());
    runner
        .stop()
        .expect("release after active-ownership assertion");
    fixture.assert_released();
    drop(duplicate);
}

#[test]
fn live_model_handback_and_reconsumption_keep_same_lease_until_final_owner_drop() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let original_fd = file.as_raw_fd();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    let runner = cleaned_runner(&model);
    fixture.assert_held();

    runner.stop().expect("hand back to live model");
    fixture.assert_held();
    {
        let slot = model.lease.as_ref().unwrap().1.lock().unwrap();
        assert_eq!(slot.0.as_ref().unwrap().as_raw_fd(), original_fd);
    }
    let runner = cleaned_runner(&model);
    assert_eq!(runner.lock.as_ref().unwrap().as_raw_fd(), original_fd);
    fixture.assert_held();
    drop(runner);
    fixture.assert_held();

    drop(model);
    fixture.assert_released();
    drop(duplicate);
}

#[test]
fn unknown_cleanup_ownership_state_simulation_keeps_duplicate_excluded_after_raw_close() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    let slot = Arc::clone(&model.lease.as_ref().unwrap().1);
    let active = slot.lock().unwrap().0.take().unwrap();

    // Ownership-state simulation only: an empty parked slot must not unlock
    // raw active ownership. This does not execute a child-exit timeout branch.
    drop(model);
    drop(slot);
    fixture.assert_held();
    drop(active);
    fixture.assert_held();

    // Explicit positive cleanup control, not evidence of actual child exit or
    // close-last-duplicate behavior (a parallel fork could inherit another FD).
    unlock(&duplicate);
    fixture.assert_released();
    drop(duplicate);
}

#[test]
fn poisoned_populated_final_slot_releases_lease_with_duplicate_alive() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    poison(&model.lease.as_ref().unwrap().1);
    fixture.assert_held();

    drop(model);
    fixture.assert_released();
    drop(duplicate);
}

#[test]
fn poisoned_empty_final_slot_does_not_unlock_raw_active_lease() {
    let fixture = LeaseFixture::new();
    let file = fixture.locked_file();
    let duplicate = file.try_clone().expect("duplicate same OFD");
    let model = fixture.model(file);
    let slot = &model.lease.as_ref().unwrap().1;
    let active = slot.lock().unwrap().0.take().unwrap();
    poison(slot);

    drop(model);
    fixture.assert_held();
    drop(active);
    fixture.assert_held();
    // Test cleanup control only; no child lifecycle is exercised here.
    unlock(&duplicate);
    fixture.assert_released();
    drop(duplicate);
}
