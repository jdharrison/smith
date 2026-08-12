//! Host-side durable memory for the sealed brain.
//!
//! Layout under `<data_dir>/smith/brain/`:
//! - `profile.md`
//! - `episodic/` (dated session notes)
//! - `working/analysis.json`
//! - `journal/` (append-only attempt logs)

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_PROFILE: &str = r#"# Brain profile

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

/// Ensure memory directories and default profile exist.
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
        fs::write(&profile, DEFAULT_PROFILE)
            .map_err(|e| format!("Failed to write default profile: {}", e))?;
    }
    Ok(root)
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
}
