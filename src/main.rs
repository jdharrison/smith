mod commands;
mod docker;
mod github;

use clap::{CommandFactory, Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// ANSI color codes for status output (circle: red = not built, blue = built, green = online).
const ANSI_RED: &str = "\x1b[31m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RESET: &str = "\x1b[0m";

/// Colored bullet for status/install output (avoids format! in format args).
const BULLET_GREEN: &str = "\x1b[32m●\x1b[0m";
const BULLET_BLUE: &str = "\x1b[34m●\x1b[0m";
const BULLET_RED: &str = "\x1b[31m●\x1b[0m";
const BULLET_YELLOW: &str = "\x1b[33m●\x1b[0m";

/// OSC 8 hyperlink so the URL is clickable in supported terminals (e.g. VS Code, iTerm2, Windows Terminal).
fn clickable_agent_url(port: u16) -> String {
    let url = format!("http://localhost:{}", port);
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, url)
}

/// active = container running, reachable = health endpoint responds (only meaningful when active).
/// is_cloud = cloud agents are always green (no build needed).
/// Local agents: red (none) -> blue (built) -> green (online).
fn status_circle(active: bool, reachable: Option<bool>, built: bool, is_cloud: bool) -> String {
    let bullet = "●";
    let color = if active && reachable == Some(false) {
        ANSI_YELLOW
    } else if active || is_cloud {
        ANSI_GREEN
    } else if built {
        ANSI_BLUE
    } else {
        ANSI_RED
    };
    format!("{}{}{}", color, bullet, ANSI_RESET)
}

fn is_valid_short_plan_id(value: &str) -> bool {
    value.len() == 4
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn short_plan_id_from_dir_name(plan_dir: &str) -> String {
    if let Some(rest) = plan_dir.strip_prefix("plan-") {
        if is_valid_short_plan_id(rest) {
            return rest.to_string();
        }
    }

    let mut h = 1469598103934665603u64;
    for b in plan_dir.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(4);
    for _ in 0..4 {
        out.push(ALPHABET[(h % 36) as usize] as char);
        h /= 36;
    }
    out
}

fn generate_short_plan_id(attempt: u64) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut n =
        nanos ^ ((std::process::id() as u64) << 16) ^ attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15);

    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(4);
    for _ in 0..4 {
        let idx = (n % 36) as usize;
        out.push(ALPHABET[idx] as char);
        n /= 36;
        if n == 0 {
            n = n.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
    }
    out
}

fn resolve_plan_id_filter(filter: &str, plan_dirs: &[String]) -> Result<String, String> {
    if plan_dirs.iter().any(|d| d == filter) {
        return Ok(filter.to_string());
    }

    let as_prefixed = format!("plan-{}", filter);
    if plan_dirs.iter().any(|d| d == &as_prefixed) {
        return Ok(as_prefixed);
    }

    let target = filter.to_lowercase();
    let matches = plan_dirs
        .iter()
        .filter(|d| short_plan_id_from_dir_name(d) == target)
        .cloned()
        .collect::<Vec<_>>();

    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => Err(format!("No plan found matching '{}'", filter)),
        _ => Err(format!(
            "Multiple plans match '{}': {}",
            filter,
            matches.join(", ")
        )),
    }
}

const CORE_ROLE_NAMES: &[&str] = &[
    "producer",
    "architect",
    "designer",
    "planner",
    "developer",
    "assurance",
    "devops",
];

#[derive(Clone, Default)]
struct PipelineRoles {
    setup_run: Option<RoleInfo>,
    setup_check: Option<RoleInfo>,
    execute_run: Option<RoleInfo>,
    execute_check: Option<RoleInfo>,
    validate_run: Option<RoleInfo>,
    validate_check: Option<RoleInfo>,
    commit_run: Option<RoleInfo>,
    commit_check: Option<RoleInfo>,
}

#[derive(Clone)]
struct RoleInfo {
    model: Option<String>,
    prompt: Option<String>,
}

impl RoleInfo {
    fn new(model: Option<String>, prompt: Option<String>) -> Self {
        Self { model, prompt }
    }
}

fn normalize_role_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn is_core_role(name: &str) -> bool {
    let normalized = normalize_role_name(name);
    CORE_ROLE_NAMES.iter().any(|core| *core == normalized)
}

fn validate_role_name(name: &str) -> Result<String, String> {
    let normalized = normalize_role_name(name);
    if normalized.is_empty() {
        return Err("Role name cannot be empty".to_string());
    }
    if normalized.contains('/') || normalized.contains('\\') {
        return Err("Role name cannot contain path separators".to_string());
    }
    if normalized.contains("..") {
        return Err("Role name cannot contain '..'".to_string());
    }
    if !normalized
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err("Role name must use lowercase letters, digits, '-' or '_'".to_string());
    }
    Ok(normalized)
}

fn opencode_roles_dir() -> Result<PathBuf, String> {
    let base =
        dirs::config_dir().ok_or_else(|| "Could not determine config directory".to_string())?;
    Ok(base.join("opencode").join("agents"))
}

fn role_file_path(name: &str) -> Result<PathBuf, String> {
    let normalized = validate_role_name(name)?;
    Ok(opencode_roles_dir()?.join(format!("{}.md", normalized)))
}

fn role_content_has_subagent_mode(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("mode: subagent")
}

fn list_role_files() -> Result<Vec<(String, PathBuf)>, String> {
    let dir = opencode_roles_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut roles = Vec::new();
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read roles directory '{}': {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed reading roles directory entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = normalize_role_name(stem);
        if !name.is_empty() {
            roles.push((name, path));
        }
    }
    roles.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(roles)
}

