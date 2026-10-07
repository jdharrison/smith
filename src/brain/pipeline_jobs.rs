//! Background `smith run` jobs started from chat pipeline intents.
//!
//! Jobs live under `<data_dir>/pipeline-runs/<job_id>/{meta.json,log.txt}`.
//! Ship = plan → develop, then pending release signoff (no auto-push).

use super::intent::{PipelineKind, SmithIntent};
use super::memory::{brain_memory_dir, save_pending_intent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    AwaitingSignoff,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineJobMeta {
    pub id: String,
    pub kind: String,
    pub state: JobState,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub pr: bool,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    /// Forward-compatible persisted fields that this binary does not yet understand.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

impl PipelineJobMeta {
    pub fn snapshot_summary(&self) -> String {
        if !self.summary.trim().is_empty() {
            return self.summary.clone();
        }
        match self.state {
            JobState::Queued => format!("Pipeline {} queued ({})", self.kind, self.id),
            JobState::Running => format!("Pipeline {} running ({})", self.kind, self.id),
            JobState::Succeeded => format!("Pipeline {} finished ({})", self.kind, self.id),
            JobState::Failed => format!(
                "Pipeline {} failed ({}): {}",
                self.kind,
                self.id,
                self.error.as_deref().unwrap_or("unknown error")
            ),
            JobState::AwaitingSignoff => format!(
                "Develop finished for {}. Affirm to release/push (job {}).",
                self.plan.as_deref().unwrap_or("plan"),
                self.id
            ),
            JobState::Interrupted => format!(
                "Pipeline {} was interrupted by a Smith supervisor restart ({}). Review the preserved job details and retry when ready.",
                self.kind, self.id
            ),
        }
    }
}

pub fn pipeline_runs_dir() -> Result<PathBuf, String> {
    let brain = brain_memory_dir()?;
    let parent = brain
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| brain.clone());
    Ok(parent.join("pipeline-runs"))
}

fn job_dir(id: &str) -> Result<PathBuf, String> {
    Ok(pipeline_runs_dir()?.join(id))
}

fn meta_path(id: &str) -> Result<PathBuf, String> {
    Ok(job_dir(id)?.join("meta.json"))
}

fn log_path(id: &str) -> Result<PathBuf, String> {
    Ok(job_dir(id)?.join("log.txt"))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_job_id() -> String {
    format!("job-{}", now_unix())
}

fn write_meta(meta: &PipelineJobMeta) -> Result<(), String> {
    let dir = job_dir(&meta.id)?;
    fs::create_dir_all(&dir).map_err(|e| format!("pipeline-runs dir: {e}"))?;
    let body =
        serde_json::to_string_pretty(meta).map_err(|e| format!("serialize job meta: {e}"))?;
    fs::write(meta_path(&meta.id)?, body).map_err(|e| format!("write job meta: {e}"))
}

fn load_meta(id: &str) -> Result<PipelineJobMeta, String> {
    let raw = fs::read_to_string(meta_path(id)?).map_err(|e| format!("read job meta: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse job meta: {e}"))
}

fn update_meta<F>(id: &str, f: F) -> Result<PipelineJobMeta, String>
where
    F: FnOnce(&mut PipelineJobMeta),
{
    let mut meta = load_meta(id)?;
    f(&mut meta);
    meta.updated_at = now_unix();
    write_meta(&meta)?;
    Ok(meta)
}

const INTERRUPTED_ERROR: &str =
    "Smith restarted before this pipeline job finished; review the preserved job details and retry when ready.";
const INTERRUPTED_SUMMARY: &str =
    "Recovery required: this pipeline job was interrupted by a Smith supervisor restart. Review its preserved details and retry when ready.";
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
    reconcile_interrupted_in_dir(&pipeline_runs_dir()?, now_unix())
}

fn reconcile_interrupted_in_dir(root: &Path, reconciled_at: u64) -> Result<usize, String> {
    if !root.is_dir() {
        return Ok(0);
    }

    let mut reconciled = 0;
    for entry in
        fs::read_dir(root).map_err(|e| format!("list pipeline jobs for reconciliation: {e}"))?
    {
        let entry = entry.map_err(|e| format!("pipeline job reconciliation entry: {e}"))?;
        let meta_file = entry.path().join("meta.json");
        if !meta_file.is_file() {
            continue;
        }

        let name = entry
            .file_name()
            .to_str()
            .unwrap_or("<unknown>")
            .to_string();
        let raw = fs::read_to_string(&meta_file)
            .map_err(|e| format!("read persisted pipeline job {name} for reconciliation: {e}"))?;
        let mut meta: PipelineJobMeta = serde_json::from_str(&raw)
            .map_err(|e| format!("parse persisted pipeline job {name} for reconciliation: {e}"))?;
        if !matches!(meta.state, JobState::Queued | JobState::Running) {
            continue;
        }

        meta.state = JobState::Interrupted;
        meta.error = append_recovery_error(meta.error.take(), INTERRUPTED_ERROR);
        meta.summary = append_recovery_guidance(&meta.summary, INTERRUPTED_SUMMARY);
        meta.updated_at = reconciled_at;
        let body = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("serialize interrupted pipeline job {name}: {e}"))?;
        atomic_replace(&meta_file, body.as_bytes())
            .map_err(|e| format!("persist interrupted pipeline job {name}: {e}"))?;
        reconciled += 1;
    }
    Ok(reconciled)
}

