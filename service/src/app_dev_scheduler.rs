//! Container concurrency is separate from inference ownership. Only the caller of
//! run_dev_completion may touch Provider; job threads synchronously use this mailbox.
use super::*;
use std::collections::VecDeque;
use std::sync::atomic::AtomicU32;

const SLOTS: usize = 3;
const QUEUE_MS: u64 = 10 * 60 * 1000;

struct Completion {
    job: LocalJob,
    ordinal: u32,
    expected: Arc<AtomicU32>,
    body: Value,
    deadline: Instant,
    live: Arc<AtomicBool>,
    response: mpsc::SyncSender<AppResult<Value>>,
}
#[derive(Default)]
pub(super) struct Scheduler {
    mailbox: Mutex<VecDeque<Completion>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    dispatch: Mutex<()>,
}
impl Scheduler {
    pub(super) fn reject(&self) {
        if let Ok(mut mailbox) = self.mailbox.lock() {
            for request in mailbox.drain(..) {
                request.live.store(false, Ordering::Release);
                let _ = request.response.try_send(Err(unavailable()));
            }
        }
    }
    #[cfg(test)]
    pub(super) fn has_workers(&self) -> AppResult<bool> {
        Ok(self
            .workers
            .lock()
            .map_err(|_| unavailable())?
            .iter()
            .any(|h| !h.is_finished()))
    }
    #[cfg(test)]
    pub(super) fn reap_finished(&self) {
        for handle in self.take_workers(false) {
            let _ = handle.join();
        }
    }
    fn take_workers(&self, all: bool) -> Vec<JoinHandle<()>> {
        let mut workers = self
            .workers
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut joining = Vec::new();
        let mut i = 0;
        while i < workers.len() {
            if all || workers[i].is_finished() {
                joining.push(workers.swap_remove(i));
            } else {
                i += 1;
            }
        }
        joining
    }
}
fn wake(sender: &mpsc::SyncSender<()>) {
    // Full means an existing wake already covers the newly published work.
    let _ = sender.try_send(());
}
fn model_fence(inner: &Inner, job: &LocalJob, revision: &str) -> AppResult<()> {
    if inner.fault || !inner.owned {
        return Err(storage());
    }
    if inner.durable.config.revision.to_string() != revision
        || model_state(inner).1.as_ref() != Some(&job.model)
    {
        return Err(AppError::new(
            "LOCAL_MODEL_UNAVAILABLE",
            LocalError::ModelUnavailable.message(),
        ));
    }
    Ok(())
}
fn completion_fence(owner: &Owner, inner: &Mutex<Inner>, request: &Completion) -> AppResult<()> {
    if !request.live.load(Ordering::Acquire)
        || request.expected.load(Ordering::Acquire) != request.ordinal
    {
        return Err(protocol());
    }
    if Instant::now() >= request.deadline {
        return Err(AppError::new(
            "PROVIDER_TIMEOUT",
            LocalError::Deadline.message(),
        ));
    }
    let inner = inner.lock().map_err(|_| storage())?;
    let state = owner.state.lock().map_err(|_| unavailable())?;
    owner.available(&state)?;
    let record = state
        .ledger
        .records
        .iter()
        .find(|r| r.summary.request_id == request.job.request_id)
        .ok_or_else(unavailable)?;
    model_fence(&inner, &request.job, &record.config_revision)?;
    drop(state);
    owner.completion_fence(&request.job.request_id)
}

