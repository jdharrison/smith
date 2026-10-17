//! Artifact publication only: never execute a payload, manager or provider.

use crate::fs::{self, require, Effects, Fact, FileData, Filesystem, Selector, Trace, Tree};
use crate::manifest::{digest, manifest_bytes, EntryKind, APP, MANIFEST};
use crate::Result;
use std::cell::RefCell;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

const LOCK_SECONDS: u64 = 5;
const LOCK_NAME: &str = ".smith-next-install.lock";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Installed,
    Declined,
    DryRun,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Plan {
    tree: Option<Tree>,
    pub(crate) release: String,
    pub(crate) helper: Vec<u8>,
    installed: Option<FileData>,
    current: Option<Selector>,
    // Preserve the original desktop-then-wrapper publication order.
    files: Vec<(PathBuf, FileData)>,
}

/// An immutable, single-use review owned by the publisher that prepared it.
/// The ID binds local consent to this review; it is not authentication.
pub struct PreparedArtifacts {
    plan: Plan,
    trace: Trace,
    description: String,
    review_id: String,
    owner: Effects,
}

impl PreparedArtifacts {
    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn review_id(&self) -> &str {
        &self.review_id
    }
}

pub struct Installer {
    client: Option<PathBuf>,
    service: Option<PathBuf>,
    home: Option<PathBuf>,
    data: PathBuf,
    base: PathBuf,
    client_root: PathBuf,
    current: PathBuf,
    binary: PathBuf,
    effects: Effects,
}

fn absolute_source(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| format!("Cannot resolve artifact directory: {e}"))?
            .join(path))
    }
}

fn require_unprivileged(uid: u32) -> Result<()> {
    require(
        cfg!(target_os = "linux") && uid != 0,
        "Installer requires unprivileged Linux user",
    )
}

