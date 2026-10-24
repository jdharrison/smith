use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::process::{Command, Stdio};
use std::time::Duration;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "smith-retained-source-{}",
            crate::app::new_id().unwrap()
        ));
        mkdir(&path).unwrap();
        Self(path)
    }
    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        mkdir(&path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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
fn spec(raw: &[u8], mode: u32) -> SourceFile {
    SourceFile {
        sha256: hash(raw),
        size: raw.len() as u64,
        mode,
    }
}
fn source(root: PathBuf) -> PreparedSource {
    fs::write(root.join("hello.txt"), b"hello\n").unwrap();
    fs::set_permissions(root.join("hello.txt"), fs::Permissions::from_mode(0o600)).unwrap();
    PreparedSource {
        source_id: "b".repeat(32),
        project: project(),
        mode: JobType::Read,
        repositories: vec![PreparedRepository {
            root,
            commit: "c".repeat(40),
            files: BTreeMap::from([("hello.txt".into(), spec(b"hello\n", 0o100644))]),
        }],
    }
}
fn resolve(resolver: &PreparedSources) -> Result<SourceBinding> {
    resolver.prepare(
        &project(),
        JobType::Read,
        Instant::now() + Duration::from_secs(10),
        &AtomicBool::new(false),
    )
}
#[test]
fn actual_snapshot_manifest_and_durable_consumption() {
    let f = Fixture::new();
    let registry = f.dir("registry");
    let src = source(f.dir("input"));
    let resolver = PreparedSources::new(registry.clone(), vec![src.clone()]).unwrap();
    let binding = resolve(&resolver).unwrap();
    let published = registry.join(&binding.source_id);
    let raw = fs::read(published.join("manifest.json")).unwrap();
    assert_eq!(hash(&raw), binding.manifest_sha256);
    let manifest: Manifest = serde_json::from_slice(&raw).unwrap();
    assert_eq!(manifest.v, 1);
    assert_eq!(manifest.source_id, src.source_id);
    assert_eq!(
        manifest.project_sha256,
        hash(&serde_json::to_vec(&project()).unwrap())
    );
    assert_eq!(manifest.repositories[0].url, project().repositories[0].url);
    assert_eq!(manifest.repositories[0].reference.as_deref(), Some("main"));
    assert_eq!(manifest.repositories[0].commit, "c".repeat(40));
    assert_eq!(manifest.repositories[0].path, "r0");
    assert!(manifest.repositories[0].read_only);
    assert_eq!(manifest.expanded_bytes, 6);
    assert_eq!(manifest.files["r0/hello.txt"], spec(b"hello\n", 0o100644));
    assert_eq!(
        fs::read(published.join("snapshot/r0/hello.txt")).unwrap(),
        b"hello\n"
    );
    assert_eq!(
        fs::metadata(published.join("manifest.json"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
    assert!(matches!(resolve(&resolver), Err(SourceError::Consumed)));
    let reopened = PreparedSources::new(registry, vec![src]).unwrap();
    assert!(matches!(resolve(&reopened), Err(SourceError::Consumed)));
}
#[test]
fn rejects_tampering_and_never_retries_failed_input() {
    let f = Fixture::new();
    let registry = f.dir("registry");
    let src = source(f.dir("input"));
    let resolver = PreparedSources::new(registry.clone(), vec![src.clone()]).unwrap();
    fs::write(src.repositories[0].root.join("hello.txt"), b"wrong\n").unwrap();
    assert!(matches!(resolve(&resolver), Err(SourceError::Invalid)));
    assert!(!registry.join(&src.source_id).exists());
    fs::write(src.repositories[0].root.join("hello.txt"), b"hello\n").unwrap();
    assert!(matches!(resolve(&resolver), Err(SourceError::Consumed)));
}
#[test]
fn rejects_symlinks_special_files_hardlinks_and_extra_config() {
    for kind in ["symlink", "fifo", "hardlink", "config", "unlisted"] {
        let f = Fixture::new();
        let registry = f.dir("registry");
        let src = source(f.dir("input"));
        let root = &src.repositories[0].root;
        match kind {
            "symlink" => {
                fs::remove_file(root.join("hello.txt")).unwrap();
                symlink("missing", root.join("hello.txt")).unwrap();
            }
            "fifo" => {
                fs::remove_file(root.join("hello.txt")).unwrap();
                use std::os::unix::ffi::OsStrExt;
                let path =
                    std::ffi::CString::new(root.join("hello.txt").as_os_str().as_bytes()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            }
            "hardlink" => {
                fs::hard_link(root.join("hello.txt"), f.0.join("alias")).unwrap();
            }
            "config" => {
                fs::write(root.join("opencode.json"), b"{}").unwrap();
            }
            _ => {
                fs::write(root.join("extra"), b"x").unwrap();
            }
        }
        let resolver = PreparedSources::new(registry, vec![src]).unwrap();
        assert!(
            matches!(resolve(&resolver), Err(SourceError::Invalid)),
            "{kind}"
        );
    }
}
#[test]
fn validates_urls_pins_capabilities_and_expansion_before_copy() {
    let f = Fixture::new();
    let registry = f.dir("registry");
    let src = source(f.dir("input"));
    for url in [
        "file:///tmp/source",
        "https://user:pass@github.com/repo",
        "https://127.0.0.1/repo",
        "https://github.com/a?token=x",
    ] {
        let mut bad = src.clone();
        bad.project.repositories[0].url = url.into();
        assert!(matches!(
            PreparedSources::new(registry.clone(), vec![bad]),
            Err(SourceError::Invalid)
        ));
    }
    for pin in ["main", "deadbeef", &"0".repeat(40), &"A".repeat(40)] {
        let mut bad = src.clone();
        bad.repositories[0].commit = pin.into();
        assert!(matches!(
            PreparedSources::new(registry.clone(), vec![bad]),
            Err(SourceError::Invalid)
        ));
    }
    let mut bad = src.clone();
    bad.mode = JobType::Write;
    bad.project.repositories[0].read_only = true;
    assert!(matches!(
        PreparedSources::new(registry.clone(), vec![bad]),
        Err(SourceError::Invalid)
    ));
    let mut bad = src.clone();
    bad.repositories[0].files.get_mut("hello.txt").unwrap().size = MAX_EXPANDED_BYTES + 1;
    assert!(matches!(
        PreparedSources::new(registry.clone(), vec![bad]),
        Err(SourceError::Invalid)
    ));
    let mut bad = src.clone();
    bad.repositories = vec![bad.repositories[0].clone(); 11];
    bad.project.repositories = vec![bad.project.repositories[0].clone(); 11];
    assert!(matches!(
        PreparedSources::new(registry.clone(), vec![bad]),
        Err(SourceError::Invalid)
    ));
    for path in [
        "../escape",
        "a//b",
        ".git/config",
        "AGENTS.md",
        ".env.production",
        "x/private.pem",
        "x\\y",
    ] {
        let mut bad = src.clone();
        bad.repositories[0].files = BTreeMap::from([(path.into(), spec(b"", 0o100644))]);
        assert!(
            matches!(
                PreparedSources::new(registry.clone(), vec![bad]),
                Err(SourceError::Invalid)
            ),
            "{path}"
        );
    }
}
#[test]
fn exact_frozen_selection_cancellation_and_nonreplacement() {
    let f = Fixture::new();
    let registry = f.dir("registry");
    let src = source(f.dir("input"));
    let resolver = PreparedSources::new(registry.clone(), vec![src.clone()]).unwrap();
    let mut changed = project();
    changed.context.push('x');
    assert!(resolver
        .resolve(
            &changed,
            JobType::Read,
            Instant::now() + Duration::from_secs(1),
            &AtomicBool::new(false)
        )
        .is_none());
    assert!(resolver
        .resolve(
            &project(),
            JobType::Write,
            Instant::now() + Duration::from_secs(1),
            &AtomicBool::new(false)
        )
        .is_none());
    assert!(matches!(
        resolver.prepare(
            &project(),
            JobType::Read,
            Instant::now() + Duration::from_secs(1),
            &AtomicBool::new(true)
        ),
        Err(SourceError::Interrupted)
    ));
    assert!(matches!(resolve(&resolver), Err(SourceError::Consumed)));
    let mut second = src;
    second.source_id = "d".repeat(32);
    mkdir(&registry.join(&second.source_id)).unwrap();
    let resolver = PreparedSources::new(registry.clone(), vec![second.clone()]).unwrap();
    assert!(matches!(resolve(&resolver), Err(SourceError::Unavailable)));
    assert_eq!(
        fs::read_dir(registry.join(&second.source_id))
            .unwrap()
            .count(),
        0
    );
}

// These Git invocations are synthetic test fixtures only. No production code
// invokes host Git; no user checkout/config, network, filters or hooks are used.
fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> Vec<u8> {
    let mut c = Command::new("/usr/bin/git");
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_AUTHOR_NAME", "Synthetic")
        .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
        .env("GIT_COMMITTER_NAME", "Synthetic")
        .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
        .current_dir(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "synthetic Git fixture failed");
    output.stdout
}
#[test]
fn real_git_pack_import_exact_pin_checkout_and_registry() {
    let f = Fixture::new();
    let origin = f.dir("synthetic");
    git(
        &origin,
        &["init", "--template=", "--initial-branch=main"],
        None,
    );
    fs::write(origin.join("hello.txt"), b"hello\n").unwrap();
    fs::write(origin.join("tool.sh"), b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(origin.join("tool.sh"), fs::Permissions::from_mode(0o700)).unwrap();
    git(&origin, &["add", "--", "hello.txt", "tool.sh"], None);
    git(&origin, &["commit", "-m", "synthetic fixture only"], None);
    let pin = String::from_utf8(git(&origin, &["rev-parse", "HEAD^{commit}"], None))
        .unwrap()
        .trim()
        .to_owned();
    let pack = git(
        &origin,
        &["pack-objects", "--stdout", "--revs"],
        Some(format!("{pin}\n").as_bytes()),
    );
    assert!(pack.starts_with(b"PACK"));
    let isolated = f.dir("isolated");
    git(&isolated, &["init", "--template="], None);
    git(
        &isolated,
        &["index-pack", "--stdin", "--strict"],
        Some(&pack),
    );
    git(&isolated, &["checkout", "--detach", &pin], None);
    assert_eq!(
        String::from_utf8(git(&isolated, &["rev-parse", "HEAD^{commit}"], None))
            .unwrap()
            .trim(),
        pin
    );
    // Simulates the isolated extractor's data-only export; Git metadata is never
    // registered or interpreted by production source preparation.
    let root = f.dir("prepared");
    for (name, mode) in [("hello.txt", 0o600), ("tool.sh", 0o700)] {
        fs::copy(isolated.join(name), root.join(name)).unwrap();
        // Explicit modes: the exporter never inherits checkout umask bits.
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(mode)).unwrap();
    }
    let mut src = source(root);
    src.repositories[0].commit = pin.clone();
    src.repositories[0]
        .files
        .insert("tool.sh".into(), spec(b"#!/bin/sh\nexit 0\n", 0o100755));
    let registry = f.dir("registry");
    let resolver = PreparedSources::new(registry.clone(), vec![src]).unwrap();
    let binding = resolve(&resolver).unwrap();
    let published = registry.join(binding.source_id);
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(published.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.repositories[0].commit, pin);
    assert_eq!(manifest.files["r0/tool.sh"].mode, 0o100755);
    assert_eq!(
        fs::metadata(published.join("snapshot/r0/tool.sh"))
            .unwrap()
            .mode()
            & 0o777,
        0o700
    );
    assert!(!published.join("snapshot/r0/.git").exists());
}