fn list_role_files_in_dir(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    if !dir.exists() {
        return Err(format!(
            "Role source directory does not exist: {}",
            dir.display()
        ));
    }
    if !dir.is_dir() {
        return Err(format!(
            "Role source path is not a directory: {}",
            dir.display()
        ));
    }

    let mut roles = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| {
        format!(
            "Failed to read roles source directory '{}': {}",
            dir.display(),
            e
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("Failed reading roles source directory entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let normalized = validate_role_name(stem)?;
        roles.push((normalized, path));
    }

    if roles.is_empty() {
        return Err(format!("No role markdown files found in {}", dir.display()));
    }

    roles.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(roles)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone)]
struct PlanArtifacts {
    producer: String,
    architect: String,
    designer: String,
    planner: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct PlanIssue {
    id: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PlanReply {
    submitted_at_unix: u64,
    text: String,
}

#[derive(Clone)]
struct AssurancePreview {
    verdict: String,
    summary: Vec<String>,
    blocking_findings: usize,
    total_findings: usize,
}

#[derive(Serialize, Deserialize, Clone)]
struct PlanManifest {
    plan_id: String,
    #[serde(default)]
    short_id: String,
    version: u8,
    project: String,
    branch: String,
    prompt: String,
    state: String,
    phase: String,
    created_at_unix: u64,
    updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at_unix: Option<u64>,
    artifacts: PlanArtifacts,
    role_status: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    summary: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    issues: Vec<PlanIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    replies: Vec<PlanReply>,
    errors: Vec<String>,
}

impl PlanManifest {
    fn new(plan_id: String, project: String, branch: String, prompt: String) -> Self {
        let now = now_unix();
        let short_id = short_plan_id_from_dir_name(&plan_id);
        let mut role_status = HashMap::new();
        role_status.insert("producer".to_string(), "not_started".to_string());
        role_status.insert("architect".to_string(), "not_started".to_string());
        role_status.insert("designer".to_string(), "not_started".to_string());
        role_status.insert("planner".to_string(), "not_started".to_string());

        Self {
            artifacts: PlanArtifacts {
                producer: "producer.json".to_string(),
                architect: "architect.json".to_string(),
                designer: "designer.json".to_string(),
                planner: "planner.json".to_string(),
            },
            plan_id,
            short_id,
            version: 1,
            project,
            branch,
            prompt,
            state: "planning".to_string(),
            phase: "init".to_string(),
            created_at_unix: now,
            updated_at_unix: now,
            completed_at_unix: None,
            role_status,
            summary: Vec::new(),
            issues: Vec::new(),
            replies: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn set_state(&mut self, state: &str, phase: &str) {
        self.state = state.to_string();
        self.phase = phase.to_string();
        self.updated_at_unix = now_unix();
        if state == "completed" || state == "failed" {
            self.completed_at_unix = Some(self.updated_at_unix);
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct DevCoverageItem {
    id: String,
    status: String,
    evidence: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct DevCoverageReport {
    #[serde(default)]
    requirements: Vec<DevCoverageItem>,
    #[serde(default)]
    acceptance_criteria: Vec<DevCoverageItem>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DevAssuranceIssue {
    id: String,
    severity: String,
    title: String,
    detail: String,
    #[serde(default)]
    related_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DevAssuranceReport {
    schema_version: u8,
    verdict: String,
    #[serde(default)]
    summary: Vec<String>,
    #[serde(default)]
    coverage: DevCoverageReport,
    #[serde(default)]
    blocking_issues: Vec<DevAssuranceIssue>,
    #[serde(default)]
    non_blocking_issues: Vec<DevAssuranceIssue>,
    #[serde(default)]
    required_remediation: Vec<String>,
    generated_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct DevAttemptRecord {
    attempt: u32,
    develop_artifact: String,
    assurance_artifact: String,
    verdict: String,
    blocking_issues: usize,
    non_blocking_issues: usize,
}

#[derive(Serialize, Deserialize, Clone)]
struct DevRunManifest {
    dev_run_id: String,
    version: u8,
    project: String,
    branch: String,
    base: String,
    plan_id: String,
    short_plan_id: String,
    task: String,
    state: String,
    phase: String,
    created_at_unix: u64,
    updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at_unix: Option<u64>,
    max_validate_passes: u32,
    attempts: Vec<DevAttemptRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    non_blocking_issues: Vec<DevAssuranceIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

impl DevRunManifest {
    #[allow(clippy::too_many_arguments)]
    fn new(
        dev_run_id: String,
        project: String,
        branch: String,
        base: String,
        plan_id: String,
        short_plan_id: String,
        task: String,
        max_validate_passes: u32,
    ) -> Self {
        let now = now_unix();
        Self {
            dev_run_id,
            version: 1,
            project,
            branch,
            base,
            plan_id,
            short_plan_id,
            task,
            state: "in_progress".to_string(),
            phase: "init".to_string(),
            created_at_unix: now,
            updated_at_unix: now,
            completed_at_unix: None,
            max_validate_passes,
            attempts: Vec::new(),
            final_verdict: None,
            final_commit: None,
            non_blocking_issues: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn set_phase(&mut self, phase: &str) {
        self.phase = phase.to_string();
        self.updated_at_unix = now_unix();
    }

    fn set_state(&mut self, state: &str, phase: &str) {
        self.state = state.to_string();
        self.phase = phase.to_string();
        self.updated_at_unix = now_unix();
        if state == "completed" || state == "failed" {
            self.completed_at_unix = Some(self.updated_at_unix);
        }
    }
}

fn write_dev_manifest(
    project: &str,
    branch: &str,
    run_dir: &str,
    manifest: &DevRunManifest,
) -> Result<(), String> {
    let manifest_path = format!("{}/manifest.json", run_dir);
    let body = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize develop manifest: {}", e))?;
    docker::write_spawn_file(project, branch, &manifest_path, &body)
}

fn parse_dev_assurance_report(raw: &str) -> Result<DevAssuranceReport, String> {
    let report = serde_json::from_str::<DevAssuranceReport>(raw)
        .map_err(|e| format!("Invalid assurance artifact JSON: {}", e))?;

    if report.schema_version != 1 {
        return Err(format!(
            "Unsupported assurance schema_version '{}', expected 1",
            report.schema_version
        ));
    }
    let verdict = report.verdict.trim().to_lowercase();
    if verdict != "pass" && verdict != "pass_with_risk" && verdict != "fail" {
        return Err(format!(
            "Invalid assurance verdict '{}'; expected pass|pass_with_risk|fail",
            report.verdict
        ));
    }
    if report.generated_at.trim().is_empty() {
        return Err("Assurance report missing generated_at".to_string());
    }

    Ok(report)
}

fn planner_has_actionable_sections(planner_raw: &str) -> Result<(), String> {
    let value = serde_json::from_str::<Value>(planner_raw)
        .map_err(|e| format!("Invalid planner artifact JSON: {}", e))?;

    let has_summary = value
        .get("high_level_summary")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if !has_summary {
        return Err("Planner artifact missing non-empty high_level_summary".to_string());
    }

    let has_requirements = value
        .get("requirements")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if !has_requirements {
        return Err("Planner artifact missing non-empty requirements".to_string());
    }

    let has_acceptance = value
        .get("acceptance_criteria")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if !has_acceptance {
        return Err("Planner artifact missing non-empty acceptance_criteria".to_string());
    }

    Ok(())
}

fn unresolved_plan_issues(manifest: &PlanManifest) -> Vec<String> {
    manifest
        .issues
        .iter()
        .filter(|issue| {
            issue
                .answer
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        })
        .map(|issue| format!("{}: {}", issue.id, issue.text))
        .collect()
}

fn build_spawn_develop_prompt(
    task: &str,
    plan_dir: &str,
    execution_brief_path: &str,
    develop_artifact_path: &str,
    attempt: u32,
) -> String {
    let escaped_task = task.replace('"', "\\\"");
    format!(
        r#"Implement this task using the approved plan context: \"{task}\".

Required context files:
- Plan directory: {plan_dir}
- Execution brief JSON: {execution_brief_path}

Rules:
1) Treat the plan and execution brief as authoritative requirements.
2) Make code changes in /workspace only.
3) Before finishing, run targeted validation commands relevant to your edits.
4) Write a JSON artifact to {develop_artifact_path} with this shape:
{{
  "schema_version": 1,
  "attempt": {attempt},
  "summary": ["2-4 bullets"],
  "changed_files": ["path"],
  "validation": [{{"command": "...", "result": "pass|fail", "notes": "..."}}],
  "residual_risks": ["..."]
}}
5) Print a short completion note.

Do not skip writing the JSON artifact.
"#,
        task = escaped_task,
        plan_dir = plan_dir,
        execution_brief_path = execution_brief_path,
        develop_artifact_path = develop_artifact_path,
        attempt = attempt
    )
}

fn build_spawn_assurance_prompt(
    task: &str,
    plan_dir: &str,
    execution_brief_path: &str,
    develop_artifact_path: &str,
    assurance_artifact_path: &str,
    attempt: u32,
) -> String {
    let escaped_task = task.replace('"', "\\\"");
    format!(
        r#"Validate implementation attempt {attempt} against the approved plan.

Task: \"{task}\"
Plan directory: {plan_dir}
Execution brief: {execution_brief_path}
Developer artifact: {develop_artifact_path}

Produce a STRICT JSON artifact at {assurance_artifact_path} using this exact schema:
{{
  "schema_version": 1,
  "verdict": "pass|pass_with_risk|fail",
  "summary": ["2-4 bullets"],
  "coverage": {{
    "requirements": [{{"id": "REQ-001", "status": "covered|partial|not_covered", "evidence": "..."}}],
    "acceptance_criteria": [{{"id": "AC-001", "status": "met|partial|unmet", "evidence": "..."}}]
  }},
  "blocking_issues": [{{"id": "BLK-001", "severity": "critical|high", "title": "...", "detail": "...", "related_ids": ["REQ-001"]}}],
  "non_blocking_issues": [{{"id": "NB-001", "severity": "medium|low", "title": "...", "detail": "...", "related_ids": ["AC-001"]}}],
  "required_remediation": ["..."] ,
  "generated_at": "ISO-8601"
}}

Rules:
1) Put ALL release-blocking concerns in blocking_issues.
2) Put informational/low-risk concerns in non_blocking_issues.
3) Blocking issues must be empty only when verdict is pass or pass_with_risk.
4) Do not emit markdown or prose outside the JSON artifact.
"#,
        attempt = attempt,
        task = escaped_task,
        plan_dir = plan_dir,
        execution_brief_path = execution_brief_path,
        develop_artifact_path = develop_artifact_path,
        assurance_artifact_path = assurance_artifact_path,
    )
}

#[derive(Serialize, Deserialize, Clone)]
struct ReleaseIssue {
    id: String,
    severity: String,
    title: String,
    detail: String,
    #[serde(default)]
    related_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ReleaseReviewReport {
    schema_version: u8,
    release_ready: bool,
    #[serde(default)]
    summary: Vec<String>,
    #[serde(default)]
    blocking_issues: Vec<ReleaseIssue>,
    #[serde(default)]
    non_blocking_issues: Vec<ReleaseIssue>,
    #[serde(default)]
    evidence: Vec<String>,
    generated_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ReleaseRunManifest {
    release_run_id: String,
    version: u8,
    project: String,
    branch: String,
    base: String,
    plan_id: String,
    short_plan_id: String,
    dev_run_id: String,
    state: String,
    phase: String,
    created_at_unix: u64,
    updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at_unix: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    non_blocking_issues: Vec<ReleaseIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

impl ReleaseRunManifest {
    fn new(
        release_run_id: String,
        project: String,
        branch: String,
        base: String,
        plan_id: String,
        short_plan_id: String,
        dev_run_id: String,
    ) -> Self {
        let now = now_unix();
        Self {
            release_run_id,
            version: 1,
            project,
            branch,
            base,
            plan_id,
            short_plan_id,
            dev_run_id,
            state: "in_progress".to_string(),
            phase: "init".to_string(),
            created_at_unix: now,
            updated_at_unix: now,
            completed_at_unix: None,
            review_ready: None,
            integration_status: None,
            merge_strategy: None,
            merge_commit: None,
            non_blocking_issues: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn set_phase(&mut self, phase: &str) {
        self.phase = phase.to_string();
        self.updated_at_unix = now_unix();
    }

    fn set_state(&mut self, state: &str, phase: &str) {
        self.state = state.to_string();
        self.phase = phase.to_string();
        self.updated_at_unix = now_unix();
        if state == "completed" || state == "failed" {
            self.completed_at_unix = Some(self.updated_at_unix);
        }
    }
}

fn write_release_manifest(
    project: &str,
    branch: &str,
    run_dir: &str,
    manifest: &ReleaseRunManifest,
) -> Result<(), String> {
    let manifest_path = format!("{}/manifest.json", run_dir);
    let body = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize release manifest: {}", e))?;
    docker::write_spawn_file(project, branch, &manifest_path, &body)
}

fn parse_release_review_report(raw: &str) -> Result<ReleaseReviewReport, String> {
    let report = serde_json::from_str::<ReleaseReviewReport>(raw)
        .map_err(|e| format!("Invalid release review JSON: {}", e))?;

    if report.schema_version != 1 {
        return Err(format!(
            "Unsupported release review schema_version '{}', expected 1",
            report.schema_version
        ));
    }
    if report.generated_at.trim().is_empty() {
        return Err("Release review report missing generated_at".to_string());
    }
    if report.release_ready && !report.blocking_issues.is_empty() {
        return Err(
            "Release review report is inconsistent: release_ready=true but blocking_issues not empty"
                .to_string(),
        );
    }
    Ok(report)
}

fn build_spawn_release_review_prompt(
    task: &str,
    plan_dir: &str,
    dev_run_dir: &str,
    develop_artifact_path: &str,
    assurance_artifact_path: &str,
    review_artifact_path: &str,
) -> String {
    let escaped_task = task.replace('"', "\\\"");
    format!(
        r#"Run a release readiness review as @producer.

Task: \"{task}\"
Plan directory: {plan_dir}
Development run directory: {dev_run_dir}
Developer artifact: {develop_artifact_path}
Assurance artifact: {assurance_artifact_path}

Write STRICT JSON to {review_artifact_path} with this exact shape:
{{
  "schema_version": 1,
  "release_ready": true,
  "summary": ["2-4 bullets"],
  "blocking_issues": [{{"id": "RB-001", "severity": "critical|high", "title": "...", "detail": "...", "related_ids": ["REQ-001"]}}],
  "non_blocking_issues": [{{"id": "RN-001", "severity": "medium|low", "title": "...", "detail": "...", "related_ids": ["AC-001"]}}],
  "evidence": ["..."],
  "generated_at": "ISO-8601"
}}

Rules:
1) release_ready must be false whenever blocking_issues is non-empty.
2) Only include critical/high in blocking_issues.
3) Include specific evidence for every blocking issue.
4) Do not print markdown output outside the JSON artifact.
"#,
        task = escaped_task,
        plan_dir = plan_dir,
        dev_run_dir = dev_run_dir,
        develop_artifact_path = develop_artifact_path,
        assurance_artifact_path = assurance_artifact_path,
        review_artifact_path = review_artifact_path,
    )
}

fn build_spawn_release_sync_prompt(
    plan_dir: &str,
    review_artifact_path: &str,
    integrate_artifact_path: &str,
    sync_artifact_path: &str,
) -> String {
    format!(
        r#"Run final release sync as @planner.

Plan directory: {plan_dir}
Release review artifact: {review_artifact_path}
Integration artifact: {integrate_artifact_path}

Write STRICT JSON to {sync_artifact_path} with this shape:
{{
  "schema_version": 1,
  "status": "released|blocked|failed",
  "summary": ["2-4 bullets"],
  "updated_plan_state": "released|release_blocked|release_failed",
  "followups": ["..."],
  "generated_at": "ISO-8601"
}}

Rules:
1) status must reflect integration outcome accurately.
2) If integration failed/conflicted, status must be failed.
3) Keep summary concise and specific.
4) Do not print markdown output outside the JSON artifact.
"#,
        plan_dir = plan_dir,
        review_artifact_path = review_artifact_path,
        integrate_artifact_path = integrate_artifact_path,
        sync_artifact_path = sync_artifact_path,
    )
}

fn find_latest_completed_dev_run_for_plan(
    project: &str,
    branch: &str,
    selected_plan: &str,
) -> Result<(String, DevRunManifest), String> {
    let dirs_raw = docker::run_spawn_shell(
        project,
        branch,
        "for d in /state/dev-*; do [ -d \"$d\" ] && basename \"$d\"; done; true",
    )?;

    let mut matches: Vec<(String, DevRunManifest)> = Vec::new();
    for dir_name in dirs_raw.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let manifest_path = format!("/state/{}/manifest.json", dir_name);
        let raw = match docker::read_spawn_file(project, branch, &manifest_path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let manifest = match serde_json::from_str::<DevRunManifest>(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if manifest.plan_id == selected_plan && manifest.state == "completed" {
            matches.push((dir_name.to_string(), manifest));
        }
    }

    if matches.is_empty() {
        return Err(format!(
            "No completed develop run found for plan '{}'",
            selected_plan
        ));
    }

    matches.sort_by(|(_, a), (_, b)| b.created_at_unix.cmp(&a.created_at_unix));
    Ok(matches.remove(0))
}

fn extract_kv_line<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    raw.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
    })
}

fn write_plan_manifest(
    project: &str,
    branch: &str,
    run_dir: &str,
    manifest: &PlanManifest,
) -> Result<(), String> {
    let manifest_path = format!("{}/manifest.json", run_dir);
    let body = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize plan manifest: {}", e))?;
    docker::write_spawn_file(project, branch, &manifest_path, &body)
}

fn extract_high_level_summary_from_planner(planner_raw: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(planner_raw) else {
        return Vec::new();
    };
    let Some(items) = value.get("high_level_summary").and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(4)
        .map(ToOwned::to_owned)
        .collect()
}

fn extract_plan_issues_from_planner(planner_raw: &str) -> Vec<PlanIssue> {
    let Ok(value) = serde_json::from_str::<Value>(planner_raw) else {
        return Vec::new();
    };

    let mut out = Vec::new();

    if let Some(items) = value.get("issues").and_then(Value::as_array) {
        for (idx, item) in items.iter().enumerate() {
            if let Some(text) = item.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(PlanIssue {
                        id: format!("ISSUE-{}", idx + 1),
                        text: trimmed.to_string(),
                        answer: None,
                    });
                }
                continue;
            }

            if let Some(obj) = item.as_object() {
                let text = obj
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| obj.get("question").and_then(Value::as_str))
                    .or_else(|| obj.get("issue").and_then(Value::as_str))
                    .map(str::trim)
                    .unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                let id = obj
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("");
                out.push(PlanIssue {
                    id: if id.is_empty() {
                        format!("ISSUE-{}", idx + 1)
                    } else {
                        id.to_string()
                    },
                    text: text.to_string(),
                    answer: None,
                });
            }
        }
    }

    out
}

