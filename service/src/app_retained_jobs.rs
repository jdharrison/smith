//! App3 retained jobs. Production has no prepared runtime; trusted constructor
//! injection connects the same admission/worker path without launching tools.
#[cfg(test)]
#[path = "app_retained_jobs_tests.rs"]
mod tests;
use super::*;
use retained_jobs::{Change, ChangeKind, Error as RetainedError};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    session_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptGet {
    operation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    operation_id: String,
    instance_id: String,
    expected_revision: u64,
    expected_projects_revision: u64,
    project_id: String,
    job_type: retained_jobs::JobType,
    prompt: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    operation_id: String,
    instance_id: String,
    expected_revision: u64,
    session_id: String,
    #[serde(default)]
    prompt: Option<String>,
}
fn error(e: RetainedError) -> AppError {
    let (code, message) = match e {
        RetainedError::Invalid => ("INVALID_REQUEST", "Invalid retained job request"),
        RetainedError::Stale => ("STALE_REVISION", "Retained jobs changed; review again"),
        RetainedError::Conflict => ("REQUEST_CONFLICT", "Operation identity already used"),
        RetainedError::NotFound => ("JOB_NOT_FOUND", "Retained job not found"),
        RetainedError::Blocked => (
            "JOB_BLOCKED",
            "Job cannot perform this action; unknown outcomes are observation-only",
        ),
        RetainedError::Capacity => ("CAPACITY", "Retained job history is full"),
        RetainedError::ContextLimit => (
            "CONTEXT_LIMIT",
            "Prompt and saved context exceed the runtime input budget",
        ),
        RetainedError::Storage => (
            "DEV_UNAVAILABLE",
            "Retained job storage unavailable; no replay",
        ),
    };
    AppError::new(code, message)
}
fn unavailable_runtime() -> AppError {
    AppError::new(
        "DEV_RUNTIME_UNAVAILABLE",
        "Persistent job runtime is not prepared",
    )
}
fn receipt_reply(
    store: &retained_jobs::Store,
    receipt: retained_jobs::Receipt,
) -> AppResult<Value> {
    let session = store.get(&receipt.session_id).map_err(error)?;
    Ok(json!({"receipt":receipt,"job":session.job}))
}
impl Application {
    pub(super) fn retained_reserved(&self) -> usize {
        self.dev_jobs
            .retained
            .as_ref()
            .map(|s| s.reserved())
            .unwrap_or(retained_jobs::SLOT_LIMIT)
    }
    /// Exactly one command runs on the service-owned worker, outside the app mutex.
    /// Admission/reservation and acknowledgement share inspection's app-lock fence.
    pub(super) fn run_retained_command(&self) -> AppResult<bool> {
        let Some(runtime) = self.dev_jobs.retained_runtime.as_ref() else {
            return Ok(false);
        };
        if !runtime.admission_ready() {
            return Ok(false);
        }
        let store = self.dev_jobs.retained.as_ref().map_err(|e| error(*e))?;
        let command = {
            let inner = self.inner.lock().map_err(|_| storage())?;
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            self.retained_inspection_integrity()?;
            match store.take_pending().map_err(error)? {
                Some(c) => Some(c),
                None => store
                    .prepare_next(self.retained_inspection_occupied()?)
                    .map_err(error)?,
            }
        };
        let Some(command) = command else {
            return Ok(false);
        };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.execute(&command)));
        let ack = match result {
            Ok(ack)
                if ack.session_id == command.session_id
                    && ack.operation_id == command.operation_id
                    && ack.action == command.action =>
            {
                ack
            }
            _ => retained_jobs::Acknowledgement {
                session_id: command.session_id,
                operation_id: command.operation_id,
                action: command.action,
                outcome: retained_jobs::Outcome::Unknown,
            },
        };
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault || !inner.owned {
            return Err(storage());
        }
        self.retained_inspection_integrity()?;
        store.acknowledge(ack).map_err(error)?;
        Ok(true)
    }
    pub(super) fn call_retained_jobs(&self, req: AppRequest) -> AppResult<Value> {
        if !req.args.is_object() {
            return Err(invalid());
        }
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault || !inner.owned {
            return Err(storage());
        }
        let store = self.dev_jobs.retained.as_ref().map_err(|e| error(*e))?;
        let available = self
            .dev_jobs
            .retained_runtime
            .as_ref()
            .is_some_and(|r| r.admission_ready());
        let mutation = matches!(
            req.op.as_str(),
            "dev_session_start_v1"
                | "dev_session_prompt_v1"
                | "dev_session_finish_v1"
                | "dev_session_delete_v1"
        );
        if mutation {
            self.retained_inspection_integrity()?;
        }
        let result = match req.op.as_str() {
            "dev_sessions_state_v1" => {
                let _: Empty = decode(req.args)?;
                let sessions = store.sessions().map_err(error)?;
                let queued = sessions
                    .iter()
                    .filter(|s| s.job.status == retained_jobs::Status::Queued)
                    .count();
                let accepting = available && sessions.len() < 32;
                let jobs: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| !s.job.deleted)
                    .map(|s| s.job)
                    .collect();
                let inspection = self.retained_inspection_occupied()?;
                Ok(
                    json!({"revision":store.revision().map_err(error)?,"jobs":jobs,
                    "capability":{"available":available,"reason":if available {None} else {Some("runtime_not_prepared")},"job_types":["read","write"]},
                    "scheduler":{"limit":3,"occupied":inspection.saturating_add(store.reserved()),"queued":queued,"accepting":accepting}}),
                )
            }
            "dev_session_receipt_v1" => {
                let a: ReceiptGet = decode(req.args)?;
                if !valid_id(&a.operation_id) {
                    return Err(invalid());
                }
                receipt_reply(store, store.receipt(&a.operation_id).map_err(error)?)
            }
            "dev_session_get_v1" => {
                let a: Get = decode(req.args)?;
                if !valid_id(&a.session_id) {
                    return Err(invalid());
                }
                let s = store.get(&a.session_id).map_err(error)?;
                Ok(
                    json!({"revision":store.revision().map_err(error)?,"job":s.job,"turns":s.turns,"repositories":s.project.repositories}),
                )
            }
            "dev_session_start_v1" => {
                let a: Start = decode(req.args)?;
                if !valid_id(&a.operation_id)
                    || !valid_id(&a.instance_id)
                    || !valid_id(&a.project_id)
                    || a.prompt.trim().is_empty()
                    || a.prompt.len() > retained_jobs::MAX_PROMPT
                {
                    return Err(invalid());
                }
                if a.instance_id != self.instance_id {
                    return Err(AppError::new("STALE_INSTANCE", "Service instance changed"));
                }
                if !available && a.expected_revision != store.revision().map_err(error)? {
                    return Err(error(RetainedError::Stale));
                }
                let project = inner.durable.projects.freeze_retained(
                    &a.project_id,
                    a.expected_projects_revision,
                    &inner.durable.instructions.dev_settings_text(),
                )?;
                retained_jobs::compose_prompt(&project, &a.prompt).map_err(error)?;
                if !available {
                    return Err(unavailable_runtime());
                }
                let receipt = store
                    .start(retained_jobs::Start {
                        operation_id: a.operation_id,
                        session_id: new_id().map_err(|_| storage())?,
                        expected_revision: a.expected_revision,
                        project,
                        job_type: a.job_type,
                        prompt: a.prompt,
                        created_ms: self.cloud.clock.wall_ms(),
                    })
                    .map_err(error)?;
                receipt_reply(store, receipt)
            }
            "dev_session_prompt_v1" | "dev_session_finish_v1" | "dev_session_delete_v1" => {
                let a: Mutation = decode(req.args)?;
                if !valid_id(&a.operation_id)
                    || !valid_id(&a.instance_id)
                    || !valid_id(&a.session_id)
                {
                    return Err(invalid());
                }
                if a.instance_id != self.instance_id {
                    return Err(AppError::new("STALE_INSTANCE", "Service instance changed"));
                }
                let change = match req.op.as_str() {
                    "dev_session_prompt_v1" => {
                        let prompt = a.prompt.ok_or_else(invalid)?;
                        if prompt.trim().is_empty() || prompt.len() > retained_jobs::MAX_PROMPT {
                            return Err(invalid());
                        }
                        let session = store.get(&a.session_id).map_err(error)?;
                        retained_jobs::compose_prompt(&session.project, &prompt).map_err(error)?;
                        if !available {
                            return Err(unavailable_runtime());
                        }
                        ChangeKind::Prompt { prompt }
                    }
                    "dev_session_finish_v1" if a.prompt.is_none() => ChangeKind::Finish,
                    "dev_session_delete_v1" if a.prompt.is_none() => ChangeKind::Delete,
                    _ => return Err(invalid()),
                };
                if !available && store.get(&a.session_id).map_err(error)?.job.retained {
                    return Err(unavailable_runtime());
                }
                let receipt = store
                    .change(Change {
                        operation_id: a.operation_id,
                        session_id: a.session_id,
                        expected_revision: a.expected_revision,
                        change,
                    })
                    .map_err(error)?;
                receipt_reply(store, receipt)
            }
            _ => Err(invalid()),
        };
        drop(inner);
        if mutation && result.is_ok() {
            self.wake()?;
        }
        result
    }
}
