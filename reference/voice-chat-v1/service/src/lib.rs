//! Ubuntu-only local service. Lifecycle control and tool-free application protocols are separate.
pub mod app;
mod app_transport;
mod control;
mod filesystem;
mod manager;
pub mod providers;
mod transport;

pub use app_transport::{stdin_request as app_stdin_request, stdout_reply as app_stdout_reply};
pub use control::{Action, Controller, Observation};
pub use filesystem::Roots;
pub use manager::{install_control_cancellation, Manager, ManagerFacts, SystemManager};
pub use transport::{request, serve, BridgeReply};

pub const IDENTITY: &str = "smith-next/1 smith-service/0.1.0";
pub type Result<T> = std::result::Result<T, String>;