pub fn latest_job() -> Result<Option<PipelineJobMeta>, String> {
    let root = pipeline_runs_dir()?;
    if !root.is_dir() {
        return Ok(None);
    }
    let mut best: Option<PipelineJobMeta> = None;
    for entry in fs::read_dir(&root).map_err(|e| format!("list pipeline-runs: {e}"))? {
        let entry = entry.map_err(|e| format!("pipeline-runs entry: {e}"))?;
        let meta_file = entry.path().join("meta.json");
        if !meta_file.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&meta_file) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<PipelineJobMeta>(&raw) else {
            continue;
        };
        match &best {
            None => best = Some(meta),
            Some(prev) if meta.updated_at >= prev.updated_at => best = Some(meta),
            _ => {}
        }
    }
    Ok(best)
}

pub fn job_status(id: &str) -> Result<PipelineJobMeta, String> {
    load_meta(id)
}

/// Start a background pipeline job from a run-phase intent. Returns initial meta.
pub fn start_from_intent(intent: &SmithIntent) -> Result<PipelineJobMeta, String> {
    let kind = intent.pipeline_kind();
    if matches!(
        kind,
        PipelineKind::None | PipelineKind::Unknown | PipelineKind::Status
    ) {
        return Err("pipeline kind required (plan|develop|review|release|ship)".to_string());
    }

    let project = intent.project_trimmed();
    let branch = intent.branch_trimmed();
    let task = intent.task_trimmed();
    let plan = intent.plan_trimmed();
    let prompt = intent.query_trimmed().or_else(|| task.clone());

    match kind {
        PipelineKind::Ship | PipelineKind::Plan => {
            if project.is_none() || branch.is_none() || prompt.is_none() {
                return Err(
                    "ship/plan needs project, branch, and query (feature prompt)".to_string(),
                );
            }
        }
        PipelineKind::Develop => {
            if project.is_none() || branch.is_none() || plan.is_none() || task.is_none() {
                return Err("develop needs project, branch, plan, and task".to_string());
            }
        }
        PipelineKind::Release => {
            if project.is_none() || branch.is_none() || plan.is_none() {
                return Err("release needs project, branch, and plan".to_string());
            }
        }
        PipelineKind::Review if project.is_none() || branch.is_none() => {
            return Err("review needs project and branch".to_string());
        }
        _ => {}
    }

    let id = new_job_id();
    let mut meta = PipelineJobMeta {
        id: id.clone(),
        kind: kind_str(&kind).to_string(),
        state: JobState::Queued,
        project: project.clone(),
        branch: branch.clone(),
        base: intent.base.clone().filter(|s| !s.trim().is_empty()),
        plan: plan.clone(),
        task: task.clone().or_else(|| prompt.clone()),
        pr: intent.pr,
        summary: format!("Starting {}…", kind_str(&kind)),
        error: None,
        created_at: now_unix(),
        updated_at: now_unix(),
        extra: BTreeMap::new(),
    };
    write_meta(&meta)?;
    let _ = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(log_path(&id)?)
        .map_err(|e| format!("create job log: {e}"))?;

    let intent_clone = intent.clone();
    let job_id = id.clone();
    thread::spawn(move || {
        if let Err(e) = run_job_thread(&job_id, &intent_clone) {
            let _ = update_meta(&job_id, |m| {
                m.state = JobState::Failed;
                m.error = Some(e.clone());
                m.summary = format!("Pipeline failed: {e}");
            });
            let _ = append_log(&job_id, &format!("ERROR: {e}\n"));
        }
    });

    meta.state = JobState::Running;
    meta.summary = format!(
        "Started {} ({}). I'll keep working in the background — ask for status anytime.",
        kind_str(&kind),
        id
    );
    let _ = write_meta(&meta);
    Ok(meta)
}

