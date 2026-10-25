use super::*;
use crate::retained_jobs::retained_sources::Manifest;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "smith-retained-acquire-{}",
            crate::app::new_id().unwrap()
        ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Synthetic")
        .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
        .env("GIT_COMMITTER_NAME", "Synthetic")
        .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
        .current_dir(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "synthetic Git fixture failed");
    output.stdout
}
struct Entry {
    path: String,
    mode: u32,
    bytes: Vec<u8>,
}
/// Synthetic origin covering included, excluded, symlink, submodule and nested entries.
fn origin(f: &Fixture) -> (String, Vec<u8>, Vec<Entry>) {
    let origin = f.dir("origin");
    git(&origin, &["init", "--template=", "--initial-branch=main"]);
    let write = |path: &str, bytes: &[u8], mode: u32| {
        let target = origin.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, bytes).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
    };
    write("hello.txt", b"hello\n", 0o644);
    write("src/lib/deep.rs", b"fn main() {}\n", 0o644);
    write("tool.sh", b"#!/bin/sh\nexit 0\n", 0o755);
    write("AGENTS.md", b"repository instructions\n", 0o644);
    write(".env.local", b"TOKEN=x\n", 0o644);
    write("has space.txt", b"x\n", 0o644);
    write("fixture.txt", b"-----BEGIN PRIVATE KEY-----\n", 0o644);
    // CRLF attribute must not alter exported object bytes.
    write(".gitattributes", b"*.txt text eol=crlf\n", 0o644);
    symlink("hello.txt", origin.join("link")).unwrap();
    git(&origin, &["add", "-A"]);
    git(
        &origin,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},vendor/sub", "1".repeat(40)),
        ],
    );
    git(&origin, &["commit", "-q", "-m", "synthetic"]);
    let pin = String::from_utf8(git(&origin, &["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    let commit = git(&origin, &["cat-file", "commit", "HEAD"]);
    let list = git(&origin, &["ls-tree", "-r", "-z", "--full-tree", "HEAD"]);
    let mut entries = Vec::new();
    for record in list.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let record = std::str::from_utf8(record).unwrap();
        let (meta, path) = record.split_once('\t').unwrap();
        let fields: Vec<_> = meta.split(' ').collect();
        let mode = u32::from_str_radix(fields[0], 8).unwrap();
        let bytes = if mode == GITLINK {
            fields[2].as_bytes().to_vec()
        } else {
            git(&origin, &["cat-file", "blob", fields[2]])
        };
        entries.push(Entry {
            path: path.into(),
            mode,
            bytes,
        });
    }
    (pin, commit, entries)
}
fn receive(
    root: PathBuf,
    pin: &str,
    commit: &[u8],
    entries: &[Entry],
) -> Result<PreparedRepository> {
    let mut receiver = Receiver::new(root, pin.into());
    let mut budget = Budget::default();
    receiver.commit(commit)?;
    for e in entries {
        // Exercise chunk reassembly with deliberately tiny chunks.
        let chunks: Vec<&[u8]> = e.bytes.chunks(3).collect();
        if chunks.is_empty() {
            receiver.entry(e.path.clone(), e.mode, Vec::new(), true, &mut budget)?;
        }
        for (i, chunk) in chunks.iter().enumerate() {
            let last = i + 1 == chunks.len();
            receiver.entry(e.path.clone(), e.mode, chunk.to_vec(), last, &mut budget)?;
        }
    }
    receiver.end(entries.len())
}

#[test]
fn exact_commit_tree_with_policy_exclusions() {
    let f = Fixture::new();
    let (pin, commit, entries) = origin(&f);
    let root = f.dir("root");
    let prepared = receive(root.clone(), &pin, &commit, &entries).unwrap();
    assert_eq!(prepared.commit, pin);
    assert_eq!(
        prepared.files.keys().collect::<Vec<_>>(),
        ["hello.txt", "src/lib/deep.rs", "tool.sh"]
    );
    assert_eq!(prepared.files["tool.sh"].mode, EXECUTABLE);
    // Object bytes, not CRLF-converted working-tree bytes.
    assert_eq!(fs::read(root.join("hello.txt")).unwrap(), b"hello\n");
    assert_eq!(
        fs::metadata(root.join("tool.sh")).unwrap().mode() & 0o777,
        0o700
    );
    for absent in [
        "AGENTS.md",
        ".env.local",
        "has space.txt",
        "fixture.txt",
        ".gitattributes",
        "link",
        "vendor",
    ] {
        assert!(fs::symlink_metadata(root.join(absent)).is_err(), "{absent}");
    }
}

#[test]
fn any_tampering_fails_tree_verification() {
    let f = Fixture::new();
    let (pin, commit, entries) = origin(&f);
    let attempt = |name: &str, change: &dyn Fn(&mut Vec<Entry>)| {
        let mut changed: Vec<Entry> = entries
            .iter()
            .map(|e| Entry {
                path: e.path.clone(),
                mode: e.mode,
                bytes: e.bytes.clone(),
            })
            .collect();
        change(&mut changed);
        receive(f.dir(name), &pin, &commit, &changed)
    };
    assert!(attempt("ok", &|_| ()).is_ok());
    let changed = attempt("content", &|e| e[0].bytes.push(b'x'));
    assert_eq!(changed.err(), Some(INVALID));
    let dropped = attempt("dropped", &|e| {
        e.remove(1);
    });
    assert_eq!(dropped.err(), Some(INVALID));
    let added = attempt("added", &|e| {
        e.push(Entry {
            path: "extra.txt".into(),
            mode: REGULAR,
            bytes: b"x".to_vec(),
        })
    });
    assert_eq!(added.err(), Some(INVALID));
    let mode = attempt("mode", &|e| {
        let tool = e.iter_mut().find(|e| e.path == "tool.sh").unwrap();
        tool.mode = REGULAR;
    });
    assert_eq!(mode.err(), Some(INVALID));
    let gitlink = attempt("gitlink", &|e| {
        let sub = e.iter_mut().find(|e| e.mode == GITLINK).unwrap();
        sub.bytes = b"2".repeat(40);
    });
    assert_eq!(gitlink.err(), Some(INVALID));
    let malformed = attempt("malformed", &|e| {
        let sub = e.iter_mut().find(|e| e.mode == GITLINK).unwrap();
        sub.bytes = b"not-hex".to_vec();
    });
    assert_eq!(malformed.err(), Some(INVALID));
}

#[test]
fn protocol_order_commit_identity_and_counts_are_enforced() {
    let f = Fixture::new();
    let (pin, commit, entries) = origin(&f);
    let mut budget = Budget::default();
    let mut before = Receiver::new(f.dir("before"), pin.clone());
    assert_eq!(
        before
            .entry("a".into(), REGULAR, vec![], true, &mut budget)
            .err(),
        Some(INVALID)
    );
    let mut wrong = Receiver::new(f.dir("wrong"), "0".repeat(40));
    assert_eq!(wrong.commit(&commit).err(), Some(INVALID));
    let mut twice = Receiver::new(f.dir("twice"), pin.clone());
    twice.commit(&commit).unwrap();
    assert_eq!(twice.commit(&commit).err(), Some(INVALID));
    let mut interleaved = Receiver::new(f.dir("interleaved"), pin.clone());
    interleaved.commit(&commit).unwrap();
    interleaved
        .entry("a".into(), REGULAR, b"x".to_vec(), false, &mut budget)
        .unwrap();
    assert_eq!(
        interleaved
            .entry("b".into(), REGULAR, b"y".to_vec(), true, &mut budget)
            .err(),
        Some(INVALID)
    );
    let mut unfinished = Receiver::new(f.dir("unfinished"), pin.clone());
    unfinished.commit(&commit).unwrap();
    unfinished
        .entry("a".into(), REGULAR, b"x".to_vec(), false, &mut budget)
        .unwrap();
    assert!(unfinished.end(0).is_err());
    let counted = receive(f.dir("counted"), &pin, &commit, &entries).unwrap();
    assert!(!counted.files.is_empty());
    let mut miscounted = Receiver::new(f.dir("miscounted"), pin.clone());
    miscounted.commit(&commit).unwrap();
    for e in &entries {
        miscounted
            .entry(
                e.path.clone(),
                e.mode,
                e.bytes.clone(),
                true,
                &mut Budget::default(),
            )
            .unwrap();
    }
    assert!(miscounted.end(entries.len() + 1).is_err());
}

#[test]
fn path_length_is_capped_and_charged_to_the_received_budget() {
    let f = Fixture::new();
    let (pin, commit, _) = origin(&f);
    let mut receiver = Receiver::new(f.dir("paths"), pin);
    receiver.commit(&commit).unwrap();
    let mut budget = Budget::default();
    // Exceeds the cap even though the content is empty and the frame would fit.
    assert_eq!(
        receiver
            .entry(
                "a".repeat(git_tree::MAX_PATH + 1),
                REGULAR,
                vec![],
                true,
                &mut budget
            )
            .err(),
        Some(INVALID)
    );
    let path = format!("{}/{}", "d".repeat(200), "f".repeat(200));
    receiver
        .entry(path.clone(), REGULAR, b"x".to_vec(), false, &mut budget)
        .unwrap();
    receiver
        .entry(path.clone(), REGULAR, b"y".to_vec(), true, &mut budget)
        .unwrap();
    // Charged once per entry, not once per chunk.
    assert_eq!(budget.received, path.len() as u64 + 2);
    let mut full = Budget {
        received: dev_worker_local::EXPORT_BYTES - 2,
        ..Budget::default()
    };
    assert_eq!(
        receiver
            .entry("abc".into(), REGULAR, vec![], true, &mut full)
            .err(),
        Some(INVALID)
    );
}

#[test]
fn received_and_included_budgets_are_bounded() {
    let f = Fixture::new();
    let (pin, commit, _) = origin(&f);
    let mut receiver = Receiver::new(f.dir("budget"), pin);
    receiver.commit(&commit).unwrap();
    let mut budget = Budget {
        received: dev_worker_local::EXPORT_BYTES,
        ..Budget::default()
    };
    assert_eq!(
        receiver
            .entry("a".into(), REGULAR, b"x".to_vec(), true, &mut budget)
            .err(),
        Some(INVALID)
    );
    let mut budget = Budget {
        files: MAX_FILES,
        ..Budget::default()
    };
    assert_eq!(
        receiver
            .entry("b".into(), REGULAR, b"x".to_vec(), true, &mut budget)
            .err(),
        Some(INVALID)
    );
}

/// Replays the synthetic origin through the real Receiver, as the container would.
struct ReplayExporter {
    pin: String,
    commit: Vec<u8>,
    entries: Vec<(String, u32, Vec<u8>)>,
    calls: Mutex<Vec<(String, usize)>>,
    fail: bool,
}
impl Exporter for ReplayExporter {
    fn export(
        &self,
        request_id: &str,
        repositories: &[crate::retained_jobs::Repository],
        roots: &[PathBuf],
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<Vec<PreparedRepository>> {
        self.calls
            .lock()
            .unwrap()
            .push((request_id.into(), repositories.len()));
        if self.fail {
            return Err(SourceError::Unavailable);
        }
        let entries: Vec<Entry> = self
            .entries
            .iter()
            .map(|(path, mode, bytes)| Entry {
                path: path.clone(),
                mode: *mode,
                bytes: bytes.clone(),
            })
            .collect();
        roots
            .iter()
            .map(|root| receive(root.clone(), &self.pin, &self.commit, &entries))
            .collect()
    }
}
fn project() -> FrozenProject {
    FrozenProject {
        project_id: "a".repeat(32),
        project_key: "fixture".into(),
        context: "exact context".into(),
        repositories: vec![crate::retained_jobs::Repository {
            url: "https://github.com/smith-fixtures/source.git".into(),
            reference: Some("main".into()),
            read_only: false,
        }],
    }
}
fn exporter(f: &Fixture, fail: bool) -> Arc<ReplayExporter> {
    let (pin, commit, entries) = origin(f);
    Arc::new(ReplayExporter {
        pin,
        commit,
        entries: entries
            .into_iter()
            .map(|e| (e.path, e.mode, e.bytes))
            .collect(),
        calls: Mutex::new(Vec::new()),
        fail,
    })
}

#[test]
fn acquiring_resolver_publishes_registry_and_removes_staging() {
    let f = Fixture::new();
    let work = f.dir("work");
    let registry = f.dir("registry");
    let replay = exporter(&f, false);
    let sources = AcquiringSources::new(work.clone(), registry.clone(), replay.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let binding = sources
        .resolve(
            &project(),
            JobType::Write,
            deadline,
            &AtomicBool::new(false),
        )
        .unwrap();
    let calls = replay.calls.lock().unwrap().clone();
    assert_eq!(calls, [(binding.source_id.clone(), 1)]);
    let published = registry.join(&binding.source_id);
    let raw = fs::read(published.join("manifest.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&raw)),
        binding.manifest_sha256
    );
    let manifest: Manifest = serde_json::from_slice(&raw).unwrap();
    assert_eq!(manifest.repositories[0].commit, replay.pin);
    assert!(!manifest.repositories[0].read_only);
    assert_eq!(
        manifest.files.keys().collect::<Vec<_>>(),
        ["r0/hello.txt", "r0/src/lib/deep.rs", "r0/tool.sh"]
    );
    assert_eq!(
        fs::read(published.join("snapshot/r0/src/lib/deep.rs")).unwrap(),
        b"fn main() {}\n"
    );
    assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
    // Each resolve is a fresh acquisition with a fresh identity.
    let second = sources
        .resolve(
            &project(),
            JobType::Write,
            deadline,
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_ne!(second.source_id, binding.source_id);
}

#[test]
fn failed_export_publishes_nothing_and_cleans_staging() {
    let f = Fixture::new();
    let work = f.dir("work");
    let registry = f.dir("registry");
    let replay = exporter(&f, true);
    let sources = AcquiringSources::new(work.clone(), registry.clone(), replay.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    assert!(sources
        .resolve(&project(), JobType::Read, deadline, &AtomicBool::new(false))
        .is_none());
    assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&registry).unwrap().count(), 0);
    let mut empty = project();
    empty.repositories.clear();
    assert!(sources
        .resolve(&empty, JobType::Read, deadline, &AtomicBool::new(false))
        .is_none());
    assert_eq!(replay.calls.lock().unwrap().len(), 1);
    // Overlapping roots are refused.
    let nested = work.join("nested");
    DirBuilder::new().mode(0o700).create(&nested).unwrap();
    assert!(AcquiringSources::new(work.clone(), nested, replay).is_err());
}
