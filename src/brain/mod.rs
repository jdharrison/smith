//! Sealed local brain: durable host memory and analysis contracts.

pub mod intent;
pub mod memory;

pub use intent::{looks_like_affirmation, split_intent};
pub use memory::{
    append_chat_turn, append_journal, clear_pending_intent, ensure_memory_layout, load_analysis,
    load_chat_session_meta, load_pending_intent, load_profile, recent_chat_history_text,
    recent_episodic, reset_chat_session, save_analysis, save_pending_intent,
    set_opencode_session_id, AnalysisState,
};