fn kind_str(k: &PipelineKind) -> &'static str {
    match k {
        PipelineKind::Plan => "plan",
        PipelineKind::Develop => "develop",
        PipelineKind::Review => "review",
        PipelineKind::Release => "release",
        PipelineKind::Ship => "ship",
        PipelineKind::Status => "status",
        PipelineKind::None | PipelineKind::Unknown => "pipeline",
    }
}

fn run_job_thread(job_id: &str, intent: &SmithIntent) -> Result<(), String> {
    let _ = update_meta(job_id, |m| {
        m.state = JobState::Running;
        m.summary = format!("{} running…", m.kind);
    });

    let project = intent.project_trimmed().ok_or("missing project")?;
    let branch = intent.branch_trimmed().ok_or("missing branch")?;
    let base = intent
        .base
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match intent.pipeline_kind() {
        PipelineKind::Plan => {
            let prompt = intent.query_trimmed().ok_or("missing plan prompt")?;
            run_cli_plan(job_id, &project, &branch, &prompt)?;
            let plan_id = newest_plan_id(&project, &branch, &HashSet::new())?;
            update_meta(job_id, |m| {
                m.state = JobState::Succeeded;
                m.plan = Some(plan_id.clone());
                m.summary = format!("Plan ready ({plan_id}). Say when to develop or ship next.");
            })?;
        }
        PipelineKind::Develop => {
            let plan = intent.plan_trimmed().ok_or("missing plan")?;
            let task = intent.task_trimmed().ok_or("missing task")?;
            run_cli_develop(job_id, &project, &branch, base, &plan, &task, intent.pr)?;
            finish_develop_signoff(job_id, &project, &branch, base, &plan, intent.pr)?;
        }
        PipelineKind::Review => {
            let plan = intent.plan_trimmed();
            run_cli_review(job_id, &project, &branch, plan.as_deref())?;
            update_meta(job_id, |m| {
                m.state = JobState::Succeeded;
                m.summary = "Review finished. See details in the job log if needed.".to_string();
            })?;
        }
        PipelineKind::Release => {
            let plan = intent.plan_trimmed().ok_or("missing plan")?;
            run_cli_release(job_id, &project, &branch, base, &plan, intent.pr)?;
            update_meta(job_id, |m| {
                m.state = JobState::Succeeded;
                m.summary = format!("Release finished for {plan}.");
            })?;
        }
        PipelineKind::Ship => {
            let prompt = intent.task_trimmed().ok_or("missing ship prompt/task")?;
            let before: HashSet<String> = crate::docker::list_spawn_plan_dirs(&project, &branch)
                .unwrap_or_default()
                .into_iter()
                .collect();
            // Ensure spawn exists via plan (plan auto-starts container).
            run_cli_plan(job_id, &project, &branch, &prompt)?;
            let plan_id = newest_plan_id(&project, &branch, &before)?;
            let _ = update_meta(job_id, |m| {
                m.plan = Some(plan_id.clone());
                m.summary = format!("Plan {plan_id} created; developing…");
            });
            run_cli_develop(
                job_id, &project, &branch, base, &plan_id, &prompt, intent.pr,
            )?;
            finish_develop_signoff(job_id, &project, &branch, base, &plan_id, intent.pr)?;
        }
        _ => return Err("unsupported pipeline kind".to_string()),
    }
    Ok(())
}

