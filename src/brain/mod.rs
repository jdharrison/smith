//! Sealed local brain: durable host memory and analysis contracts.

pub mod memory;

pub use memory::{
    append_journal, ensure_memory_layout, load_analysis, load_profile, recent_episodic,
    save_analysis, write_episodic, AnalysisState,
};
