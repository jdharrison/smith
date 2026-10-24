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
#[path = "../../repro-dev-fetch.rs"]
mod dev_fetch;
fn main() {
    let input = std::fs::read("synthetic-advertisement.bin").unwrap();
    let output = dev_fetch::sanitize_fixture(&input).expect("production advertisement sanitizer");
    std::fs::write("sanitized-advertisement.bin", &output).unwrap();
    println!("sanitizer=ok input_bytes={} output_bytes={}", input.len(), output.len());
}
