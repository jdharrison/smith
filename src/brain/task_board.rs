//! Host task / PJM board — durable ledger for chat intake.
//!
//! Every user message becomes a task. Fast replies complete in-process;
//! async workers (search/files/code/pipeline) update the task when done.

use super::memory::brain_memory_dir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    AwaitingSignoff,
    Interrupted,
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
    /// Forward-compatible persisted fields that this binary does not yet understand.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
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
            TaskStatus::Interrupted => "interrupted",
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
        extra: BTreeMap::new(),
    };
    save_task(&record)?;
    Ok(record)
}

pub fn save_task(task: &TaskRecord) -> Result<(), String> {
    let dir = tasks_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("tasks dir: {e}"))?;
    let body = serde_json::to_string_pretty(task).map_err(|e| format!("serialize task: {e}"))?;
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

const INTERRUPTED_ERROR: &str =
    "Smith restarted before this task finished; review the preserved task details and retry when ready.";
const INTERRUPTED_RESULT: &str =
    "Recovery required: this task was interrupted by a Smith supervisor restart. Review its preserved details and retry when ready.";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn append_recovery_guidance(existing: &str, guidance: &str) -> String {
    if existing.is_empty() {
        guidance.to_string()
    } else {
        format!("{existing}\n\n{guidance}")
    }
}

fn append_recovery_error(existing: Option<String>, guidance: &str) -> Option<String> {
    Some(match existing {
        Some(existing) => append_recovery_guidance(&existing, guidance),
        None => guidance.to_string(),
    })
}

fn create_reconcile_temp_file(destination: &Path) -> io::Result<(PathBuf, File)> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "reconciliation destination has no parent directory",
        )
    })?;
    let file_name = destination.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "reconciliation destination has no file name",
        )
    })?;

    for _ in 0..100 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = OsString::from(".");
        temp_name.push(file_name);
        temp_name.push(format!(
            ".smith-reconcile-{}-{sequence}.tmp",
            std::process::id()
        ));
        let temp_path = parent.join(temp_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique reconciliation temp file",
    ))
}

fn cleanup_temp_after_error(temp_path: &Path, error: io::Error) -> io::Error {
    match fs::remove_file(temp_path) {
        Ok(()) => error,
        Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => error,
        Err(cleanup_error) => io::Error::new(
            error.kind(),
            format!(
                "{error}; failed to remove reconciliation temp file {}: {cleanup_error}",
                temp_path.display()
            ),
        ),
    }
}

fn atomic_replace(path: &Path, body: &[u8]) -> io::Result<()> {
    atomic_replace_with(path, body, |_| Ok(()))
}

fn atomic_replace_with<F>(path: &Path, body: &[u8], before_rename: F) -> io::Result<()>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "reconciliation destination has no parent directory",
        )
    })?;
    let permissions = fs::metadata(path)?.permissions();
    let (temp_path, mut temp_file) = create_reconcile_temp_file(path)?;

    let prepare_result = (|| {
        temp_file.write_all(body)?;
        temp_file.flush()?;
        temp_file.set_permissions(permissions)?;
        temp_file.sync_all()?;
        before_rename(&temp_path)
    })();
    drop(temp_file);

    if let Err(error) = prepare_result {
        return Err(cleanup_temp_after_error(&temp_path, error));
    }
    if let Err(error) = fs::rename(&temp_path, path) {
        return Err(cleanup_temp_after_error(&temp_path, error));
    }

    File::open(parent)?.sync_all()
}

/// Mark persisted work that cannot still be executing after a supervisor restart.
///
/// Only queued/running records are changed. Re-running reconciliation is a no-op.
pub fn reconcile_interrupted() -> Result<usize, String> {
    reconcile_interrupted_in_dir(&tasks_dir()?, now_unix())
}

