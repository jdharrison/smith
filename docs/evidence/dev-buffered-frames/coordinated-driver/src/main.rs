//! Local one-shot operational adapter; no alternative publisher or lifecycle path.
mod hook;
use sha2::{Digest, Sha256};
use smith_installer::{
    consent,
    maintenance::{Native, Operation, Prompt, Runner},
    Result,
};
use smith_service::Roots;
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const OLD: &str = "b7790a585d568373c0c3207e34ea4bbbeec080f863ff9ecec0a08b60a25c9d10";
const NEW: &str = "9bb0228c61bf762c7899f15f7105924d666b6e534102a33c6ecea8779fee77ec";
const CLIENT: &str = "49ba82e101bd54799948d4104cd6faa892d54ed1b2bc3511c4ac28e029abc9b5";
const PACKAGE: &str = "ddb1e8077249da1b5d9bdd229519f7894d8443e4eeb775318c3429da3e01436a";
const SELECTED: &str = "9f670a310ffc1f883c5fdb6b93ebcebdf93b035de8928b8ba98e789b0c9343fb";
const UNIT: &str = "d1deeb0d1525212edbc56bbb5a8eb72bec327612e97f7b30aba045cf95c49ab3";
const REQUEST: &str = "ae8149524f0cd9e3d9f95a8205d7ee4a";
const TITLES: [&str; 3] = [
    "Upgrade Smith artifacts",
    "Stop existing Smith service",
    "Start",
];
const BACKED_UP: &str = "Old artifacts/unit preserved; lifecycle remains separately consented";
const STOPPED: &str = "Artifacts upgraded; service stopped; no models provisioned";
const STOP: &str = "Stop only the verified old-owned smith-next.service; verify PID0, empty cgroup and both sockets absent. No enablement, data deletion or retry.";
const START: &str = "Start only smith-next.service through the user manager; create its private runtime namespace/socket. Service survives desktop detach; no enable or detached fallback.";
const KEEP: &str = "Keep ALL Core/app state, history, preferences, models, releases, backups, stages, lockfiles and unrelated files. Saved API keys remain in the keyring and are NOT revoked or removed by uninstall. Manage/revoke credentials externally; this version has no key removal action. Retained data continues to use disk space. No purge, automatic retry/rollback, model download or old-helper execution.";
const FAILURE: &str = "Coordinated maintenance incomplete; no automatic retry or rollback. Inspect retained evidence privately.";
fn ensure(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err("Approval binding refused".into())
    }
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("fixed source layout")
        .to_path_buf()
}
fn evidence() -> PathBuf {
    root().join("docs/evidence/dev-buffered-frames")
}
fn bundle() -> PathBuf {
    root().join(".smith-dev-coordinated-candidate-v2")
}

