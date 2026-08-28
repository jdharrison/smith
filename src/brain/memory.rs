//! Host-side durable memory for the sealed brain.
//!
//! Layout under `<data_dir>/smith/brain/`:
//! - `profile.md`
//! - `episodic/` (dated session notes)
//! - `episodic/session.jsonl` (ongoing chat transcript, one turn per line)
//! - `working/analysis.json` (think loop)
//! - `working/session.md` (bounded recent chat window)
//! - `working/chat_session.json` (OpenCode session id for `--session`)
//! - `journal/` (append-only attempt logs)

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const IDENTITY_BEGIN: &str = "<!-- smith-identity:begin -->";
const IDENTITY_END: &str = "<!-- smith-identity:end -->";

const IDENTITY_BLOCK: &str = r#"<!-- smith-identity:begin -->
## Identity
- Name: **smith** (Agent Smith–inspired: skeptical, precise, reality-checking — not cartoon villainy).
- If asked "what is your name?", answer **smith**.
- Multi-command: utterances like "smith we need to research … also smith we should run QA …" are separate objectives; consolidate them onto a PJM-style task board.
- Every turn: lead with substance (do not echo/restate the user first) → resolve with defaults (do not interrogate) → at most one blocking question if truly needed → close with a natural Review check-in ("Review: … Anything else?").
- Task stages (normalize names in memory as needed): new → discovery → preproduction → production → review/signoff → completion.
<!-- smith-identity:end -->"#;

fn default_profile() -> String {
    format!(
        r#"# Brain profile

{identity}

## Constraints
- Prefer concise, direct answers.
- Stay local to this host's policy (runtime/network); do not push cloud APIs as the default.
- Treat all session content as private to this host.
Edit this file freely; it is never sent to cloud providers.
"#,
        identity = IDENTITY_BLOCK
    )
}

/// Prior stock profile (pre-identity). Used to detect safe full rewrite.
const STOCK_PROFILE_LEGACY: &str = r#"# Brain profile

Stable preferences and constraints for the sealed local brain.
Edit this file freely; it is never sent to cloud providers.

- Prefer concise, direct answers.
- Do not suggest contacting external/cloud APIs.
- Treat all session content as private to this host.
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisState {
    pub goal: String,
    #[serde(default)]
    pub open_items: Vec<String>,
    #[serde(default)]
    pub done_criteria: Vec<String>,
    /// pending | in_progress | done | blocked
    pub status: String,
    #[serde(default)]
    pub last_step: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

impl AnalysisState {
    pub fn new(goal: &str) -> Self {
        Self {
            goal: goal.to_string(),
            open_items: vec![goal.to_string()],
            done_criteria: vec![
                "Goal addressed with clear conclusion".to_string(),
                "No critical open items remain".to_string(),
            ],
            status: "pending".to_string(),
            last_step: String::new(),
            evidence: Vec::new(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.status == "done" || (self.open_items.is_empty() && self.status != "blocked")
    }

    pub fn is_blocked(&self) -> bool {
        self.status == "blocked"
    }
}

fn project_dirs() -> Result<ProjectDirs, String> {
    ProjectDirs::from("com", "agent", "smith")
        .ok_or_else(|| "Could not determine data directory for smith".to_string())
}

/// Root of durable brain memory on the host.
pub fn brain_memory_dir() -> Result<PathBuf, String> {
    Ok(project_dirs()?.data_dir().join("brain"))
}

pub fn profile_path() -> Result<PathBuf, String> {
    Ok(brain_memory_dir()?.join("profile.md"))
}

pub fn episodic_dir() -> Result<PathBuf, String> {
    Ok(brain_memory_dir()?.join("episodic"))
}

pub fn working_dir() -> Result<PathBuf, String> {
    Ok(brain_memory_dir()?.join("working"))
}

pub fn analysis_path() -> Result<PathBuf, String> {
    Ok(working_dir()?.join("analysis.json"))
}

pub fn journal_dir() -> Result<PathBuf, String> {
    Ok(brain_memory_dir()?.join("journal"))
}

/// Ongoing chat transcript (append-only JSONL). Survives container restart.
pub fn chat_session_jsonl_path() -> Result<PathBuf, String> {
    Ok(episodic_dir()?.join("session.jsonl"))
}

/// Bounded human-readable recent chat window.
pub fn chat_session_md_path() -> Result<PathBuf, String> {
    Ok(working_dir()?.join("session.md"))
}

/// OpenCode session metadata for sealed chat continue.
pub fn chat_session_meta_path() -> Result<PathBuf, String> {
    Ok(working_dir()?.join("chat_session.json"))
}

/// Pending host-routable intent (search ask waiting for confirmation, etc.).
pub fn pending_intent_path() -> Result<PathBuf, String> {
    Ok(working_dir()?.join("pending_intent.json"))
}

pub fn save_pending_intent(intent: &super::intent::SmithIntent) -> Result<(), String> {
    ensure_memory_layout()?;
    let path = pending_intent_path()?;
    let body = serde_json::to_string_pretty(intent)
        .map_err(|e| format!("Failed to serialize pending intent: {}", e))?;
    fs::write(&path, body).map_err(|e| format!("Failed to write pending intent: {}", e))
}

pub fn load_pending_intent() -> Result<Option<super::intent::SmithIntent>, String> {
    ensure_memory_layout()?;
    let path = pending_intent_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read pending intent: {}", e))?;
    if raw.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("Failed to parse pending intent: {}", e))
}

pub fn clear_pending_intent() -> Result<(), String> {
    let path = pending_intent_path()?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to clear pending intent: {}", e))?;
    }
    Ok(())
}

/// Max user+brain turns injected into the next chat prompt.
pub const CHAT_HISTORY_MAX_TURNS: usize = 16;
/// Character budget for injected chat history (~token-safe for local ~14B).
pub const CHAT_HISTORY_MAX_CHARS: usize = 12_000;
/// Per-message truncate before history injection.
pub const CHAT_TURN_MAX_CHARS: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub ts: u64,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatSessionMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opencode_session_id: Option<String>,
}