fn finish_develop_signoff(
    job_id: &str,
    project: &str,
    branch: &str,
    base: Option<&str>,
    plan: &str,
    pr: bool,
) -> Result<(), String> {
    let ask =
        format!("Develop finished for {plan} on {project}/{branch}. Sign off to release/push?");
    let pending = SmithIntent {
        v: 1,
        action: super::intent::IntentAction::Pipeline,
        phase: super::intent::IntentPhase::Ask,
        ask: Some(ask.clone()),
        kind: Some(PipelineKind::Release),
        project: Some(project.to_string()),
        branch: Some(branch.to_string()),
        base: base.map(|s| s.to_string()),
        plan: Some(plan.to_string()),
        pr,
        ..Default::default()
    };
    save_pending_intent(&pending)?;
    update_meta(job_id, |m| {
        m.state = JobState::AwaitingSignoff;
        m.plan = Some(plan.to_string());
        m.summary = ask;
    })?;
    Ok(())
}

fn newest_plan_id(project: &str, branch: &str, before: &HashSet<String>) -> Result<String, String> {
    // Plan may still be warming the container; retry briefly.
    for _ in 0..30 {
        let dirs = crate::docker::list_spawn_plan_dirs(project, branch).unwrap_or_default();
        let mut new: Vec<_> = dirs.into_iter().filter(|d| !before.contains(d)).collect();
        if new.is_empty() {
            // Fall back to any plan dir if we couldn't diff (first plan).
            let all = crate::docker::list_spawn_plan_dirs(project, branch).unwrap_or_default();
            if let Some(last) = all.into_iter().max() {
                return Ok(last);
            }
        } else {
            new.sort();
            if let Some(last) = new.pop() {
                return Ok(last);
            }
        }
        thread::sleep(std::time::Duration::from_secs(2));
    }
    Err("could not determine new plan id after plan run".to_string())
}

fn smith_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("current_exe: {e}"))
}

fn append_log(job_id: &str, line: &str) -> Result<(), String> {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(job_id)?)
        .map_err(|e| format!("open log: {e}"))?;
    f.write_all(line.as_bytes())
        .map_err(|e| format!("write log: {e}"))
}

fn run_smith_args(job_id: &str, args: &[String]) -> Result<(), String> {
    let exe = smith_exe()?;
    append_log(job_id, &format!("$ {} {}\n", exe.display(), args.join(" ")))?;
    let log = log_path(job_id)?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|e| format!("open log for child: {e}"))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("clone log: {e}"))?;

    let status = Command::new(&exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err))
        .status()
        .map_err(|e| format!("spawn smith: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "smith {} exited {}",
            args.first().cloned().unwrap_or_default(),
            status.code().unwrap_or(-1)
        ))
    }
}

fn run_cli_plan(job_id: &str, project: &str, branch: &str, prompt: &str) -> Result<(), String> {
    run_smith_args(
        job_id,
        &[
            "run".into(),
            "plan".into(),
            prompt.into(),
            "--project".into(),
            project.into(),
            "--branch".into(),
            branch.into(),
        ],
    )
}

fn run_cli_develop(
    job_id: &str,
    project: &str,
    branch: &str,
    base: Option<&str>,
    plan: &str,
    task: &str,
    pr: bool,
) -> Result<(), String> {
    let mut args = vec![
        "run".into(),
        "develop".into(),
        task.into(),
        "--project".into(),
        project.into(),
        "--branch".into(),
        branch.into(),
        "--plan".into(),
        plan.into(),
    ];
    if let Some(b) = base {
        args.push("--base".into());
        args.push(b.into());
    }
    if pr {
        args.push("--pr".into());
    }
    run_smith_args(job_id, &args)
}

