use super::*;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn queued() -> Durable {
    let mut d = Durable::default();
    let c = d.allocate(Some(id(1)), "A chat".into()).unwrap();
    let credential = CredentialRef {
        provider: Vendor::Openai,
        generation: id(2),
        save_request_id: id(3),
    };
    d.cloud.providers[0].credential = Some(credential.clone());
    d.cloud.providers[0].revision = 1;
    let route = Route {
        provider: "openai".into(),
        model: "gpt-6-astra".into(),
        credential_generation: Some(id(2)),
    };
    d.cloud.conversations.insert(
        c.clone(),
        Routing {
            last_route: Some(route.clone()),
            approved_destinations: vec![route.destination().unwrap()],
            turn_routes: vec![],
        },
    );
    let messages = vec![Message {
        role: "user".into(),
        content: "test input".into(),
    }];
    let task = RoutedTask {
        route,
        credential: Some(credential),
        provider_revision: Some(1),
        local_digest: None,
        endpoint: Some(Vendor::Openai.endpoint().into()),
        policy: cloud::POLICY.into(),
        config_revision: 0,
        history_revision: 0,
        context_sha256: digest_messages(&messages).unwrap(),
        messages,
        output_tokens: 8192,
        budget_ms: 180000,
        dispatch_expires_at_ms: Some(900000),
        delivery: Delivery::NotSent,
    };
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(1),
            kind: "submit_new_routed".into(),
            status: "queued".into(),
            phase: "queued".into(),
            config_revision: 0,
            result: None,
            error: None,
        },
        fingerprint: "a".repeat(64),
        version: 3,
        conversation_id: Some(c),
        history_revision: Some(0),
        task: Some(Task::Routed(task)),
        accepted_ms: 0,
    });
    d
}
#[test]
fn cloud_transition_reservation_includes_durable_send_marker() {
    let mut d = queued();
    d.revision = MAX_REVISION - 3;
    assert_eq!(transitions(&d.jobs[0]), 3);
    let bytes = d.reserved_bytes().unwrap();
    d.validate().unwrap();
    let mut exceeded = d.clone();
    exceeded.revision += 1;
    assert_eq!(exceeded.preflight().unwrap_err().code, "CAPACITY");
    d.jobs[0].operation.status = "running".into();
    d.jobs[0].operation.phase = "generation".into();
    d.revision += 1;
    assert_eq!(transitions(&d.jobs[0]), 2);
    assert_eq!(d.reserved_bytes().unwrap(), bytes);
    d.validate().unwrap();
    if let Some(Task::Routed(t)) = &mut d.jobs[0].task {
        t.delivery = Delivery::MayHaveSent;
    }
    d.revision += 1;
    assert_eq!(transitions(&d.jobs[0]), 1);
    assert_eq!(d.reserved_bytes().unwrap(), bytes);
    d.validate().unwrap();
    d.jobs[0].operation.status = "failed".into();
    d.jobs[0].operation.phase = "complete".into();
    d.jobs[0].operation.error = Some(cloud::error("REMOTE_OUTCOME_UNKNOWN"));
    d.revision += 1;
    d.validate().unwrap();
}
#[test]
fn disk3_snapshot_corruption_is_not_normalized() {
    let d = queued();
    d.validate().unwrap();
    for pointer in [
        "/jobs/0/task/Routed/credential",
        "/jobs/0/task/Routed/endpoint",
        "/jobs/0/task/Routed/local_digest",
        "/jobs/0/task/Routed/dispatch_expires_at_ms",
        "/cloud/providers/0/credential",
    ] {
        let mut v = serde_json::to_value(&d).unwrap();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            Durable::load(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for (pointer, value) in [
        (
            "/jobs/0/task/Routed/endpoint",
            json!("https://evil.invalid"),
        ),
        ("/jobs/0/task/Routed/output_tokens", json!(8193)),
        ("/jobs/0/task/Routed/messages/0/content", json!("drift")),
        ("/jobs/0/task/Routed/provider_revision", json!(2)),
        ("/cloud/providers/0/provider", json!("anthropic")),
    ] {
        let mut v = serde_json::to_value(&d).unwrap();
        *v.pointer_mut(pointer).unwrap() = value;
        let decoded: Durable = serde_json::from_value(v).unwrap();
        assert!(decoded.validate().is_err(), "{pointer}");
    }
}
#[test]
fn history_drift_and_permission_removal_invalidate_pending_snapshot() {
    let mut d = queued();
    d.conversations[0].history = vec![
        Message {
            role: "user".into(),
            content: "intervening".into(),
        },
        Message {
            role: "assistant".into(),
            content: "answer".into(),
        },
    ];
    let c = d.conversations[0].conversation_id.clone();
    d.cloud.conversations.get_mut(&c).unwrap().turn_routes = vec![Provenance {
        request_id: None,
        route: None,
    }];
    assert!(d.validate().is_err());
    let mut d = queued();

    let id = d.conversations[0].conversation_id.clone();
    d.cloud
        .conversations
        .get_mut(&id)
        .unwrap()
        .approved_destinations
        .clear();
    assert!(d.validate().is_err());
}
#[test]
fn expired_preparation_ids_are_bounded_without_silent_renewal() {
    let mut v = Volatile::default();
    let mut now = Instant::now();
    for batch in 0..32 {
        for slot in 0..32 {
            let id = id(batch * 32 + slot);
            assert!(v.prepare(&id, "public", now).unwrap().is_none());
            v.preparations.insert(
                id,
                Preparation {
                    fingerprint: "public".into(),
                    token: "a".repeat(32),
                    expires: now + Duration::from_secs(60),
                    dispatch: now + Duration::from_secs(600),
                    result: json!({}),
                    kind: Prepared::Route {
                        context: "b".repeat(64),
                        requires_consent: true,
                    },
                },
            );
        }
        now += Duration::from_secs(61);
        v.expire(now);
    }
    assert_eq!(v.expired.len(), 1024);
    assert_eq!(
        v.prepare(&id(0), "public", now).unwrap_err().code,
        "CONSENT_EXPIRED"
    );
    assert_eq!(
        v.prepare(&id(2000), "public", now).unwrap_err().code,
        "CAPACITY"
    );
}
#[test]
fn unavailable_historical_model_does_not_invalidate_retained_chat() {
    let mut d = queued();
    d.jobs[0].operation.status = "interrupted".into();
    d.jobs[0].operation.phase = "complete".into();
    d.jobs[0].operation.error = Some(cloud::error("INTERRUPTED"));
    if let Some(Task::Routed(t)) = &mut d.jobs[0].task {
        t.route.model = "historical-unavailable-model".into();
    }
    d.cloud
        .conversations
        .values_mut()
        .next()
        .unwrap()
        .last_route
        .as_mut()
        .unwrap()
        .model = "historical-unavailable-model".into();
    d.validate().unwrap();
}