impl Installer {
    /// Resolve relevant roots without creating files or contacting a manager/provider.
    pub fn new(client: Option<PathBuf>, service: Option<PathBuf>) -> Result<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data = data_root()?;
        Self::with_roots(client, service, home, data)
    }

    pub fn with_roots(
        client: Option<PathBuf>,
        service: Option<PathBuf>,
        home: Option<PathBuf>,
        data: PathBuf,
    ) -> Result<Self> {
        require(
            client.is_some() || service.is_some(),
            "Select --client and/or --service",
        )?;
        let client = client.map(absolute_source).transpose()?;
        let service = service.map(absolute_source).transpose()?;
        fs::path_parts(&data)?;
        for path in client.iter().chain(service.iter()) {
            fs::path_parts(path)?;
        }
        if client.is_some() {
            fs::path_parts(home.as_ref().ok_or("Existing HOME root required")?)?;
        }
        let base = data.join("smith-next");
        let client_root = base.join("client");
        Ok(Self {
            client,
            service,
            home,
            current: client_root.join("current"),
            binary: base.join("bin/smith-service"),
            data,
            base,
            client_root,
            effects: Rc::new(RefCell::new(Vec::new())),
        })
    }

    pub(crate) fn maintenance_effects(&mut self, effects: Effects) {
        self.effects = effects;
    }

    pub(crate) fn filesystem(&self) -> Filesystem {
        Filesystem::new(self.effects.clone())
    }

    /// Inspect and capture exact artifacts/effects without destination writes,
    /// locks, stdout, or manager/provider operations. Keep this on its owner worker.
    pub fn prepare_review(&self) -> Result<PreparedArtifacts> {
        let mut fs = self.filesystem();
        let plan = self.inspect(&mut fs)?;
        let mut bytes = Vec::new();
        self.describe(&plan, &mut bytes)?;
        let mut description =
            String::from_utf8(bytes).expect("installation descriptions use validated ASCII paths");
        let review_id = fs::stage_name("smith-next-artifact-review-")?;
        use std::fmt::Write as _;
        writeln!(description, "Artifact review ID: {review_id}").expect("String write");
        Ok(PreparedArtifacts {
            plan,
            trace: fs.trace,
            description,
            review_id,
            owner: self.effects.clone(),
        })
    }

    /// Consume explicit approval of the displayed review, then revalidate its
    /// original facts. Output is caller-owned; no stdin, stdout or implicit yes.
    pub fn install_reviewed(
        &mut self,
        prepared: PreparedArtifacts,
        approved_review_id: &str,
        output: &mut dyn Write,
    ) -> Result<Outcome> {
        self.effects.borrow_mut().clear();
        let result = (|| {
            require(
                Rc::ptr_eq(&self.effects, &prepared.owner),
                "Artifact review belongs to a different installer",
            )?;
            require(
                prepared.review_id == approved_review_id,
                "Artifact review approval does not match the displayed review",
            )?;
            crate::consent::check()?;
            output
                .write_all(prepared.description.as_bytes())
                .map_err(|e| format!("Cannot display installation plan: {e}"))?;
            output
                .flush()
                .map_err(|e| format!("Cannot flush installation plan: {e}"))?;
            self.install_captured(&prepared.plan, &prepared.trace, output)
        })();
        self.report_result(result, output)
    }

    pub(crate) fn current_state(&self, fs: &mut Filesystem) -> Result<Option<Selector>> {
        verified_current(fs, &self.client_root)
    }

    fn inspect(&self, fs: &mut Filesystem) -> Result<Plan> {
        self.inspect_bound(fs, None)
    }

    fn inspect_bound(&self, fs: &mut Filesystem, old_sha256: Option<&str>) -> Result<Plan> {
        crate::consent::check()?;
        require_unprivileged(fs::effective_uid())?;
        require(
            fs.directory(&self.data, false, false)?.is_some(),
            "Existing XDG data root required",
        )?;
        if self.client.is_some() {
            require(
                fs.directory(
                    self.home.as_ref().ok_or("Existing HOME root required")?,
                    false,
                    false,
                )?
                .is_some(),
                "Existing HOME root required",
            )?;
        }
        for path in [
            self.base.clone(),
            self.base.join("bin"),
            self.client_root.clone(),
            self.client_root.join("releases"),
        ] {
            fs.directory(&path, false, true)?;
        }
        let mut helper = Vec::new();
        let mut release = String::new();
        let tree = if let Some(client) = &self.client {
            let tree = fs.tree(client, false)?;
            for name in ["smith_client", "smith_backend/smith-service"] {
                require(
                    tree.contents
                        .get(name)
                        .is_some_and(|bytes| !bytes.is_empty())
                        && tree
                            .entries
                            .get(name)
                            .is_some_and(|entry| entry.mode == 0o755),
                    format!("Bundle requires exact 0755 {name}"),
                )?;
            }
            for name in ["lib", "data", "data/flutter_assets"] {
                require(
                    tree.entries
                        .get(name)
                        .is_some_and(|entry| entry.kind == EntryKind::Directory),
                    format!("Incomplete Flutter bundle: {name}"),
                )?;
            }
            for name in [
                "lib/libflutter_linux_gtk.so".to_string(),
                "lib/libapp.so".to_string(),
                "data/icudtl.dat".to_string(),
                format!("share/pixmaps/{APP}.png"),
                format!("share/applications/{APP}.desktop"),
            ] {
                require(
                    tree.contents
                        .get(&name)
                        .is_some_and(|bytes| !bytes.is_empty()),
                    format!("Missing bundle input: {name}"),
                )?;
            }
            helper = tree.contents["smith_backend/smith-service"].clone();
            release = digest(&manifest_bytes(&tree.entries));
            Some(tree)
        } else {
            None
        };
        if let Some(service) = &self.service {
            let source = fs
                .file(service, Some(0o755))?
                .ok_or("Service artifact missing or empty")?;
            require(
                !source.bytes.is_empty(),
                "Service artifact missing or empty",
            )?;
            require(
                helper.is_empty() || helper == source.bytes,
                "Bundled/standalone helper mismatch",
            )?;
            helper = source.bytes;
        }
        let installed = fs.file(&self.binary, Some(0o755))?;
        require(
            installed.as_ref().is_none_or(|existing| {
                existing.bytes == helper
                    || old_sha256.is_some_and(|sha| digest(&existing.bytes) == sha)
            }),
            "Existing service differs; upgrades require separate approval",
        )?;
        let current = self.current_state(fs)?;
        if self.service.is_some() && self.client.is_none() {
            if let Some(current) = &current {
                let selected = fs.file(
                    &self
                        .client_root
                        .join(&current.target)
                        .join("smith_backend/smith-service"),
                    Some(0o755),
                )?;
                require(
                    selected
                        .as_ref()
                        .is_some_and(|selected| selected.bytes == helper),
                    "Standalone service differs from selected client helper",
                )?;
            }
        }
        let mut files = Vec::new();
        if self.client.is_some() {
            let path = self.client_root.join("releases").join(&release);
            if fs.directory(&path, false, false)?.is_some() {
                verify_release(fs, &path)?;
            }
            let executable = self.current.join("smith_client");
            let icon = self.current.join(format!("share/pixmaps/{APP}.png"));
            let desktop = format!(
                "[Desktop Entry]\nType=Application\nName=smith\nComment=Smith desktop client\nExec={}\nIcon={}\nStartupWMClass={APP}\nTerminal=false\nCategories=Utility;\n",
                executable.display(), icon.display()
            );
            files.push((
                self.data.join(format!("applications/{APP}.desktop")),
                FileData {
                    bytes: desktop.into_bytes(),
                    mode: 0o644,
                },
            ));
            let wrapper = format!(
                "#!/bin/sh\n# Managed by Smith next artifact installer.\nexec \"{}\" \"$@\"\n",
                executable.display()
            );
            files.push((
                self.home
                    .as_ref()
                    .ok_or("Existing HOME root required")?
                    .join(".local/bin/smith-next"),
                FileData {
                    bytes: wrapper.into_bytes(),
                    mode: 0o755,
                },
            ));
            for (destination, expected) in &files {
                let existing = fs.file(destination, Some(expected.mode))?;
                require(
                    existing
                        .as_ref()
                        .is_none_or(|existing| existing == expected),
                    format!("Unknown destination content: {}", destination.display()),
                )?;
            }
        }
        Ok(Plan {
            tree,
            release,
            helper,
            installed,
            current,
            files,
        })
    }

    fn describe(&self, plan: &Plan, output: &mut dyn Write) -> Result<()> {
        let mut description = String::from("Artifact installation only; no build, GUI launch, unit registration or service start.\n");
        use std::fmt::Write as _;
        if let Some(client) = &self.client {
            writeln!(description, "Client source: {}", client.display()).expect("String write");
            writeln!(description, "Client manifest SHA256: {}", plan.release)
                .expect("String write");
            writeln!(
                description,
                "Release: {}",
                self.client_root
                    .join("releases")
                    .join(&plan.release)
                    .display()
            )
            .expect("String write");
            writeln!(
                description,
                "Select current last: {}",
                self.current.display()
            )
            .expect("String write");
            for (path, _) in &plan.files {
                writeln!(description, "Managed file: {}", path.display()).expect("String write");
            }
        }
        writeln!(description, "Helper SHA256: {}", digest(&plan.helper)).expect("String write");
        if let Some(service) = &self.service {
            writeln!(description, "Service source: {}", service.display()).expect("String write");
            writeln!(
                description,
                "Standalone binary (no overwrite): {}",
                self.binary.display()
            )
            .expect("String write");
        }
        writeln!(
            description,
            "Installation lock: {}",
            self.data.join(LOCK_NAME).display()
        )
        .expect("String write");
        description.push_str("Creates named parents/staging files and a persistent private installation lock; preserves earlier releases. Multiple publications are not a global transaction. Failed stages are retained for inspection, not deleted.\n");
        if self.client.is_some() {
            writeln!(description, "Selector updates use atomic exchange and retain displaced objects under {}/.smith-next-selector-*. Conflicts may leave current changed; no automatic rollback or same-UID filesystem CAS is claimed.", self.client_root.display()).expect("String write");
        }
        output
            .write_all(description.as_bytes())
            .map_err(|e| format!("Cannot display installation plan: {e}"))?;
        output
            .flush()
            .map_err(|e| format!("Cannot flush installation plan: {e}"))
    }

    pub(crate) fn lock(
        &self,
        fs: &mut Filesystem,
        budget: Duration,
    ) -> Result<std::os::fd::OwnedFd> {
        let fd = fs
            .directory(&self.data, false, false)?
            .ok_or("Existing XDG data root required")?;
        let path = self.data.join(LOCK_NAME);
        crate::consent::check()?;
        let (lock, created) = match fs::open_at(
            fd.as_raw_fd(),
            LOCK_NAME,
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        ) {
            Ok(lock) => (lock, true),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (
                fs::open_at(
                    fd.as_raw_fd(),
                    LOCK_NAME,
                    libc::O_RDWR | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                    0,
                )
                .map_err(|e| format!("Open installation lock: {e}"))?,
                false,
            ),
            Err(e) => return Err(format!("Create installation lock: {e}")),
        };
        let identity = fs::verify_lock(lock.as_raw_fd())?;
        if created {
            fs.effect(format!("created installation lock {}", path.display()));
            fs::sync(lock.as_raw_fd(), &path)?;
            fs::sync(fd.as_raw_fd(), &self.data)?;
        }
        let deadline = Instant::now() + budget;
        loop {
            crate::consent::check()?;
            // SAFETY: lock stays owned for the entire critical section. Closing it releases flock.
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                break;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err(format!("Installation flock: {error}"));
            }
            require(Instant::now() < deadline, "Installation lock timeout")?;
            std::thread::sleep(
                Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        #[cfg(test)]
        fs::testing::event(fs::testing::Event::LockAcquired)?;
        fs::verify_named_identity(fd.as_raw_fd(), LOCK_NAME, &identity)
            .map_err(|e| format!("Installation lock replaced: {e}"))?;
        Ok(lock)
    }

    pub(crate) fn validate_maintenance_lock(&self, lock: &std::os::fd::OwnedFd) -> Result<()> {
        let mut fs = self.filesystem();
        let parent = fs
            .directory(&self.data, false, false)?
            .ok_or("Installation lock parent missing")?;
        let identity = fs::verify_lock(lock.as_raw_fd())?;
        fs::verify_named_identity(parent.as_raw_fd(), LOCK_NAME, &identity)
    }

    fn release(&self, fs: &mut Filesystem, plan: &Plan) -> Result<()> {
        crate::consent::check()?;
        let path = self.client_root.join("releases").join(&plan.release);
        if fs.directory(&path, false, false)?.is_some() {
            return verify_release(fs, &path);
        }
        let releases = self.client_root.join("releases");
        let fd = fs
            .directory(&releases, true, true)?
            .ok_or("Release parent missing")?;
        let stage = fs::stage_name(".smith-next-stage-")?;
        let root = releases.join(&stage);
        fs::mkdir_at(fd.as_raw_fd(), &stage, &root)?;
        fs.effect(format!(
            "created release staging directory {}",
            root.display()
        ));
        fs::sync(fd.as_raw_fd(), &releases)?;
        let tree = plan.tree.as_ref().ok_or("Client plan missing")?;
        for (rel, entry) in &tree.entries {
            crate::consent::check()?;
            if entry.kind == EntryKind::Directory {
                let mut scoped = self.filesystem();
                scoped.directory(&root.join(rel), true, false)?;
            } else {
                fs.publish(&root.join(rel), &tree.contents[rel], entry.mode)?;
            }
        }
        crate::consent::check()?;
        fs.publish(&root.join(MANIFEST), &manifest_bytes(&tree.entries), 0o600)?;
        for (rel, entry) in tree.entries.iter().rev() {
            crate::consent::check()?;
            if entry.kind == EntryKind::Directory {
                let mut scoped = self.filesystem();
                let path = root.join(rel);
                let child = scoped
                    .directory(&path, false, false)?
                    .ok_or("Staged directory disappeared")?;
                fs::chmod(child.as_raw_fd(), entry.mode, &path)?;
                fs::sync(child.as_raw_fd(), &path)?;
            }
        }
        let stage_fd = fs
            .directory(&root, false, true)?
            .ok_or("Staged release disappeared")?;
        fs::sync(stage_fd.as_raw_fd(), &root)?;
        crate::consent::check()?;
        fs::rename_at(
            fd.as_raw_fd(),
            &stage,
            fd.as_raw_fd(),
            &plan.release,
            libc::RENAME_NOREPLACE,
            &path,
        )?;
        fs.effect(format!("published immutable release {}", path.display()));
        fs::sync(fd.as_raw_fd(), &releases)?;
        verify_release(fs, &path)
    }

    fn verify_dependencies(&self, fs: &mut Filesystem, plan: &Plan) -> Result<()> {
        for path in [
            self.base.clone(),
            self.base.join("bin"),
            self.client_root.clone(),
            self.client_root.join("releases"),
        ] {
            fs.directory(&path, false, true)?;
        }
        if self.client.is_some() {
            verify_release(fs, &self.client_root.join("releases").join(&plan.release))?;
            for (path, expected) in &plan.files {
                require(
                    fs.file(path, Some(expected.mode))?.as_ref() == Some(expected),
                    format!("Published launcher/wrapper changed: {}", path.display()),
                )?;
            }
        } else {
            // Binary-only publication must also reverify its selected-client dependency.
            require(
                self.current_state(fs)? == plan.current,
                "Selected client changed during service installation",
            )?;
            if let Some(current) = &plan.current {
                let selected = fs.file(
                    &self
                        .client_root
                        .join(&current.target)
                        .join("smith_backend/smith-service"),
                    Some(0o755),
                )?;
                require(
                    selected
                        .as_ref()
                        .is_some_and(|selected| selected.bytes == plan.helper),
                    "Selected client helper changed",
                )?;
            }
        }
        let installed = fs.file(&self.binary, Some(0o755))?;
        if self.service.is_some() || plan.installed.is_some() {
            require(
                installed
                    .as_ref()
                    .is_some_and(|installed| installed.bytes == plan.helper),
                "Service changed before final verification",
            )?;
        } else {
            require(
                installed
                    .as_ref()
                    .is_none_or(|installed| installed.bytes == plan.helper),
                "Service changed",
            )?;
        }
        Ok(())
    }

    fn apply(&self, fs: &mut Filesystem, plan: &Plan) -> Result<()> {
        crate::consent::check()?;
        #[cfg(test)]
        fs::testing::event(fs::testing::Event::BeforeApply)?;
        if self.service.is_some() && plan.installed.is_none() {
            fs.directory(&self.base, true, true)?;
            fs.directory(&self.base.join("bin"), true, true)?;
            fs.publish(&self.binary, &plan.helper, 0o755)?;
        }
        crate::consent::check()?;
        if self.client.is_none() {
            return self.verify_dependencies(fs, plan);
        }
        fs.directory(&self.base, true, true)?;
        fs.directory(&self.client_root, true, true)?;
        self.release(fs, plan)?;
        for (path, expected) in &plan.files {
            crate::consent::check()?;
            let existing = fs.file(path, Some(expected.mode))?;
            require(
                existing
                    .as_ref()
                    .is_none_or(|existing| existing == expected),
                "Destination changed",
            )?;
            if existing.is_none() {
                fs.publish(path, &expected.bytes, expected.mode)?;
            }
        }
        let fd = fs
            .directory(&self.client_root, false, true)?
            .ok_or("Client namespace missing")?;
        let target = format!("releases/{}", plan.release);
        if plan
            .current
            .as_ref()
            .is_some_and(|current| current.target == target)
        {
            self.verify_dependencies(fs, plan)?;
            require(
                self.current_state(fs)? == plan.current,
                "Current selector changed",
            )?;
            return Ok(());
        }
        let stage = fs::stage_name(".smith-next-selector-")?;
        let retained = self.client_root.join(&stage);
        fs::symlink_at(fd.as_raw_fd(), &target, &stage, &retained)?;
        fs.effect(format!(
            "created selector staging object {}",
            retained.display()
        ));
        #[cfg(test)]
        fs::testing::event(fs::testing::Event::SelectorStaged(retained.clone()))?;
        let staged =
            fs::selector_at(fd.as_raw_fd(), &stage)?.ok_or("Staged selector disappeared")?;
        require(
            self.current_state(fs)? == plan.current,
            "Current selector changed",
        )?;
        self.verify_dependencies(fs, plan)?;
        crate::consent::check()?;
        if plan.current.is_none() {
            fs::rename_at(
                fd.as_raw_fd(),
                &stage,
                fd.as_raw_fd(),
                "current",
                libc::RENAME_NOREPLACE,
                &self.current,
            )?;
            fs.effect("published current selector; verification pending");
        } else {
            fs::rename_at(
                fd.as_raw_fd(),
                &stage,
                fd.as_raw_fd(),
                "current",
                libc::RENAME_EXCHANGE,
                &self.current,
            )?;
            fs.effect(format!("exchanged current selector; displaced object retained at {}; verification pending, no rollback", retained.display()));
        }
        #[cfg(test)]
        fs::testing::event(fs::testing::Event::AfterRename {
            path: self.current.clone(),
            retained: retained.clone(),
        })?;
        fs::sync(fd.as_raw_fd(), &self.client_root)?;
        if let Some(expected) = &plan.current {
            let displaced = (|| {
                let displaced = fs::selector_at(fd.as_raw_fd(), &stage)?
                    .ok_or("Displaced selector disappeared")?;
                require(
                    displaced.same_moved(expected),
                    "Captured selector identity mismatch",
                )
            })();
            displaced.map_err(|e| format!("Displaced selector conflict; retained at {}; current may have changed, no rollback: {e}", retained.display()))?;
        }
        require(
            fs::selector_at(fd.as_raw_fd(), "current")?
                .as_ref()
                .is_some_and(|current| current.same_moved(&staged)),
            "Current changed after publication; retained objects require inspection",
        )?;
        self.verify_dependencies(fs, plan)?;
        require(
            fs::selector_at(fd.as_raw_fd(), "current")?
                .as_ref()
                .is_some_and(|current| current.same_moved(&staged)),
            "Current changed during final verification; no rollback",
        )?;
        fs.effect(format!(
            "selected verified current release {}",
            plan.release
        ));
        Ok(())
    }

    fn revalidate(&self, plan: &Plan, trace: &Trace, message: &str) -> Result<()> {
        let mut fs = self.filesystem();
        require(
            self.inspect(&mut fs)? == *plan && fs.trace == *trace,
            message,
        )
    }

    /// Maintenance-only candidate capture. The old digest comes from independent
    /// selected-release inspection, not an executable override or force option.
    pub(crate) fn maintenance_candidate(&self, old_sha256: &str) -> Result<(Plan, Trace)> {
        let mut fs = self.filesystem();
        let plan = self.inspect_bound(&mut fs, Some(old_sha256))?;
        Ok((plan, fs.trace))
    }

    pub(crate) fn maintenance_revalidate(
        &self,
        plan: &Plan,
        trace: &Trace,
        old_sha256: &str,
    ) -> Result<()> {
        let (actual, facts) = self.maintenance_candidate(old_sha256)?;
        require(
            actual == *plan && facts == *trace,
            "Maintenance candidate or reviewed objects changed",
        )
    }

    /// Caller owns the installation lock and has exchanged/verified the binary.
    /// Reuse normal publication without recursively acquiring that same flock.
    pub(crate) fn maintenance_publish(&self, captured: &Plan) -> Result<()> {
        let mut fs = self.filesystem();
        let actual = self.inspect(&mut fs)?;
        let mut expected = captured.clone();
        expected.installed = Some(FileData {
            bytes: captured.helper.clone(),
            mode: 0o755,
        });
        require(
            actual == expected,
            "Candidate changed before held-lock publication",
        )?;
        self.apply(&mut fs, &actual)
    }

    fn install_captured(
        &self,
        plan: &Plan,
        trace: &Trace,
        output: &mut dyn Write,
    ) -> Result<Outcome> {
        crate::consent::check()?;
        self.revalidate(
            plan,
            trace,
            "Artifacts or destinations changed after confirmation",
        )?;
        let mut locking = self.filesystem();
        let _lock = self.lock(&mut locking, Duration::from_secs(LOCK_SECONDS))?;
        self.revalidate(
            plan,
            trace,
            "Artifacts or destinations changed before installation",
        )?;
        crate::consent::check()?;
        self.apply(&mut self.filesystem(), plan)?;
        crate::consent::check()?;
        writeln!(output, "Installation complete; nothing launched.").map_err(|e| e.to_string())?;
        Ok(Outcome::Installed)
    }

    pub fn run(
        &mut self,
        dry_run: bool,
        assume_yes: bool,
        confirm: &mut dyn FnMut(&str) -> Result<String>,
    ) -> Result<Outcome> {
        self.run_to(dry_run, assume_yes, confirm, &mut io::stdout())
    }

    fn run_to(
        &mut self,
        dry_run: bool,
        assume_yes: bool,
        confirm: &mut dyn FnMut(&str) -> Result<String>,
        output: &mut dyn Write,
    ) -> Result<Outcome> {
        self.effects.borrow_mut().clear();
        let result = (|| {
            let mut fs = self.filesystem();
            let plan = self.inspect(&mut fs)?;
            let trace = fs.trace;
            self.describe(&plan, output)?;
            if dry_run {
                writeln!(output, "Dry run: no mutation.").map_err(|e| e.to_string())?;
                return Ok(Outcome::DryRun);
            }
            if !assume_yes
                && confirm("Install these exact artifacts/effects? [y/N] ")?.to_lowercase() != "y"
            {
                writeln!(output, "Declined: no mutation.").map_err(|e| e.to_string())?;
                return Ok(Outcome::Declined);
            }
            self.install_captured(&plan, &trace, output)
        })();
        self.report_result(result, output)
    }

    fn report_result(&self, result: Result<Outcome>, output: &mut dyn Write) -> Result<Outcome> {
        if let Err(error) = &result {
            let _ = writeln!(output, "Installation stopped: {error}");
            let _ = writeln!(
                output,
                "No rollback assumed; inspect reported paths before retry."
            );
        }
        let effects = self.effects.borrow();
        if !effects.is_empty() {
            let _ = writeln!(
                output,
                "Completed/partial filesystem effects (including staging):"
            );
            for effect in effects.iter() {
                let _ = writeln!(output, "  {effect}");
            }
        }
        result
    }
}