fn extract_assurance_preview(assurance_raw: &str) -> Option<AssurancePreview> {
    let Ok(value) = serde_json::from_str::<Value>(assurance_raw) else {
        return None;
    };

    let verdict = value
        .get("verdict")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string();

    let summary = value
        .get("summary")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .take(4)
                .map(ToOwned::to_owned)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let mut total_findings = 0usize;
    let mut blocking_findings = 0usize;
    if let Some(findings) = value.get("findings").and_then(Value::as_array) {
        total_findings = findings.len();
        for finding in findings {
            let severity = finding
                .get("severity")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_lowercase();
            if severity == "critical" || severity == "high" {
                blocking_findings += 1;
            }
        }
    }

    Some(AssurancePreview {
        verdict,
        summary,
        blocking_findings,
        total_findings,
    })
}

fn effective_short_plan_id(manifest: &PlanManifest, plan_id: &str) -> String {
    if is_valid_short_plan_id(&manifest.short_id) {
        manifest.short_id.clone()
    } else {
        short_plan_id_from_dir_name(plan_id)
    }
}

fn print_plan_block(plan_id: &str, manifest: &PlanManifest, assurance: Option<&AssurancePreview>) {
    let short_id = effective_short_plan_id(manifest, plan_id);
    println!("Plan: {} (id: {})", plan_id, short_id);
    println!("  Active: {} (phase: {})", manifest.state, manifest.phase);
    println!("  Target: {}:{}", manifest.project, manifest.branch);

    let mut role_line = Vec::new();
    for role in ["producer", "architect", "designer", "planner"] {
        let status = manifest
            .role_status
            .get(role)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        role_line.push(format!("{}={}", role, status));
    }
    println!("  Roles: {}", role_line.join(" "));
    println!("  Prompt: {}", manifest.prompt);

    println!("  Summary:");
    if manifest.summary.is_empty() {
        println!("    - unavailable");
    } else {
        for bullet in manifest.summary.iter().take(4) {
            println!("    - {}", bullet);
        }
    }

    if !manifest.errors.is_empty() {
        println!("  Errors:");
        for err in manifest.errors.iter().take(4) {
            println!("    - {}", err);
        }
    }

    if !manifest.issues.is_empty() {
        println!("  Issues:");
        for issue in manifest.issues.iter().take(8) {
            if let Some(answer) = &issue.answer {
                println!("    - {}: {} [reply: {}]", issue.id, issue.text, answer);
            } else {
                println!("    - {}: {}", issue.id, issue.text);
            }
        }
    }

    if !manifest.replies.is_empty() {
        println!("  Replies:");
        for reply in manifest.replies.iter().rev().take(2) {
            println!("    - {}", reply.text);
        }
    }

    if let Some(assurance) = assurance {
        println!(
            "  Assurance: verdict={} findings={} blocking={}",
            assurance.verdict, assurance.total_findings, assurance.blocking_findings
        );
        if assurance.summary.is_empty() {
            println!("    - summary unavailable");
        } else {
            for bullet in assurance.summary.iter().take(4) {
                println!("    - {}", bullet);
            }
        }
    }
}

fn plan_id_timestamp(plan_id: &str) -> Option<u64> {
    let mut parts = plan_id.split('-');
    match (parts.next(), parts.next()) {
        (Some("plan"), Some(ts)) => ts.parse::<u64>().ok(),
        _ => None,
    }
}

fn format_plan_role_progress(done: bool) -> &'static str {
    if done {
        "done"
    } else {
        "..."
    }
}

fn spawn_plan_progress_tracker(
    project: String,
    branch: String,
    run_dir: String,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let started = Instant::now();
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }

            let producer_done =
                docker::spawn_file_exists(&project, &branch, &format!("{}/producer.json", run_dir))
                    .unwrap_or(false);
            let architect_done = docker::spawn_file_exists(
                &project,
                &branch,
                &format!("{}/architect.json", run_dir),
            )
            .unwrap_or(false);
            let designer_done =
                docker::spawn_file_exists(&project, &branch, &format!("{}/designer.json", run_dir))
                    .unwrap_or(false);
            let planner_done =
                docker::spawn_file_exists(&project, &branch, &format!("{}/planner.json", run_dir))
                    .unwrap_or(false);

            let elapsed = started.elapsed().as_secs_f32();
            print!(
                "\r\x1b[2K  {} Plan runtime: {:>5.1}s | producer:{} architect:{} designer:{} planner:{}",
                BULLET_BLUE,
                elapsed,
                format_plan_role_progress(producer_done),
                format_plan_role_progress(architect_done),
                format_plan_role_progress(designer_done),
                format_plan_role_progress(planner_done)
            );
            let _ = io::stdout().flush();

            thread::sleep(Duration::from_millis(1000));
        }
    })
}

