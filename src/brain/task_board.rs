//! Host task / PJM board — durable ledger for chat intake.
//!
//! Every user message becomes a task. Fast replies complete in-process;
//! async workers (search/files/code/pipeline) update the task when done.

use super::memory::brain_memory_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    AwaitingSignoff,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Reply,
    Research,
    Files,
    Code,
    Pipeline,
    #[serde(other)]
    Unknown,
}

impl TaskKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskKind::Reply => "reply",
            TaskKind::Research => "research",
            TaskKind::Files => "files",
            TaskKind::Code => "code",
            TaskKind::Pipeline => "pipeline",
            TaskKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub kind: TaskKind,
    /// Board stage: new | discovery | preproduction | production | review/signoff | completion
    pub stage: String,
    pub status: TaskStatus,
    pub user_text: String,
    #[serde(default)]
    pub ack_text: String,
    #[serde(default)]
    pub result_text: String,
    #[serde(default)]
    pub expected_result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(default)]
    pub session: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TaskRecord {
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Queued | TaskStatus::Running | TaskStatus::AwaitingSignoff
        )
    }

    pub fn status_str(&self) -> &'static str {
        match self.status {
            TaskStatus::Queued => "queued",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::AwaitingSignoff => "awaiting_signoff",
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_task_id() -> String {
    // Include nanos to avoid collisions under load.
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("task-{ns}")
}

pub fn tasks_dir() -> Result<PathBuf, String> {
    let brain = brain_memory_dir()?;
    let parent = brain
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| brain.clone());
    Ok(parent.join("tasks"))
}

fn task_path(id: &str) -> Result<PathBuf, String> {
    Ok(tasks_dir()?.join(format!("{id}.json")))
}

pub fn create_task(
    user_text: &str,
    kind: TaskKind,
    title: Option<&str>,
    expected: Option<&str>,
) -> Result<TaskRecord, String> {
    let dir = tasks_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("tasks dir: {e}"))?;
    let now = now_unix();
    let title = title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let t = user_text.trim();
            if t.chars().count() <= 72 {
                t.to_string()
            } else {
                format!("{}…", t.chars().take(69).collect::<String>())
            }
        });
    let stage = match kind {
        TaskKind::Reply => "completion",
        TaskKind::Research => "discovery",
        TaskKind::Files => "discovery",
        TaskKind::Code => "production",
        TaskKind::Pipeline => "production",
        TaskKind::Unknown => "new",
    }
    .to_string();
    let record = TaskRecord {
        id: new_task_id(),
        title,
        kind,
        stage,
        status: TaskStatus::Running,
        user_text: user_text.to_string(),
        ack_text: String::new(),
        result_text: String::new(),
        expected_result: expected.unwrap_or("").to_string(),
        job_id: None,
        session: "default".to_string(),
        created_at: now,
        updated_at: now,
        error: None,
    };
    save_task(&record)?;
    Ok(record)
}

pub fn save_task(task: &TaskRecord) -> Result<(), String> {
    let dir = tasks_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("tasks dir: {e}"))?;
    let body =
        serde_json::to_string_pretty(task).map_err(|e| format!("serialize task: {e}"))?;
    fs::write(task_path(&task.id)?, body).map_err(|e| format!("write task: {e}"))
}

pub fn get_task(id: &str) -> Result<TaskRecord, String> {
    let raw = fs::read_to_string(task_path(id)?).map_err(|e| format!("read task: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse task: {e}"))
}

pub fn update_task<F>(id: &str, f: F) -> Result<TaskRecord, String>
where
    F: FnOnce(&mut TaskRecord),
{
    let mut task = get_task(id)?;
    f(&mut task);
    task.updated_at = now_unix();
    save_task(&task)?;
    Ok(task)
}

pub fn set_ack(id: &str, ack: &str) -> Result<TaskRecord, String> {
    update_task(id, |t| {
        t.ack_text = ack.to_string();
    })
}

pub fn complete_task(id: &str, result: &str) -> Result<TaskRecord, String> {
    update_task(id, |t| {
        t.result_text = result.to_string();
        t.status = TaskStatus::Completed;
        t.stage = "completion".to_string();
        t.error = None;
    })
}

pub fn fail_task(id: &str, error: &str) -> Result<TaskRecord, String> {
    update_task(id, |t| {
        t.status = TaskStatus::Failed;
        t.error = Some(error.to_string());
        t.result_text = format!("Failed: {error}");
    })
}

pub fn awaiting_signoff(id: &str, result: &str) -> Result<TaskRecord, String> {
    update_task(id, |t| {
        t.status = TaskStatus::AwaitingSignoff;
        t.stage = "review/signoff".to_string();
        t.result_text = result.to_string();
    })
}

pub fn list_tasks(active_only: bool, since: Option<u64>) -> Result<Vec<TaskRecord>, String> {
    let dir = tasks_dir()?;
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("list tasks: {e}"))? {
        let entry = entry.map_err(|e| format!("tasks entry: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(task) = serde_json::from_str::<TaskRecord>(&raw) else {
            continue;
        };
        if active_only && !task.is_active() {
            continue;
        }
        if let Some(s) = since {
            if task.updated_at < s {
                continue;
            }
        }
        out.push(task);
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
    Ok(out)
}

pub fn default_ack(kind: &TaskKind) -> String {
    match kind {
        TaskKind::Research => "On it — looking that up.".to_string(),
        TaskKind::Files => "On it — searching this machine.".to_string(),
        TaskKind::Code => "On it — building that.".to_string(),
        TaskKind::Pipeline => "On it — queued the pipeline.".to_string(),
        TaskKind::Reply | TaskKind::Unknown => "Got it.".to_string(),
    }
}
