//! Ubuntu native installer. Artifact publication and recipient orchestration
//! remain separate authority boundaries; injectable factories are library-only
//! validation seams, never executable manager/provider overrides.
pub mod artifact;
pub mod catalog;
pub mod cli;
pub mod consent;
pub mod fs;
pub mod maintenance;
pub mod manifest;
pub mod model;
pub mod setup;
pub mod wizard;

pub type Result<T> = std::result::Result<T, String>;