pub(crate) fn data_root() -> Result<PathBuf> {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(data) => Ok(PathBuf::from(data)),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share"))
            .ok_or_else(|| "Existing XDG data root required (or HOME for its default)".into()),
    }
}

fn verified_current(fs: &mut Filesystem, client_root: &Path) -> Result<Option<Selector>> {
    let Some(fd) = fs.directory(client_root, false, false)? else {
        return Ok(None);
    };
    let Some(current) = fs::selector_at(fd.as_raw_fd(), "current")? else {
        fs.trace.insert(client_root.join("current"), Fact::Missing);
        return Ok(None);
    };
    let target = current.target.as_bytes();
    require(
        target.len() == 73
            && target.starts_with(b"releases/")
            && target[9..]
                .iter()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c)),
        "Unmanaged current target",
    )?;
    verify_release(fs, &client_root.join(&current.target))?;
    require(
        fs::selector_at(fd.as_raw_fd(), "current")?.as_ref() == Some(&current),
        "Current selector changed during verification",
    )?;
    fs.trace
        .insert(client_root.join("current"), Fact::Selector(current.clone()));
    Ok(Some(current))
}

/// A freshly checked installed release, never an installer payload or PATH entry.
#[derive(Debug)]
pub(crate) struct InstalledClient {
    pub(crate) executable: PathBuf,
    pub(crate) release_dir: PathBuf,
}

