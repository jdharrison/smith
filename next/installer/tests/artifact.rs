//! Public-library and real native artifact-mode tests, using private disposable roots only.
//
// R4 TEST MAPPING — all 43 tests in next/dagger/tests/test_install.py.
// I = this integration harness; U = artifact::tests (private cfg(test) seams).
// Names retain the Python suffix without `test_`; none of its safety invariants is dropped.
//
// | # | Python test suffix | Native evidence |
// |---|--------------------|-----------------|
// | 1 | dry_run_and_decline_are_read_only | I, plus construction/EOF |
// | 2 | both_install_complete_and_idempotent | I |
// | 3 | service_then_client | I |
// | 4 | client_then_service | I |
// | 5 | service_only_rejects_selected_client_helper_mismatch | I |
// | 6 | service_only_does_not_require_home | I |
// | 7 | service_only_post_publication_changes_fail_final_verification | U (bytes/mode/delete) |
// | 8 | existing_service_only_changes_at_apply_fail_final_verification | U (bytes/mode/delete) |
// | 9 | service_only_verified_idempotent_install_has_no_publication | I + U |
// | 10 | preserves_old_release_on_client_update | I |
// | 11 | no_unknown_destination_overwrite | I |
// | 12 | different_service_blocks_client_switch | I |
// | 13 | mismatched_helpers_block_all_writes | I |
// | 14 | source_mode_hardlinks_and_special_files | I |
// | 15 | symlink_ancestors_and_destinations | I |
// | 16 | unsafe_and_missing_roots | I |
// | 17 | private_namespace_required | I |
// | 18 | root_refusal | U (the production admission predicate) |
// | 19 | changed_source_after_confirmation | I, including same-byte inode replacement |
// | 20 | changed_destination_after_confirmation | I |
// | 21 | revalidate_under_lock | U |
// | 22 | bounded_concurrent_lock | U (private short budget; production remains 5s) |
// | 23 | unsafe_lock | I, plus U replaced-held-lock |
// | 24 | current_must_be_managed_and_verified | I |
// | 25 | existing_release_rejects_extra_files_modes_and_manifest_drift | I |
// | 26 | incomplete_bundle_blocks_all_writes | I |
// | 27 | no_clobber_service_publication_race | U |
// | 28 | partial_failure_preserved_reported_and_retry | U |
// | 29 | directory_no_replace_rejects_even_empty_unknown_destination | U |
// | 30 | descriptor_relative_traversal_blocks_symlink_swap | U |
// | 31 | current_no_clobber_race | U |
// | 32 | verified_current_replacement_revalidated_before_switch | U |
// | 33 | late_foreign_regular_current_preserved_by_exchange | U |
// | 34 | late_same_target_new_selector_inode_is_conflict | U |
// | 35 | exchange_never_rolls_back_over_newer_current | U |
// | 36 | exchange_preserves_unknown_replacement_at_displaced_path | U |
// | 37 | exchange_sync_failure_reports_partial_selection_and_retains_old | U |
// | 38 | dependencies_reverified_before_selection | U (release/desktop/wrapper/service) |
// | 39 | dependencies_reverified_after_exchange_before_success | U (all four dependencies) |
// | 40 | staging_failure_retained_and_retry_uses_fresh_stage | U |
// | 41 | large_bundle_with_bounded_file_descriptors | I (600 files, RLIMIT_NOFILE=128) |
// | 42 | simultaneous_cli_installers_have_safe_outcomes | I (two actual native processes) |
// | 43 | cli_help_required_arguments_and_dry_run | I; required-source check now uses explicit --artifacts-only, because no args intentionally means recipient mode |
//
// Additional native evidence: canonical Python manifest bytes/digest (manifest::tests),
// raw normalized paths, source changes during reads, entry/file/depth limits,
// directory snapshots, retained failed file stages, service-only selected-client
// final verification, and default-public-library behavior without any hooks.
// M1: consent-time root mtime changes reject both an in-bundle service source
// and a service-only selected release; the separate-service source is a control.
// UI2/UI4/UI6: opaque read-only review, exact originating-review approval,
// captured source/destination/full-directory facts, and caller-owned capped output.