// Do not follow a substituted leaf; native validation remains authoritative for
// ancestor identity, publication, manager facts, leases and original consent.
fn open(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "Read refused".into())
}
fn hash(path: &Path) -> Result<String> {
    let mut f = open(path)?;
    let before = f.metadata().map_err(|_| "Metadata refused")?;
    ensure(
        before.is_file()
            && before.uid() == unsafe { libc::geteuid() }
            && before.mode() & 0o022 == 0,
    )?;
    let mut digest = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        consent::check()?;
        let n = f.read(&mut buf).map_err(|_| "Read refused")?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    let after = f.metadata().map_err(|_| "Metadata refused")?;
    ensure(
        before.len() == after.len()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec(),
    )?;
    Ok(format!("{:x}", digest.finalize()))
}
fn json(path: &Path) -> Result<serde_json::Value> {
    let f = open(path)?;
    ensure(f.metadata().map_err(|_| "Metadata refused")?.len() <= 1024 * 1024)?;
    let mut bytes = Vec::new();
    f.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Read refused")?;
    ensure(bytes.len() <= 1024 * 1024)?;
    serde_json::from_slice(&bytes).map_err(|_| "JSON refused".into())
}
fn candidate() -> Result<()> {
    ensure(hash(&bundle().join("manifest.json"))? == PACKAGE)?;
    let manifest = json(&bundle().join("manifest.json"))?;
    ensure(manifest["service_sha256"] == NEW && manifest["client_manifest_sha256"] == CLIENT)?;
    let files = manifest["files"].as_object().ok_or("Inventory refused")?;
    ensure(files.len() == 97)?;
    for (name, entry) in files {
        let relative = Path::new(name);
        ensure(
            relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        )?;
        let p = bundle().join(relative);
        ensure(hash(&p)? == entry["sha256"].as_str().ok_or("Digest refused")?)?;
        ensure(
            fs::symlink_metadata(&p)
                .map_err(|_| "Metadata refused")?
                .permissions()
                .mode()
                & 0o7777
                == entry["mode"].as_u64().ok_or("Mode refused")? as u32,
        )?;
    }
    ensure(hash(&bundle().join("payload/service/smith-service"))? == NEW)?;
    ensure(hash(&bundle().join("payload/client/smith_backend/smith-service"))? == NEW)
}
fn installed(sha: &str, release: &str) -> Result<()> {
    let r = Roots::environment()?;
    ensure(hash(&r.binary())? == sha && hash(&r.unit())? == UNIT)?;
    let client = r.data.join("smith-next/client");
    ensure(
        fs::read_link(client.join("current")).map_err(|_| "Selector refused")?
            == Path::new("releases").join(release),
    )?;
    let selected = client.join("releases").join(release);
    ensure(hash(&selected.join(".smith-next-manifest.json"))? == release)?;
    ensure(hash(&selected.join("smith_backend/smith-service"))? == sha)?;
    ensure(fs::read_link(r.enablement_link()).map_err(|_| "Enablement refused")? == r.unit())
}
fn review() -> Result<Vec<String>> {
    let r = Roots::environment()?;
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME required")?);
    Ok(vec![format!("Upgrade exact manifest-owned installation; old helper SHA256 {OLD}"), KEEP.into(),
        format!("Active object: {}", r.data.join("applications/dev.smith.smith_client.desktop").display()),
        format!("Active object: {}", home.join(".local/bin/smith-next").display()),
        format!("Active object: {}", r.data.join("smith-next/client/current").display()),
        format!("Active object: {}", r.binary().display()),
        format!("Unit: {}; running=true, registered=true; lifecycle confirmations are separate", r.unit().display()),
        format!("Candidate client manifest SHA256 {CLIENT}; helper SHA256 {NEW}; sources {}/payload/client and {}/payload/service/smith-service; publish paired client via existing guarded publisher", bundle().display(), bundle().display())])
}
fn reconciliation(value: &serde_json::Value) -> Result<()> {
    ensure(
        value["status"] == "reconciled"
            && value["request_id"] == REQUEST
            && value["candidate_sha256"] == NEW,
    )
}
trait Checks {
    fn fresh(&mut self, upgraded: bool) -> Result<()>;
    fn stopped_hook(&mut self) -> Result<()>;
    fn reconciled(&mut self) -> Result<()>;
}
#[derive(Default)]
struct Local {
    log: Option<File>,
}
impl Checks for Local {
    fn fresh(&mut self, upgraded: bool) -> Result<()> {
        candidate()?;
        installed(
            if upgraded { NEW } else { OLD },
            if upgraded { CLIENT } else { SELECTED },
        )
    }
    fn stopped_hook(&mut self) -> Result<()> {
        // Invoked synchronously from Native::run's progress callback BEFORE
        // drop(runner): installation, Core and runner leases are still held.
        ensure(
            fs::symlink_metadata(evidence().join("coordinated-reconciliation.json"))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
        )?;
        let mut command = Command::new("python3");
        command
            .arg("-B")
            .arg(evidence().join("coordinated-maintenance.py"))
            .arg("stopped-hook")
            .current_dir(root());
        hook::run(
            &mut command,
            self.log.take().ok_or("Private log required")?,
            Duration::from_secs(45),
            consent::cancelled,
        )
    }
    fn reconciled(&mut self) -> Result<()> {
        reconciliation(&json(&evidence().join("coordinated-reconciliation.json"))?)
    }
}
struct Approved<C> {
    checks: C,
    review: Vec<String>,
    next: usize,
    backup: bool,
    hook_attempted: bool,
    hook_ok: bool,
    poisoned: bool,
}
impl<C: Checks> Approved<C> {
    fn new(checks: C, review: Vec<String>) -> Self {
        Self {
            checks,
            review,
            next: 0,
            backup: false,
            hook_attempted: false,
            hook_ok: false,
            poisoned: false,
        }
    }
    fn confirm_inner(
        &mut self,
        title: &str,
        effects: &[String],
        deadline: Instant,
    ) -> Result<bool> {
        consent::check()?;
        ensure(
            !self.poisoned && Instant::now() < deadline && TITLES.get(self.next) == Some(&title),
        )?;
        let expected = match self.next {
            0 => self.review.clone(),
            1 => {
                ensure(self.backup)?;
                vec![STOP.into()]
            }
            2 => {
                ensure(self.hook_ok)?;
                self.checks.reconciled()?;
                vec![START.into()]
            }
            _ => return Err("Confirmation refused".into()),
        };
        ensure(effects == expected)?;
        self.checks.fresh(self.next == 2)?;
        consent::check()?;
        ensure(Instant::now() < deadline)?;
        self.next += 1;
        println!("{title}");
        Ok(true)
    }
    fn progress_inner(&mut self, message: &str) -> Result<()> {
        consent::check()?;
        ensure(!self.poisoned)?;
        match message {
            BACKED_UP => {
                ensure(self.next == 1 && !self.backup)?;
                self.backup = true;
                Ok(())
            }
            STOPPED => {
                ensure(self.next == 2 && self.backup && !self.hook_attempted)?;
                self.hook_attempted = true;
                self.checks.fresh(true)?;
                self.checks.stopped_hook()?;
                consent::check()?;
                self.checks.reconciled()?;
                self.hook_ok = true;
                Ok(())
            }
            _ => Err("Progress refused".into()),
        }
    }
}
impl<C: Checks> Prompt for Approved<C> {
    fn confirm(&mut self, title: &str, effects: &[String], deadline: Instant) -> Result<bool> {
        let result = self.confirm_inner(title, effects, deadline);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn progress(&mut self, message: &str) -> Result<()> {
        let result = self.progress_inner(message);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}
fn execute<R: Runner, C: Checks>(runner: &mut R, prompt: &mut Approved<C>) -> Result<()> {
    let result = runner.execute(Operation::Upgrade, prompt)?;
    ensure(result.complete && prompt.next == 3 && prompt.hook_ok && !prompt.poisoned)?;
    consent::check()
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure(args.len() == 1)?;
    if args[0] == "--check-only" {
        consent::install_cancellation()?;
        Local::default().fresh(false)?;
        println!("Read-only artifact bindings verified; live manager/quiescence not checked.");
        return Ok(());
    }
    ensure(args[0] == "--apply-approved-update")?;
    consent::install_cancellation()?;
    Local::default().fresh(false)?;
    // Fail before any maintenance effect if the parent hook is not available.
    ensure(
        fs::symlink_metadata(evidence().join("coordinated-maintenance.py")).is_ok_and(|m| {
            m.is_file() && m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o022 == 0
        }),
    )?;
    ensure(
        fs::symlink_metadata(evidence().join("coordinated-reconciliation.json"))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
    )?;
    // Logging must be prepared before Native can acquire locks or make effects.
    // The outer coordinator launches the driver as a new session leader.
    ensure(unsafe { libc::getsid(0) == libc::getpid() && libc::getpgrp() == libc::getpid() })?;
    let local = Local {
        log: Some(hook::private_log()?),
    };
    let mut prompt = Approved::new(local, review()?);
    let mut native = Native::new(bundle())?;
    execute(&mut native, &mut prompt)?;
    println!("Coordinated maintenance complete; model readiness not verified.");
    Ok(())
}
fn main() {
    std::panic::set_hook(Box::new(|_| eprintln!("{FAILURE}")));
    if run().is_err() {
        eprintln!("{FAILURE}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        fresh: Vec<bool>,
        hooks: usize,
        drift: bool,
        hook_fails: bool,
        result_fails: bool,
        cancel_in_hook: bool,
    }
    impl Checks for Fake {
        fn fresh(&mut self, upgraded: bool) -> Result<()> {
            self.fresh.push(upgraded);
            ensure(!self.drift)
        }
        fn stopped_hook(&mut self) -> Result<()> {
            self.hooks += 1;
            if self.cancel_in_hook {
                consent::request_cancel();
            }
            ensure(!self.hook_fails)
        }
        fn reconciled(&mut self) -> Result<()> {
            ensure(!self.result_fails)
        }
    }
    fn prompt() -> Approved<Fake> {
        Approved::new(Fake::default(), vec!["exact bound review".into()])
    }
    fn confirm(p: &mut Approved<Fake>, n: usize) -> Result<bool> {
        let effects = match n {
            0 => p.review.clone(),
            1 => vec![STOP.into()],
            _ => vec![START.into()],
        };
        p.confirm(TITLES[n], &effects, Instant::now() + Duration::from_secs(1))
    }
    fn stopped(p: &mut Approved<Fake>) {
        confirm(p, 0).unwrap();
        p.progress(BACKED_UP).unwrap();
        confirm(p, 1).unwrap();
    }
    struct FakeRunner;
    impl Runner for FakeRunner {
        fn execute(
            &mut self,
            op: Operation,
            p: &mut dyn Prompt,
        ) -> Result<smith_installer::maintenance::Outcome> {
            assert_eq!(op, Operation::Upgrade);
            for (n, effects) in [
                vec!["exact bound review".into()],
                vec![STOP.into()],
                vec![START.into()],
            ]
            .iter()
            .enumerate()
            {
                p.confirm(TITLES[n], effects, Instant::now() + Duration::from_secs(1))?;
                if n == 0 {
                    p.progress(BACKED_UP)?;
                }
                if n == 1 {
                    p.progress(STOPPED)?;
                }
            }
            Ok(smith_installer::maintenance::Outcome {
                complete: true,
                message: "not printed".into(),
                details: vec![],
            })
        }
    }
    #[test]
    fn consent_cancel_during_hook_inhibits_start() {
        // Consent cancellation is process-global and terminal. Isolate it from
        // every other test; this child executes only a fake maintenance prompt.
        if std::env::var_os("SMITH_QA_CANCEL_FIXTURE").is_some() {
            consent::install_cancellation().unwrap();
            let mut p = prompt();
            stopped(&mut p);
            p.checks.cancel_in_hook = true;
            assert!(p.progress(STOPPED).is_err());
            assert!(consent::cancelled());
            assert!(confirm(&mut p, 2).is_err());
            assert_eq!(p.next, 2);
            assert!(!p.hook_ok);
            return;
        }
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::consent_cancel_during_hook_inhibits_start",
            ])
            .env("SMITH_QA_CANCEL_FIXTURE", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }
    #[test]
    fn ordered_once_and_fresh_each_effect() {
        let mut p = prompt();
        execute(&mut FakeRunner, &mut p).unwrap();
        assert_eq!(p.checks.hooks, 1);
        assert_eq!(p.checks.fresh, vec![false, false, true, true]);
        assert!(confirm(&mut p, 2).is_err());
    }
    #[test]
    fn unexpected_titles_and_order_refused() {
        for title in [
            "InstallUnit",
            "Enable",
            "Start Smith service",
            "Uninstall Smith artifacts",
            "Start",
            "Stop existing Smith service",
        ] {
            let mut p = prompt();
            assert!(p
                .confirm(title, &[], Instant::now() + Duration::from_secs(1))
                .is_err());
            assert!(confirm(&mut p, 0).is_err());
        }
    }
    #[test]
    fn every_effect_must_match() {
        for (n, title) in TITLES.iter().enumerate() {
            let mut p = prompt();
            if n > 0 {
                confirm(&mut p, 0).unwrap();
                p.progress(BACKED_UP).unwrap();
                if n == 2 {
                    confirm(&mut p, 1).unwrap();
                    p.progress(STOPPED).unwrap();
                }
            }
            assert!(p
                .confirm(
                    title,
                    &["unexpected effect".into()],
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err());
        }
    }
    #[test]
    fn expired_and_drift_refused() {
        let mut p = prompt();
        let review = p.review.clone();
        assert!(p.confirm(TITLES[0], &review, Instant::now()).is_err());
        let mut p = prompt();
        p.checks.drift = true;
        assert!(confirm(&mut p, 0).is_err());
        assert_eq!(p.next, 0);
    }
    #[test]
    fn start_requires_successful_hook_and_result() {
        let mut p = prompt();
        stopped(&mut p);
        assert!(confirm(&mut p, 2).is_err());
        for hook_fails in [true, false] {
            let mut p = prompt();
            stopped(&mut p);
            p.checks.hook_fails = hook_fails;
            p.checks.result_fails = !hook_fails;
            assert!(p.progress(STOPPED).is_err());
            assert!(confirm(&mut p, 2).is_err());
            assert_eq!(p.checks.hooks, 1);
        }
    }
    #[test]
    fn hook_only_exact_progress_once() {
        let mut p = prompt();
        stopped(&mut p);
        assert!(p.progress("Artifacts upgraded").is_err());
        assert_eq!(p.checks.hooks, 0);
        let mut p = prompt();
        stopped(&mut p);
        p.progress(STOPPED).unwrap();
        assert!(p.progress(STOPPED).is_err());
        assert_eq!(p.checks.hooks, 1);
        let mut p = prompt();
        assert!(p.progress(STOPPED).is_err());
        assert_eq!(p.checks.hooks, 0);
    }
    #[test]
    fn reconciliation_identity_is_exact() {
        let good = serde_json::json!({"status":"reconciled", "request_id":REQUEST, "candidate_sha256":NEW});
        reconciliation(&good).unwrap();
        for key in ["status", "request_id", "candidate_sha256"] {
            let mut bad = good.clone();
            bad[key] = "foreign".into();
            assert!(reconciliation(&bad).is_err());
        }
        assert!(reconciliation(&serde_json::json!({})).is_err());
    }
    #[test]
    fn drift_is_checked_again_before_stop_hook_and_start() {
        for stage in 1..=3 {
            let mut p = prompt();
            confirm(&mut p, 0).unwrap();
            p.progress(BACKED_UP).unwrap();
            if stage > 1 {
                confirm(&mut p, 1).unwrap();
            }
            if stage > 2 {
                p.progress(STOPPED).unwrap();
            }
            p.checks.drift = true;
            if stage == 2 {
                assert!(p.progress(STOPPED).is_err());
                assert_eq!(p.checks.hooks, 0);
            } else {
                assert!(confirm(&mut p, if stage == 1 { 1 } else { 2 }).is_err());
            }
        }
    }
    #[test]
    fn incomplete_native_outcome_is_failure() {
        struct Incomplete;
        impl Runner for Incomplete {
            fn execute(
                &mut self,
                _: Operation,
                _: &mut dyn Prompt,
            ) -> Result<smith_installer::maintenance::Outcome> {
                Ok(smith_installer::maintenance::Outcome {
                    complete: false,
                    message: "private error not emitted".into(),
                    details: vec![],
                })
            }
        }
        assert!(execute(&mut Incomplete, &mut prompt()).is_err());
    }
    #[test]
    fn failed_hook_cannot_be_invoked_again() {
        let mut p = prompt();
        stopped(&mut p);
        p.checks.hook_fails = true;
        assert!(p.progress(STOPPED).is_err());
        p.checks.hook_fails = false;
        assert!(p.progress(STOPPED).is_err());
        assert_eq!(p.checks.hooks, 1);
    }
    #[test]
    fn changed_result_before_start_refused() {
        let mut p = prompt();
        stopped(&mut p);
        p.progress(STOPPED).unwrap();
        p.checks.result_fails = true;
        assert!(confirm(&mut p, 2).is_err());
    }
}
