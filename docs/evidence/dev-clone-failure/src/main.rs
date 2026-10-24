//! Fixed metadata-only diagnostic; copied production module is built without cfg(test).
#![allow(dead_code)]
mod dev_local {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct Repository {
        pub url: String,
        pub reference: Option<String>,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct Checkout {
        pub url: String,
        pub reference: String,
        pub commit: String,
    }
    #[derive(Clone, Copy, Debug)]
    pub(crate) enum LocalError {
        RepositoryRejected,
        RepositoryUnavailable,
        Interrupted,
        Deadline,
    }
    pub(crate) type LocalResult<T> = Result<T, LocalError>;
}
#[path = "../production-dev-fetch.rs"]
mod dev_fetch;
fn main() {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    let started = Instant::now();
    let mut fetch = dev_fetch::FetchSession::new(vec![dev_local::Repository {
        url: "https://github.com/jdharrison/smith".into(),
        reference: Some("main".into()),
    }])
    .expect("fixed public destination");
    let result = fetch.request(
        0,
        true,
        &[],
        started + Duration::from_secs(20),
        &AtomicBool::new(false),
    );
    match result {
        Ok((kind, bytes)) => {
            let pins = fetch
                .pinned_checkouts()
                .expect("validated advertisement pin");
            println!("result=ok content_type={kind} advertisement_bytes={} pin_count={} selected_ref={} elapsed_ms={}", bytes.len(), pins.len(), pins[0].reference, started.elapsed().as_millis());
            // Capability names only, not ref/object metadata or response headers.
            let first_ref = bytes.split(|b| *b == 0).nth(1).unwrap_or_default();
            let caps = first_ref.split(|b| *b == b'\n').next().unwrap_or_default();
            println!("retained_capabilities={}", String::from_utf8_lossy(caps));
        }
        Err(error) => {
            println!(
                "result=error error={error:?} elapsed_ms={}",
                started.elapsed().as_millis()
            );
            std::process::exit(2);
        }
    }
}