pub(crate) fn resolve_installed_client(data: &Path) -> Result<InstalledClient> {
    require_unprivileged(fs::effective_uid())?;
    let mut fs = Filesystem::new(Rc::new(RefCell::new(Vec::new())));
    require(
        fs.directory(data, false, false)?.is_some(),
        "Existing XDG data root required",
    )?;
    let base = data.join("smith-next");
    let client_root = base.join("client");
    for path in [&base, &client_root, &client_root.join("releases")] {
        require(
            fs.directory(path, false, true)?.is_some(),
            "Incomplete installed client namespaces",
        )?;
    }
    let selector =
        verified_current(&mut fs, &client_root)?.ok_or("No selected installed client")?;
    let release_dir = client_root.join(&selector.target);
    let executable = release_dir.join("smith_client");
    let verified = fs.trace.get(&executable).cloned();
    require(
        fs.file(&executable, Some(0o755))?
            .is_some_and(|file| !file.bytes.is_empty()),
        "Selected release lacks a safe client",
    )?;
    require(
        matches!((verified.as_ref(), fs.trace.get(&executable)),
            (Some(Fact::TreeEntry(before)), Some(Fact::File(after, _))) if before == after),
        "Installed client changed after release verification",
    )?;
    let fd = fs
        .directory(&client_root, false, true)?
        .ok_or("Installed client disappeared")?;
    require(
        fs::selector_at(fd.as_raw_fd(), "current")?.as_ref() == Some(&selector),
        "Current selector changed before launch",
    )?;
    Ok(InstalledClient {
        executable,
        release_dir,
    })
}

fn verify_release(fs: &mut Filesystem, path: &Path) -> Result<()> {
    require(
        fs.directory(path, false, true)?.is_some(),
        "Release missing",
    )?;
    let stored = fs
        .file(&path.join(MANIFEST), Some(0o600))?
        .ok_or("Release has no managed manifest")?;
    let tree = fs.tree(path, true)?;
    require(
        stored.bytes == manifest_bytes(&tree.entries)
            && path
                .file_name()
                .is_some_and(|name| name == digest(&stored.bytes).as_str()),
        "Release manifest/content identity mismatch",
    )
}

#[cfg(test)]
#[path = "../tests/fixtures/publisher.rs"]
pub(crate) mod fixture;

#[cfg(test)]
mod tests {
    use super::fixture::{chmod, write, Fixture};
    use super::*;
    use crate::fs::testing::{with_hook, Event};
    use std::fs as stdfs;
    use std::os::unix::fs::{symlink, MetadataExt};

    #[test]
    fn launch_resolution_is_installed_only_and_read_only() {
        let f = Fixture::new();
        let before = f.snapshot();
        assert!(resolve_installed_client(&f.data).is_err());
        assert_eq!(f.snapshot(), before);
        installed(&f, true, true);
        // The package is not needed to open the installed client.
        stdfs::remove_dir_all(&f.bundle).unwrap();
        let before = f.snapshot();
        let target = resolve_installed_client(&f.data).unwrap();
        let release = f
            .current()
            .parent()
            .unwrap()
            .join(stdfs::read_link(f.current()).unwrap());
        assert_eq!(target.release_dir, release);
        assert_eq!(target.executable, release.join("smith_client"));
        assert!(!target.executable.starts_with(&f.bundle));
        assert_eq!(f.snapshot(), before);
    }

