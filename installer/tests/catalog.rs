//! Tiny synthetic model fixtures only; no real weights or network calls.
use sha2::{Digest, Sha256};
use smith_installer::{
    catalog,
    model::{DiscardProgress, ProgressOutput},
    Result,
};
use smith_service::{
    catalog::ModelTier,
    runner::{self, CatalogEntry},
    Roots,
};
use std::fs;
use std::io::{Cursor, Read};
use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: std::path::PathBuf,
    roots: Roots,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "smith-cat-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        for p in [&roots.data, &roots.config, &roots.runtime] {
            fs::DirBuilder::new().mode(0o700).create(p).unwrap();
        }
        Self { base, roots }
    }
    fn install(&self, spec: &CatalogEntry, bytes: &[u8]) -> Result<()> {
        let owned = bytes.to_vec();
        catalog::install_with(&self.roots, spec, &mut DiscardProgress, deadline(), |_| {
            Ok(Box::new(Cursor::new(owned.clone())))
        })
    }
    fn models(&self) -> std::path::PathBuf {
        runner::models_dir(&self.roots)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}
fn spec(bytes: &[u8]) -> CatalogEntry {
    CatalogEntry {
        name: "fixture:q4",
        display_name: "Fixture",
        family: "Fixture",
        parameter_label: "test",
        tier: ModelTier::Compact,
        sha256: Box::leak(format!("{:x}", Sha256::digest(bytes)).into_boxed_str()),
        size: bytes.len() as u64,
        license: "fixture",
        source: "https://huggingface.co/fixture",
        url: "https://huggingface.co/fixture/resolve/commit/model.gguf",
    }
}
#[test]
fn model_stream_cap_accepts_qwen_size_but_rejects_over_16_gib_without_weights() {
    let f = Fixture::new();
    let mut entry = spec(&[1]);
    entry.size = 9001752960;
    let error = f.install(&entry, &[]).unwrap_err();
    assert!(
        !error.contains("Invalid model publication identity or size"),
        "{error}"
    );
    entry.size = 16 * 1024 * 1024 * 1024 + 1;
    assert!(f
        .install(&entry, &[])
        .unwrap_err()
        .contains("Invalid model publication identity or size"));
}
#[test]
fn canonical_marker_comes_last_and_repeated_install_uses_verified_file() {
    let f = Fixture::new();
    let bytes = vec![7; 100001];
    let entry = spec(&bytes);
    f.install(&entry, &bytes).unwrap();
    let records = runner::list_models_with(&f.models(), &[entry]).unwrap();
    assert_eq!(records, vec![entry.record()]);
    assert_eq!(
        fs::metadata(f.models().join(format!("{}.gguf", entry.sha256)))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    catalog::install_with(&f.roots, &entry, &mut DiscardProgress, deadline(), |_| {
        panic!("must not download installed bytes")
    })
    .unwrap();
}
#[test]
fn failed_transfer_never_publishes_a_completion_marker() {
    for bytes in [vec![1; 100], vec![9; 100001], vec![1; 100002]] {
        let f = Fixture::new();
        let entry = spec(&vec![1; 100001]);
        assert!(f.install(&entry, &bytes).is_err());
        assert!(!f.models().join(format!("{}.json", entry.sha256)).exists());
        assert!(!f.models().join(format!("{}.gguf", entry.sha256)).exists());
        assert!(fs::read_dir(f.models()).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".smith-model-stage-")));
    }
}
#[test]
fn verified_orphan_can_be_adopted_without_a_second_download() {
    let f = Fixture::new();
    let bytes = b"fixture bytes";
    let entry = spec(bytes);
    fs::create_dir_all(f.models()).unwrap();
    fs::set_permissions(
        f.models().parent().unwrap(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::set_permissions(f.models(), fs::Permissions::from_mode(0o700)).unwrap();
    let file = f.models().join(format!("{}.gguf", entry.sha256));
    fs::write(&file, bytes).unwrap();
    fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    catalog::install_with(&f.roots, &entry, &mut DiscardProgress, deadline(), |_| {
        panic!("must adopt verified orphan")
    })
    .unwrap();
    assert_eq!(
        runner::list_models_with(&f.models(), &[entry])
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn conflicts_links_and_unsafe_modes_are_preserved() {
    for kind in 0..3 {
        let f = Fixture::new();
        let bytes = b"fixture bytes";
        let entry = spec(bytes);
        f.install(&entry, bytes).unwrap();
        let path = f.models().join(format!("{}.gguf", entry.sha256));
        match kind {
            0 => fs::write(&path, b"changed bytes").unwrap(),
            1 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            _ => {
                fs::rename(&path, f.models().join("retained")).unwrap();
                symlink("retained", &path).unwrap();
            }
        }
        assert!(f.install(&entry, bytes).is_err());
        assert!(path.symlink_metadata().is_ok());
    }
}
#[test]
fn failed_progress_sink_blocks_publication() {
    struct Failed;
    impl ProgressOutput for Failed {
        fn write_until(&mut self, _: &[u8], _: Instant, _: fn() -> bool) -> Result<()> {
            Err("sink closed".into())
        }
    }
    let f = Fixture::new();
    let bytes = vec![4; 100001];
    let entry = spec(&bytes);
    assert!(
        catalog::install_with(&f.roots, &entry, &mut Failed, deadline(), |_| Ok(Box::new(
            Cursor::new(bytes.clone())
        )))
        .is_err()
    );
    assert!(!f.models().join(format!("{}.json", entry.sha256)).exists());
}
#[test]
fn expired_operation_has_no_destination_effects() {
    let f = Fixture::new();
    let entry = spec(b"fixture");
    assert!(catalog::install_with(
        &f.roots,
        &entry,
        &mut DiscardProgress,
        Instant::now() - Duration::from_secs(1),
        |_| panic!()
    )
    .is_err());
    assert!(!f.models().exists());
    assert!(!f.roots.data.join(".smith-next-install.lock").exists());
}
#[test]
fn transport_error_preserves_stage_and_no_marker() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken transport"))
        }
    }
    let f = Fixture::new();
    let entry = spec(b"fixture");
    assert!(
        catalog::install_with(&f.roots, &entry, &mut DiscardProgress, deadline(), |_| Ok(
            Box::new(Broken)
        ))
        .is_err()
    );
    assert!(!f.models().join(format!("{}.json", entry.sha256)).exists());
}
