use super::*;
use crate::app::new_id;

struct Fixture {
    dir: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("smith-retained-{}", new_id().unwrap()));
        filesystem::mkdir(&dir).unwrap();
        let store = Store::open(dir.join("jobs.json")).unwrap();
        Self { dir, store }
    }
    fn start(&self, job_type: JobType) -> Receipt {
        self.store.start(self.request(job_type)).unwrap()
    }
    fn request(&self, job_type: JobType) -> Start {
        Start {
            operation_id: new_id().unwrap(),
            session_id: new_id().unwrap(),
            expected_revision: self.store.revision().unwrap(),
            job_type,
            prompt: "Inspect or edit the private snapshot".into(),
            created_ms: 1,
            project: FrozenProject {
                project_id: new_id().unwrap(),
                project_key: "smith".into(),
                context: "Exact project context".into(),
                repositories: vec![
                    Repository {
                        url: "https://example.invalid/source.git".into(),
                        reference: Some("main".into()),
                        read_only: false,
                    },
                    Repository {
                        url: "https://example.invalid/reference.git".into(),
                        reference: None,
                        read_only: true,
                    },
                ],
            },
        }
    }
    fn change(&self, r: &Receipt, change: ChangeKind) -> Receipt {
        self.store
            .change(Change {
                operation_id: new_id().unwrap(),
                session_id: r.session_id.clone(),
                expected_revision: self.store.revision().unwrap(),
                change,
            })
            .unwrap()
    }
    fn ack(&self, c: Command, outcome: Outcome) {
        self.store
            .acknowledge(Acknowledgement {
                session_id: c.session_id,
                operation_id: c.operation_id,
                action: c.action,
                outcome,
            })
            .unwrap();
    }
    fn ready(&self, r: &Receipt) {
        let c = self.store.prepare_next(0).unwrap().unwrap();
        assert_eq!(c.session_id, r.session_id);
        self.ack(c, Outcome::Ready);
        let c = self.store.take_command(r).unwrap();
        self.ack(c, Outcome::Reply("Initial response".into()));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn ordinary_replay_capacity_cannot_consume_finish_and_delete_allowance() {
    let f = Fixture::new();
    for index in 0..8 {
        let r = f.start(JobType::Read);
        f.ready(&r);
        for _ in 0..31 {
            let p = f.change(
                &r,
                ChangeKind::Prompt {
                    prompt: "Next known turn".into(),
                },
            );
            let c = f.store.take_command(&p).unwrap();
            f.ack(c, Outcome::Reply("ok".into()));
        }
        if index == 7 {
            assert_eq!(
                f.store.start(f.request(JobType::Read)).unwrap_err(),
                Error::Capacity
            );
        }
        let finish = f.change(&r, ChangeKind::Finish);
        let c = f.store.take_command(&finish).unwrap();
        f.ack(c, Outcome::Finished);
        let before = f.store.revision().unwrap();
        assert_eq!(
            f.store
                .change(Change {
                    operation_id: new_id().unwrap(),
                    session_id: r.session_id.clone(),
                    expected_revision: before,
                    change: ChangeKind::Finish
                })
                .unwrap_err(),
            Error::Blocked
        );
        assert_eq!(f.store.revision().unwrap(), before);
        f.change(&r, ChangeKind::Delete);
    }
    assert_eq!(f.store.lock().unwrap().ledger.operations.len(), 272);
}

#[test]
fn ordinary_byte_exhaustion_keeps_cleanup_headroom() {
    let f = Fixture::new();
    let first = f.start(JobType::Read);
    f.ready(&first);
    let second = f.start(JobType::Read);
    f.ready(&second);
    let mut full = false;
    'admit: for _ in 0..31 {
        for r in [&first, &second] {
            let request = Change {
                operation_id: new_id().unwrap(),
                session_id: r.session_id.clone(),
                expected_revision: f.store.revision().unwrap(),
                change: ChangeKind::Prompt {
                    prompt: "Bounded reply".into(),
                },
            };
            match f.store.change(request) {
                Ok(p) => {
                    let c = f.store.take_command(&p).unwrap();
                    f.ack(c, Outcome::Reply("\u{1}".repeat(MAX_REPLY)));
                }
                Err(Error::Capacity) => {
                    full = true;
                    break 'admit;
                }
                other => panic!("unexpected admission: {other:?}"),
            }
        }
    }
    assert!(full);
    for r in [&first, &second] {
        let finish = f.change(r, ChangeKind::Finish);
        let c = f.store.take_command(&finish).unwrap();
        f.ack(c, Outcome::Finished);
        f.change(r, ChangeKind::Delete);
    }
    assert_eq!(f.store.reserved(), 0);
}