fn reconcile_interrupted_in_dir(dir: &Path, reconciled_at: u64) -> Result<usize, String> {
    if !dir.is_dir() {
        return Ok(0);
    }

    let mut reconciled = 0;
    for entry in fs::read_dir(dir).map_err(|e| format!("list tasks for reconciliation: {e}"))? {
        let entry = entry.map_err(|e| format!("task reconciliation entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("read persisted task {name} for reconciliation: {e}"))?;
        let mut task: TaskRecord = serde_json::from_str(&raw)
            .map_err(|e| format!("parse persisted task {name} for reconciliation: {e}"))?;
        if !matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
            continue;
        }

        task.status = TaskStatus::Interrupted;
        task.error = append_recovery_error(task.error.take(), INTERRUPTED_ERROR);
        task.result_text = append_recovery_guidance(&task.result_text, INTERRUPTED_RESULT);
        task.updated_at = reconciled_at;
        let body = serde_json::to_string_pretty(&task)
            .map_err(|e| format!("serialize interrupted task {name}: {e}"))?;
        atomic_replace(&path, body.as_bytes())
            .map_err(|e| format!("persist interrupted task {name}: {e}"))?;
        reconciled += 1;
    }
    Ok(reconciled)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn temp_tasks_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "smith-task-board-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn task(id: &str, status: TaskStatus) -> TaskRecord {
        TaskRecord {
            id: id.to_string(),
            title: format!("title-{id}"),
            kind: TaskKind::Pipeline,
            stage: "production".to_string(),
            status,
            user_text: format!("user-{id}"),
            ack_text: format!("ack-{id}"),
            result_text: format!("result-{id}"),
            expected_result: format!("expected-{id}"),
            job_id: Some(format!("job-{id}")),
            session: "default".to_string(),
            created_at: 10,
            updated_at: 20,
            error: Some(format!("error-{id}")),
            extra: BTreeMap::from([(
                "approval_binding".to_string(),
                serde_json::json!({"id": format!("approval-{id}")}),
            )]),
        }
    }

    fn write_task(dir: &Path, task: &TaskRecord) {
        let body = serde_json::to_string_pretty(task).expect("serialize test task");
        fs::write(dir.join(format!("{}.json", task.id)), body).expect("write test task");
    }

    #[test]
    fn task_status_serde_preserves_existing_labels_and_adds_interrupted() {
        let cases = [
            ("queued", TaskStatus::Queued),
            ("running", TaskStatus::Running),
            ("completed", TaskStatus::Completed),
            ("failed", TaskStatus::Failed),
            ("awaiting_signoff", TaskStatus::AwaitingSignoff),
            ("interrupted", TaskStatus::Interrupted),
        ];

        for (label, status) in cases {
            assert_eq!(
                serde_json::from_str::<TaskStatus>(&format!("\"{label}\""))
                    .expect("deserialize status"),
                status
            );
            assert_eq!(
                serde_json::to_string(&status).expect("serialize status"),
                format!("\"{label}\"")
            );
        }
    }

    #[test]
    fn reconciliation_interrupts_only_queued_and_running_tasks_once() {
        let dir = temp_tasks_dir("reconcile");
        fs::create_dir_all(&dir).expect("create test tasks dir");
        let statuses = [
            ("queued", TaskStatus::Queued),
            ("running", TaskStatus::Running),
            ("completed", TaskStatus::Completed),
            ("failed", TaskStatus::Failed),
            ("awaiting", TaskStatus::AwaitingSignoff),
            ("interrupted", TaskStatus::Interrupted),
        ];
        for (id, status) in statuses {
            write_task(&dir, &task(id, status));
        }
        #[cfg(unix)]
        for id in ["queued", "running"] {
            fs::set_permissions(
                dir.join(format!("{id}.json")),
                fs::Permissions::from_mode(0o640),
            )
            .expect("set task permissions");
        }

        let unchanged_ids = ["completed", "failed", "awaiting", "interrupted"];
        let unchanged_before: Vec<_> = unchanged_ids
            .iter()
            .map(|id| fs::read_to_string(dir.join(format!("{id}.json"))).expect("read task"))
            .collect();

        assert_eq!(reconcile_interrupted_in_dir(&dir, 30).unwrap(), 2);
        for id in ["queued", "running"] {
            let raw = fs::read_to_string(dir.join(format!("{id}.json"))).expect("read task");
            let reconciled: TaskRecord = serde_json::from_str(&raw).expect("parse task");
            assert_eq!(reconciled.id, id);
            assert_eq!(reconciled.title, format!("title-{id}"));
            assert_eq!(reconciled.kind, TaskKind::Pipeline);
            assert_eq!(reconciled.stage, "production");
            assert_eq!(reconciled.user_text, format!("user-{id}"));
            assert_eq!(reconciled.ack_text, format!("ack-{id}"));
            assert_eq!(reconciled.expected_result, format!("expected-{id}"));
            assert_eq!(reconciled.job_id, Some(format!("job-{id}")));
            assert_eq!(reconciled.session, "default");
            assert_eq!(reconciled.created_at, 10);
            assert_eq!(
                reconciled.extra.get("approval_binding"),
                Some(&serde_json::json!({"id": format!("approval-{id}")}))
            );
            assert_eq!(reconciled.updated_at, 30);
            assert_eq!(reconciled.status, TaskStatus::Interrupted);
            assert_eq!(
                reconciled.error,
                Some(format!("error-{id}\n\n{INTERRUPTED_ERROR}"))
            );
            assert_eq!(
                reconciled.result_text,
                format!("result-{id}\n\n{INTERRUPTED_RESULT}")
            );
            #[cfg(unix)]
            assert_eq!(
                fs::metadata(dir.join(format!("{id}.json")))
                    .expect("read reconciled task metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o640
            );
        }
        for (id, before) in unchanged_ids.iter().zip(&unchanged_before) {
            assert_eq!(
                fs::read_to_string(dir.join(format!("{id}.json"))).expect("read unchanged task"),
                *before
            );
        }
        assert!(reconciliation_temp_paths(&dir).is_empty());

        let after_first: Vec<_> = fs::read_dir(&dir)
            .expect("list tasks")
            .map(|entry| {
                let path = entry.expect("task entry").path();
                let body = fs::read_to_string(&path).expect("read task snapshot");
                (path, body)
            })
            .collect();
        assert_eq!(reconcile_interrupted_in_dir(&dir, 40).unwrap(), 0);
        for (path, body) in after_first {
            assert_eq!(fs::read_to_string(path).expect("read task again"), body);
        }

        assert!(reconciliation_temp_paths(&dir).is_empty());
        fs::remove_dir_all(dir).expect("remove test tasks dir");
    }

    #[test]
    fn atomic_replace_failure_preserves_original_and_cleans_temp() {
        let dir = temp_tasks_dir("atomic-failure");
        fs::create_dir_all(&dir).expect("create atomic failure test dir");
        let path = dir.join("task.json");
        fs::write(&path, "original task data").expect("write original task data");

        let error = atomic_replace_with(&path, b"replacement task data", |_| {
            Err(io::Error::other("injected pre-rename failure"))
        })
        .expect_err("injected atomic replacement failure");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(
            fs::read_to_string(&path).expect("read preserved task data"),
            "original task data"
        );
        assert!(reconciliation_temp_paths(&dir).is_empty());
        fs::remove_dir_all(dir).expect("remove atomic failure test dir");
    }

    fn reconciliation_temp_paths(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .expect("list reconciliation directory")
            .filter_map(|entry| {
                let path = entry.expect("reconciliation directory entry").path();
                path.file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| name.contains(".smith-reconcile-"))
                    .map(|_| path.clone())
            })
            .collect()
    }
}