fn run_cli_review(
    job_id: &str,
    project: &str,
    branch: &str,
    plan: Option<&str>,
) -> Result<(), String> {
    let mut args = vec![
        "run".into(),
        "review".into(),
        "--project".into(),
        project.into(),
        "--branch".into(),
        branch.into(),
    ];
    if let Some(p) = plan {
        args.push("--plan".into());
        args.push(p.into());
    }
    run_smith_args(job_id, &args)
}

fn run_cli_release(
    job_id: &str,
    project: &str,
    branch: &str,
    base: Option<&str>,
    plan: &str,
    pr: bool,
) -> Result<(), String> {
    let mut args = vec![
        "run".into(),
        "release".into(),
        "--project".into(),
        project.into(),
        "--branch".into(),
        branch.into(),
        "--plan".into(),
        plan.into(),
    ];
    if let Some(b) = base {
        args.push("--base".into());
        args.push(b.into());
    }
    if pr {
        args.push("--pr".into());
    }
    run_smith_args(job_id, &args)
}

/// Format a user-facing status line for chat.
pub fn format_status(meta: &PipelineJobMeta) -> String {
    let mut lines = vec![meta.snapshot_summary()];
    lines.push(format!(
        "Job {} · {} · state={:?}",
        meta.id, meta.kind, meta.state
    ));
    if let Some(ref p) = meta.project {
        if let Some(ref b) = meta.branch {
            lines.push(format!("Workspace: {p}/{b}"));
        }
    }
    if let Some(ref plan) = meta.plan {
        lines.push(format!("Plan: {plan}"));
    }
    if let Ok(log) = log_path(&meta.id) {
        if log.is_file() {
            lines.push(format!("Log: {}", display_path(&log)));
        }
    }
    lines.join("\n")
}