#[test]
fn pending_selection_rechecks_disk_content_and_mode_before_effects() {
    use std::os::unix::fs::PermissionsExt;
    for take_receipt in [false, true] {
        for permissions in [false, true] {
            for finish in [false, true] {
                let f = Fixture::new();
                let r = f.start(JobType::Read);
                f.ready(&r);
                let p = f.change(
                    &r,
                    if finish {
                        ChangeKind::Finish
                    } else {
                        ChangeKind::Prompt {
                            prompt: "Pending".into(),
                        }
                    },
                );
                let path = f.dir.join("jobs.json");
                if permissions {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
                        .unwrap();
                } else {
                    filesystem::publish(&path, b"{}", 0o600, true).unwrap();
                }
                if take_receipt {
                    assert_eq!(f.store.take_command(&p).unwrap_err(), Error::Storage);
                } else {
                    assert_eq!(f.store.take_pending().unwrap_err(), Error::Storage);
                }
                assert_eq!(f.store.reserved(), SLOT_LIMIT);
                assert_eq!(f.store.take_pending().unwrap_err(), Error::Storage);
            }
        }
    }
}

#[test]
fn strict_ledger_rejects_contradictory_turns_and_unbound_pending_commands() {
    let f = Fixture::new();
    let r = f.start(JobType::Read);
    f.ready(&r);
    let ready = serde_json::to_value(&f.store.lock().unwrap().ledger).unwrap();
    let p = f.change(
        &r,
        ChangeKind::Prompt {
            prompt: "Actual pending text".into(),
        },
    );
    let pending = serde_json::to_value(&f.store.lock().unwrap().ledger).unwrap();
    let mut invalid = Vec::new();
    let mut v = ready.clone();
    v["sessions"][0]["turns"][0]["reply"] = serde_json::Value::Null;
    invalid.push(v);
    let mut v = ready.clone();
    v["sessions"][0]["turns"][0]["outcome"] = serde_json::json!("failed");
    invalid.push(v);
    let mut v = ready.clone();
    let duplicate = v["sessions"][0]["turns"][0].clone();
    v["sessions"][0]["turns"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    invalid.push(v);
    let mut v = pending.clone();
    v["sessions"][0]["turns"][1]["reply"] = serde_json::json!("contradictory");
    invalid.push(v);
    let mut v = pending.clone();
    v["sessions"][0]["pending"]["prompt"] = serde_json::json!("Different pending text");
    invalid.push(v);
    let mut v = pending.clone();
    v["sessions"][0]["pending"]["operation_id"] = serde_json::json!(r.operation_id);
    invalid.push(v);
    let mut v = pending.clone();
    v["sessions"][0]["job"]["status"] = serde_json::json!("ready");
    v["sessions"][0]["pending"] = serde_json::Value::Null;
    invalid.push(v);
    let mut v = pending;
    v["sessions"][0]["pending"]
        .as_object_mut()
        .unwrap()
        .remove("prompt");
    invalid.push(v);
    for v in invalid {
        let bytes = serde_json::to_vec(&v).unwrap();
        filesystem::publish(&f.dir.join("jobs.json"), &bytes, 0o600, true).unwrap();
        assert!(
            Store::open(f.dir.join("jobs.json")).is_err(),
            "invalid fixture accepted"
        );
        assert_eq!(std::fs::read(f.dir.join("jobs.json")).unwrap(), bytes);
    }
    assert_eq!(p.session_id, r.session_id);
}

#[test]
fn composition_budget_checks_utf8_and_json_frame_before_admission() {
    let f = Fixture::new();
    let mut request = f.request(JobType::Read);
    let available = MAX_PROMPT - request.project.context.len() - "\nTask:\n".len();
    request.prompt = "x".repeat(available);
    assert_eq!(
        compose_prompt(&request.project, &request.prompt)
            .unwrap()
            .len(),
        MAX_PROMPT
    );
    let mut overflow = request.clone();
    overflow.prompt.push('x');
    assert_eq!(f.store.start(overflow).unwrap_err(), Error::ContextLimit);
    let mut escaped = request.clone();
    escaped.prompt = "\u{1}".repeat(6000);
    assert_eq!(f.store.start(escaped).unwrap_err(), Error::ContextLimit);
    assert_eq!(f.store.revision().unwrap(), 0);
    let r = f.store.start(request).unwrap();
    let create = f.store.prepare_next(0).unwrap().unwrap();
    f.ack(create, Outcome::Ready);
    let prompt = f.store.take_command(&r).unwrap();
    f.ack(prompt, Outcome::Reply("ok".into()));
    let before = f.store.revision().unwrap();
    assert_eq!(
        f.store
            .change(Change {
                operation_id: new_id().unwrap(),
                session_id: r.session_id,
                expected_revision: before,
                change: ChangeKind::Prompt {
                    prompt: "x".repeat(MAX_PROMPT)
                }
            })
            .unwrap_err(),
        Error::ContextLimit
    );
    assert_eq!(f.store.revision().unwrap(), before);
}

#[test]
fn active_delete_only_hides_after_verified_cleanup() {
    let f = Fixture::new();
    let r = f.start(JobType::Write);
    f.ready(&r);
    let delete = f.change(&r, ChangeKind::Delete);
    let s = f.store.get(&r.session_id).unwrap();
    assert!(!s.job.deleted);
    assert!(s.job.retained);
    let c = f.store.take_command(&delete).unwrap();
    assert_eq!(c.action, Action::Finish);
    f.ack(c, Outcome::Finished);
    let s = f.store.get(&r.session_id).unwrap();
    assert!(s.job.deleted);
    assert!(!s.job.retained);
}

#[test]
fn dispatch_identity_is_on_disk_before_create_and_is_never_taken_twice() {
    let f = Fixture::new();
    let r = f.start(JobType::Read);
    let c = f.store.prepare_next(0).unwrap().unwrap();
    let disk: Ledger =
        serde_json::from_slice(&std::fs::read(f.dir.join("jobs.json")).unwrap()).unwrap();
    let s = &disk.sessions[0];
    assert_eq!(s.job.status, Status::Creating);
    assert_eq!(s.pending.as_ref().unwrap().operation_id, c.operation_id);
    assert_eq!(f.store.take_command(&r).unwrap_err(), Error::Blocked);
    let reopened = Store::open(f.dir.join("jobs.json")).unwrap();
    assert_eq!(
        reopened.get(&r.session_id).unwrap().job.status,
        Status::Unknown
    );
    assert!(reopened.prepare_next(0).unwrap().is_none());
    assert_eq!(reopened.take_command(&r).unwrap_err(), Error::Blocked);
}

#[test]
fn oversized_reply_becomes_unknown_without_publishing_unbounded_text() {
    let f = Fixture::new();
    let r = f.start(JobType::Read);
    let c = f.store.prepare_next(0).unwrap().unwrap();
    f.ack(c, Outcome::Ready);
    let c = f.store.take_command(&r).unwrap();
    f.ack(c, Outcome::Reply("x".repeat(MAX_REPLY + 1)));
    let s = f.store.get(&r.session_id).unwrap();
    assert_eq!(s.job.status, Status::Unknown);
    assert!(s.turns[0].reply.is_none());
    assert_eq!(f.store.reserved(), 1);
}

#[test]
fn read_and_write_permissions_are_frozen_without_overriding_read_only() {
    let f = Fixture::new();
    let read = f.start(JobType::Read);
    assert!(f
        .store
        .get(&read.session_id)
        .unwrap()
        .project
        .repositories
        .iter()
        .all(|r| r.read_only));
    let write = f.start(JobType::Write);
    let s = f.store.get(&write.session_id).unwrap();
    assert!(!s.project.repositories[0].read_only);
    assert!(s.project.repositories[1].read_only);
}
#[test]
fn success_and_known_failure_keep_environment_and_allow_explicit_followup() {
    let f = Fixture::new();
    let r = f.start(JobType::Write);
    f.ready(&r);
    assert_eq!(f.store.reserved(), 1);
    let p = f.change(
        &r,
        ChangeKind::Prompt {
            prompt: "Try the next change".into(),
        },
    );
    let c = f.store.take_command(&p).unwrap();
    f.ack(c, Outcome::Failed);
    let s = f.store.get(&r.session_id).unwrap();
    assert_eq!(s.job.status, Status::Failed);
    assert!(s.job.retained);
    let p = f.change(
        &r,
        ChangeKind::Prompt {
            prompt: "Fix that known failure".into(),
        },
    );
    let c = f.store.take_command(&p).unwrap();
    f.ack(c, Outcome::Reply("Fixed".into()));
    let s = f.store.get(&r.session_id).unwrap();
    assert_eq!(s.turns.len(), 3);
    assert_eq!(s.turns[1].outcome, TurnOutcome::Failed);
    assert_eq!(s.job.status, Status::Ready);
}
#[test]
fn third_retained_blocks_fourth_until_verified_finish() {
    let f = Fixture::new();
    for _ in 0..3 {
        let r = f.start(JobType::Read);
        f.ready(&r);
    }
    let fourth = f.start(JobType::Read);
    assert!(f.store.prepare_next(0).unwrap().is_none());
    assert_eq!(
        f.store.get(&fourth.session_id).unwrap().job.status,
        Status::Queued
    );
    let first = f.store.sessions().unwrap()[0].job.session_id.clone();
    let r = Receipt {
        operation_id: new_id().unwrap(),
        session_id: first,
        revision: 0,
    };
    let finish = f.change(&r, ChangeKind::Finish);
    assert_eq!(f.store.reserved(), 3);
    let c = f.store.take_command(&finish).unwrap();
    f.ack(c, Outcome::Finished);
    assert_eq!(f.store.reserved(), 2);
    assert_eq!(
        f.store.prepare_next(0).unwrap().unwrap().session_id,
        fourth.session_id
    );
}
#[test]
fn inspection_occupancy_shares_three_slot_budget() {
    let f = Fixture::new();
    f.start(JobType::Read);
    assert!(f.store.prepare_next(3).unwrap().is_none());
    assert!(f.store.prepare_next(2).unwrap().is_some());
    f.start(JobType::Write);
    assert!(f.store.prepare_next(2).unwrap().is_none());
}
#[test]
fn unknown_prompt_is_observation_only_and_retains_slot() {
    let f = Fixture::new();
    let r = f.start(JobType::Read);
    f.ready(&r);
    let p = f.change(
        &r,
        ChangeKind::Prompt {
            prompt: "Next".into(),
        },
    );
    let c = f.store.take_command(&p).unwrap();
    f.ack(c, Outcome::Unknown);
    for change in [
        ChangeKind::Prompt {
            prompt: "Unsafe replay".into(),
        },
        ChangeKind::Finish,
        ChangeKind::Delete,
    ] {
        assert_eq!(
            f.store
                .change(Change {
                    operation_id: new_id().unwrap(),
                    session_id: r.session_id.clone(),
                    expected_revision: f.store.revision().unwrap(),
                    change
                })
                .unwrap_err(),
            Error::Blocked
        );
    }
    assert_eq!(f.store.reserved(), 1);
}
#[test]
fn cleanup_unknown_never_frees_capacity_or_allows_delete() {
    let f = Fixture::new();
    let r = f.start(JobType::Read);
    f.ready(&r);
    let finish = f.change(&r, ChangeKind::Finish);
    let c = f.store.take_command(&finish).unwrap();
    f.ack(c, Outcome::Unknown);
    assert_eq!(f.store.reserved(), 1);
    assert_eq!(
        f.store.get(&r.session_id).unwrap().job.status,
        Status::CleanupUnknown
    );
    assert_eq!(
        f.store
            .change(Change {
                operation_id: new_id().unwrap(),
                session_id: r.session_id,
                expected_revision: f.store.revision().unwrap(),
                change: ChangeKind::Delete
            })
            .unwrap_err(),
        Error::Blocked
    );
}
#[test]
fn restart_preserves_turns_but_never_replays_queued_or_retained_work() {
    let f = Fixture::new();
    let r = f.start(JobType::Write);
    f.ready(&r);
    let queued = f.start(JobType::Read);
    let reopened = Store::open(f.dir.join("jobs.json")).unwrap();
    assert_eq!(
        reopened.get(&r.session_id).unwrap().job.status,
        Status::Unknown
    );
    assert_eq!(
        reopened.get(&queued.session_id).unwrap().job.status,
        Status::Interrupted
    );
    assert_eq!(
        reopened.get(&r.session_id).unwrap().turns[0]
            .reply
            .as_deref(),
        Some("Initial response")
    );
    assert_eq!(reopened.reserved(), 1);
    assert!(reopened.prepare_next(0).unwrap().is_none());
}
#[test]
fn duplicate_start_and_change_return_original_receipt_without_reexecution() {
    let f = Fixture::new();
    let request = f.request(JobType::Write);
    let r = f.store.start(request.clone()).unwrap();
    assert_eq!(f.store.start(request.clone()).unwrap().revision, r.revision);
    f.ready(&r);
    let mut conflict = request;
    conflict.prompt = "Different".into();
    assert_eq!(f.store.start(conflict).unwrap_err(), Error::Conflict);
    let request = Change {
        operation_id: new_id().unwrap(),
        session_id: r.session_id,
        expected_revision: f.store.revision().unwrap(),
        change: ChangeKind::Prompt {
            prompt: "Follow up".into(),
        },
    };
    let p = f.store.change(request.clone()).unwrap();
    let c = f.store.take_command(&p).unwrap();
    assert_eq!(f.store.take_command(&p).unwrap_err(), Error::Blocked);
    f.ack(c, Outcome::Reply("Done".into()));
    assert_eq!(f.store.change(request).unwrap().revision, p.revision);
}
#[test]
fn delete_is_logical_and_preserves_operation_receipts() {
    let f = Fixture::new();
    let r = f.start(JobType::Write);
    f.ready(&r);
    let finish = f.change(&r, ChangeKind::Finish);
    let c = f.store.take_command(&finish).unwrap();
    f.ack(c, Outcome::Finished);
    f.change(&r, ChangeKind::Delete);
    assert!(f.store.get(&r.session_id).unwrap().job.deleted);
    let reopened = Store::open(f.dir.join("jobs.json")).unwrap();
    assert!(reopened.get(&r.session_id).unwrap().job.deleted);
    assert_eq!(
        reopened.receipt(&r.operation_id).unwrap().revision,
        r.revision
    );
}
#[test]
fn identity_mismatch_cannot_acknowledge_some_other_attempt() {
    let f = Fixture::new();
    f.start(JobType::Read);
    let c = f.store.prepare_next(0).unwrap().unwrap();
    assert_eq!(
        f.store
            .acknowledge(Acknowledgement {
                session_id: c.session_id.clone(),
                operation_id: new_id().unwrap(),
                action: c.action,
                outcome: Outcome::Ready
            })
            .unwrap_err(),
        Error::Blocked
    );
    assert_eq!(f.store.reserved(), 1);
}
#[test]
fn corrupt_or_changed_storage_fails_closed_before_effect() {
    let f = Fixture::new();
    f.start(JobType::Read);
    filesystem::publish(&f.dir.join("jobs.json"), b"{}", 0o600, true).unwrap();
    assert_eq!(f.store.prepare_next(0).unwrap_err(), Error::Storage);
    assert_eq!(f.store.reserved(), 3);
    assert!(Store::open(f.dir.join("jobs.json")).is_err());
}
#[test]
fn restart_during_finish_retains_cleanup_unknown() {
    let f = Fixture::new();
    let r = f.start(JobType::Write);
    f.ready(&r);
    f.change(&r, ChangeKind::Finish);
    let reopened = Store::open(f.dir.join("jobs.json")).unwrap();
    assert_eq!(
        reopened.get(&r.session_id).unwrap().job.status,
        Status::CleanupUnknown
    );
    assert_eq!(reopened.reserved(), 1);
}
