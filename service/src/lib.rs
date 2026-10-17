//! Ubuntu-only local service. Lifecycle control and tool-free application protocols are separate.
pub mod acceleration;
pub mod app;
mod app_transport;
pub mod catalog;
pub mod cloud;
mod control;
mod filesystem;
pub mod maintenance;
mod manager;
pub mod providers;
pub mod runner;
mod transport;

pub use app_transport::{
    stdin_request as app_stdin_request, stdin_request_correlated as app_stdin_request_correlated,
    stdout_reply as app_stdout_reply,
};
pub use control::{Action, Controller, Observation};
pub use filesystem::Roots;
pub use maintenance::MaintenanceLease;
pub use manager::{
    install_control_cancellation, MaintenanceManagerFacts, Manager, ManagerFacts, SystemManager,
};
pub use transport::{request, serve, BridgeReply};

pub const IDENTITY: &str = "smith-next/1 smith-service/0.1.0";
pub type Result<T> = std::result::Result<T, String>;