use smith_installer::artifact::{self, Installer, Outcome};
use smith_installer::manifest::{digest, APP, MANIFEST};
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::time::Duration;

type Result<T> = std::result::Result<T, String>;
#[path = "fixtures/publisher.rs"]
mod fixture;
use fixture::{bounded_output, chmod, write, Fixture};

fn cli(f: &Fixture) -> std::process::Command {
    f.cli(Path::new(env!("CARGO_BIN_EXE_smith-installer")))
}

#[test]
fn dry_run_and_decline_are_read_only() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut item = f.installer(true, true);
    assert_eq!(f.snapshot(), before, "construction mutated roots");
    assert_eq!(
        item.run(true, true, &mut |_| panic!("dry run prompted")),
        Ok(Outcome::DryRun)
    );
    assert_eq!(f.snapshot(), before);
    for answer in ["n", "", "yes", " y "] {
        assert_eq!(
            item.run(false, false, &mut |_| Ok(answer.into())),
            Ok(Outcome::Declined)
        );
        assert_eq!(f.snapshot(), before);
    }
    assert!(item
        .run(false, false, &mut |_| Err("EOF".into()))
        .unwrap_err()
        .contains("EOF"));
    assert_eq!(f.snapshot(), before);
}

#[test]
fn prepared_artifact_reviews_are_read_only_and_describe_exact_facts() {
    let f = Fixture::new();
    let before = f.snapshot();
    let item = f.installer(true, true);
    let prepared = item.prepare_review().unwrap();
    let next = item.prepare_review().unwrap();
    assert_ne!(prepared.review_id(), next.review_id());
    assert!(!prepared.review_id().is_empty());
    for value in [
        f.bundle.to_str().unwrap(),
        f.service.to_str().unwrap(),
        f.binary().to_str().unwrap(),
        prepared.review_id(),
        "20b56ce4694b74c2ee936538df964944e4853b295afa5289197adf258e2af14f",
        "no build, GUI launch, unit registration or service start",
        "no automatic rollback or same-UID filesystem CAS",
    ] {
        assert!(prepared.description().contains(value), "missing {value}");
    }
    assert_eq!(f.snapshot(), before);
    drop((prepared, next));
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join(".smith-next-install.lock").exists());
}

#[test]
fn reviewed_artifacts_reject_wrong_or_stale_review_ids_without_writes() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut item = f.installer(true, true);
    let stale_id = item.prepare_review().unwrap().review_id().to_owned();
    for approval in ["", "y", "wrong-review", &stale_id] {
        let prepared = item.prepare_review().unwrap();
        let mut output = Vec::new();
        let result = item.install_reviewed(prepared, approval, &mut output);
        assert_eq!(
            result,
            Err("Artifact review approval does not match the displayed review".into())
        );
        assert_eq!(f.snapshot(), before);
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("No rollback assumed"));
    }
    assert!(!f.data.join(".smith-next-install.lock").exists());
}