impl Application {
    pub(crate) fn dev_integrity_blocked(&self) -> bool {
        self.dev_jobs
            .local
            .as_ref()
            .is_some_and(|owner| owner.reserved())
    }
    pub(crate) fn dev_model_referenced(&self, model: &str) -> bool {
        self.dev_jobs.local.as_ref().is_some_and(|owner| {
            owner
                .state
                .lock()
                .map(|state| {
                    state.ledger.records.iter().any(|r| {
                        (r.summary.status == "queued" || active(&r.summary.status))
                            && r.job.model.name == model
                    })
                })
                .unwrap_or(true)
        })
    }
    pub(crate) fn dev_has_model_dependencies(&self) -> bool {
        self.dev_jobs.local.as_ref().is_some_and(|owner| {
            owner
                .state
                .lock()
                .map(|state| {
                    state
                        .ledger
                        .records
                        .iter()
                        .any(|r| r.summary.status == "queued" || active(&r.summary.status))
                })
                .unwrap_or(true)
        })
    }
    pub(crate) fn dev_queue_timeout(&self) -> Option<Duration> {
        let inner = self.inner.lock().ok()?;
        if inner.fault || !inner.owned {
            return None;
        }
        let owner = self.dev_jobs.local.as_ref()?;
        let state = owner.state.lock().ok()?;
        owner.available(&state).ok()?;
        if state.publication_ambiguous {
            return None;
        }
        let created = state
            .ledger
            .records
            .iter()
            .filter(|r| r.summary.status == "queued")
            .map(|r| r.summary.created_ms)
            .min()?;
        let age = self.cloud.clock.wall_ms().saturating_sub(created);
        Some(Duration::from_millis(QUEUE_MS.saturating_sub(age).max(1)))
    }
    pub(crate) fn dispatch_dev(&self, sender: mpsc::SyncSender<()>) -> AppResult<bool> {
        let Some(owner) = self.dev_jobs.local.as_ref() else {
            return Ok(false);
        };
        let _dispatch = owner.scheduler.dispatch.lock().map_err(|_| unavailable())?;
        for handle in owner.scheduler.take_workers(false) {
            let _ = handle.join();
        }
        let mut changed = false;
        loop {
            let (job, original_deadline) = {
                let inner = self.inner.lock().map_err(|_| storage())?;
                if inner.fault || !inner.owned {
                    return Err(storage());
                }
                let mut state = owner.state.lock().map_err(|_| unavailable())?;
                owner.available(&state)?;
                let now = self.cloud.clock.wall_ms();
                // Wall-clock rollback can make a later FIFO entry expire first.
                // Retire expired entries even while the dispatch head has no slot.
                let Some(index) = state
                    .ledger
                    .records
                    .iter()
                    .position(|r| {
                        r.summary.status == "queued"
                            && now.saturating_sub(r.summary.created_ms) >= QUEUE_MS
                    })
                    .or_else(|| {
                        state
                            .ledger
                            .records
                            .iter()
                            .position(|r| r.summary.status == "queued")
                    })
                else {
                    break;
                };
                if now.saturating_sub(state.ledger.records[index].summary.created_ms) >= QUEUE_MS {
                    let mut next = state.ledger.clone();
                    let r = &mut next.records[index];
                    r.failed_stage = Some("queued".into());
                    r.summary.status = "failed".into();
                    r.summary.message = QUEUE_EXPIRED.into();
                    owner.commit(&mut state, next)?;
                    changed = true;
                    wake(&sender);
                    continue;
                }
                let (_, unsafe_legacy, _) = self.dev_jobs.legacy_state()?;
                if unsafe_legacy
                    || state
                        .ledger
                        .records
                        .iter()
                        .filter(|r| active(&r.summary.status) || uncertain(r))
                        .count()
                        .saturating_add(self.retained_reserved())
                        >= SLOTS
                {
                    break;
                }
                let mut next = state.ledger.clone();
                let r = &mut next.records[index];
                r.summary.status = "preparing".into();
                r.summary.message = PREPARING.into();
                r.summary.cleanup_verified = false;
                r.cleanup_required = true;
                let job = r.job.clone();
                let fence = model_fence(&inner, &job, &r.config_revision);
                // Even the permission to spawn is published first. No container exists
                // while the only durable evidence is queued/cleanup-verified.
                if fence.is_err() {
                    let r = &mut next.records[index];
                    r.failed_stage = Some("queued".into());
                    r.summary.status = "failed".into();
                    r.summary.message = QUEUE_MODEL_CHANGED.into();
                    r.summary.cleanup_verified = true;
                    r.cleanup_required = false;
                    owner.commit(&mut state, next)?;
                    changed = true;
                    wake(&sender);
                    continue;
                }
                owner.commit(&mut state, next)?;
                (
                    job,
                    Instant::now() + Duration::from_secs(crate::dev_local::JOB_SECONDS),
                )
            };
            changed = true;
            wake(&sender);
            let owner_for_thread = owner.clone();
            let inner = self.inner.clone();
            let sender_for_thread = sender.clone();
            let request_id = job.request_id.clone();
            let spawned = std::thread::Builder::new()
                .name("smith-dev-container".into())
                .spawn(move || {
                    run_container(
                        owner_for_thread,
                        inner,
                        job,
                        original_deadline,
                        sender_for_thread,
                    );
                });
            match spawned {
                Ok(handle) => owner
                    .scheduler
                    .workers
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .push(handle),
                Err(_) => {
                    owner.update(&request_id, |r| {
                        r.failed_stage = Some("queued".into());
                        r.summary.status = "failed".into();
                        r.summary.message = SPAWN_FAILED.into();
                        r.summary.cleanup_verified = true;
                        r.cleanup_required = false;
                        Ok(())
                    })?;
                    wake(&sender);
                }
            }
        }
        Ok(changed)
    }
    pub(crate) fn run_dev_completion(&self, provider: &dyn Provider) -> AppResult<bool> {
        let Some(owner) = self.dev_jobs.local.as_ref() else {
            return Ok(false);
        };
        let request = owner
            .scheduler
            .mailbox
            .lock()
            .map_err(|_| unavailable())?
            .pop_front();
        let Some(request) = request else {
            return Ok(false);
        };
        let result = (|| {
            completion_fence(owner, &self.inner, &request)?;
            // No app, ledger or mailbox lock is held across inference.
            let result = provider
                .inspection_completion(&request.job.model, &request.body, request.deadline)
                .map_err(completion_error);
            completion_fence(owner, &self.inner, &request)?;
            result
        })();
        request.expected.fetch_add(1, Ordering::AcqRel);
        let _ = request.response.try_send(result);
        Ok(true)
    }
    pub(crate) fn join_dev_workers(&self) {
        if let Some(owner) = self.dev_jobs.local.as_ref() {
            // Worker-exit API: cancel callbacks before joining, without holding locks.
            owner.close();
            for handle in owner.scheduler.take_workers(true) {
                let _ = handle.join();
            }
        }
    }
}
fn run_container(
    owner: Arc<Owner>,
    inner: Arc<Mutex<Inner>>,
    job: LocalJob,
    original_deadline: Instant,
    sender: mpsc::SyncSender<()>,
) {
    let callback_failed = AtomicBool::new(false);
    let expected = Arc::new(AtomicU32::new(1));
    let mut ordinal = 0;

    let mut event = |event| {
        let result = (|| {
            if callback_failed.load(Ordering::Acquire) {
                return Err(protocol());
            }
            let inner = inner.lock().map_err(|_| storage())?;
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            {
                let state = owner.state.lock().map_err(|_| unavailable())?;
                owner.available(&state)?;
                let record = state
                    .ledger
                    .records
                    .iter()
                    .find(|r| r.summary.request_id == job.request_id)
                    .ok_or_else(unavailable)?;
                model_fence(&inner, &job, &record.config_revision)?;
            }
            owner.event(&job.request_id, event)
        })();
        if result.is_err() {
            callback_failed.store(true, Ordering::Release);
        }
        result
    };
    let mut completion = |body: &Value, deadline: Instant| {
        let result = (|| {
            if callback_failed.load(Ordering::Acquire) {
                return Err(protocol());
            }
            let deadline = deadline.min(original_deadline);
            ordinal += 1;
            let (response, receiver) = mpsc::sync_channel(1);
            let live = Arc::new(AtomicBool::new(true));
            let request = Completion {
                job: job.clone(),
                ordinal,
                expected: expected.clone(),
                body: body.clone(),
                deadline,
                live: live.clone(),
                response,
            };
            completion_fence(&owner, &inner, &request)?;
            {
                let mut mailbox = owner.scheduler.mailbox.lock().map_err(|_| unavailable())?;
                if owner.cancelled.load(Ordering::Acquire) || mailbox.len() >= SLOTS {
                    return Err(unavailable());
                }
                mailbox.push_back(request);
            }
            wake(&sender);
            let result = loop {
                if owner.cancelled.load(Ordering::Acquire) {
                    break Err(unavailable());
                }
                let now = Instant::now();
                if now >= deadline {
                    break Err(AppError::new(
                        "PROVIDER_TIMEOUT",
                        LocalError::Deadline.message(),
                    ));
                }
                match receiver.recv_timeout((deadline - now).min(Duration::from_millis(20))) {
                    Ok(result) => break result,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => break Err(unavailable()),
                }
            };
            live.store(false, Ordering::Release);
            result
        })();
        if result.is_err() {
            callback_failed.store(true, Ordering::Release);
        }
        result
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner
            .runtime
            .run(&job, &owner.cancelled, &mut event, &mut completion)
    }))
    .unwrap_or(Err(crate::dev_local::LocalFailure {
        error: LocalError::Protocol,
        may_have_sent: true,
        cleanup_unknown: true,
    }));
    let result = if callback_failed.load(Ordering::Acquire) && result.is_ok() {
        Err(crate::dev_local::LocalFailure {
            error: LocalError::Protocol,
            may_have_sent: true,
            cleanup_unknown: true,
        })
    } else {
        result
    };
    if let Ok(inner) = inner.lock() {
        if !inner.fault && inner.owned {
            let _ = owner.finish(&job.request_id, result);
        }
    }
    wake(&sender);
}