fn build_spawn_plan_prompt(user_prompt: &str, run_dir: &str) -> String {
    let escaped_prompt = user_prompt.replace('"', "\\\"");
    format!(
        r#"You are coordinating planning agents for this request: "{prompt}".

Write all role outputs as JSON files for inter-agent communication.
Use bash commands to write files under /state (do not use the Write tool for /state paths).

Requirements:
1) Ensure directory exists: {run_dir}
1b) Update {run_dir}/manifest.json at phase boundaries with state/phase progress.
2) Run @producer using the original request and save JSON to {run_dir}/producer.json
3) Run @architect using the original request and save JSON to {run_dir}/architect.json
4) Run @designer using the original request and save JSON to {run_dir}/designer.json
5) Run @planner using the original request + producer/architect/designer outputs and save JSON to {run_dir}/planner.json

JSON shape (simple, same for all files):
{{
  "role": "producer|architect|designer|planner",
  "created_at": "ISO-8601",
  "source_prompt": "...",
  "content": "full role output markdown/text",
  "high_level_summary": ["2-4 short bullets"],
  "issues": [{{"id": "ISSUE-1", "text": "open question for user/orchestrator"}}],
  "requirements": [{{"id": "REQ-001", "text": "..."}}],
  "acceptance_criteria": [{{"id": "AC-001", "text": "..."}}],
  "handoff": "summary for downstream role"
}}

Use empty arrays when requirements/acceptance_criteria are not applicable.
For planner.json, high_level_summary is required and must contain 2-4 bullets.
For planner.json, include issues as an array of unresolved decisions/questions (can be empty).

Finally print:
- absolute file paths written
- a one-line status per role (ok/failed)
- a final summary for the user.
"#,
        prompt = escaped_prompt,
        run_dir = run_dir
    )
}

#[derive(Parser)]
#[command(name = "smith")]
#[command(about = "smith — open-source control plane for local agent orchestration", long_about = None)]
#[command(disable_help_flag = true)]
#[command(disable_version_flag = true)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Dependencies, agents, projects
    #[command(next_help_heading = "System")]
    Status {
        /// Show raw output (config path, docker version/info, agent details)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Docker and config setup
    Install,
    /// Remove all data, containers, and optionally, smith entirely.
    Uninstall {
        /// Skip confirmation prompt (still prompts for config removal unless --remove-config)
        #[arg(short, long)]
        force: bool,
        /// Also remove the config directory (~/.config/smith)
        #[arg(long)]
        remove_config: bool,
        /// Also remove Docker images built by smith
        #[arg(long)]
        remove_images: bool,
    },
    /// Print help
    Help,
    /// Print version
    Version,
    #[command(next_help_heading = "Commands")]
    /// Local model/provider runtimes
    Model {
        #[command(subcommand)]
        cmd: ModelCommands,
    },
    /// Repos and config for run pipelines
    Project {
        #[command(subcommand)]
        cmd: ProjectCommands,
    },
    /// Manage OpenCode subagent roles
    Role {
        #[command(subcommand)]
        cmd: RoleCommands,
    },
    /// Run pipeline workflows (plan/develop/review/release)
    Run {
        #[command(subcommand)]
        cmd: RunCommands,
    },
    /// Project/branch execution agent containers
    Agent {
        #[command(subcommand)]
        cmd: AgentCommands,
    },
}

#[derive(Subcommand)]
enum RoleCommands {
    /// List all available roles
    List {
        /// Show full role instructions
        #[arg(long)]
        verbose: bool,
    },
    /// Add a new role from a markdown file
    Add {
        /// Role name
        name: String,
        /// Source markdown file path
        #[arg(long)]
        from: PathBuf,
    },
    /// Update an existing role from a markdown file
    Update {
        /// Role name
        name: String,
        /// Source markdown file path
        #[arg(long)]
        from: PathBuf,
    },
    /// Remove a role
    Remove {
        /// Role name
        name: String,
        /// Force removal for protected core roles
        #[arg(long)]
        force: bool,
    },
    /// Sync role files from a directory into ~/.config/opencode/agents
    Sync {
        /// Source directory containing role markdown files (default: ./roles)
        #[arg(long)]
        from: Option<PathBuf>,
        /// Overwrite existing role files in target directory
        #[arg(long)]
        force: bool,
    },
}

/// Built-in agent name and image when no config is set (OpenCode cloud wrapper).
const DEFAULT_AGENT_NAME: &str = "opencode";
const DEFAULT_AGENT_IMAGE: &str = "ghcr.io/anomalyco/opencode";

