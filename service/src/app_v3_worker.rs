//! Sole-worker execution and fsync-before-dispatch publication fences.
use super::*;

fn safe_key_error(e: AppError) -> AppError {
    cloud::error(match e.code.as_str() {
        "KEYRING_LOCKED" => "KEYRING_LOCKED",
        "KEYRING_MISSING" => "KEYRING_MISSING",
        "KEYRING_CONFLICT" => "KEYRING_CONFLICT",
        "KEYRING_PROMPT_REQUIRED" => "KEYRING_PROMPT_REQUIRED",
        "KEYRING_TIMEOUT" => "KEYRING_TIMEOUT",
        "CREDENTIAL_OUTCOME_UNKNOWN" => "CREDENTIAL_OUTCOME_UNKNOWN",
        "INTERRUPTED" => "INTERRUPTED",
        _ => "KEYRING_UNAVAILABLE",
    })
}
fn safe_http_error(e: AppError) -> AppError {
    cloud::error(match e.code.as_str() {
        "CLOUD_AUTH" => "CLOUD_AUTH",
        "CLOUD_QUOTA" => "CLOUD_QUOTA",
        "CLOUD_RATE_LIMIT" => "CLOUD_RATE_LIMIT",
        "CLOUD_UNAVAILABLE" => "CLOUD_UNAVAILABLE",
        "INTERRUPTED" => "INTERRUPTED",
        "PERMISSION_EXPIRED" => "PERMISSION_EXPIRED",
        "REMOTE_OUTCOME_UNKNOWN" => "REMOTE_OUTCOME_UNKNOWN",
        _ => "CLOUD_PROTOCOL",
    })
}
fn fence(inner: &Inner, work: &Work) -> bool {
    !inner.fault
        && inner.owned
        && inner.durable.jobs.iter().any(|j| {
            j.operation.request_id == work.id
                && j.operation.status == "running"
                && j.conversation_id == work.conversation_id
                && j.history_revision == work.history_revision
        })
        && work.conversation_id.as_ref().is_none_or(|id| {
            inner.durable.conversations.iter().any(|c| {
                &c.conversation_id == id
                    && Some(c.history_revision) == work.history_revision
                    && !c.archived
            })
        })
}
impl Application {
    fn permission(&self, work: &Work) -> AppResult<Instant> {
        let inner = self.inner.lock().map_err(|_| storage())?;
        if !fence(&inner, work) || self.cloud.cancelled.load(Ordering::SeqCst) {
            return Err(storage());
        }
        let deadline = inner
            .cloud
            .jobs
            .get(&work.id)
            .ok_or_else(|| cloud::error("PERMISSION_EXPIRED"))?
            .deadline;
        if self.cloud.clock.now() >= deadline {
            return Err(cloud::error("PERMISSION_EXPIRED"));
        }
        Ok(deadline)
    }
    fn credential_identity(
        &self,
        work: &Work,
        identity: &CredentialRef,
        revision: u64,
    ) -> AppResult<()> {
        let inner = self.inner.lock().map_err(|_| storage())?;
        if !fence(&inner, work) {
            return Err(storage());
        }
        let slot = &inner.durable.cloud.providers[identity.provider.index()];
        if slot.revision != revision || slot.credential.as_ref() != Some(identity) {
            return Err(cloud::error("CREDENTIAL_CHANGED"));
        }
        Ok(())
    }
    fn key(
        &self,
        work: &Work,
        identity: &CredentialRef,
        revision: u64,
        deadline: Instant,
    ) -> AppResult<Secret> {
        self.credential_identity(work, identity, revision)?;
        let deadline = deadline.min(self.cloud.clock.now() + cloud::KEYRING_BUDGET);
        let mut result = self
            .cloud
            .credentials
            .read(identity, deadline, &self.cloud.cancelled)
            .map_err(safe_key_error);
        if self.cloud.clock.now() >= deadline {
            result = Err(cloud::error("KEYRING_TIMEOUT"));
        }
        if self.cloud.cancelled.load(Ordering::SeqCst) {
            result = Err(cloud::error("INTERRUPTED"));
        }
        if let Ok(mut inner) = self.inner.lock() {
            if fence(&inner, work) {
                inner.cloud.observations[identity.provider.index()] =
                    Some(result.as_ref().map(|_| ()).map_err(Clone::clone));
            }
        }
        result
    }
    fn mark_dispatch(&self, work: &Work) -> AppResult<()> {
        self.permission(work)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if !fence(&inner, work) {
            return Err(storage());
        }
        let mut next = inner.durable.clone();
        let job = next
            .jobs
            .iter_mut()
            .find(|j| j.operation.request_id == work.id)
            .ok_or_else(storage)?;
        match &mut job.task {
            Some(Task::Routed(t)) => t.delivery = Delivery::MayHaveSent,
            Some(Task::Provider(t)) => t.delivery = Delivery::MayHaveSent,
            _ => return Err(storage()),
        }
        self.commit_worker(&mut inner, next)
    }
    fn cloud_completion(
        &self,
        work: &Work,
        route: &Route,
        identity: &CredentialRef,
        revision: u64,
        messages: &[Message],
        delivery: &mut Delivery,
    ) -> AppResult<String> {
        let start = self.cloud.clock.now();
        let operation_deadline = start + cloud::BUDGET;
        self.permission(work)?;
        let vendor = route.vendor().ok_or_else(invalid)?;
        let body = cloud::build_body(vendor, &route.model, messages)?;
        let secret = self.key(work, identity, revision, operation_deadline)?;
        self.credential_identity(work, identity, revision)?;
        self.mark_dispatch(work)?;
        *delivery = Delivery::MayHaveSent;
        // fsync has no real-time bound. Check original permission again after it.
        let dispatch_deadline = match self.permission(work) {
            Ok(t) => t,
            Err(e) => {
                *delivery = Delivery::NotSent;
                return Err(e);
            }
        };
        if self.cloud.clock.now() >= operation_deadline {
            *delivery = Delivery::NotSent;
            return Err(cloud::error("PERMISSION_EXPIRED"));
        }
        let response = match self.cloud.http.post(
            vendor,
            body,
            &secret,
            operation_deadline,
            dispatch_deadline,
            &self.cloud.cancelled,
        ) {
            Ok(r) => {
                *delivery = Delivery::Received;
                r
            }
            Err(f) => {
                if f.not_sent {
                    *delivery = Delivery::NotSent;
                }
                return Err(safe_http_error(f.error));
            }
        };
        if self.cloud.clock.now() >= operation_deadline {
            return Err(cloud::error("REMOTE_OUTCOME_UNKNOWN"));
        }
        let final_text = cloud::parse_response(vendor, &route.model, &response)?;
        // Even a compromised vendor must not turn the authentication header into
        // a persisted model result. Reject the whole result, never mask/retry it.
        if std::str::from_utf8(secret.expose()).is_ok_and(|key| final_text.contains(key)) {
            return Err(cloud::error("CLOUD_PROTOCOL"));
        }
        if self.cloud.clock.now() >= operation_deadline {
            return Err(cloud::error("REMOTE_OUTCOME_UNKNOWN"));
        }
        Ok(final_text)
    }
    pub(in super::super) fn run_v3(&self, work: Work, provider: &dyn Provider) {
        let mut delivery = match &work.task {
            Task::Routed(t) => t.delivery,
            Task::Provider(t) => t.delivery,
            _ => return,
        };
        let result = (|| -> AppResult<Value> {
            match &work.task {
                Task::Routed(t) => {
                    if t.policy != cloud::POLICY {
                        return Err(cloud::error("ROUTE_UNAVAILABLE"));
                    }
                    let assistant = if let Some(identity) = &t.credential {
                        self.cloud_completion(
                            &work,
                            &t.route,
                            identity,
                            t.provider_revision.ok_or_else(invalid)?,
                            &t.messages,
                            &mut delivery,
                        )?
                    } else {
                        let inner = self.inner.lock().map_err(|_| storage())?;
                        if !fence(&inner, &work) {
                            return Err(storage());
                        }
                        drop(inner);
                        let inventory = provider.inventory()?;
                        if !inventory.valid()
                            || !inventory.models.iter().any(|m| {
                                m.name == t.route.model
                                    && Some(&m.digest) == t.local_digest.as_ref()
                            })
                        {
                            return Err(AppError::new("MODEL_UNAVAILABLE","The accepted local model identity is unavailable; no substitution was made"));
                        }
                        provider.chat(&t.route.model, &t.messages)?
                    };
                    if assistant.trim().is_empty() || assistant.len() > MAX_CONTEXT {
                        return Err(cloud::error("MODEL_RESULT_LIMIT"));
                    }
                    Ok(json!({"assistant_text":assistant,"audio":null,"speech_error":null}))
                }
                Task::Provider(t) => {
                    match t.action {
                        Action::Probe => {
                            if let Some(identity) = &t.credential {
                                let result = self.key(
                                    &work,
                                    identity,
                                    t.provider_revision,
                                    self.cloud.clock.now() + cloud::KEYRING_BUDGET,
                                );
                                if result.as_ref().is_err_and(|e| {
                                    e.code == "INTERRUPTED" || e.code == "STATE_UNAVAILABLE"
                                }) {
                                    return Err(result.err().unwrap_or_else(storage));
                                }
                            }
                            Ok(Value::Null) // authoritative ProviderView constructed at commit
                        }
                        Action::Save => {
                            let deadline = self.permission(&work)?;
                            let secret = {
                                let mut inner = self.inner.lock().map_err(|_| storage())?;
                                if !fence(&inner, &work) {
                                    return Err(storage());
                                }
                                inner
                                    .cloud
                                    .jobs
                                    .get_mut(&work.id)
                                    .and_then(|l| l.secret.take())
                                    .ok_or_else(|| cloud::error("CREDENTIAL_OUTCOME_UNKNOWN"))?
                            };
                            let identity = t.credential.as_ref().ok_or_else(invalid)?;
                            self.cloud
                                .credentials
                                .save(
                                    identity,
                                    &secret,
                                    deadline.min(self.cloud.clock.now() + cloud::KEYRING_BUDGET),
                                    &self.cloud.cancelled,
                                )
                                .map_err(safe_key_error)?;
                            if self.cloud.cancelled.load(Ordering::SeqCst) {
                                return Err(cloud::error("CREDENTIAL_OUTCOME_UNKNOWN"));
                            }
                            Ok(Value::Null)
                        }
                        Action::Validate => {
                            let route = provider_route(t)?;
                            let assistant = self.cloud_completion(
                                &work,
                                &route,
                                t.credential.as_ref().ok_or_else(invalid)?,
                                t.provider_revision,
                                &[Message {
                                    role: "user".into(),
                                    content: cloud::VALIDATION_TEXT.into(),
                                }],
                                &mut delivery,
                            )?;
                            Ok(json!({"assistant_text":assistant,"route":route}))
                        }
                    }
                }
                _ => Err(invalid()),
            }
        })();
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if !fence(&inner, &work) {
            return;
        }
        let mut next = inner.durable.clone();
        let Some(index) = next
            .jobs
            .iter()
            .position(|j| j.operation.request_id == work.id)
        else {
            return;
        };
        match &mut next.jobs[index].task {
            Some(Task::Routed(t)) => t.delivery = delivery,
            Some(Task::Provider(t)) => t.delivery = delivery,
            _ => return,
        }
        next.jobs[index].operation.phase = "complete".into();
        match result {
            Ok(mut value) => {
                next.jobs[index].operation.status = "succeeded".into();
                match &work.task {
                    Task::Routed(t) => {
                        let Some(conversation) = next
                            .conversations
                            .iter_mut()
                            .find(|c| Some(&c.conversation_id) == work.conversation_id.as_ref())
                        else {
                            return;
                        };
                        let Some(user) = t.messages.last() else {
                            return;
                        };
                        let Some(assistant) = value.get("assistant_text").and_then(Value::as_str)
                        else {
                            return;
                        };
                        let previous_pairs = conversation.history.len() / 2;
                        append_provenance(
                            &mut next.cloud,
                            &conversation.conversation_id,
                            conversation.history.len() / 2,
                            Some(work.id.clone()),
                            Some(t.route.clone()),
                        );
                        conversation.history.push(user.clone());
                        conversation.history.push(Message {
                            role: "assistant".into(),
                            content: assistant.into(),
                        });
                        conversation.history_revision += 1;
                        while conversation.history.len() > 32
                            || conversation
                                .history
                                .iter()
                                .map(|m| m.content.len())
                                .sum::<usize>()
                                > MAX_CONTEXT
                        {
                            conversation.history.drain(..2);
                        }
                        trim_provenance(
                            &mut next.cloud,
                            &conversation.conversation_id,
                            conversation.history.len() / 2,
                        );
                        timestamps::append(
                            &mut next.timestamps,
                            &conversation.conversation_id,
                            previous_pairs,
                            conversation.history.len() / 2,
                            next.jobs[index].accepted_ms,
                            self.cloud.clock.wall_ms(),
                        );
                    }
                    Task::Provider(t) => {
                        let p = &mut next.cloud.providers[t.provider.index()];
                        if p.revision != t.provider_revision {
                            return;
                        }
                        if t.action == Action::Save {
                            p.credential = t.credential.clone();
                            p.revision += 1;
                            p.validation = None;
                            inner.cloud.observations[t.provider.index()] = Some(Ok(()));
                        } else if t.action == Action::Validate {
                            let mut task = t.clone();
                            task.delivery = delivery;
                            p.validation = Some(validation(
                                &next.jobs[index],
                                &task,
                                self.cloud.clock.wall_ms(),
                            ));
                        }
                        let observed =
                            view(p, inner.cloud.observations[t.provider.index()].as_ref());
                        if t.action == Action::Validate {
                            value["provider"] = json!(observed);
                        } else {
                            value = json!({"provider":observed});
                        }
                    }
                    _ => return,
                }
                next.jobs[index].operation.result = Some(value);
            }
            Err(error) => {
                next.jobs[index].operation.status = "failed".into();
                next.jobs[index].operation.error = Some(bounded_error(error));
                if let Some(Task::Provider(t)) = &next.jobs[index].task {
                    if t.action == Action::Validate {
                        let p = &mut next.cloud.providers[t.provider.index()];
                        if p.credential == t.credential {
                            p.validation =
                                Some(validation(&next.jobs[index], t, self.cloud.clock.wall_ms()));
                        }
                    }
                }
            }
        }
        if self.commit_worker(&mut inner, next).is_ok() {
            inner.cloud.jobs.remove(&work.id);
        }
    }
}