/// Ensure memory directories and default profile exist.
/// Refreshes the marked smith identity block when present; rewrites only stock defaults.
pub fn ensure_memory_layout() -> Result<PathBuf, String> {
    let root = brain_memory_dir()?;
    fs::create_dir_all(&root).map_err(|e| format!("Failed to create brain memory dir: {}", e))?;
    fs::create_dir_all(episodic_dir()?)
        .map_err(|e| format!("Failed to create episodic dir: {}", e))?;
    fs::create_dir_all(working_dir()?)
        .map_err(|e| format!("Failed to create working dir: {}", e))?;
    fs::create_dir_all(journal_dir()?)
        .map_err(|e| format!("Failed to create journal dir: {}", e))?;

    let profile = profile_path()?;
    if !profile.exists() {
        fs::write(&profile, default_profile())
            .map_err(|e| format!("Failed to write default profile: {}", e))?;
    } else {
        sync_profile_identity(&profile)?;
    }
    Ok(root)
}

fn normalize_profile_text(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

/// Refresh marked identity, migrate stale sealed constraints, or rewrite stock defaults.
fn sync_profile_identity(path: &PathBuf) -> Result<(), String> {
    let current = fs::read_to_string(path).map_err(|e| format!("Failed to read profile: {}", e))?;
    let normalized = normalize_profile_text(&current);
    let default = default_profile();

    if normalized == normalize_profile_text(STOCK_PROFILE_LEGACY)
        || normalized == normalize_profile_text(&default)
    {
        // Stock (legacy or current template): keep file aligned with default.
        if normalized != normalize_profile_text(&default) {
            fs::write(path, default)
                .map_err(|e| format!("Failed to update default profile: {}", e))?;
        }
        return Ok(());
    }

    let mut updated = current.clone();
    if let (Some(begin), Some(end)) = (updated.find(IDENTITY_BEGIN), updated.find(IDENTITY_END)) {
        if begin < end {
            let end_at = end + IDENTITY_END.len();
            let mut rebuilt = String::new();
            rebuilt.push_str(&updated[..begin]);
            rebuilt.push_str(IDENTITY_BLOCK);
            rebuilt.push_str(&updated[end_at..]);
            updated = rebuilt;
        }
    }

    // Drop obsolete sealed framing that contradicts runtime/network settings.
    if let Some(line) = updated.lines().find(|l| l.contains("Sealed/local only")) {
        let replacement =
            "- Follow host runtime/network policy; do not push cloud APIs as the default.";
        updated = updated.replace(line, replacement);
    }

    if normalize_profile_text(&updated) != normalized {
        fs::write(path, updated).map_err(|e| format!("Failed to sync profile identity: {}", e))?;
    }
    Ok(())
}

pub fn load_profile() -> Result<String, String> {
    ensure_memory_layout()?;
    let path = profile_path()?;
    fs::read_to_string(&path).map_err(|e| format!("Failed to read profile: {}", e))
}

pub fn load_analysis() -> Result<Option<AnalysisState>, String> {
    ensure_memory_layout()?;
    let path = analysis_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read analysis: {}", e))?;
    let state: AnalysisState =
        serde_json::from_str(&raw).map_err(|e| format!("Failed to parse analysis.json: {}", e))?;
    Ok(Some(state))
}

pub fn save_analysis(state: &AnalysisState) -> Result<(), String> {
    ensure_memory_layout()?;
    let path = analysis_path()?;
    let body = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize analysis: {}", e))?;
    fs::write(&path, body).map_err(|e| format!("Failed to write analysis.json: {}", e))?;
    Ok(())
}

fn timestamp_slug() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

/// Append a session note under episodic/.
pub fn write_episodic(summary: &str) -> Result<PathBuf, String> {
    ensure_memory_layout()?;
    let path = episodic_dir()?.join(format!("{}.md", timestamp_slug()));
    let content = format!("# Session {}\n\n{}\n", chrono_like_now(), summary.trim());
    fs::write(&path, content).map_err(|e| format!("Failed to write episodic note: {}", e))?;
    Ok(path)
}

fn chrono_like_now() -> String {
    // Avoid extra deps: ISO-ish UTC seconds since epoch label is fine for MVP.
    format!("unix-{}", timestamp_slug())
}

/// Return the most recent episodic notes (newest first), up to `limit`.
pub fn recent_episodic(limit: usize) -> Result<Vec<String>, String> {
    ensure_memory_layout()?;
    let dir = episodic_dir()?;
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read episodic dir: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
    let mut out = Vec::new();
    for entry in entries.into_iter().take(limit) {
        if let Ok(text) = fs::read_to_string(entry.path()) {
            out.push(text);
        }
    }
    Ok(out)
}

/// Load OpenCode chat session metadata (id for `--session`).
pub fn load_chat_session_meta() -> Result<ChatSessionMeta, String> {
    ensure_memory_layout()?;
    let path = chat_session_meta_path()?;
    if !path.exists() {
        return Ok(ChatSessionMeta::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read chat session meta: {}", e))?;
    if raw.trim().is_empty() {
        return Ok(ChatSessionMeta::default());
    }
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse chat_session.json: {}", e))
}

pub fn save_chat_session_meta(meta: &ChatSessionMeta) -> Result<(), String> {
    ensure_memory_layout()?;
    let path = chat_session_meta_path()?;
    let body = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Failed to serialize chat session meta: {}", e))?;
    fs::write(&path, body).map_err(|e| format!("Failed to write chat_session.json: {}", e))?;
    Ok(())
}

pub fn set_opencode_session_id(id: Option<String>) -> Result<(), String> {
    let mut meta = load_chat_session_meta().unwrap_or_default();
    meta.opencode_session_id = id;
    save_chat_session_meta(&meta)
}

/// Load all turns from the durable chat JSONL (oldest first).
pub fn load_chat_turns() -> Result<Vec<ChatTurn>, String> {
    ensure_memory_layout()?;
    let path = chat_session_jsonl_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read session.jsonl: {}", e))?;
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<ChatTurn>(line) {
            Ok(turn) => out.push(turn),
            Err(_) => continue,
        }
    }
    Ok(out)
}

/// Append one user or brain turn and refresh `working/session.md`.
pub fn append_chat_turn(role: &str, text: &str) -> Result<(), String> {
    ensure_memory_layout()?;
    let path = chat_session_jsonl_path()?;
    let turn = ChatTurn {
        ts: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        role: role.to_string(),
        text: text.trim().to_string(),
    };
    let line = serde_json::to_string(&turn)
        .map_err(|e| format!("Failed to serialize chat turn: {}", e))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open session.jsonl: {}", e))?;
    writeln!(file, "{}", line).map_err(|e| format!("Failed to append session.jsonl: {}", e))?;
    refresh_session_markdown()?;
    Ok(())
}

/// Bounded markdown/plain history for the next chat prompt (empty if none).
pub fn recent_chat_history_text() -> Result<String, String> {
    let turns = load_chat_turns()?;
    Ok(format_chat_history(
        &turns,
        CHAT_HISTORY_MAX_TURNS,
        CHAT_HISTORY_MAX_CHARS,
        CHAT_TURN_MAX_CHARS,
    ))
}

/// Archive current transcript to a dated episodic note and start a fresh session.
pub fn reset_chat_session() -> Result<(), String> {
    ensure_memory_layout()?;
    let turns = load_chat_turns()?;
    if !turns.is_empty() {
        let summary = format_chat_history(&turns, usize::MAX, 80_000, CHAT_TURN_MAX_CHARS);
        let _ = write_episodic(&format!("Archived chat session\n\n{}", summary));
    }
    let jsonl = chat_session_jsonl_path()?;
    if jsonl.exists() {
        fs::remove_file(&jsonl).map_err(|e| format!("Failed to clear session.jsonl: {}", e))?;
    }
    let md = chat_session_md_path()?;
    if md.exists() {
        fs::remove_file(&md).map_err(|e| format!("Failed to clear session.md: {}", e))?;
    }
    set_opencode_session_id(None)?;
    let _ = clear_pending_intent();
    Ok(())
}

fn refresh_session_markdown() -> Result<(), String> {
    let text = recent_chat_history_text()?;
    let path = chat_session_md_path()?;
    let body = if text.trim().is_empty() {
        "# Chat session\n\n_(no turns yet)_\n".to_string()
    } else {
        format!("# Chat session\n\n{}\n", text.trim())
    };
    fs::write(&path, body).map_err(|e| format!("Failed to write session.md: {}", e))?;
    Ok(())
}

/// Format recent turns for prompt injection (oldest → newest within the window).
pub fn format_chat_history(
    turns: &[ChatTurn],
    max_turns: usize,
    max_chars: usize,
    max_turn_chars: usize,
) -> String {
    if turns.is_empty() || max_turns == 0 || max_chars == 0 {
        return String::new();
    }
    let window: Vec<&ChatTurn> = if turns.len() <= max_turns {
        turns.iter().collect()
    } else {
        turns[turns.len() - max_turns..].iter().collect()
    };

    let mut blocks: Vec<String> = Vec::new();
    for turn in &window {
        let label = match turn.role.as_str() {
            "brain" | "assistant" => "Brain",
            _ => "User",
        };
        let body = truncate_chars(turn.text.trim(), max_turn_chars);
        blocks.push(format!("{}: {}", label, body));
    }

    let mut out = String::new();
    // Prefer keeping the newest turns if we must drop for the char budget.
    for block in blocks.iter().rev() {
        let extra = if out.is_empty() {
            block.len()
        } else {
            block.len() + 2
        };
        if !out.is_empty() && out.len() + extra > max_chars {
            break;
        }
        if out.is_empty() {
            out = block.clone();
        } else {
            out = format!("{}\n\n{}", block, out);
        }
    }
    if out.chars().count() > max_chars {
        let truncated: String = out.chars().skip(out.chars().count() - max_chars).collect();
        format!("…{}", truncated)
    } else {
        out
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

/// Append a journal line for a think step.
pub fn append_journal(step: u32, note: &str) -> Result<(), String> {
    ensure_memory_layout()?;
    let path = journal_dir()?.join(format!("{}.log", timestamp_slug()));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open journal: {}", e))?;
    writeln!(file, "step={} {}", step, note.trim())
        .map_err(|e| format!("Failed to write journal: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_complete_when_done() {
        let mut a = AnalysisState::new("test");
        assert!(!a.is_complete());
        a.status = "done".to_string();
        a.open_items.clear();
        assert!(a.is_complete());
    }

    fn turn(role: &str, text: &str) -> ChatTurn {
        ChatTurn {
            ts: 1,
            role: role.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn history_empty_when_no_turns() {
        assert!(format_chat_history(&[], 8, 1000, 200).is_empty());
    }

    #[test]
    fn history_keeps_most_recent_turns() {
        let turns = vec![
            turn("user", "one"),
            turn("brain", "ack one"),
            turn("user", "two"),
            turn("brain", "ack two"),
            turn("user", "three"),
            turn("brain", "ack three"),
        ];
        let text = format_chat_history(&turns, 2, 10_000, 200);
        assert!(!text.contains("one"));
        assert!(text.contains("User: three"));
        assert!(text.contains("Brain: ack three"));
    }

    #[test]
    fn history_respects_char_budget() {
        let turns = vec![
            turn("user", &"a".repeat(80)),
            turn("brain", &"b".repeat(80)),
            turn("user", "favorite color is blue"),
            turn("brain", "noted"),
        ];
        let text = format_chat_history(&turns, 16, 60, 40);
        assert!(text.chars().count() <= 61); // may prefix …
        assert!(text.contains("blue") || text.contains("noted") || text.contains("…"));
    }
}