    #[test]
    fn launch_resolution_rejects_unmanaged_selectors_without_writes() {
        for target in [
            "/tmp/foreign",
            "../bundle",
            "releases/../bundle",
            "releases/abc",
            &format!("releases/{}", "A".repeat(64)),
            &format!("releases/{}", "0".repeat(64)),
        ] {
            let f = Fixture::new();
            installed(&f, true, true);
            stdfs::remove_file(f.current()).unwrap();
            symlink(target, f.current()).unwrap();
            let before = f.snapshot();
            assert!(resolve_installed_client(&f.data).is_err(), "{target}");
            assert_eq!(f.snapshot(), before);
        }
        for regular in [false, true] {
            let f = Fixture::new();
            installed(&f, true, true);
            stdfs::remove_file(f.current()).unwrap();
            if regular {
                write(&f.current(), b"foreign", 0o600);
            }
            let before = f.snapshot();
            assert!(resolve_installed_client(&f.data).is_err());
            assert_eq!(f.snapshot(), before);
        }
    }

    #[test]
    fn launch_resolution_rejects_unsafe_or_modified_installed_files() {
        for case in 0..10 {
            let f = Fixture::new();
            installed(&f, true, true);
            let release = stdfs::canonicalize(f.current()).unwrap();
            let executable = release.join("smith_client");
            match case {
                0 => stdfs::remove_file(&executable).unwrap(),
                1 => write(&executable, b"", 0o755),
                2 => chmod(&executable, 0o644),
                3 => chmod(&executable, 0o777),
                4 => write(&executable, b"tampered", 0o755),
                5 => {
                    stdfs::remove_file(&executable).unwrap();
                    symlink(&f.service, &executable).unwrap();
                }
                6 => {
                    stdfs::remove_file(&executable).unwrap();
                    stdfs::hard_link(&f.service, &executable).unwrap();
                }
                7 => write(&release.join(MANIFEST), b"{}", 0o600),
                8 => chmod(&f.data.join("smith-next/client/releases"), 0o755),
                9 => {
                    let retained = f.data.join("retained-client");
                    stdfs::rename(f.data.join("smith-next/client"), &retained).unwrap();
                    symlink(&retained, f.data.join("smith-next/client")).unwrap();
                }
                _ => unreachable!(),
            }
            let before = f.snapshot();
            assert!(resolve_installed_client(&f.data).is_err(), "case {case}");
            assert_eq!(f.snapshot(), before);
        }
    }

    #[test]
    fn launch_resolution_rejects_selector_and_executable_drift() {
        for selector_drift in [true, false] {
            let f = Fixture::new();
            installed(&f, true, true);
            let current = f.current();
            let executable = stdfs::canonicalize(&current).unwrap().join("smith_client");
            let mut reads = 0;
            let result = with_hook(
                move |event| {
                    if matches!(event, Event::BeforeRead(name) if name == "smith_client") {
                        reads += 1;
                        if reads == 2 {
                            if selector_drift {
                                let target = stdfs::read_link(&current).unwrap();
                                stdfs::remove_file(&current).unwrap();
                                symlink(target, &current).unwrap();
                            } else {
                                write(&executable, b"changed-after-manifest", 0o755);
                            }
                        }
                    }
                    Ok(())
                },
                || resolve_installed_client(&f.data),
            );
            assert!(result.is_err());
        }
    }

    fn run(f: &Fixture, client: bool, service: bool) -> (Result<Outcome>, String) {
        let mut output = Vec::new();
        let result = f.installer(client, service).run_to(
            false,
            true,
            &mut |_| panic!("unexpected consent"),
            &mut output,
        );
        (result, String::from_utf8(output).unwrap())
    }

    fn installed(f: &Fixture, client: bool, service: bool) {
        let (result, output) = run(f, client, service);
        assert_eq!(result, Ok(Outcome::Installed), "{output}");
    }

    fn update() -> Fixture {
        let f = Fixture::new();
        installed(&f, true, true);
        write(
            &f.bundle.join("data/flutter_assets/asset"),
            b"updated",
            0o644,
        );
        f
    }

    fn stopped(result: &(Result<Outcome>, String)) {
        assert!(result.0.is_err(), "{}", result.1);
        assert!(result.1.contains("Installation stopped:"));
        assert!(result.1.contains("No rollback assumed"));
        assert!(!result.1.contains("Installation complete"));
        assert!(!result.1.contains("selected verified current release"));
    }

    fn reviewed(f: &Fixture, client: bool, service: bool) -> (Result<Outcome>, String) {
        let mut item = f.installer(client, service);
        let prepared = item.prepare_review().unwrap();
        let id = prepared.review_id().to_owned();
        let mut output = Vec::new();
        let result = item.install_reviewed(prepared, &id, &mut output);
        (result, String::from_utf8(output).unwrap())
    }

