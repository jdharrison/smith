//! Public provider contract tests. Private HTTP fixtures live in model's cfg(test) module;
//! integration tests must never contact the production endpoint or expose an override API.
use smith_installer::model::{
    valid_model, DiscardProgress, FdProgress, Ollama, ProgressOutput, Provider, VectorProgress,
};
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

#[test]
fn exact_local_model_validation_matches_python_wrapper() {
    for name in [
        "tiny:tag",
        "library/tiny:Q4_0",
        "org/sub/model:v1.2-3",
        "a::tag",
        "a:b/c",
        "A_B:C.D",
    ] {
        assert!(valid_model(name), "{name}");
    }
    for name in [
        "",
        "tiny",
        ":tag",
        "tiny:",
        "http:tag",
        "https:tag",
        "tiny:CLOUD",
        "CloUd:tag",
        "tiny:tag\n",
        " tiny:tag",
        "tiny:tag ",
        "tiny:@tag",
        "tiny:tag?x",
        "tiny:tag#x",
        "tiny:🦀",
        "tiny:tag\u{202e}",
    ] {
        assert!(!valid_model(name), "{name:?}");
    }
    assert!(valid_model(&format!("{}:t", "a".repeat(254))));
    assert!(!valid_model(&format!("{}:t", "a".repeat(255))));
}

#[test]
fn provider_and_progress_apis_are_object_safe_and_construction_is_effect_free() {
    fn accepts_provider(_: &mut dyn Provider) {}
    fn accepts_progress(_: &mut dyn ProgressOutput) {}
    let constructor: fn() -> smith_installer::Result<Ollama> = Ollama::new;
    let mut provider = constructor().unwrap();
    accepts_provider(&mut provider);
    accepts_progress(&mut VectorProgress::new());
    accepts_progress(&mut DiscardProgress);
    let _: fn() -> smith_installer::Result<FdProgress> = FdProgress::stdout;
    // Deliberately do not invoke tags/pull/inference or construct a real stdout sink.
}

fn not_cancelled() -> bool {
    false
}
fn cancelled() -> bool {
    true
}

#[test]
fn vector_and_discard_outputs_are_bounded_and_observe_deadline_and_cancellation() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut output = VectorProgress::with_limit(3).unwrap();
    output.write_until(b"abc", deadline, not_cancelled).unwrap();
    assert_eq!(output.as_bytes(), b"abc");
    assert!(output.write_until(b"d", deadline, not_cancelled).is_err());
    assert_eq!(output.into_bytes(), b"abc");
    assert!(VectorProgress::with_limit(32 * 1024 * 1024 + 1).is_err());
    for output in [
        &mut VectorProgress::new() as &mut dyn ProgressOutput,
        &mut DiscardProgress,
    ] {
        assert!(output
            .write_until(b"x", Instant::now(), not_cancelled)
            .unwrap_err()
            .contains("deadline"));
        assert!(output
            .write_until(b"x", deadline, cancelled)
            .unwrap_err()
            .contains("cancelled"));
        assert!(output
            .write_until(&vec![b'x'; 1281], deadline, not_cancelled)
            .unwrap_err()
            .contains("bound"));
    }
}

#[test]
fn regular_file_output_is_rejected_without_changing_descriptor_flags_or_writing() {
    // Anonymous Linux memory file: no destination path or host/source filesystem write.
    // SAFETY: the name is NUL-terminated and memfd_create returns a new owned fd.
    let raw = unsafe { libc::memfd_create(c"smith-model-progress".as_ptr(), libc::MFD_CLOEXEC) };
    assert!(raw >= 0);
    // SAFETY: raw is newly allocated and owned only by this local test object.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: all fcntl/fstat calls operate on the live, owned descriptor.
    let before = unsafe { libc::fcntl(raw, libc::F_GETFL) };
    let error = match FdProgress::from_fd(fd.as_fd()) {
        Ok(_) => panic!("regular file accepted"),
        Err(error) => error,
    };
    assert!(error.contains("regular files"));
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFL) }, before);
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::fstat(raw, &mut stat) }, 0);
    assert_eq!(stat.st_size, 0);
}
