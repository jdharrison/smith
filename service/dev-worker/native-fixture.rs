//! Isolated rustc harness for the new modules, using existing dependency artifacts only.
//! No cargo, daemon, Git/OpenCode process, model, or network calls are made by these tests.
//! Host application error types and JSON parser are fixture substitutes; main must run cargo
//! against the real crate. The actual shared Frame and old I/O primitives are included unchanged.
#![allow(dead_code)]
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
mod providers {
    pub struct AppError {
        pub code: String,
    }
    pub type AppResult<T> = std::result::Result<T, AppError>;
}
mod dev_runtime {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RuntimeDiagnostic {
        Fixture,
    }
    impl RuntimeDiagnostic {
        pub fn message(self) -> &'static str {
            "fixture"
        }
    }
}
mod cloud {
    pub fn unique_json(bytes: &[u8]) -> std::result::Result<serde_json::Value, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
#[path = "../src/dev_local.rs"]
mod dev_local;
#[path = "../src/dev_read.rs"]
mod dev_read;
#[path = "../src/dev_worker.rs"]
mod dev_worker;
#[path = "../src/dev_worker_local.rs"]
mod dev_worker_local;