    #[test]
    fn reviewed_artifacts_revalidate_original_facts_under_lock() {
        let f = Fixture::new();
        let source = f.service.clone();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::LockAcquired) {
                    write(&source, b"changed under reviewed lock", 0o755);
                }
                Ok(())
            },
            || reviewed(&f, false, true),
        );
        stopped(&result);
        assert!(result.0.unwrap_err().contains("before installation"));
        assert!(result.1.contains("created installation lock"));
        assert!(!f.binary().exists());
    }

    #[test]
    fn reviewed_artifacts_report_partial_effects_and_require_fresh_review_for_retry() {
        let f = Fixture::new();
        let desktop = f.data.join(format!("applications/{APP}.desktop"));
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforePublish(path) if path == &desktop) {
                    return Err("injected reviewed launcher failure".into());
                }
                Ok(())
            },
            || reviewed(&f, true, true),
        );
        stopped(&result);
        assert!(result
            .1
            .contains("Completed/partial filesystem effects (including staging):"));
        assert!(result.1.contains("published immutable release"));
        assert!(f.binary().exists());
        assert!(!f.current().exists());
        let retried = reviewed(&f, true, true);
        assert_eq!(retried.0, Ok(Outcome::Installed), "{}", retried.1);
    }

    #[test]
    fn reviewed_selector_conflicts_retain_displaced_unknown_objects_without_rollback() {
        let f = update();
        let current = f.current();
        let foreign = f.root.join("reviewed-foreign");
        write(&foreign, b"reviewed unknown data must survive", 0o640);
        let inode = stdfs::metadata(&foreign).unwrap().ino();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforeRename { path, flags, .. } if path == &current && *flags == libc::RENAME_EXCHANGE)
                {
                    stdfs::rename(&foreign, &current).unwrap();
                }
                Ok(())
            },
            || reviewed(&f, true, true),
        );
        stopped(&result);
        let retained = f.named(f.current().parent().unwrap(), ".smith-next-selector-");
        assert_eq!(retained.len(), 1);
        assert_eq!(
            stdfs::read(&retained[0]).unwrap(),
            b"reviewed unknown data must survive"
        );
        assert_eq!(stdfs::metadata(&retained[0]).unwrap().ino(), inode);
        assert!(result.1.contains("current may have changed, no rollback"));
        assert!(result.1.contains(retained[0].to_str().unwrap()));
    }

    #[test]
    fn cli_and_reviewed_idempotent_service_share_description_and_reporting() {
        let f = Fixture::new();
        installed(&f, false, true);
        let mut item = f.installer(false, true);
        let mut cli = Vec::new();
        assert_eq!(
            item.run_to(false, false, &mut |_| Ok("y".into()), &mut cli),
            Ok(Outcome::Installed)
        );
        let prepared = item.prepare_review().unwrap();
        let id = prepared.review_id().to_owned();
        let mut gui = Vec::new();
        assert_eq!(
            item.install_reviewed(prepared, &id, &mut gui),
            Ok(Outcome::Installed)
        );
        let gui = String::from_utf8(gui).unwrap();
        assert_eq!(
            gui.replace(&format!("Artifact review ID: {id}\n"), ""),
            String::from_utf8(cli).unwrap()
        );
    }

    #[test]
    fn reviewed_artifacts_cancellation_checkpoints_preserve_accepted_effects() {
        const ROOT: &str = "SMITH_PUBLISHER_REVIEW_CANCEL_ROOT";
        const PHASE: &str = "SMITH_PUBLISHER_REVIEW_CANCEL_PHASE";
        if let Some(root) = std::env::var_os(ROOT) {
            // Test-only child protocol: production has no environment bypass.
            // The parent owns this private fixture and its lifetime; the child
            // never constructs another /tmp ancestor or resets terminal flags.
            let root = PathBuf::from(root);
            let phase = std::env::var(PHASE).unwrap();
            let binary = root.join("data/smith-next/bin/smith-service");
            let current = root.join("data/smith-next/client/current");
            crate::consent::install_cancellation().unwrap();
            let mut item = Installer::with_roots(
                Some(root.join("bundle")),
                Some(root.join("smith-service")),
                Some(root.join("home")),
                root.join("data"),
            )
            .unwrap();
            let prepared = item.prepare_review().unwrap();
            let id = prepared.review_id().to_owned();
            fn interrupt() {
                // Only this disposable child receives the composed Core/installer signal.
                assert_eq!(unsafe { libc::raise(libc::SIGINT) }, 0);
                assert!(crate::consent::cancelled());
            }
            if phase == "before" {
                interrupt();
            }
            let mut output = Vec::new();
            let phase_for_hook = phase.clone();
            let binary_for_hook = binary.clone();
            let result = with_hook(
                move |event| {
                    if phase_for_hook == "after-service"
                        && matches!(event, Event::AfterPublish(path) if path == &binary_for_hook)
                        || phase_for_hook == "before-selector"
                            && matches!(event, Event::SelectorStaged(_))
                    {
                        interrupt();
                    }
                    Ok(())
                },
                || item.install_reviewed(prepared, &id, &mut output),
            );
            let result = (result, String::from_utf8(output).unwrap());
            stopped(&result);
            let error = result.0.unwrap_err();
            assert!(error.contains("interrupted"), "{phase}: {error}");
            assert!(!current.exists());
            if phase == "before" {
                assert!(!root.join("data/.smith-next-install.lock").exists());
                assert!(!binary.exists());
            } else {
                assert_eq!(stdfs::read(&binary).unwrap(), b"service-v1");
                if phase == "after-service" {
                    assert!(!current.parent().unwrap().exists());
                    assert!(result
                        .1
                        .contains(&format!("published {}", binary.display())));
                } else {
                    let retained: Vec<_> = stdfs::read_dir(current.parent().unwrap())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| {
                            path.file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .starts_with(".smith-next-selector-")
                        })
                        .collect();
                    assert_eq!(retained.len(), 1);
                    assert!(result.1.contains("published immutable release"));
                    assert!(result.1.contains(retained[0].to_str().unwrap()));
                }
            }
            return;
        }
        let f = Fixture::new();
        for phase in ["before", "after-service", "before-selector"] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.env_clear().env(ROOT, &f.root).env(PHASE, phase)
                .args(["--exact", "artifact::tests::reviewed_artifacts_cancellation_checkpoints_preserve_accepted_effects", "--nocapture"]);
            let output = fixture::bounded_output(command, Duration::from_secs(10));
            assert!(
                output.status.success(),
                "{phase}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn complete_bundle_manifest_matches_python_golden_and_release_identity() {
        let f = Fixture::new();
        let before = f.snapshot();
        let item = f.installer(true, true);
        let plan = item.inspect(&mut item.filesystem()).unwrap();
        assert_eq!(
            manifest_bytes(&plan.tree.unwrap().entries),
            include_bytes!("../tests/fixtures/release-manifest-v1.json")
        );
        assert_eq!(
            plan.release,
            "20b56ce4694b74c2ee936538df964944e4853b295afa5289197adf258e2af14f"
        );
        assert_eq!(f.snapshot(), before);
    }

    #[test]
    fn root_refusal() {
        let f = Fixture::new();
        let before = f.snapshot();
        assert!(require_unprivileged(0)
            .unwrap_err()
            .contains("unprivileged Linux"));
        assert!(require_unprivileged(fs::effective_uid()).is_ok());
        assert_eq!(f.snapshot(), before);
    }

    #[test]
    fn service_only_post_publication_changes_fail_final_verification() {
        for change in ["bytes", "mode", "delete"] {
            let f = Fixture::new();
            let binary = f.binary();
            let path = binary.clone();
            let result = with_hook(
                move |event| {
                    if matches!(event, Event::AfterPublish(p) if p == &path) {
                        match change {
                            "bytes" => write(&path, b"post-publication foreign bytes", 0o755),
                            "mode" => chmod(&path, 0o750),
                            _ => stdfs::remove_file(&path).unwrap(),
                        }
                    }
                    Ok(())
                },
                || run(&f, false, true),
            );
            stopped(&result);
            assert!(result
                .1
                .contains(&format!("published {}", binary.display())));
            match change {
                "bytes" => assert_eq!(
                    stdfs::read(&binary).unwrap(),
                    b"post-publication foreign bytes"
                ),
                "mode" => assert_eq!(stdfs::metadata(&binary).unwrap().mode() & 0o7777, 0o750),
                _ => assert!(!binary.exists()),
            }
        }
    }

    #[test]
    fn existing_service_only_changes_at_apply_fail_final_verification() {
        for change in ["bytes", "mode", "delete"] {
            let f = Fixture::new();
            installed(&f, false, true);
            let path = f.binary();
            let result = with_hook(
                move |event| {
                    if matches!(event, Event::BeforeApply) {
                        match change {
                            "bytes" => write(&path, b"late existing-service bytes", 0o755),
                            "mode" => chmod(&path, 0o750),
                            _ => stdfs::remove_file(&path).unwrap(),
                        }
                    }
                    if matches!(event, Event::BeforePublish(_)) {
                        panic!("an existing service must not be republished");
                    }
                    Ok(())
                },
                || run(&f, false, true),
            );
            stopped(&result);
            assert!(!result.1.contains("published "));
            match change {
                "bytes" => assert_eq!(
                    stdfs::read(f.binary()).unwrap(),
                    b"late existing-service bytes"
                ),
                "mode" => assert_eq!(stdfs::metadata(f.binary()).unwrap().mode() & 0o7777, 0o750),
                _ => assert!(!f.binary().exists()),
            }
        }
    }

    #[test]
    fn service_only_verified_idempotent_install_has_no_publication() {
        let f = Fixture::new();
        installed(&f, false, true);
        let before = f.snapshot();
        let result = with_hook(
            |event| {
                assert!(
                    !matches!(event, Event::BeforePublish(_)),
                    "idempotent install published"
                );
                Ok(())
            },
            || run(&f, false, true),
        );
        assert_eq!(result.0, Ok(Outcome::Installed), "{}", result.1);
        assert_eq!(f.snapshot(), before);
    }

    #[test]
    fn service_only_reverifies_selected_client_before_success() {
        for change in ["release", "selector", "private-mode"] {
            let f = Fixture::new();
            installed(&f, true, false);
            let current = f.current();
            let binary = f.binary();
            let result = with_hook(
                move |event| {
                    if matches!(event, Event::AfterPublish(p) if p == &binary) {
                        match change {
                            "release" => write(
                                &current.join("smith_backend/smith-service"),
                                b"changed",
                                0o755,
                            ),
                            "selector" => {
                                stdfs::remove_file(&current).unwrap();
                                symlink("foreign", &current).unwrap();
                            }
                            _ => chmod(binary.parent().unwrap(), 0o750),
                        }
                    }
                    Ok(())
                },
                || run(&f, false, true),
            );
            stopped(&result);
            assert!(result.1.contains("published "));
            assert!(f.binary().exists());
        }
    }

    #[test]
    fn revalidate_under_lock() {
        let f = Fixture::new();
        let source = f.service.clone();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::LockAcquired) {
                    write(&source, b"changed", 0o755);
                }
                Ok(())
            },
            || run(&f, false, true),
        );
        stopped(&result);
        assert!(result.0.unwrap_err().contains("before installation"));
        assert!(!f.data.join("smith-next").exists());
    }

    #[test]
    fn bounded_concurrent_lock() {
        let f = Fixture::new();
        let path = f.data.join(LOCK_NAME);
        write(&path, b"", 0o600);
        let held = stdfs::File::open(&path).unwrap();
        assert_eq!(
            unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        let item = f.installer(false, true);
        let started = Instant::now();
        let result = item.lock(&mut item.filesystem(), Duration::from_millis(50));
        assert!(result.unwrap_err().contains("lock timeout"));
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(LOCK_SECONDS, 5);
        assert!(!f.data.join("smith-next").exists());
    }

    #[test]
    fn replaced_held_lock_is_rejected() {
        let f = Fixture::new();
        let path = f.data.join(LOCK_NAME);
        let replacement = f.root.join("replacement-lock");
        write(&replacement, b"", 0o600);
        let result = with_hook(
            move |event| {
                if matches!(event, Event::LockAcquired) {
                    stdfs::rename(&replacement, &path).unwrap();
                }
                Ok(())
            },
            || run(&f, false, true),
        );
        stopped(&result);
        assert!(result.0.unwrap_err().contains("Installation lock replaced"));
        assert!(!f.data.join("smith-next").exists());
    }

    #[test]
    fn no_clobber_service_publication_race() {
        let f = Fixture::new();
        let path = f.binary();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforePublish(p) if p == &path) {
                    write(&path, b"racer", 0o755);
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert_eq!(stdfs::read(f.binary()).unwrap(), b"racer");
        assert!(!f.current().exists());
        let stages = f.named(f.binary().parent().unwrap(), ".smith-next-stage-");
        assert_eq!(stages.len(), 1);
        assert_eq!(stdfs::read(&stages[0]).unwrap(), b"service-v1");
        assert!(result.1.contains(stages[0].to_str().unwrap()));
    }

    #[test]
    fn partial_failure_preserved_reported_and_retry() {
        let f = Fixture::new();
        let path = f.data.join(format!("applications/{APP}.desktop"));
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforePublish(p) if p == &path) {
                    return Err("injected launcher failure".into());
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert!(result.1.contains("published immutable release"));
        assert!(f.binary().exists());
        assert!(!f.current().exists());
        installed(&f, true, true);
    }

    #[test]
    fn directory_no_replace_rejects_even_empty_unknown_destination() {
        let f = Fixture::new();
        let item = f.installer(true, false);
        let mut fs = item.filesystem();
        let fd = fs.directory(&f.root, false, false).unwrap().unwrap();
        stdfs::create_dir(f.root.join("stage")).unwrap();
        stdfs::create_dir(f.root.join("unknown")).unwrap();
        assert!(fs::rename_at(
            fd.as_raw_fd(),
            "stage",
            fd.as_raw_fd(),
            "unknown",
            libc::RENAME_NOREPLACE,
            &f.root.join("unknown")
        )
        .is_err());
        assert!(f.root.join("stage").is_dir());
        assert!(f.root.join("unknown").is_dir());
    }

    #[test]
    fn late_empty_release_directory_is_not_clobbered() {
        let f = Fixture::new();
        let result = with_hook(
            |event| {
                if let Event::BeforeRename { path, flags, .. } = event {
                    if *flags == libc::RENAME_NOREPLACE
                        && path
                            .parent()
                            .is_some_and(|p| p.file_name().unwrap() == "releases")
                    {
                        stdfs::create_dir(path).unwrap();
                        chmod(path, 0o700);
                    }
                }
                Ok(())
            },
            || run(&f, true, false),
        );
        stopped(&result);
        assert!(!f.current().exists());
        let releases = f.data.join("smith-next/client/releases");
        assert_eq!(f.named(&releases, ".smith-next-stage-").len(), 1);
        assert_eq!(stdfs::read_dir(&releases).unwrap().count(), 2);
    }

    #[test]
    fn descriptor_relative_traversal_blocks_symlink_swap() {
        let f = Fixture::new();
        let path = f.data.join("smith-next");
        let target = f.bundle.clone();
        let mut swapped = false;
        let result = with_hook(
            move |event| {
                if !swapped && matches!(event, Event::BeforeOpen(p) if p == &path) {
                    swapped = true;
                    symlink(&target, &path).unwrap();
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert!(!f.bundle.join("bin").exists());
    }

    #[test]
    fn current_no_clobber_race() {
        let f = Fixture::new();
        let current = f.current();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforeRename { path, flags, .. } if path == &current && *flags == libc::RENAME_NOREPLACE)
                {
                    symlink("foreign", &current).unwrap();
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert_eq!(stdfs::read_link(f.current()).unwrap(), Path::new("foreign"));
        assert_eq!(
            f.named(f.current().parent().unwrap(), ".smith-next-selector-")
                .len(),
            1
        );
    }

    #[test]
    fn verified_current_replacement_revalidated_before_switch() {
        let f = update();
        let current = f.current();
        let result = with_hook(
            move |event| {
                if let Event::SelectorStaged(path) = event {
                    assert_eq!(path.parent(), current.parent());
                    stdfs::remove_file(&current).unwrap();
                    symlink("foreign", &current).unwrap();
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert_eq!(stdfs::read_link(f.current()).unwrap(), Path::new("foreign"));
        assert!(!result.1.contains("exchanged current selector"));
    }

    #[test]
    fn late_foreign_regular_current_preserved_by_exchange() {
        let f = update();
        let current = f.current();
        let foreign = f.root.join("foreign");
        write(&foreign, b"foreign data must survive", 0o640);
        let inode = stdfs::metadata(&foreign).unwrap().ino();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforeRename { path, flags, .. } if path == &current && *flags == libc::RENAME_EXCHANGE)
                {
                    stdfs::rename(&foreign, &current).unwrap();
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        let retained = f.named(f.current().parent().unwrap(), ".smith-next-selector-");
        assert_eq!(retained.len(), 1);
        assert_eq!(
            stdfs::read(&retained[0]).unwrap(),
            b"foreign data must survive"
        );
        assert_eq!(stdfs::metadata(&retained[0]).unwrap().ino(), inode);
        assert_eq!(
            stdfs::metadata(&retained[0]).unwrap().mode() & 0o7777,
            0o640
        );
        assert_eq!(
            stdfs::read(f.current().join("data/flutter_assets/asset")).unwrap(),
            b"updated"
        );
        assert!(result.1.contains("Displaced selector conflict"));
        assert!(result.1.contains(retained[0].to_str().unwrap()));
        assert!(result.1.contains("current may have changed, no rollback"));
    }

    #[test]
    fn late_same_target_new_selector_inode_is_conflict() {
        let f = update();
        let current = f.current();
        let target = stdfs::read_link(&current).unwrap();
        let foreign = f.root.join("foreign");
        symlink(&target, &foreign).unwrap();
        let inode = stdfs::symlink_metadata(&foreign).unwrap().ino();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforeRename { path, flags, .. } if path == &current && *flags == libc::RENAME_EXCHANGE)
                {
                    stdfs::rename(&foreign, &current).unwrap();
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        let retained = f.named(f.current().parent().unwrap(), ".smith-next-selector-");
        assert_eq!(retained.len(), 1);
        assert_eq!(stdfs::symlink_metadata(&retained[0]).unwrap().ino(), inode);
        assert_eq!(stdfs::read_link(&retained[0]).unwrap(), target);
        assert!(result.1.contains("Displaced selector conflict"));
    }

    #[test]
    fn exchange_never_rolls_back_over_newer_current() {
        let f = update();
        let current = f.current();
        let inode = stdfs::symlink_metadata(&current).unwrap().ino();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::AfterRename { path, .. } if path == &current) {
                    stdfs::remove_file(&current).unwrap();
                    write(&current, b"newer foreign data", 0o600);
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert_eq!(stdfs::read(f.current()).unwrap(), b"newer foreign data");
        let retained = f.named(f.current().parent().unwrap(), ".smith-next-selector-");
        assert_eq!(retained.len(), 1);
        assert_eq!(stdfs::symlink_metadata(&retained[0]).unwrap().ino(), inode);
    }

    #[test]
    fn exchange_preserves_unknown_replacement_at_displaced_path() {
        let f = update();
        let result = with_hook(
            |event| {
                if let Event::AfterRename { retained, .. } = event {
                    stdfs::remove_file(retained).unwrap();
                    write(retained, b"late displaced-path foreign data", 0o600);
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        let retained = f.named(f.current().parent().unwrap(), ".smith-next-selector-");
        assert_eq!(retained.len(), 1);
        assert_eq!(
            stdfs::read(&retained[0]).unwrap(),
            b"late displaced-path foreign data"
        );
        assert!(result.1.contains("Displaced selector conflict"));
    }

    #[test]
    fn exchange_sync_failure_reports_partial_selection_and_retains_old() {
        let f = update();
        let inode = stdfs::symlink_metadata(f.current()).unwrap().ino();
        let mut exchanged = false;
        let result = with_hook(
            move |event| {
                if matches!(event, Event::AfterRename { .. }) {
                    exchanged = true;
                }
                if exchanged && matches!(event, Event::Sync(path) if path.ends_with("client")) {
                    return Err("injected sync failure".into());
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        let retained = f.named(f.current().parent().unwrap(), ".smith-next-selector-");
        assert_eq!(retained.len(), 1);
        assert_eq!(stdfs::symlink_metadata(&retained[0]).unwrap().ino(), inode);
        assert_eq!(
            stdfs::read(f.current().join("data/flutter_assets/asset")).unwrap(),
            b"updated"
        );
        assert!(result.1.contains("exchanged current selector"));
        assert!(result.1.contains("verification pending, no rollback"));
    }

    fn dependency(f: &Fixture, name: &str) -> PathBuf {
        match name {
            "desktop" => f.data.join(format!("applications/{APP}.desktop")),
            "wrapper" => f.home.join(".local/bin/smith-next"),
            "service" => f.binary(),
            _ => panic!("release path is computed from the new selector"),
        }
    }

    #[test]
    fn dependencies_reverified_before_selection() {
        for name in ["release", "desktop", "wrapper", "service"] {
            let f = update();
            let old = stdfs::read_link(f.current()).unwrap();
            let path = (name != "release").then(|| dependency(&f, name));
            let result = with_hook(
                move |event| {
                    if let Event::SelectorStaged(stage) = event {
                        let path = path.clone().unwrap_or_else(|| {
                            stage
                                .parent()
                                .unwrap()
                                .join(stdfs::read_link(stage).unwrap())
                                .join("lib/libapp.so")
                        });
                        let mode = stdfs::metadata(&path).unwrap().mode() & 0o7777;
                        write(&path, b"late dependency tampering", mode);
                    }
                    Ok(())
                },
                || run(&f, true, true),
            );
            stopped(&result);
            assert_eq!(stdfs::read_link(f.current()).unwrap(), old);
            assert!(!result.1.contains("exchanged current selector"));
        }
    }

    #[test]
    fn dependencies_reverified_after_exchange_before_success() {
        for name in ["release", "desktop", "wrapper", "service"] {
            let f = update();
            let path = if name == "release" {
                f.current().join("lib/libapp.so")
            } else {
                dependency(&f, name)
            };
            let result = with_hook(
                move |event| {
                    if matches!(event, Event::AfterRename { .. }) {
                        let mode = stdfs::metadata(&path).unwrap().mode() & 0o7777;
                        write(&path, b"late tampering at exchange", mode);
                    }
                    Ok(())
                },
                || run(&f, true, true),
            );
            stopped(&result);
            assert!(result.1.contains("exchanged current selector"));
            assert_eq!(
                f.named(f.current().parent().unwrap(), ".smith-next-selector-")
                    .len(),
                1
            );
        }
    }

    #[test]
    fn staging_failure_retained_and_retry_uses_fresh_stage() {
        let f = Fixture::new();
        let result = with_hook(
            |event| {
                if matches!(event, Event::BeforePublish(path) if path.ends_with("data/icudtl.dat"))
                {
                    return Err("injected stage failure".into());
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        let releases = f.data.join("smith-next/client/releases");
        let stages = f.named(&releases, ".smith-next-stage-");
        assert_eq!(stages.len(), 1);
        assert!(!f.current().exists());
        assert!(result.1.contains(stages[0].to_str().unwrap()));
        installed(&f, true, true);
        assert!(stages[0].is_dir());
        assert_eq!(f.named(&releases, ".smith-next-stage-").len(), 1);
    }

    #[test]
    fn retained_unknown_file_stage_is_never_deleted() {
        let f = Fixture::new();
        let binary = f.binary();
        let result = with_hook(
            move |event| {
                if let Event::BeforeRename { path, source, .. } = event {
                    if path == &binary {
                        let stage = path.parent().unwrap().join(source);
                        stdfs::remove_file(&stage).unwrap();
                        write(&stage, b"unknown replacement stage", 0o600);
                        return Err("injected stop after replacement".into());
                    }
                }
                Ok(())
            },
            || run(&f, false, true),
        );
        stopped(&result);
        let stages = f.named(f.binary().parent().unwrap(), ".smith-next-stage-");
        assert_eq!(stages.len(), 1);
        assert_eq!(
            stdfs::read(&stages[0]).unwrap(),
            b"unknown replacement stage"
        );
        assert!(!f.binary().exists());
        installed(&f, false, true);
        assert!(stages[0].exists());
    }

    #[test]
    fn source_changes_during_read_are_rejected() {
        let f = Fixture::new();
        let source = f.service.clone();
        let result = with_hook(
            move |event| {
                if matches!(event, Event::BeforeRead(name) if name == "smith-service") {
                    write(&source, b"changed while reading", 0o755);
                }
                Ok(())
            },
            || run(&f, false, true),
        );
        stopped(&result);
        assert!(result.0.unwrap_err().contains("File changed while reading"));
        assert!(!f.data.join(LOCK_NAME).exists());
    }

    #[test]
    fn tree_directory_snapshot_detects_changed_entries() {
        let f = Fixture::new();
        let path = f.bundle.join("data/flutter_assets/late");
        let mut changed = false;
        let result = with_hook(
            move |event| {
                if !changed && matches!(event, Event::BeforeRead(name) if name == "asset") {
                    changed = true;
                    write(&path, b"late addition", 0o644);
                }
                Ok(())
            },
            || run(&f, true, true),
        );
        stopped(&result);
        assert!(result
            .0
            .unwrap_err()
            .contains("Bundle changed during traversal"));
        assert!(!f.data.join(LOCK_NAME).exists());
    }

    #[test]
    fn changed_destination_inode_after_consent_is_rejected() {
        let f = Fixture::new();
        installed(&f, true, true);
        let before_target = stdfs::read_link(f.current()).unwrap();
        let mut item = f.installer(true, true);
        let mut output = Vec::new();
        let result = item.run_to(
            false,
            false,
            &mut |_| {
                let replacement = f.root.join("replacement");
                symlink(&before_target, &replacement).unwrap();
                stdfs::rename(replacement, f.current()).unwrap();
                Ok("Y".into())
            },
            &mut output,
        );
        assert!(result.unwrap_err().contains("after confirmation"));
        assert_eq!(stdfs::read_link(f.current()).unwrap(), before_target);
    }
}