#[derive(Subcommand)]
enum ModelCommands {
    /// Show status of all configured agents
    Status,
    /// Add an agent
    Add {
        /// Agent name (id)
        name: String,
        /// Docker image (e.g. ghcr.io/anomalyco/opencode); default: official OpenCode image
        #[arg(long)]
        image: Option<String>,
        /// Agent type: "local" or "cloud" (default: cloud)
        #[arg(long)]
        agent_type: Option<String>,
        /// Model to use (e.g. "anthropic/claude-sonnet-4-5", "qwen3:8b")
        #[arg(long)]
        model: Option<String>,
        /// Smaller model for internal operations (reduces API costs)
        #[arg(long)]
        small_model: Option<String>,
        /// Provider name (e.g. "ollama", "anthropic", "openai", "openrouter")
        #[arg(long)]
        provider: Option<String>,
        /// Custom base URL for provider (for proxies/custom endpoints)
        #[arg(long)]
        base_url: Option<String>,
        /// Port for opencode serve (default: 4096 + index)
        #[arg(long)]
        port: Option<u16>,
        /// Whether this agent is enabled (default: true)
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Update an existing agent
    Update {
        /// Agent name
        name: String,
        /// Docker image (pass empty to clear)
        #[arg(long)]
        image: Option<String>,
        /// Agent type: "local" or "cloud" (pass empty to clear)
        #[arg(long)]
        agent_type: Option<String>,
        /// Model (pass empty to clear = use container default)
        #[arg(long)]
        model: Option<String>,
        /// Small model (pass empty to clear)
        #[arg(long)]
        small_model: Option<String>,
        /// Provider name (pass empty to clear)
        #[arg(long)]
        provider: Option<String>,
        /// Base URL (pass empty to clear)
        #[arg(long)]
        base_url: Option<String>,
        /// Port for opencode serve (pass empty to clear)
        #[arg(long)]
        port: Option<u16>,
        /// Whether this agent is enabled (pass empty to clear)
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Remove an agent
    Remove {
        /// Agent name
        name: String,
    },
    /// Sync agent config to host opencode (writes ~/.config/opencode/opencode.json)
    Sync,
    /// Build Docker image for local agents (generate Dockerfile if missing, then docker build)
    Build {
        /// Agent name to build (e.g. opencode); omit to build all
        #[arg()]
        name: Option<String>,
        /// Build all configured agents
        #[arg(short = 'a', long = "all", alias = "A")]
        all: bool,
        /// Remove existing image and build with --no-cache (clean build)
        #[arg(long)]
        force: bool,
        /// Print Dockerfile path and docker build command per agent
        #[arg(long)]
        verbose: bool,
    },
    /// Start local agent containers (1 agent -> 1 container). Idempotent: skips if already running.
    Start {
        /// Print docker command and health-check details
        #[arg(short, long)]
        verbose: bool,
    },
    /// Stop local agent containers
    Stop,
    /// Stream live logs from an agent container (docker logs -f)
    Logs {
        /// Agent name (e.g. opencode)
        name: String,
    },
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum ProjectCommands {
    /// Add a new project
    Add {
        /// Project name
        name: String,
        /// Repository path or URL
        #[arg(long)]
        repo: String,
        /// Docker image to use for this project (required)
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for this project (optional)
        #[arg(long)]
        ssh_key: Option<String>,
        /// Base branch to use for clone/compare (optional, default: main)
        #[arg(long)]
        base_branch: Option<String>,
        /// Remote name for fetch/push (optional, default: origin)
        #[arg(long)]
        remote: Option<String>,
        /// GitHub personal access token for PR creation (optional)
        #[arg(long)]
        github_token: Option<String>,
        /// Script to run in container before pipeline (optional, e.g., install OpenCode)
        #[arg(long)]
        script: Option<String>,
        /// Git author name (optional, overrides local git config)
        #[arg(long)]
        commit_name: Option<String>,
        /// Git author email (optional, overrides local git config)
        #[arg(long)]
        commit_email: Option<String>,
    },
    /// List all registered projects
    List,
    /// Validate project configuration and connectivity
    Status {
        /// Project name (omit to run status for all projects)
        #[arg(long)]
        project: Option<String>,
        /// Show detailed validation output
        #[arg(long)]
        verbose: bool,
    },
    /// Update an existing project's repository URL, image, or SSH key
    Update {
        /// Project name
        name: String,
        /// New repository path or URL
        #[arg(long)]
        repo: Option<String>,
        /// New Docker image to use for this project
        #[arg(long)]
        image: Option<String>,
        /// SSH key path for this project (pass empty to clear)
        #[arg(long)]
        ssh_key: Option<String>,
        /// Base branch (pass empty to clear)
        #[arg(long)]
        base_branch: Option<String>,
        /// Remote name (pass empty to clear)
        #[arg(long)]
        remote: Option<String>,
        /// GitHub token for PR creation (pass empty to clear)
        #[arg(long)]
        github_token: Option<String>,
        /// Script to run in container before pipeline (pass empty to clear)
        #[arg(long)]
        script: Option<String>,
        /// Git author name (pass empty to clear)
        #[arg(long)]
        commit_name: Option<String>,
        /// Git author email (pass empty to clear)
        #[arg(long)]
        commit_email: Option<String>,
        /// Agent name to use for this project (pass empty to clear)
        #[arg(long)]
        agent: Option<String>,
        /// Ask pipeline: setup_run and setup_check roles (e.g., "installer" or "installer analyst")
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        ask_setup: Option<Vec<String>>,
        /// Ask pipeline: execute_run and execute_check roles (e.g., "engineer" or "engineer analyst")
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        ask_execute: Option<Vec<String>>,
        /// Ask pipeline: validate_run and validate_check roles (e.g., "validator" or "validator reviewer")
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        ask_validate: Option<Vec<String>>,
        /// Dev pipeline: setup_run and setup_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_setup: Option<Vec<String>>,
        /// Dev pipeline: execute_run and execute_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_execute: Option<Vec<String>>,
        /// Dev pipeline: validate_run and validate_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_validate: Option<Vec<String>>,
        /// Dev pipeline: commit_run and commit_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        dev_commit: Option<Vec<String>>,
        /// Review pipeline: setup_run and setup_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        review_setup: Option<Vec<String>>,
        /// Review pipeline: execute_run and execute_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        review_execute: Option<Vec<String>>,
        /// Review pipeline: validate_run and validate_check roles
        #[arg(long, value_delimiter = ' ', num_args = 1..=2)]
        review_validate: Option<Vec<String>>,
    },
    /// Remove a project
    Remove {
        /// Project name
        name: String,
    },
}

/// Pipeline commands (run via `smith run <cmd>`).
#[derive(Subcommand)]
enum RunCommands {
    /// Internal pipeline entrypoint (use `smith run plan`)
    #[command(hide = true)]
    Plan {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
        /// Feature/request prompt to plan
        prompt: String,
    },
    /// Internal pipeline entrypoint (use `smith run develop`)
    #[command(hide = true)]
    Develop {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Base branch used when target branch does not exist remotely
        #[arg(long)]
        base: Option<String>,
        /// Plan id to execute (full id or short id)
        #[arg(long)]
        plan: String,
        /// Maximum develop/validate passes before failing
        #[arg(long, default_value_t = 3)]
        max_validate_passes: u32,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
        /// Create or update a pull request after successful develop run
        #[arg(long)]
        pr: bool,
        /// Development task to execute
        task: String,
    },
    /// Internal pipeline entrypoint (use `smith run release`)
    #[command(hide = true)]
    Release {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Base branch for integration (default: project base branch or main)
        #[arg(long)]
        base: Option<String>,
        /// Plan id to release (full id or short id)
        #[arg(long)]
        plan: String,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
        /// Close matching open pull request after successful integration
        #[arg(long)]
        pr: bool,
    },
    /// Internal pipeline entrypoint (use `smith run review`)
    #[command(hide = true)]
    Review {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Limit number of plans shown (newest first)
        #[arg(long)]
        limit: Option<usize>,
        /// Optional state filter (not_started|planning|in_progress|completed|failed)
        #[arg(long)]
        state: Option<String>,
        /// Filter to a specific plan by full id (plan-xxxx) or short id (xxxx)
        #[arg(long)]
        plan: Option<String>,
        /// Submit a user reply for plan issues (requires --plan)
        #[arg(long)]
        reply: Option<String>,
    },
}

/// Commands for persistent project/branch-scoped agent containers.
#[derive(Subcommand)]
enum AgentCommands {
    /// Start a persistent agent for project/branch
    Start {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch to work on (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Port for opencode serve (auto: hash-based in range 4096-8191)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Stop a spawned agent
    Stop {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Stop all spawned agents
        #[arg(long, short)]
        all: bool,
    },
    /// Restart a spawned agent
    Restart {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
    },
    /// Run a prompt against a spawned agent and print the response
    Run {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
        /// Prompt to send to the spawned agent
        prompt: String,
    },
    /// Run producer/architect/designer/planner and persist JSON artifacts under /state
    Plan {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
        /// Feature/request prompt to plan
        prompt: String,
    },
    /// Execute a plan-driven development loop in a spawned container
    Develop {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Base branch used when target branch does not exist remotely
        #[arg(long)]
        base: Option<String>,
        /// Plan id to execute (full id or short id)
        #[arg(long)]
        plan: String,
        /// Maximum develop/validate passes before failing
        #[arg(long, default_value_t = 3)]
        max_validate_passes: u32,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
        /// Development task to execute
        task: String,
    },
    /// Run release pipeline for a completed plan (review -> integrate -> sync)
    Release {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Base branch for integration (default: project base branch or main)
        #[arg(long)]
        base: Option<String>,
        /// Plan id to release (full id or short id)
        #[arg(long)]
        plan: String,
        /// Show detailed agent output (enables print-logs and thinking)
        #[arg(long)]
        verbose: bool,
    },
    /// Review all plan artifacts in a spawned container
    Review {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Limit number of plans shown (newest first)
        #[arg(long)]
        limit: Option<usize>,
        /// Optional state filter (not_started|planning|in_progress|completed|failed)
        #[arg(long)]
        state: Option<String>,
        /// Filter to a specific plan by full id (plan-xxxx) or short id (xxxx)
        #[arg(long)]
        plan: Option<String>,
        /// Submit a user reply for plan issues (requires --plan)
        #[arg(long)]
        reply: Option<String>,
    },
    /// Remove plan runs from /state in a spawned container
    Clear {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Remove all plan runs
        #[arg(long, short)]
        all: bool,
        /// Remove a specific plan id (e.g. plan-1772346551-112891)
        #[arg(long)]
        plan: Option<String>,
        /// Remove plans by state (not_started|planning|in_progress|completed|failed)
        #[arg(long)]
        state: Option<String>,
    },
    /// Show logs from a spawned agent
    Logs {
        /// Project name (auto-detected from git repo if not specified)
        #[arg(long)]
        project: Option<String>,
        /// Branch name (auto-detected from current git branch if not specified)
        #[arg(long)]
        branch: Option<String>,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    /// List all spawned agents
    List,
    /// Remove all stopped spawned containers
    Prune,
}

#[derive(Serialize, Deserialize, Default)]
struct SmithConfig {
    projects: Vec<ProjectConfig>,
    /// Legacy: global github token (no longer used; PRs use project.github_token)
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<GitHubConfig>,
    /// Legacy: single agent image (used when agents list is empty)
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<AgentConfig>,
    /// Named agents (e.g. opencode = OpenCode image). When set, used for resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    agents: Option<Vec<AgentEntry>>,
    /// Which agent name to use for ask/dev/review (default: "opencode")
    #[serde(skip_serializing_if = "Option::is_none")]
    current_agent: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct AgentConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct AgentRole {
    /// Mode for this role (e.g., "build", "plan", "ask", "review", "edit")
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    /// Model override for this role (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Prompt prefix for this role
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct AgentEntry {
    /// Unique id/name for the agent
    name: String,
    /// Docker image (e.g. ghcr.io/anomalyco/opencode); custom images allowed
    image: String,
    /// Agent type: "local" or "cloud" (default: "cloud")
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_type: Option<String>,
    /// Model to use (e.g. "anthropic/claude-sonnet-4-5", "qwen3:8b")
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Smaller model for internal operations (reduces API costs)
    #[serde(skip_serializing_if = "Option::is_none")]
    small_model: Option<String>,
    /// Provider name: "ollama", "anthropic", "openai", "openrouter", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    /// Custom base URL (for proxies/custom endpoints)
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    /// Port for opencode serve (default: 4096 + index)
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    /// Whether this agent is enabled (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    /// Default role name for this agent
    #[serde(skip_serializing_if = "Option::is_none")]
    default_role: Option<String>,
    /// Roles defined for this agent (keyed by role name)
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<HashMap<String, AgentRole>>,
}

/// Resolve port for an agent: port if set, else OPENCODE_SERVER_PORT + index.
fn agent_port(entry: &AgentEntry, index: usize) -> u16 {
    entry
        .port
        .unwrap_or_else(|| docker::OPENCODE_SERVER_PORT + index as u16)
}

#[derive(Serialize, Deserialize, Clone)]
struct GitHubConfig {
    token: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ProjectConfig {
    name: String,
    repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ssh_key: Option<String>,
    /// Base branch to clone and compare against (e.g. main). All actions fetch and use remote ref.
    #[serde(skip_serializing_if = "Option::is_none")]
    base_branch: Option<String>,
    /// Remote name for fetch/push (e.g. origin). All comparisons use refs on this remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    remote: Option<String>,
    /// GitHub personal access token for PR creation (--pr). Per-repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    github_token: Option<String>,
    /// Script to run in container before pipeline (e.g., install OpenCode).
    /// Example: "curl -fsSL https://opencode.ai/install.sh | sh"
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<String>,
    /// Commit author name (overrides local git config)
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_name: Option<String>,
    /// Commit author email (overrides local git config)
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_email: Option<String>,
    /// Agent name to use for this project (overrides current_agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    /// Pipeline step: ask.setup.run
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_setup_run: Option<String>,
    /// Pipeline step: ask.setup.check
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_setup_check: Option<String>,
    /// Pipeline step: ask.execute.run
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_execute_run: Option<String>,
    /// Pipeline step: ask.execute.check
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_execute_check: Option<String>,
    /// Pipeline step: ask.validate.run
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_validate_run: Option<String>,
    /// Pipeline step: ask.validate.check
    #[serde(skip_serializing_if = "Option::is_none")]
    ask_validate_check: Option<String>,
    /// Pipeline step: dev.setup.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_setup_run: Option<String>,
    /// Pipeline step: dev.setup.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_setup_check: Option<String>,
    /// Pipeline step: dev.execute.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_execute_run: Option<String>,
    /// Pipeline step: dev.execute.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_execute_check: Option<String>,
    /// Pipeline step: dev.validate.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_validate_run: Option<String>,
    /// Pipeline step: dev.validate.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_validate_check: Option<String>,
    /// Pipeline step: dev.commit.run
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_commit_run: Option<String>,
    /// Pipeline step: dev.commit.check
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_commit_check: Option<String>,
    /// Pipeline step: review.setup.run
    #[serde(skip_serializing_if = "Option::is_none")]
    review_setup_run: Option<String>,
    /// Pipeline step: review.setup.check
    #[serde(skip_serializing_if = "Option::is_none")]
    review_setup_check: Option<String>,
    /// Pipeline step: review.execute.run
    #[serde(skip_serializing_if = "Option::is_none")]
    review_execute_run: Option<String>,
    /// Pipeline step: review.execute.check
    #[serde(skip_serializing_if = "Option::is_none")]
    review_execute_check: Option<String>,
    /// Pipeline step: review.validate.run
    #[serde(skip_serializing_if = "Option::is_none")]
    review_validate_run: Option<String>,
    /// Pipeline step: review.validate.check
    #[serde(skip_serializing_if = "Option::is_none")]
    review_validate_check: Option<String>,
}

fn config_dir() -> Result<PathBuf, String> {
    ProjectDirs::from("com", "agent", "smith")
        .ok_or_else(|| "Could not determine config directory".to_string())
        .map(|dirs| dirs.config_dir().to_path_buf())
}

/// Build the Docker image for one agent: ensure agent dir and Dockerfile exist, then run docker build.
/// `port` is written into the Dockerfile (EXPOSE and CMD) and should match the agent's port or default.
#[allow(clippy::too_many_arguments)]
fn build_agent_image(
    config_dir: &Path,
    name: &str,
    base_image: &str,
    port: u16,
    model: Option<&str>,
    small_model: Option<&str>,
    _provider: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let agent_dir = config_dir.join("agents").join(name);
    fs::create_dir_all(&agent_dir).map_err(|e| format!("Failed to create agent dir: {}", e))?;
    let dockerfile_path = agent_dir.join("Dockerfile");

    let port_str = port.to_string();
    let mut env_lines = String::new();
    let mut cmd_args = vec!["serve", "--hostname", "0.0.0.0", "--port", &port_str];

    if let Some(m) = model {
        cmd_args.push("--model");
        cmd_args.push(m);
        env_lines.push_str(&format!("ENV OPENCODE_MODEL=\"{}\"\n", m));
    }

    if let Some(sm) = small_model {
        env_lines.push_str(&format!("ENV OPENCODE_SMALL_MODEL=\"{}\"\n", sm));
    }

    let opencode_config = if model.is_some() || small_model.is_some() {
        let mut cfg = String::from("{\n");
        if let Some(m) = model {
            cfg.push_str(&format!("  \"model\": \"{}\",\n", m));
        }
        if let Some(sm) = small_model {
            cfg.push_str(&format!("  \"small_model\": \"{}\",\n", sm));
        }
        cfg.push_str("\n}\n");
        Some(cfg)
    } else {
        None
    };

    if !dockerfile_path.exists() || force {
        let mut content = format!(
            r#"FROM {}

LABEL smith.agent.name="{}"

EXPOSE {}

"#,
            base_image, name, port
        );

        content.push_str(&env_lines);

        if let Some(ref cfg) = opencode_config {
            let config_path = agent_dir.join("opencode.jsonc");
            fs::write(&config_path, cfg)
                .map_err(|e| format!("Failed to write opencode config: {}", e))?;
            content.push_str("COPY opencode.jsonc /home/opencode.jsonc\n");
            content.push_str("ENV OPENCODE_CONFIG=/home/opencode.jsonc\n");
        }

        content.push_str(&format!(
            r#"
ENTRYPOINT ["opencode"]
CMD ["{}"]
"#,
            cmd_args.join("\", \"")
        ));

        fs::write(&dockerfile_path, content)
            .map_err(|e| format!("Failed to write Dockerfile: {}", e))?;
    }
    let tag = docker::agent_built_image_tag(name);
    if force {
        let _ = Command::new("docker").args(["rmi", "-f", &tag]).output();
    }
    let mut args = vec!["build", "-t", &tag];
    if force {
        args.push("--no-cache");
    }
    let output = Command::new("docker")
        .args(&args)
        .arg(agent_dir.as_path())
        .output()
        .map_err(|e| format!("Failed to run docker build: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("docker build failed: {}", stderr.trim()))
    }
}

fn config_file_path() -> Result<PathBuf, String> {
    config_dir().map(|dir| dir.join("config.toml"))
}

/// Prompt for confirmation; returns true if user types "yes"/"y" (case-insensitive) or if force is true.
fn confirm_reset(prompt: &str, force: bool) -> bool {
    if force {
        return true;
    }
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    let line = line.trim().to_lowercase();
    line == "yes" || line == "y"
}

/// Read a line from stdin (trimmed). For interactive install wizard.
fn prompt_line(prompt: &str) -> String {
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    line.trim().to_string()
}

/// Prompt [y/N]; returns true for y/yes, false for n/no or empty (default no).
fn prompt_yn(prompt: &str, default_no: bool) -> bool {
    let hint = if default_no { " [y/N]" } else { " [Y/n]" };
    let line = prompt_line(&format!("{}{}: ", prompt, hint)).to_lowercase();
    if line.is_empty() {
        return !default_no;
    }
    matches!(line.as_str(), "y" | "yes")
}

/// Prompt [y/N/skip]; returns Some(true)=y, Some(false)=n, None=skip.
fn prompt_yn_skip(prompt: &str) -> Option<bool> {
    let line = prompt_line(&format!("{} [y/N/skip]: ", prompt)).to_lowercase();
    if line.is_empty() || line == "n" || line == "no" {
        return Some(false);
    }
    if line == "skip" || line == "s" {
        return None;
    }
    if line == "y" || line == "yes" {
        return Some(true);
    }
    Some(false)
}

/// Add an agent to config (used by CLI `agent add` and install wizard). Does not save.
#[allow(clippy::too_many_arguments)]
fn add_agent_to_config(
    cfg: &mut SmithConfig,
    name: String,
    image: Option<String>,
    agent_type: Option<String>,
    model: Option<String>,
    small_model: Option<String>,
    provider: Option<String>,
    base_url: Option<String>,
    port: Option<u16>,
    enabled: Option<bool>,
    default_role: Option<String>,
    roles: Option<HashMap<String, AgentRole>>,
) -> Result<(), String> {
    if cfg
        .agents
        .as_ref()
        .is_some_and(|a| a.iter().any(|e| e.name == name))
    {
        return Err(format!("Agent '{}' already exists", name));
    }
    let image = image
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_IMAGE.to_string());
    let agent_type = agent_type.filter(|s| !s.is_empty());
    let model = model.filter(|s| !s.is_empty());
    let small_model = small_model.filter(|s| !s.is_empty());
    let provider = provider.filter(|s| !s.is_empty());
    let base_url = base_url.filter(|s| !s.is_empty());
    let agents = cfg.agents.get_or_insert_with(Vec::new);

    // First agent becomes "default" if no name specified
    let agent_name = if name.is_empty() && agents.is_empty() {
        "default".to_string()
    } else {
        name
    };

    // If no roles provided, create default "*" role
    let roles = if roles.is_none() {
        let mut default_roles = HashMap::new();
        default_roles.insert(
            "*".to_string(),
            AgentRole {
                mode: None,
                model: model.clone(),
                prompt: None,
            },
        );
        Some(default_roles)
    } else {
        roles
    };

    let default_role = default_role.or_else(|| Some("*".to_string()));
    let port = port.or_else(|| Some(docker::OPENCODE_SERVER_PORT + agents.len() as u16));
    let enabled = enabled.or(Some(true));
    agents.push(AgentEntry {
        name: agent_name.clone(),
        image,
        agent_type,
        model,
        small_model,
        provider,
        base_url,
        port,
        enabled,
        default_role,
        roles,
    });
    if cfg.current_agent.is_none() {
        cfg.current_agent = Some(agent_name);
    }
    Ok(())
}

/// Add a project to config (used by CLI `project add` and install wizard). Does not save.
fn add_project_to_config(cfg: &mut SmithConfig, project: ProjectConfig) -> Result<(), String> {
    if cfg.projects.iter().any(|p| p.name == project.name) {
        return Err(format!("Project '{}' already exists", project.name));
    }
    cfg.projects.push(project);
    Ok(())
}

fn load_config() -> Result<SmithConfig, String> {
    let file = config_file_path()?;
    if !file.exists() {
        return Ok(SmithConfig::default());
    }
    let content = fs::read_to_string(&file).map_err(|e| format!("Failed to read config: {}", e))?;
    toml::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
}

fn save_config(config: &SmithConfig) -> Result<(), String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config directory: {}", e))?;
    let file = config_file_path()?;
    let content =
        toml::to_string_pretty(config).map_err(|e| format!("Failed to serialize config: {}", e))?;

    // Atomic write: write to temp file then rename. On EXDEV (cross-filesystem), fall back to copy + remove.
    let temp_file = file.with_extension("toml.tmp");
    fs::write(&temp_file, content).map_err(|e| format!("Failed to write config: {}", e))?;
    if let Err(e) = fs::rename(&temp_file, &file) {
        // EXDEV = cross-filesystem rename not supported (MSRV 1.83: avoid ErrorKind::CrossesDevices)
        let is_cross_device = e.raw_os_error() == Some(libc::EXDEV);
        if is_cross_device {
            fs::copy(&temp_file, &file).map_err(|e| format!("Failed to copy config: {}", e))?;
            fs::remove_file(&temp_file)
                .map_err(|e| format!("Failed to remove temp config: {}", e))?;
        } else {
            return Err(format!("Failed to finalize config: {}", e));
        }
    }
    Ok(())
}

const INSTALLED_MARKER: &str = ".smith-installed";

/// True if the user has run `smith install` (marker file in config dir).
fn is_installed() -> bool {
    config_dir()
        .map(|d| d.join(INSTALLED_MARKER).exists())
        .unwrap_or(false)
}

/// Version last recorded by `smith install` (for migrations). None = not installed; Some("") = legacy empty marker.
fn installed_version() -> Option<String> {
    let path = config_dir().ok()?.join(INSTALLED_MARKER);
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

/// If Docker is not available, try to install it via the official get.docker.com script (Linux only).
/// Requires sudo. On success, starts the docker service. Non-fatal on failure.
#[cfg(target_os = "linux")]
fn try_install_docker() {
    if docker::check_docker_available().is_ok() {
        println!("  {} docker - available", BULLET_GREEN);
        return;
    }
    println!(
        "  {} docker - installing (https://get.docker.com) ...",
        BULLET_BLUE
    );
    let script_path = std::env::temp_dir().join("get-docker.sh");
    let curl = Command::new("curl")
        .args(["-fsSL", "https://get.docker.com", "-o"])
        .arg(&script_path)
        .status();
    if let Ok(s) = curl {
        if !s.success() {
            eprintln!(
                "  {} docker - install failed (could not download script)",
                BULLET_RED
            );
            return;
        }
    } else {
        eprintln!("  {} docker - install skipped (curl not found)", BULLET_RED);
        return;
    }
    let install = Command::new("sudo")
        .args(["sh", script_path.to_str().unwrap_or("get-docker.sh")])
        .status();
    let _ = fs::remove_file(&script_path);
    match install {
        Ok(s) if s.success() => {
            let _ = Command::new("sudo")
                .args(["systemctl", "start", "docker"])
                .status();
            let _ = Command::new("sudo")
                .args(["systemctl", "enable", "docker"])
                .status();
            println!(
                "  {} docker - available",
                BULLET_GREEN
            );
            println!("       Log out and back in to use Docker without sudo (docker group).");
        }
        _ => eprintln!(
            "  {} docker - install failed (run manually: curl -fsSL https://get.docker.com | sudo sh)",
            BULLET_RED
        ),
    }
}

#[cfg(not(target_os = "linux"))]
fn try_install_docker() {}

/// On Linux, ensure Docker service is started and enabled (so it runs on boot). No-op if Docker not available. Non-fatal.
#[cfg(target_os = "linux")]
fn ensure_docker_started_and_enabled() {
    if docker::check_docker_available().is_err() {
        return;
    }
    let _ = Command::new("sudo")
        .args(["systemctl", "start", "docker"])
        .status();
    let _ = Command::new("sudo")
        .args(["systemctl", "enable", "docker"])
        .status();
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
fn ensure_docker_started_and_enabled() {}

/// Write config dir and install marker with current version (called at end of install wizard).
/// Storing the version allows future migrations to run when upgrading.
fn run_install_finish() -> Result<(), String> {
    if let Ok(dir) = config_dir() {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
        let version = env!("CARGO_PKG_VERSION");
        fs::write(dir.join(INSTALLED_MARKER), version)
            .map_err(|e| format!("Failed to write install marker: {}", e))?;
    }
    Ok(())
}

/// Get the remote URL for "origin" from the git repo containing the current directory, if any.
fn git_remote_origin_url() -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if out.status.success() {
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if url.is_empty() {
            None
        } else {
            Some(url)
        }
    } else {
        None
    }
}

/// Get the canonical repo root path for the git repo containing the current directory, if any.
fn git_repo_root_path() -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    std::fs::canonicalize(root).ok()
}

/// Normalize a repo string (URL or path) to a comparable form for matching.
/// - GitHub URLs -> "github.com/owner/name"
/// - Other URLs -> "host/path" (strip .git, lowercase host)
/// - Paths -> canonical absolute path if it exists
fn normalize_repo_for_match(repo: &str) -> Option<String> {
    let repo = repo.trim();
    if repo.is_empty() {
        return None;
    }
    // URL: has protocol or SSH form
    if repo.contains("://") || repo.starts_with("git@") {
        // Only use GitHub parser for GitHub URLs
        if repo.contains("github.com") {
            if let Ok(info) = github::extract_repo_info(repo) {
                return Some(format!("github.com/{}/{}", info.owner, info.name));
            }
        }
        // Non-GitHub URL: normalize to host/path
        let s = repo.trim_end_matches(".git");
        let (host, path) = if s.starts_with("git@") {
            let colon = s.find(':')?;
            let host = s[4..colon].to_lowercase();
            let path = s[colon + 1..].trim_start_matches('/');
            (host, path)
        } else if let Some(after) = s
            .strip_prefix("https://")
            .or_else(|| s.strip_prefix("http://"))
        {
            let rest = after.splitn(2, '/').collect::<Vec<_>>();
            let host = rest.first()?.to_string().to_lowercase();
            let path = rest.get(1).copied().unwrap_or("").trim_start_matches('/');
            (host, path)
        } else {
            return None;
        };
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            Some(host)
        } else {
            Some(format!("{}/{}", host, path))
        }
    } else {
        // Treat as filesystem path
        let p = Path::new(repo);
        if p.exists() {
            std::fs::canonicalize(p)
                .ok()
                .map(|pb| pb.to_string_lossy().into_owned())
        } else {
            Some(repo.to_string())
        }
    }
}

/// If cwd is inside a git repo and exactly one configured project's repo matches
/// (by remote URL or repo root path), return that project name. Otherwise None.
/// Explicit --project / --repo should be used when zero or multiple matches.
fn detect_project_from_cwd() -> Result<Option<String>, String> {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    if cfg.projects.is_empty() {
        return Ok(None);
    }

    let remote_url = git_remote_origin_url();
    let repo_root = git_repo_root_path();

    let current_normalized_url = remote_url.as_deref().and_then(normalize_repo_for_match);
    let current_normalized_path = repo_root.as_ref().map(|p| p.to_string_lossy().into_owned());

    let mut matches: Vec<&str> = Vec::new();
    for proj in &cfg.projects {
        let normalized = match normalize_repo_for_match(proj.repo.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let url_match = current_normalized_url.as_ref() == Some(&normalized);
        let path_match = current_normalized_path.as_ref() == Some(&normalized);
        if url_match || path_match {
            matches.push(proj.name.as_str());
        }
    }

    if matches.len() == 1 {
        Ok(Some(matches[0].to_string()))
    } else {
        if matches.len() > 1 {
            eprintln!(
                "Multiple projects match this repo ({}); use --project to choose.",
                matches.join(", ")
            );
        }
        Ok(None)
    }
}

fn resolve_repo(repo: Option<String>, project: Option<String>) -> Result<String, String> {
    if let Some(r) = repo {
        return Ok(r);
    }
    if let Some(p) = project {
        let cfg = load_config()?;
        let proj = cfg
            .projects
            .iter()
            .find(|pr| pr.name == p)
            .ok_or_else(|| format!("Project '{}' not found", p))?;
        return Ok(proj.repo.clone());
    }
    Err("Either --repo or --project must be provided".to_string())
}

fn resolve_project_config(project: Option<String>) -> Result<Option<ProjectConfig>, String> {
    if let Some(p) = project {
        let cfg = load_config()?;
        let proj = cfg
            .projects
            .iter()
            .find(|pr| pr.name == p)
            .ok_or_else(|| format!("Project '{}' not found", p))?;
        return Ok(Some(proj.clone()));
    }
    Ok(None)
}

/// Resolve SSH key path: explicit --ssh-key > project ssh_key > SSH_KEY_PATH env
fn resolve_ssh_key(
    explicit: Option<&PathBuf>,
    project_config: Option<&ProjectConfig>,
) -> Option<PathBuf> {
    explicit
        .cloned()
        .or_else(|| {
            project_config
                .and_then(|p| p.ssh_key.as_ref())
                .map(PathBuf::from)
        })
        .or_else(|| std::env::var("SSH_KEY_PATH").ok().map(PathBuf::from))
}

/// Resolve base branch: explicit CLI --base > project base_branch > "main"
fn resolve_base_branch(explicit: Option<&str>, project_config: Option<&ProjectConfig>) -> String {
    explicit
        .map(String::from)
        .or_else(|| project_config.and_then(|p| p.base_branch.clone()))
        .unwrap_or_else(|| "main".to_string())
}

/// Resolve commit name/email from project config (returns None if not set = use local git)
fn resolve_commit_author(
    project_config: Option<&ProjectConfig>,
) -> (Option<String>, Option<String>) {
    let commit_name = project_config.and_then(|p| p.commit_name.clone());
    let commit_email = project_config.and_then(|p| p.commit_email.clone());
    (commit_name, commit_email)
}

/// Resolve pipeline step role: returns (agent_name, role_name, mode, model, prompt)
/// Looks up step in project config, parses "agent:role", resolves role from agent config
#[allow(clippy::type_complexity)]
fn resolve_pipeline_role(
    project_config: Option<&ProjectConfig>,
    step: &str,
    current_agent: Option<&str>,
) -> Option<(
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Get agent name from project or current_agent
    let agent_name = project_config
        .and_then(|p| p.agent.clone())
        .or_else(|| current_agent.map(String::from))?;

    // Get step mapping from project config
    let step_mapping = match step {
        "ask_setup_run" => project_config?.ask_setup_run.clone(),
        "ask_setup_check" => project_config?.ask_setup_check.clone(),
        "ask_execute_run" => project_config?.ask_execute_run.clone(),
        "ask_execute_check" => project_config?.ask_execute_check.clone(),
        "ask_validate_run" => project_config?.ask_validate_run.clone(),
        "ask_validate_check" => project_config?.ask_validate_check.clone(),
        "dev_setup_run" => project_config?.dev_setup_run.clone(),
        "dev_setup_check" => project_config?.dev_setup_check.clone(),
        "dev_execute_run" => project_config?.dev_execute_run.clone(),
        "dev_execute_check" => project_config?.dev_execute_check.clone(),
        "dev_validate_run" => project_config?.dev_validate_run.clone(),
        "dev_validate_check" => project_config?.dev_validate_check.clone(),
        "dev_commit_run" => project_config?.dev_commit_run.clone(),
        "dev_commit_check" => project_config?.dev_commit_check.clone(),
        "review_setup_run" => project_config?.review_setup_run.clone(),
        "review_setup_check" => project_config?.review_setup_check.clone(),
        "review_execute_run" => project_config?.review_execute_run.clone(),
        "review_execute_check" => project_config?.review_execute_check.clone(),
        "review_validate_run" => project_config?.review_validate_run.clone(),
        "review_validate_check" => project_config?.review_validate_check.clone(),
        _ => None,
    };

    // Parse "agent:role" or just "role" (use project agent)
    let (resolved_agent, role_name) = if let Some(ref mapping) = step_mapping {
        if mapping.contains(':') {
            let parts: Vec<&str> = mapping.split(':').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (agent_name.clone(), mapping.clone())
        }
    } else {
        // Fall back to agent's default_role
        let agents = cfg.agents.as_ref()?;
        let agent = agents.iter().find(|a| a.name == agent_name)?;
        let default_role = agent.default_role.as_ref()?.clone();
        (agent_name, default_role)
    };

    // Get role config from agent, with * fallback
    let agents = cfg.agents.as_ref()?;
    let agent = agents.iter().find(|a| a.name == resolved_agent)?;
    let roles = agent.roles.as_ref()?;

    // Try the specified role first, then fall back to "*"
    let role = roles.get(&role_name).or_else(|| roles.get("*"))?;

    Some((
        resolved_agent,
        role_name,
        role.mode.clone(),
        role.model.clone(),
        role.prompt.clone(),
    ))
}

fn resolve_pipeline_roles(
    project_config: Option<&ProjectConfig>,
    pipeline_type: &str,
) -> PipelineRoles {
    let mut roles = PipelineRoles::default();

    let steps = [
        ("setup_run", format!("{}_setup_run", pipeline_type)),
        ("setup_check", format!("{}_setup_check", pipeline_type)),
        ("execute_run", format!("{}_execute_run", pipeline_type)),
        ("execute_check", format!("{}_execute_check", pipeline_type)),
        ("validate_run", format!("{}_validate_run", pipeline_type)),
        (
            "validate_check",
            format!("{}_validate_check", pipeline_type),
        ),
        ("commit_run", format!("{}_commit_run", pipeline_type)),
        ("commit_check", format!("{}_commit_check", pipeline_type)),
    ];

    for (step_key, step_name) in steps {
        if let Some((_, _, _, model, prompt)) =
            resolve_pipeline_role(project_config, &step_name, None)
        {
            let role_info = RoleInfo::new(model, prompt);
            match step_key {
                "setup_run" => roles.setup_run = Some(role_info),
                "setup_check" => roles.setup_check = Some(role_info),
                "execute_run" => roles.execute_run = Some(role_info),
                "execute_check" => roles.execute_check = Some(role_info),
                "validate_run" => roles.validate_run = Some(role_info),
                "validate_check" => roles.validate_check = Some(role_info),
                "commit_run" => roles.commit_run = Some(role_info),
                "commit_check" => roles.commit_check = Some(role_info),
                _ => {}
            }
        }
    }

    roles
}

/// Column width for subcommand names so descriptions align (clap-style).
const HELP_NAME_WIDTH: usize = 18;

fn print_smith_help() {
    let c = Cli::command();
    if let Some(about) = c.get_about() {
        println!("{}\n", about);
    }
    println!("Usage: smith [COMMAND]");
    const SYSTEM: &[&str] = &["status", "install", "uninstall", "help", "version"];
    const COMMANDS: &[&str] = &["model", "project", "role", "agent", "run"];
    println!("\nCommands:");
    for sub in c.get_subcommands() {
        let name = sub.get_name();
        if SYSTEM.contains(&name) {
            let short = sub.get_about().unwrap_or_default();
            println!("  {name:<HELP_NAME_WIDTH$}  {short}");
        }
    }
    println!();
    for sub in c.get_subcommands() {
        let name = sub.get_name();
        if COMMANDS.contains(&name) {
            let short = sub.get_about().unwrap_or_default();
            println!("  {name:<HELP_NAME_WIDTH$}  {short}");
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            print_smith_help();
            std::process::exit(0);
        }
        Some(Commands::Status { verbose }) => commands::system::handle_status(verbose).await,
        Some(Commands::Install) => commands::system::handle_install().await,
        Some(Commands::Help) => {
            print_smith_help();
            std::process::exit(0);
        }
        Some(Commands::Version) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Uninstall {
            force,
            remove_config,
            remove_images,
        }) => commands::system::handle_uninstall(force, remove_config, remove_images).await,
        Some(Commands::Model { cmd }) => commands::model::handle(cmd).await,
        Some(Commands::Project { cmd }) => commands::project::handle(cmd).await,
        Some(Commands::Role { cmd }) => commands::role::handle(cmd).await,
        Some(Commands::Run { cmd }) => commands::run::handle(cmd).await,
        Some(Commands::Agent { cmd }) => commands::agent::handle(cmd).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_repo_prefers_explicit_repo() {
        assert_eq!(
            resolve_repo(Some("explicit".to_string()), Some("project".to_string())).unwrap(),
            "explicit"
        );
    }

    #[test]
    fn resolve_repo_requires_input() {
        assert!(resolve_repo(None, None).is_err());
    }

    #[test]
    fn toml_round_trip() {
        let mut cfg = SmithConfig::default();
        cfg.projects.push(ProjectConfig {
            name: "test".to_string(),
            repo: "https://example.com/repo".to_string(),
            image: None,
            ssh_key: None,
            base_branch: None,
            remote: None,
            github_token: None,
            script: None,
            commit_name: None,
            commit_email: None,
            agent: None,
            ask_setup_run: None,
            ask_setup_check: None,
            ask_execute_run: None,
            ask_execute_check: None,
            ask_validate_run: None,
            ask_validate_check: None,
            dev_setup_run: None,
            dev_setup_check: None,
            dev_execute_run: None,
            dev_execute_check: None,
            dev_validate_run: None,
            dev_validate_check: None,
            dev_commit_run: None,
            dev_commit_check: None,
            review_setup_run: None,
            review_setup_check: None,
            review_execute_run: None,
            review_execute_check: None,
            review_validate_run: None,
            review_validate_check: None,
        });
        let serialized = toml::to_string(&cfg).unwrap();
        let deserialized: SmithConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.projects.len(), 1);
        assert_eq!(deserialized.projects[0].name, "test");
    }

    #[test]
    fn normalize_repo_for_match_github_https() {
        assert_eq!(
            normalize_repo_for_match("https://github.com/owner/repo.git"),
            Some("github.com/owner/repo".to_string())
        );
        assert_eq!(
            normalize_repo_for_match("https://github.com/owner/repo"),
            Some("github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalize_repo_for_match_github_ssh() {
        assert_eq!(
            normalize_repo_for_match("git@github.com:owner/repo.git"),
            Some("github.com/owner/repo".to_string())
        );
        assert_eq!(
            normalize_repo_for_match("git@github.com:owner/repo"),
            Some("github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalize_repo_for_match_empty() {
        assert_eq!(normalize_repo_for_match(""), None);
        assert_eq!(normalize_repo_for_match("   "), None);
    }

    #[test]
    fn normalize_repo_for_match_non_github_url() {
        assert_eq!(
            normalize_repo_for_match("https://gitlab.com/group/sub/repo.git"),
            Some("gitlab.com/group/sub/repo".to_string())
        );
        assert_eq!(
            normalize_repo_for_match("git@gitlab.com:group/repo.git"),
            Some("gitlab.com/group/repo".to_string())
        );
    }

    #[test]
    fn normalize_repo_for_match_path() {
        // Path that exists: current dir canonicalizes to something
        let cwd = std::env::current_dir().unwrap();
        let cwd_str = cwd.to_string_lossy();
        let normalized = normalize_repo_for_match(cwd_str.as_ref());
        assert_eq!(
            normalized,
            Some(cwd.canonicalize().unwrap().to_string_lossy().into_owned())
        );
        // Path that does not exist: returned as-is
        let missing = "/nonexistent/path/12345";
        assert_eq!(normalize_repo_for_match(missing), Some(missing.to_string()));
    }

    #[test]
    fn detect_project_from_cwd_no_repo() {
        // From a temp dir with no .git, detect should return Ok(None)
        let temp = std::env::temp_dir().join("smith_test_no_git");
        let _ = std::fs::create_dir_all(&temp);
        let restorable = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp).unwrap();
        let result = detect_project_from_cwd();
        std::env::set_current_dir(&restorable).ok();
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn validate_role_name_accepts_safe_values() {
        assert_eq!(validate_role_name("assurance").unwrap(), "assurance");
        assert_eq!(
            validate_role_name("Custom-Role_1").unwrap(),
            "custom-role_1"
        );
    }

    #[test]
    fn validate_role_name_rejects_unsafe_values() {
        assert!(validate_role_name("../bad").is_err());
        assert!(validate_role_name("bad/name").is_err());
        assert!(validate_role_name("bad name").is_err());
    }

    #[test]
    fn parse_release_review_report_rejects_inconsistent_ready_state() {
        let raw = r#"{
  "schema_version": 1,
  "release_ready": true,
  "summary": ["ok"],
  "blocking_issues": [{
    "id": "RB-1",
    "severity": "high",
    "title": "blocking",
    "detail": "must fix",
    "related_ids": []
  }],
  "non_blocking_issues": [],
  "evidence": ["x"],
  "generated_at": "2026-01-01T00:00:00Z"
}"#;
        assert!(parse_release_review_report(raw).is_err());
    }

    #[test]
    fn core_roles_include_devops() {
        assert!(is_core_role("devops"));
        assert!(is_core_role("DevOps"));
    }
}