#[test]
fn reviewed_artifacts_are_bound_to_the_originating_installer() {
    let f = Fixture::new();
    let before = f.snapshot();
    let origin = f.installer(true, true);
    let prepared = origin.prepare_review().unwrap();
    let id = prepared.review_id().to_owned();
    let mut other = f.installer(true, true);
    let result = other.install_reviewed(prepared, &id, &mut Vec::new());
    assert_eq!(
        result,
        Err("Artifact review belongs to a different installer".into())
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn reviewed_artifacts_reject_source_drift_before_lock() {
    let f = Fixture::new();
    let mut item = f.installer(false, true);
    let prepared = item.prepare_review().unwrap();
    let id = prepared.review_id().to_owned();
    write(&f.service, b"changed after review", 0o755);
    let before = f.snapshot();
    assert_eq!(
        item.install_reviewed(prepared, &id, &mut Vec::new()),
        Err("Artifacts or destinations changed after confirmation".into())
    );
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join(".smith-next-install.lock").exists());
}

#[test]
fn reviewed_artifacts_reject_destination_drift_before_lock() {
    for bytes in [b"foreign".as_slice(), b"service-v1"] {
        let f = Fixture::new();
        let mut item = f.installer(false, true);
        let prepared = item.prepare_review().unwrap();
        let id = prepared.review_id().to_owned();
        f.private_parents(f.binary().parent().unwrap());
        write(&f.binary(), bytes, 0o755);
        let before = f.snapshot();
        assert!(item
            .install_reviewed(prepared, &id, &mut Vec::new())
            .is_err());
        assert_eq!(f.snapshot(), before);
        assert!(!f.data.join(".smith-next-install.lock").exists());
    }
}

struct CappedMemory {
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
}

impl std::io::Write for CappedMemory {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let retained = bytes.len().min(self.limit.saturating_sub(self.bytes.len()));
        self.bytes.extend_from_slice(&bytes[..retained]);
        self.truncated |= retained != bytes.len();
        // A bounded UI log may explicitly discard excess details; Outcome, not
        // parsing this log, remains the installation result.
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn reviewed_artifacts_publish_with_valid_approval_and_capped_owned_output() {
    for limit in [128, 32 * 1024] {
        let f = Fixture::new();
        let mut item = f.installer(true, true);
        let prepared = item.prepare_review().unwrap();
        let id = prepared.review_id().to_owned();
        let mut output = CappedMemory {
            bytes: Vec::new(),
            limit,
            truncated: false,
        };
        assert_eq!(
            item.install_reviewed(prepared, &id, &mut output),
            Ok(Outcome::Installed)
        );
        assert!(output.bytes.len() <= limit);
        assert_eq!(output.truncated, limit == 128);
        assert_eq!(fs::read(f.binary()).unwrap(), b"service-v1");
        assert_eq!(
            fs::read(f.current().join(MANIFEST)).unwrap(),
            include_bytes!("fixtures/release-manifest-v1.json")
        );
        assert!(f.home.join(".local/bin/smith-next").is_file());
        assert!(!f.data.join("systemd").exists());
        assert!(!f.root.join("unused-config").exists());
        assert!(!f.root.join("unused-runtime").exists());
        let before = f.snapshot();
        let repeated = item.prepare_review().unwrap();
        let id = repeated.review_id().to_owned();
        assert_eq!(
            item.install_reviewed(repeated, &id, &mut Vec::new()),
            Ok(Outcome::Installed)
        );
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn reviewed_artifact_output_failure_before_publication_is_read_only() {
    struct FailedOutput;
    impl std::io::Write for FailedOutput {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "capped output unavailable",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let f = Fixture::new();
    let before = f.snapshot();
    let mut item = f.installer(true, true);
    let prepared = item.prepare_review().unwrap();
    let id = prepared.review_id().to_owned();
    assert!(item
        .install_reviewed(prepared, &id, &mut FailedOutput)
        .is_err());
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join(".smith-next-install.lock").exists());
}

#[test]
fn reviewed_artifacts_keep_full_bundle_directory_facts() {
    let f = Fixture::new();
    let mut item = Installer::with_roots(
        Some(f.bundle.clone()),
        Some(f.bundle.join("smith_backend/smith-service")),
        Some(f.home.clone()),
        f.data.clone(),
    )
    .unwrap();
    let prepared = item.prepare_review().unwrap();
    let id = prepared.review_id().to_owned();
    change_directory_mtime(&f.bundle);
    let before = f.snapshot();
    assert_eq!(
        item.install_reviewed(prepared, &id, &mut Vec::new()),
        Err("Artifacts or destinations changed after confirmation".into())
    );
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join(".smith-next-install.lock").exists());
}

#[test]
fn reviewed_service_only_keeps_full_selected_release_directory_facts_without_home() {
    let f = Fixture::new();
    assert_eq!(f.install(true, false), Ok(Outcome::Installed));
    let current = f.current();
    let selected = current
        .parent()
        .unwrap()
        .join(fs::read_link(&current).unwrap());
    let mut item =
        Installer::with_roots(None, Some(f.service.clone()), None, f.data.clone()).unwrap();
    let prepared = item.prepare_review().unwrap();
    let id = prepared.review_id().to_owned();
    change_directory_mtime(&selected);
    let before = f.snapshot();
    assert_eq!(
        item.install_reviewed(prepared, &id, &mut Vec::new()),
        Err("Artifacts or destinations changed after confirmation".into())
    );
    assert_eq!(f.snapshot(), before);
    assert!(!f.binary().exists());
}

#[test]
fn both_install_complete_and_idempotent() {
    let f = Fixture::new();
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
    let binary = f.binary();
    assert_eq!(fs::read(&binary).unwrap(), b"service-v1");
    assert_eq!(fs::metadata(&binary).unwrap().mode() & 0o7777, 0o755);
    assert_eq!(fs::metadata(&binary).unwrap().nlink(), 1);
    let current = f.current();
    let selected = fs::read_link(&current).unwrap();
    let selected = selected.to_str().unwrap();
    assert!(selected.starts_with("releases/"));
    assert_eq!(selected.len(), 73);
    let manifest = fs::read(current.join(MANIFEST)).unwrap();
    assert!(manifest.ends_with(b"\n"));
    assert_eq!(
        manifest,
        include_bytes!("fixtures/release-manifest-v1.json")
    );
    assert_eq!(&selected[9..], digest(&manifest));
    assert_eq!(
        selected,
        "releases/20b56ce4694b74c2ee936538df964944e4853b295afa5289197adf258e2af14f"
    );
    assert_eq!(
        fs::read(current.join("data/flutter_assets/asset")).unwrap(),
        b"asset"
    );
    let desktop = fs::read_to_string(f.data.join(format!("applications/{APP}.desktop"))).unwrap();
    assert!(desktop.contains("Name=smith\n"));
    assert!(desktop.contains(&format!(
        "Exec={}\n",
        current.join("smith_client").display()
    )));
    assert!(desktop.contains(&format!(
        "Icon={}\n",
        current.join(format!("share/pixmaps/{APP}.png")).display()
    )));
    let shim = f.home.join(".local/bin/smith-next");
    assert!(fs::read_to_string(&shim).unwrap().contains("\"$@\""));
    assert_eq!(fs::metadata(shim).unwrap().mode() & 0o7777, 0o755);
    let before = f.snapshot();
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join("systemd").exists());
    assert!(!f.root.join("unused-config").exists());
    assert!(!f.root.join("unused-runtime").exists());
}

#[test]
fn service_then_client() {
    let f = Fixture::new();
    assert_eq!(f.install(false, true), Ok(Outcome::Installed));
    assert_eq!(f.install(true, false), Ok(Outcome::Installed));
}

#[test]
fn client_then_service() {
    let f = Fixture::new();
    assert_eq!(f.install(true, false), Ok(Outcome::Installed));
    assert!(!f.binary().exists());
    assert_eq!(f.install(false, true), Ok(Outcome::Installed));
}

#[test]
fn service_only_rejects_selected_client_helper_mismatch() {
    let f = Fixture::new();
    assert_eq!(f.install(true, false), Ok(Outcome::Installed));
    write(&f.service, b"different", 0o755);
    let before = f.snapshot();
    assert!(f.install(false, true).is_err());
    assert_eq!(f.snapshot(), before);
    assert!(!f.binary().exists());
}

#[test]
fn service_only_does_not_require_home() {
    let f = Fixture::new();
    fs::remove_dir(&f.home).unwrap();
    let mut item =
        Installer::with_roots(None, Some(f.service.clone()), None, f.data.clone()).unwrap();
    assert_eq!(
        item.run(false, true, &mut |_| panic!("unexpected consent")),
        Ok(Outcome::Installed)
    );
    assert!(!f.home.exists());
    assert!(!f.data.join("applications").exists());
    let mut command = cli(&f);
    command
        .env_remove("HOME")
        .args(["--artifacts-only", "--service"])
        .arg(&f.service)
        .arg("--yes");
    assert!(bounded_output(command, Duration::from_secs(10))
        .status
        .success());
}

#[test]
fn service_only_verified_idempotent_install_has_no_publication() {
    let f = Fixture::new();
    assert_eq!(f.install(false, true), Ok(Outcome::Installed));
    let before = f.snapshot();
    assert_eq!(f.install(false, true), Ok(Outcome::Installed));
    assert_eq!(f.snapshot(), before);
}

#[test]
fn preserves_old_release_on_client_update() {
    let f = Fixture::new();
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
    let current = f.current();
    let old = fs::read_link(&current).unwrap();
    let old_inode = fs::symlink_metadata(&current).unwrap().ino();
    write(
        &f.bundle.join("data/flutter_assets/asset"),
        b"new asset",
        0o644,
    );
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
    assert_ne!(old, fs::read_link(&current).unwrap());
    assert_eq!(
        fs::read(
            current
                .parent()
                .unwrap()
                .join(&old)
                .join("data/flutter_assets/asset")
        )
        .unwrap(),
        b"asset"
    );
    let retained = f.named(current.parent().unwrap(), ".smith-next-selector-");
    assert_eq!(retained.len(), 1);
    assert_eq!(fs::read_link(&retained[0]).unwrap(), old);
    assert_eq!(fs::symlink_metadata(&retained[0]).unwrap().ino(), old_inode);
}

#[test]
fn no_unknown_destination_overwrite() {
    for destination in ["desktop", "wrapper", "service"] {
        let f = Fixture::new();
        let (path, mode) = match destination {
            "desktop" => (f.data.join(format!("applications/{APP}.desktop")), 0o644),
            "wrapper" => (f.home.join(".local/bin/smith-next"), 0o755),
            _ => (f.binary(), 0o755),
        };
        f.private_parents(path.parent().unwrap());
        write(&path, b"unknown", mode);
        let before = f.snapshot();
        assert!(f.install(true, true).is_err());
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn different_service_blocks_client_switch() {
    let f = Fixture::new();
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
    let target = fs::read_link(f.current()).unwrap();
    write(&f.service, b"v2", 0o755);
    write(&f.bundle.join("smith_backend/smith-service"), b"v2", 0o755);
    let before = f.snapshot();
    assert!(f.install(true, true).is_err());
    assert_eq!(f.snapshot(), before);
    assert_eq!(fs::read(f.binary()).unwrap(), b"service-v1");
    assert_eq!(fs::read_link(f.current()).unwrap(), target);
}

#[test]
fn mismatched_helpers_block_all_writes() {
    let f = Fixture::new();
    write(&f.service, b"different", 0o755);
    let before = f.snapshot();
    assert!(f.install(true, true).is_err());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn source_mode_hardlinks_and_special_files() {
    let f = Fixture::new();
    chmod(&f.service, 0o750);
    assert!(f.install(false, true).is_err());
    chmod(&f.service, 0o755);
    let alias = f.root.join("alias");
    fs::hard_link(&f.service, &alias).unwrap();
    assert!(f.install(false, true).is_err());
    fs::remove_file(alias).unwrap();
    let fifo = f.bundle.join("fifo");
    let c_path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    assert!(f.install(true, true).is_err());
    fs::remove_file(fifo).unwrap();
    symlink(&f.service, f.bundle.join("alias")).unwrap();
    assert!(f.install(true, true).is_err());
}

#[test]
fn symlink_ancestors_and_destinations() {
    let f = Fixture::new();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    chmod(&outside, 0o700);
    symlink(&outside, f.data.join("smith-next")).unwrap();
    let before = f.snapshot();
    assert!(f.install(true, true).is_err());
    assert_eq!(f.snapshot(), before);
    fs::remove_file(f.data.join("smith-next")).unwrap();
    symlink(&outside, f.data.join("applications")).unwrap();
    assert!(f.install(true, true).is_err());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn unsafe_and_missing_roots() {
    let f = Fixture::new();
    chmod(&f.data, 0o777);
    assert!(f.install(true, true).is_err());
    chmod(&f.data, 0o700);
    fs::remove_dir(&f.data).unwrap();
    assert!(f.install(true, true).is_err());
    assert!(!f.data.exists());
}

#[test]
fn private_namespace_required() {
    for namespace in [
        "smith-next",
        "smith-next/bin",
        "smith-next/client",
        "smith-next/client/releases",
    ] {
        let f = Fixture::new();
        let path = f.data.join(namespace);
        f.private_parents(&path);
        chmod(&path, 0o755);
        let before = f.snapshot();
        assert!(f.install(true, true).is_err());
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn changed_source_after_confirmation() {
    for replace in [false, true] {
        let f = Fixture::new();
        let service = f.service.clone();
        let mut item = f.installer(true, true);
        let mut confirm = |_: &str| {
            if replace {
                let replacement = f.root.join("replacement");
                write(&replacement, b"service-v1", 0o755);
                fs::rename(replacement, &service).unwrap();
            } else {
                write(&service, b"changed", 0o755);
            }
            Ok("y".into())
        };
        assert!(item.run(false, false, &mut confirm).is_err());
        assert!(!f.data.join(".smith-next-install.lock").exists());
        assert!(!f.data.join("smith-next").exists());
    }
}

fn change_directory_mtime(path: &Path) {
    let directory = fs::File::open(path).unwrap();
    let before = directory.metadata().unwrap();
    assert!(before.is_dir());
    let modified = before.modified().unwrap() + Duration::from_secs(2);
    directory
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let after = directory.metadata().unwrap();
    assert_eq!(after.modified().unwrap(), modified);
    assert_ne!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(
        (before.dev(), before.ino(), before.uid(), before.gid(), before.mode(), before.nlink(), before.size()),
        (after.dev(), after.ino(), after.uid(), after.gid(), after.mode(), after.nlink(), after.size()),
        "the consent-time mutation must change timestamps, not directory identity, permissions or contents"
    );
}

#[test]
fn bundle_root_mtime_change_after_confirmation_with_in_bundle_service_is_rejected() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut item = Installer::with_roots(
        Some(f.bundle.clone()),
        Some(f.bundle.join("smith_backend/smith-service")),
        Some(f.home.clone()),
        f.data.clone(),
    )
    .unwrap();
    let result = item.run(false, false, &mut |_| {
        change_directory_mtime(&f.bundle);
        Ok("y".into())
    });
    assert_eq!(
        result,
        Err("Artifacts or destinations changed after confirmation".into())
    );
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join(".smith-next-install.lock").exists());
    assert!(!f.data.join("smith-next").exists());
}

#[test]
fn service_only_selected_release_root_mtime_change_after_confirmation_is_rejected() {
    let f = Fixture::new();
    assert_eq!(f.install(true, false), Ok(Outcome::Installed));
    let current = f.current();
    let selected = current
        .parent()
        .unwrap()
        .join(fs::read_link(&current).unwrap());
    let before = f.snapshot();
    let mut item =
        Installer::with_roots(None, Some(f.service.clone()), None, f.data.clone()).unwrap();
    let result = item.run(false, false, &mut |_| {
        change_directory_mtime(&selected);
        Ok("y".into())
    });
    assert_eq!(
        result,
        Err("Artifacts or destinations changed after confirmation".into())
    );
    assert_eq!(f.snapshot(), before);
    assert!(!f.binary().exists());
}

#[test]
fn bundle_root_mtime_change_after_confirmation_with_separate_service_is_rejected() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut item = f.installer(true, true);
    let result = item.run(false, false, &mut |_| {
        change_directory_mtime(&f.bundle);
        Ok("y".into())
    });
    assert_eq!(
        result,
        Err("Artifacts or destinations changed after confirmation".into())
    );
    assert_eq!(f.snapshot(), before);
    assert!(!f.data.join(".smith-next-install.lock").exists());
    assert!(!f.data.join("smith-next").exists());
}

#[test]
fn changed_destination_after_confirmation() {
    let f = Fixture::new();
    let mut item = f.installer(true, true);
    assert!(item
        .run(false, false, &mut |_| {
            f.private_parents(f.binary().parent().unwrap());
            write(&f.binary(), b"foreign", 0o755);
            Ok("y".into())
        })
        .is_err());
    assert_eq!(fs::read(f.binary()).unwrap(), b"foreign");
    assert!(!f.data.join(".smith-next-install.lock").exists());
}

#[test]
fn unsafe_lock() {
    for kind in ["link", "mode", "hardlink", "fifo"] {
        let f = Fixture::new();
        let path = f.data.join(".smith-next-install.lock");
        match kind {
            "link" => symlink(&f.service, &path).unwrap(),
            "mode" => write(&path, b"", 0o644),
            "hardlink" => {
                write(&path, b"", 0o600);
                fs::hard_link(&path, f.root.join("lock-alias")).unwrap();
            }
            _ => {
                let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            }
        }
        assert!(f.install(true, true).is_err());
        assert!(!f.data.join("smith-next").exists());
    }
}

#[test]
fn current_must_be_managed_and_verified() {
    let f = Fixture::new();
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
    let current = f.current();
    let target = fs::read_link(&current).unwrap();
    fs::remove_file(&current).unwrap();
    symlink(&f.bundle, &current).unwrap();
    assert!(f.install(true, true).is_err());
    fs::remove_file(&current).unwrap();
    symlink(target, &current).unwrap();
    write(&current.join("data/icudtl.dat"), b"tampered", 0o644);
    assert!(f.install(true, true).is_err());
}

#[test]
fn existing_release_rejects_extra_files_modes_and_manifest_drift() {
    for change in ["extra", "mode", "manifest", "manifest-mode"] {
        let f = Fixture::new();
        assert_eq!(f.install(true, true), Ok(Outcome::Installed));
        let current = f.current();
        match change {
            "extra" => write(&current.join("extra"), b"extra", 0o644),
            "mode" => chmod(&current.join("lib"), 0o700),
            "manifest" => write(&current.join(MANIFEST), b"{}", 0o600),
            _ => chmod(&current.join(MANIFEST), 0o644),
        }
        let before = f.snapshot();
        assert!(f.install(true, true).is_err());
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn incomplete_bundle_blocks_all_writes() {
    let f = Fixture::new();
    fs::remove_file(f.bundle.join("lib/libapp.so")).unwrap();
    let before = f.snapshot();
    assert!(f.install(true, true).is_err());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn large_bundle_with_bounded_file_descriptors() {
    let f = Fixture::new();
    for i in 0..600 {
        write(
            &f.bundle.join(format!("data/flutter_assets/asset-{i}")),
            b"asset",
            0o644,
        );
    }
    let mut command = cli(&f);
    command
        .args(["--artifacts-only", "--client"])
        .arg(&f.bundle)
        .arg("--yes");
    // Only the disposable native child receives the lower descriptor budget.
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 128,
                rlim_max: 128,
            };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = bounded_output(command, Duration::from_secs(30));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(f.current().join("data/flutter_assets/asset-599")).unwrap(),
        b"asset"
    );
}

#[test]
fn simultaneous_cli_installers_have_safe_outcomes() {
    let f = Fixture::new();
    let commands: Vec<_> = (0..2)
        .map(|_| {
            let mut command = cli(&f);
            command
                .args(["--artifacts-only", "--client"])
                .arg(&f.bundle)
                .arg("--service")
                .arg(&f.service)
                .arg("--yes");
            command
        })
        .collect();
    let children: Vec<_> = commands
        .into_iter()
        .map(|command| std::thread::spawn(move || bounded_output(command, Duration::from_secs(15))))
        .collect();
    let codes: Vec<_> = children
        .into_iter()
        .map(|child| child.join().unwrap().status.code().unwrap())
        .collect();
    assert!(codes.iter().all(|code| matches!(code, 0 | 1)), "{codes:?}");
    assert!(
        codes.contains(&0),
        "neither isolated installer succeeded: {codes:?}"
    );
    assert_eq!(f.install(true, true), Ok(Outcome::Installed));
}

#[test]
fn cli_help_required_arguments_and_dry_run() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut help = cli(&f);
    help.arg("--help")
        .env_remove("HOME")
        .env_remove("XDG_DATA_HOME");
    let output = bounded_output(help, Duration::from_secs(5));
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--dry-run"));
    let mut missing = cli(&f);
    missing.arg("--artifacts-only");
    assert_eq!(
        bounded_output(missing, Duration::from_secs(5))
            .status
            .code(),
        Some(2)
    );
    let mut dry = cli(&f);
    dry.args(["--artifacts-only", "--client"])
        .arg(&f.bundle)
        .arg("--service")
        .arg(&f.service)
        .args(["--dry-run", "--yes"]);
    let output = bounded_output(dry, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(f.snapshot(), before);
}
