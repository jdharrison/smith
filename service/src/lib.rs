//! Ubuntu-only local service. Lifecycle control and tool-free application protocols are separate.
pub mod acceleration;
pub mod app;
mod app_transport;
#[doc(hidden)]
pub mod artifact_dagger;
#[doc(hidden)]
pub mod artifact_dagger_process;
#[doc(hidden)]
pub mod artifact_pipeline;
pub mod catalog;
pub mod cloud;
mod control;
mod dev_broker;
mod dev_completion;
mod dev_fetch;
#[doc(hidden)]
pub mod dev_local;
mod dev_read;
pub(crate) mod dev_runtime;
#[doc(hidden)]
pub mod dev_worker;
#[doc(hidden)]
pub mod dev_worker_local;
mod filesystem;
mod git_tree;
mod inspection_diagnostics;
pub mod maintenance;
mod manager;
pub mod providers;
pub mod runner;
mod transport;

#[doc(hidden)]
pub use app::retained_jobs;
pub use app_transport::{
    stdin_request as app_stdin_request, stdin_request_correlated as app_stdin_request_correlated,
    stdout_reply as app_stdout_reply,
};
pub use control::{Action, Controller, Observation};
/// Unwired retained-job repository exporter; not enabled by any service constructor.
#[doc(hidden)]
pub use dev_runtime::DockerSourceExporter;
pub use filesystem::Roots;
pub use maintenance::MaintenanceLease;
pub use manager::{
    install_control_cancellation, MaintenanceManagerFacts, Manager, ManagerFacts, SystemManager,
};
pub use transport::{request, serve, BridgeReply};

pub const IDENTITY: &str = "smith-next/1 smith-service/0.1.0";
pub type Result<T> = std::result::Result<T, String>;