fn display_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn temp_jobs_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "smith-pipeline-jobs-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn job(id: &str, state: JobState) -> PipelineJobMeta {
        PipelineJobMeta {
            id: id.to_string(),
            kind: "develop".to_string(),
            state,
            project: Some("project".to_string()),
            branch: Some("feature/task".to_string()),
            base: Some("base-revision".to_string()),
            plan: Some("plan-id".to_string()),
            task: Some(format!("task-{id}")),
            pr: true,
            summary: format!("summary-{id}"),
            error: Some(format!("error-{id}")),
            created_at: 10,
            updated_at: 20,
            extra: BTreeMap::from([(
                "approval_binding".to_string(),
                serde_json::json!({"id": format!("approval-{id}")}),
            )]),
        }
    }

    fn write_job(root: &Path, job: &PipelineJobMeta) {
        let dir = root.join(&job.id);
        fs::create_dir_all(&dir).expect("create test job dir");
        let body = serde_json::to_string_pretty(job).expect("serialize test job");
        fs::write(dir.join("meta.json"), body).expect("write test job");
        fs::write(dir.join("log.txt"), format!("log-{}", job.id)).expect("write test log");
    }

    #[test]
    fn job_state_serde_preserves_existing_labels_and_adds_interrupted() {
        let cases = [
            ("queued", JobState::Queued),
            ("running", JobState::Running),
            ("succeeded", JobState::Succeeded),
            ("failed", JobState::Failed),
            ("awaiting_signoff", JobState::AwaitingSignoff),
            ("interrupted", JobState::Interrupted),
        ];

        for (label, state) in cases {
            assert_eq!(
                serde_json::from_str::<JobState>(&format!("\"{label}\""))
                    .expect("deserialize state"),
                state
            );
            assert_eq!(
                serde_json::to_string(&state).expect("serialize state"),
                format!("\"{label}\"")
            );
        }
    }

    #[test]
    fn reconciliation_interrupts_only_queued_and_running_jobs_once() {
        let root = temp_jobs_dir("reconcile");
        fs::create_dir_all(&root).expect("create test jobs root");
        let states = [
            ("queued", JobState::Queued),
            ("running", JobState::Running),
            ("succeeded", JobState::Succeeded),
            ("failed", JobState::Failed),
            ("awaiting", JobState::AwaitingSignoff),
            ("interrupted", JobState::Interrupted),
        ];
        for (id, state) in states {
            write_job(&root, &job(id, state));
        }
        #[cfg(unix)]
        for id in ["queued", "running"] {
            fs::set_permissions(
                root.join(id).join("meta.json"),
                fs::Permissions::from_mode(0o640),
            )
            .expect("set job metadata permissions");
        }

        let unchanged_ids = ["succeeded", "failed", "awaiting", "interrupted"];
        let unchanged_before: Vec<_> = unchanged_ids
            .iter()
            .map(|id| fs::read_to_string(root.join(id).join("meta.json")).expect("read job meta"))
            .collect();

        assert_eq!(reconcile_interrupted_in_dir(&root, 30).unwrap(), 2);
        for id in ["queued", "running"] {
            let raw = fs::read_to_string(root.join(id).join("meta.json")).expect("read job meta");
            let reconciled: PipelineJobMeta = serde_json::from_str(&raw).expect("parse job meta");
            assert_eq!(reconciled.id, id);
            assert_eq!(reconciled.kind, "develop");
            assert_eq!(reconciled.project.as_deref(), Some("project"));
            assert_eq!(reconciled.branch.as_deref(), Some("feature/task"));
            assert_eq!(reconciled.base.as_deref(), Some("base-revision"));
            assert_eq!(reconciled.plan.as_deref(), Some("plan-id"));
            assert_eq!(reconciled.task, Some(format!("task-{id}")));
            assert!(reconciled.pr);
            assert_eq!(reconciled.created_at, 10);
            assert_eq!(
                reconciled.extra.get("approval_binding"),
                Some(&serde_json::json!({"id": format!("approval-{id}")}))
            );
            assert_eq!(reconciled.updated_at, 30);
            assert_eq!(reconciled.state, JobState::Interrupted);
            assert_eq!(
                reconciled.error,
                Some(format!("error-{id}\n\n{INTERRUPTED_ERROR}"))
            );
            assert_eq!(
                reconciled.summary,
                format!("summary-{id}\n\n{INTERRUPTED_SUMMARY}")
            );
            #[cfg(unix)]
            assert_eq!(
                fs::metadata(root.join(id).join("meta.json"))
                    .expect("read reconciled job metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o640
            );
            assert_eq!(
                fs::read_to_string(root.join(id).join("log.txt")).expect("read preserved log"),
                format!("log-{id}")
            );
        }
        for (id, before) in unchanged_ids.iter().zip(&unchanged_before) {
            assert_eq!(
                fs::read_to_string(root.join(id).join("meta.json"))
                    .expect("read unchanged job meta"),
                *before
            );
        }
        for id in ["queued", "running"] {
            assert!(reconciliation_temp_paths(&root.join(id)).is_empty());
        }

        let after_first: Vec<_> = fs::read_dir(&root)
            .expect("list jobs")
            .map(|entry| {
                let path = entry.expect("job entry").path().join("meta.json");
                let body = fs::read_to_string(&path).expect("read job snapshot");
                (path, body)
            })
            .collect();
        assert_eq!(reconcile_interrupted_in_dir(&root, 40).unwrap(), 0);
        for (path, body) in after_first {
            assert_eq!(fs::read_to_string(path).expect("read job again"), body);
        }

        for id in ["queued", "running"] {
            assert!(reconciliation_temp_paths(&root.join(id)).is_empty());
        }
        fs::remove_dir_all(root).expect("remove test jobs root");
    }

    #[test]
    fn atomic_replace_failure_preserves_original_and_cleans_temp() {
        let root = temp_jobs_dir("atomic-failure");
        fs::create_dir_all(&root).expect("create atomic failure test dir");
        let path = root.join("meta.json");
        fs::write(&path, "original job data").expect("write original job data");

        let error = atomic_replace_with(&path, b"replacement job data", |_| {
            Err(io::Error::other("injected pre-rename failure"))
        })
        .expect_err("injected atomic replacement failure");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(
            fs::read_to_string(&path).expect("read preserved job data"),
            "original job data"
        );
        assert!(reconciliation_temp_paths(&root).is_empty());
        fs::remove_dir_all(root).expect("remove atomic failure test dir");
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
