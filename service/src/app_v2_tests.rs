use super::*;
use std::os::unix::fs::PermissionsExt;

struct Fixture {
    app: Application,
    dir: PathBuf,
}
impl Fixture {
    fn new(durable: Durable) -> Self {
        let dir = std::env::temp_dir().join(format!("smith-v2-{}", new_id().unwrap()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let app = Application {
            inner: Arc::new(Mutex::new(Inner {
                durable,
                inventory: Inventory::default(),
                inventory_observed: false,
                fault: false,
                owned: true,
                cloud: v3::Volatile::default(),
            })),
            owner: None,
            dev_jobs: Arc::new(dev_jobs::Owner::open(
                dir.join("dev-jobs.json"),
                Arc::new(crate::dev_runtime::DockerRuntime::new(dir.clone())),
            )),
            path: dir.join("state.json"),
            instance_id: new_id().unwrap(),
            cloud: Arc::new(crate::cloud::Dependencies::disabled()),
        };
        Self { app, dir }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn queued(d: &mut Durable, n: usize, time: u64) -> String {
    let target = d.allocate(Some(id(n)), "New chat".into()).unwrap();
    d.instructions.capture(&id(n));
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(n),
            kind: "submit_new".into(),
            status: "queued".into(),
            phase: "queued".into(),
            config_revision: 0,
            result: None,
            error: None,
        },
        fingerprint: "a".repeat(64),
        version: 2,
        conversation_id: Some(target.clone()),
        history_revision: Some(0),
        task: Some(Task::Submit {
            text: format!("prompt {n}"),
        }),
        accepted_ms: time,
    });
    target
}
#[test]
fn title_scalar_boundaries_and_sanitization() {
    for scalar in ['a', 'é', '界', '🦀'] {
        for count in [39, 40, 41] {
            let title = scalar.to_string().repeat(count);
            assert_eq!(valid_new_title(&title), count <= 40);
            assert_eq!(
                initial_title(&title),
                scalar.to_string().repeat(count.min(40))
            );
        }
    }
    // Combining marks and ZWJ are scalars, not separate grapheme-count rules.
    assert!(valid_new_title(&"e\u{301}".repeat(20)));
    assert!(!valid_new_title(&"e\u{301}".repeat(21)));
    assert_eq!(
        initial_title(&"👩\u{200d}💻".repeat(14)).chars().count(),
        40
    );
    assert_eq!(
        initial_title(" \tHello\r\n  world\u{a0}again\u{2028}today \u{202e}\u{2066}\0! "),
        "Hello world again today !"
    );
    assert_eq!(
        initial_title("a\u{061c}\u{200e}\u{200f}\u{202d}\u{2069}\u{7f}b"),
        "ab"
    );
    assert_eq!(initial_title("\0\u{202e}\t\n"), "New chat");
    assert_eq!(
        initial_title(&format!("{} more", "a".repeat(39))),
        "a".repeat(39)
    );
}

#[test]
fn title_generation_removes_feff_before_truncation_without_changing_stored_validity() {
    for input in ["\u{feff}Hello", "Hello\u{feff}", "\u{feff}Hello\u{feff}"] {
        assert!(valid_title(input));
        assert_eq!(initial_title(input), "Hello");
    }
    assert_eq!(initial_title("a\u{feff}b"), "ab");
    assert_eq!(initial_title(" \u{feff}\t\u{feff}\n"), "New chat");
    assert_eq!(
        initial_title(" \u{feff} Hello \u{feff} world \u{feff} "),
        "Hello world"
    );
    // FEFF would otherwise become the final scalar at the truncation boundary.
    assert_eq!(
        initial_title(&format!("{}\u{feff}bc", "🦀".repeat(39))),
        format!("{}b", "🦀".repeat(39))
    );
    assert_eq!(
        initial_title(&format!("{} \u{feff}suffix", "a".repeat(38))),
        format!("{} s", "a".repeat(38))
    );
}

#[test]
fn title_legacy_disk_and_receipt_replay_remain_compatible() {
    let mut d = Durable::default();
    let target = d.allocate(None, "Original".into()).unwrap();
    let f = Fixture::new(d);
    let long_title = "🦀".repeat(64); // The historical 256-byte boundary.
    assert!(valid_title(&long_title));
    assert!(!valid_new_title(&long_title));
    let args = json!({"request_id":id(1),"conversation_id":target,"expected_metadata_revision":0,"title":long_title});
    let receipt = json!({"request_id":id(1),"conversation":{
        "conversation_id":target,"title":long_title,"archived":false,
        "metadata_revision":1,"history_revision":0,"sequence":0,"origin_request_id":null
    }});
    let mut d = f.app.inner.lock().unwrap().durable.clone();
    d.conversations[0].title = long_title.clone();
    d.conversations[0].metadata_revision = 1;
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(1),
            kind: "conversation_rename".into(),
            status: "succeeded".into(),
            phase: "complete".into(),
            config_revision: 0,
            result: Some(receipt.clone()),
            error: None,
        },
        fingerprint: request_fingerprint(2, "conversation_rename", &args).unwrap(),
        version: 2,
        conversation_id: Some(target.clone()),
        history_revision: Some(0),
        task: None,
        accepted_ms: 0,
    });
    f.app.commit(&mut f.app.inner.lock().unwrap(), d).unwrap();
    let bytes = std::fs::read(&f.app.path).unwrap();
    let (loaded, migrated) = Durable::load(&bytes).unwrap();
    assert!(!migrated);
    loaded.validate().unwrap();
    f.app.inner.lock().unwrap().durable = loaded;
    let request = |op: &str, args| AppRequest {
        schema_version: 2,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(99),
        op: op.into(),
        args,
    };
    let detail = f
        .app
        .call_v2(request(
            "conversation_get",
            json!({"conversation_id":target}),
        ))
        .unwrap();
    assert_eq!(detail["conversation"]["title"], long_title);
    assert_eq!(
        f.app
            .call_v2(request("conversation_rename", args.clone()))
            .unwrap(),
        receipt
    );
    assert_eq!(std::fs::read(&f.app.path).unwrap(), bytes);
    let mut fresh = args;
    fresh["request_id"] = json!(id(2));
    fresh["expected_metadata_revision"] = json!(1);
    assert!(f
        .app
        .call_v2(request("conversation_rename", fresh.clone()))
        .is_err());
    assert_eq!(std::fs::read(&f.app.path).unwrap(), bytes);
    fresh["title"] = json!("🦀".repeat(40));
    f.app
        .call_v2(request("conversation_rename", fresh))
        .unwrap();
    let (loaded, _) = Durable::load(&std::fs::read(&f.app.path).unwrap()).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.jobs[0].operation.result, Some(receipt));
}

#[test]
fn migration_preserves_legacy_ids_without_inventing_submitted_text() {
    let bytes = include_bytes!("../tests/fixtures/voice-v1.json");
    let old: LegacyDurable = serde_json::from_slice(bytes).unwrap();
    let (d, migrated) = Durable::load(bytes).unwrap();
    assert!(migrated);
    d.validate().unwrap();
    assert_eq!(d.conversations.len(), 1);
    let c = &d.conversations[0];
    assert_eq!(c.title, "Current chat");
    assert!(c.origin_request_id.is_none());
    assert_eq!(json!(c.history), json!(old.history));
    let mut expected_config = old.config.clone();
    expected_config.revision += 1;
    assert_eq!(json!(d.config), json!(expected_config));
    assert_eq!(d.expired_ids, old.expired_ids);
    for (new, old) in d.jobs.iter().zip(old.jobs) {
        assert_eq!(new.operation.request_id, old.operation.request_id);
        assert_eq!(new.fingerprint, old.fingerprint);
        assert!(new.task.is_none());
    }
    let duplicate = String::from_utf8(bytes.to_vec()).unwrap().replacen(
        "\"schema_version\":",
        "\"schema_version\":1,\"schema_version\":",
        1,
    );
    assert!(Durable::load(duplicate.as_bytes()).is_err());
    let bytes = serde_json::to_string(&d).unwrap().replacen(
        "\"schema_version\":",
        "\"schema_version\":2,\"schema_version\":",
        1,
    );
    assert!(Durable::load(bytes.as_bytes()).is_err());
}
#[test]
fn fake_clock_expiry_before_dispatch_and_fifo_running_is_durable() {
    let mut d = Durable::default();
    queued(&mut d, 1, 10);
    queued(&mut d, 2, 11);
    let f = Fixture::new(d);
    let work = f.app.next_work_at(10 + QUEUE_MS + 1).unwrap().unwrap();
    assert_eq!(work.id, id(2));
    let disk: Durable = serde_json::from_slice(&std::fs::read(&f.app.path).unwrap()).unwrap();
    assert_eq!(
        disk.jobs[0].operation.error.as_ref().unwrap().code,
        "QUEUE_EXPIRED"
    );
    assert_eq!(disk.jobs[1].operation.status, "running");
    assert_eq!(disk.jobs[1].operation.phase, "generation");
    disk.validate().unwrap();
}
#[test]
fn all_expired_jobs_terminate_without_a_provider_or_pending_eviction() {
    let mut d = Durable::default();
    for n in 1..=4 {
        queued(&mut d, n, 0);
    }
    let f = Fixture::new(d);
    assert!(f.app.next_work_at(QUEUE_MS + 1).unwrap().is_none());
    let inner = f.app.inner.lock().unwrap();
    assert_eq!(inner.durable.jobs.len(), 4);
    assert!(inner
        .durable
        .jobs
        .iter()
        .all(|j| j.operation.status == "failed"));
    assert!(!inner.fault);
}
#[test]
fn terminal_eviction_skips_pending_and_never_forgets_replay_id() {
    let mut d = Durable::default();
    queued(&mut d, 1, 0);
    for n in 2..=32 {
        d.jobs.push(Job {
            operation: Operation {
                request_id: id(n),
                kind: "models".into(),
                status: "failed".into(),
                phase: "complete".into(),
                config_revision: 0,
                result: None,
                error: Some(AppError::new("FAILED", "test")),
            },
            fingerprint: "a".repeat(64),
            version: 1,
            conversation_id: None,
            history_revision: None,
            task: None,
            accepted_ms: 0,
        });
    }
    d.make_room().unwrap();
    assert_eq!(d.jobs[0].operation.request_id, id(1));
    assert_eq!(d.expired_ids, vec![id(2)]);
    d.expired_ids = (1000..2024).map(id).collect();
    let last = d.jobs.last().unwrap().clone();
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(33),
            ..last.operation.clone()
        },
        ..last
    });
    assert_eq!(d.make_room().unwrap_err().code, "CAPACITY");
    assert_eq!(d.jobs.len(), 32);
}
#[test]
fn encoded_capacity_reserves_completion_and_refusal_does_not_latch_fault() {
    let mut d = Durable::default();
    // All raw histories fit their limits, but JSON expands each control byte sixfold.
    for n in 1..=32 {
        let target = d.allocate(None, "Bounded history".into()).unwrap();
        let c = d
            .conversations
            .iter_mut()
            .find(|c| c.conversation_id == target)
            .unwrap();
        c.history = vec![
            Message {
                role: "user".into(),
                content: "\u{0001}".repeat(MAX_TEXT),
            },
            Message {
                role: "assistant".into(),
                content: "\u{0002}".repeat(MAX_CONTEXT - MAX_TEXT),
            },
        ];
        if n <= 12 {
            d.jobs.push(Job { operation:Operation { request_id:id(n),kind:"submit".into(),status:"succeeded".into(),phase:"complete".into(),config_revision:0,result:Some(json!({"assistant_text":"\u{0003}".repeat(MAX_CONTEXT),"audio":null,"speech_error":null})),error:None },fingerprint:"a".repeat(64),version:2,conversation_id:Some(target),history_revision:Some(0),task:None,accepted_ms:0 });
        }
    }
    assert_eq!(d.preflight().unwrap_err().code, "CAPACITY");
    let f = Fixture::new(Durable::default());
    let mut inner = f.app.inner.lock().unwrap();
    assert_eq!(
        f.app.commit(&mut inner, d.clone()).unwrap_err().code,
        "CAPACITY"
    );
    assert!(!inner.fault);
    assert!(!f.app.path.exists());
    d.jobs.truncate(9);
    d.preflight().unwrap();
    let c = &d.conversations[0];
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(100),
            kind: "submit".into(),
            status: "queued".into(),
            phase: "queued".into(),
            config_revision: 0,
            result: None,
            error: None,
        },
        fingerprint: "b".repeat(64),
        version: 2,
        conversation_id: Some(c.conversation_id.clone()),
        history_revision: Some(0),
        task: Some(Task::Submit { text: "new".into() }),
        accepted_ms: 0,
    });
    assert_eq!(d.preflight().unwrap_err().code, "CAPACITY");
    f.app.commit(&mut inner, Durable::default()).unwrap();
    assert!(!inner.fault);
}
#[test]
fn conversation_limit_and_deleted_legacy_target_never_remap() {
    let mut d = Durable::default();
    let legacy = d.legacy_target().unwrap();
    d.conversations.clear();
    assert_eq!(
        d.legacy_target().unwrap_err().code,
        "CONVERSATION_NOT_FOUND"
    );
    for _ in 0..32 {
        assert_ne!(d.allocate(None, "Chat".into()).unwrap(), legacy);
    }
    assert_eq!(
        d.allocate(None, "Chat".into()).unwrap_err().code,
        "CAPACITY"
    );
}
#[test]
fn global_worker_occupancy_blocks_v2_submit_without_creation() {
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(1),
            kind: "models".into(),
            status: "running".into(),
            phase: "inventory".into(),
            config_revision: 0,
            result: None,
            error: None,
        },
        fingerprint: "a".repeat(64),
        version: 1,
        conversation_id: None,
        history_revision: None,
        task: Some(Task::Models),
        accepted_ms: 0,
    });
    let f = Fixture::new(d);
    let req = AppRequest {
        schema_version: 2,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(90),
        op: "submit_new".into(),
        args: json!({"request_id":id(2),"text":"not admitted","output":"chat","expected_config_revision":0}),
    };
    assert_eq!(f.app.call(req).unwrap_err().code, "BUSY");
    assert!(f.app.inner.lock().unwrap().durable.conversations.is_empty());
    assert!(!f.app.path.exists());
}
#[test]
fn completion_is_fenced_by_running_request_target_and_revision() {
    struct Never;
    impl Provider for Never {
        fn inventory(&self) -> AppResult<Inventory> {
            unreachable!()
        }
        fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
            Ok("late result".into())
        }
    }
    for mismatch in ["status", "target", "revision", "current_history", "request"] {
        let mut d = Durable::default();
        d.config.model = Some("model".into());
        let target = queued(&mut d, 1, 0);
        d.jobs[0].operation.status = "running".into();
        d.jobs[0].operation.phase = "generation".into();
        let mut work = Work {
            id: id(1),
            task: Task::Submit {
                text: "prompt".into(),
            },
            config: d.config.clone(),
            history: vec![],
            conversation_id: Some(target),
            history_revision: Some(0),
            instructions: Some(d.instructions.preview()),
        };
        match mismatch {
            "status" => d.jobs[0].operation.status = "interrupted".into(),
            "target" => work.conversation_id = Some(id(999)),
            "revision" => work.history_revision = Some(1),
            "current_history" => d.conversations[0].history_revision = 1,
            _ => work.id = id(999),
        }
        let f = Fixture::new(d);
        f.app.run(work, &Never);
        assert!(f.app.inner.lock().unwrap().durable.conversations[0]
            .history
            .is_empty());
        assert!(!f.app.path.exists());
    }
}

#[test]
fn assurance_reserved_revision_must_allow_dispatch() {
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    d.revision = MAX_REVISION - 3;
    let f = Fixture::new(d.clone());
    queued(&mut d, 1, 0);
    f.app.commit(&mut f.app.inner.lock().unwrap(), d).unwrap();
    assert_eq!(
        f.app.inner.lock().unwrap().durable.revision,
        MAX_REVISION - 2
    );
    let work = f
        .app
        .next_work_at(0)
        .unwrap()
        .expect("accepted job must dispatch");
    f.app.run(work, &MaximalReply);
    let inner = f.app.inner.lock().unwrap();
    assert!(!inner.fault);
    assert_eq!(inner.durable.revision, MAX_REVISION);
    assert_eq!(inner.durable.jobs[0].operation.status, "succeeded");
}

#[test]
fn assurance_exact_byte_reservation_must_allow_dispatch() {
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    for n in 0..32 {
        let target = d.allocate(None, "Chat".into()).unwrap();
        d.conversations[n].history = vec![
            Message {
                role: "user".into(),
                content: "\u{0001}".repeat(MAX_TEXT),
            },
            Message {
                role: "assistant".into(),
                content: "\u{0002}".repeat(MAX_CONTEXT - MAX_TEXT),
            },
        ];
        if n < 9 {
            d.jobs.push(Job {operation:Operation {request_id:id(n+1),kind:"submit".into(),status:"succeeded".into(),phase:"complete".into(),config_revision:0,result:Some(json!({"assistant_text":if n<8 {"\u{0003}".repeat(MAX_CONTEXT)} else {"x".into()},"audio":null,"speech_error":null})),error:None},fingerprint:"a".repeat(64),version:1,conversation_id:Some(target),history_revision:Some(0),task:None,accepted_ms:0});
        }
    }
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(100),
            kind: "submit".into(),
            status: "queued".into(),
            phase: "queued".into(),
            config_revision: 0,
            result: None,
            error: None,
        },
        fingerprint: "b".repeat(64),
        version: 2,
        conversation_id: Some(d.conversations[31].conversation_id.clone()),
        history_revision: Some(0),
        task: Some(Task::Submit { text: "new".into() }),
        accepted_ms: 0,
    });
    d.instructions.capture(&id(100));
    let reserve = d.reserved_bytes().unwrap() - serde_json::to_vec(&d).unwrap().len();
    let gap = STATE_LIMIT as usize - reserve - serde_json::to_vec(&d).unwrap().len();
    let encoded = gap + 1;
    let fill = "\u{0004}".repeat(encoded / 6) + &"x".repeat(encoded % 6);
    assert!(fill.len() <= MAX_CONTEXT);
    d.jobs[8].operation.result.as_mut().unwrap()["assistant_text"] = json!(fill);
    assert_eq!(
        serde_json::to_vec(&d).unwrap().len() + reserve,
        STATE_LIMIT as usize
    );
    d.validate().unwrap();
    let f = Fixture::new(Durable::default());
    f.app.commit(&mut f.app.inner.lock().unwrap(), d).unwrap();
    assert!(
        f.app.next_work_at(0).unwrap().is_some(),
        "accepted at byte bound but running transition cannot use reservation"
    );
}

struct MaximalReply;
impl Provider for MaximalReply {
    fn inventory(&self) -> AppResult<Inventory> {
        unreachable!()
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        Ok("\u{0001}".repeat(MAX_CONTEXT - 3))
    }
}
#[test]
fn four_jobs_at_exact_byte_and_revision_capacity_all_complete() {
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    d.revision = MAX_REVISION - 9;
    for n in 0..32 {
        let target = d.allocate(None, "Chat".into()).unwrap();
        d.conversations[n].history = vec![
            Message {
                role: "user".into(),
                content: "\u{0001}".repeat(MAX_TEXT),
            },
            Message {
                role: "assistant".into(),
                content: "\u{0002}".repeat(MAX_CONTEXT - MAX_TEXT),
            },
        ];
        if n < 3 {
            d.jobs.push(Job { operation:Operation {request_id:id(n+1),kind:"submit".into(),status:"succeeded".into(),phase:"complete".into(),config_revision:0,result:Some(json!({"assistant_text":if n<2 {"\u{0003}".repeat(MAX_CONTEXT)} else {"x".into()},"audio":null,"speech_error":null})),error:None},fingerprint:"a".repeat(64),version:1,conversation_id:Some(target),history_revision:Some(0),task:None,accepted_ms:0 });
        }
    }
    for n in 0..4 {
        d.instructions.capture(&id(100 + n));
        d.jobs.push(Job {
            operation: Operation {
                request_id: id(100 + n),
                kind: "submit".into(),
                status: "queued".into(),
                phase: "queued".into(),
                config_revision: 0,
                result: None,
                error: None,
            },
            fingerprint: "b".repeat(64),
            version: 2,
            conversation_id: Some(d.conversations[28 + n].conversation_id.clone()),
            history_revision: Some(0),
            task: Some(Task::Submit { text: "new".into() }),
            accepted_ms: 0,
        });
    }
    let encoded = STATE_LIMIT as usize - d.reserved_bytes().unwrap() + 1;
    let fill = "\u{0004}".repeat(encoded / 6) + &"x".repeat(encoded % 6);
    assert!(fill.len() <= MAX_CONTEXT);
    d.jobs[2].operation.result.as_mut().unwrap()["assistant_text"] = json!(fill);
    assert_eq!(d.reserved_bytes().unwrap(), STATE_LIMIT as usize);
    d.validate().unwrap();
    let f = Fixture::new(Durable::default());
    f.app.commit(&mut f.app.inner.lock().unwrap(), d).unwrap();
    for n in 0..4 {
        let work = f.app.next_work_at(0).unwrap().unwrap();
        assert_eq!(work.id, id(100 + n));
        f.app.run(work, &MaximalReply);
        let inner = f.app.inner.lock().unwrap();
        assert!(!inner.fault);
        inner.durable.validate().unwrap();
        assert_eq!(inner.durable.jobs[3 + n].operation.status, "succeeded");
        assert!(std::fs::metadata(&f.app.path).unwrap().len() <= STATE_LIMIT);
    }
    assert_eq!(f.app.inner.lock().unwrap().durable.revision, MAX_REVISION);
    assert!(f.app.next_work_at(0).unwrap().is_none());
}
#[test]
fn unexpected_dispatch_capacity_failure_is_reported_and_fenced() {
    let mut d = Durable::default();
    queued(&mut d, 1, 0);
    d.revision = MAX_REVISION;
    let f = Fixture::new(d);
    assert_eq!(
        f.app.next_work_at(0).err().unwrap().code,
        "STATE_UNAVAILABLE"
    );
    assert!(f.app.inner.lock().unwrap().fault);
    assert!(!f.app.path.exists());
    assert!(f.app.next_work_at(0).is_err());
}
#[test]
fn assurance_duplicate_origins_must_fail_validation() {
    let mut d = Durable::default();
    d.allocate(Some(id(1)), "A".into()).unwrap();
    d.allocate(Some(id(1)), "B".into()).unwrap();
    d.expired_ids.push(id(1));
    assert!(
        d.validate().is_err(),
        "two targets claim the same accepted submit_new identity"
    );
    assert_corrupt_load_preserves_bytes(&d);
}

#[test]
fn assurance_delete_receipt_must_not_describe_existing_target() {
    let mut d = Durable::default();
    let target = d.allocate(None, "Still present".into()).unwrap();
    d.jobs.push(Job {
        operation: Operation {
            request_id: id(1),
            kind: "conversation_delete".into(),
            status: "succeeded".into(),
            phase: "complete".into(),
            config_revision: 0,
            result: Some(json!({"request_id":id(1),"conversation_id":target,"deleted":true})),
            error: None,
        },
        fingerprint: "a".repeat(64),
        version: 2,
        conversation_id: Some(target),
        history_revision: Some(0),
        task: None,
        accepted_ms: 0,
    });
    assert!(
        d.validate().is_err(),
        "deletion success accepted while content remains"
    );
    assert_corrupt_load_preserves_bytes(&d);
}

#[test]
fn independent_r2_origin_must_match_retained_target() {
    let mut d = completed_origin_fixture();
    let actual = d.conversations[0].conversation_id.clone();
    d.conversations[0].origin_request_id = None;
    let wrong = d.allocate(Some(id(1)), "Unrelated chat".into()).unwrap();
    assert_ne!(actual, wrong);
    assert!(
        d.validate().is_err(),
        "origin discovery must agree with retained target"
    );
    assert_corrupt_load_preserves_bytes(&d);
}

fn completed_origin_fixture() -> Durable {
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    queued(&mut d, 1, 0);
    d.jobs[0].operation.status = "succeeded".into();
    d.jobs[0].operation.phase = "complete".into();
    d.jobs[0].operation.result =
        Some(json!({"assistant_text":"answer","audio":null,"speech_error":null}));
    d.validate().unwrap();
    d
}

#[test]
fn retained_submit_new_requires_its_target_origin() {
    for origin in [None, Some(id(2))] {
        let mut d = completed_origin_fixture();
        d.conversations[0].origin_request_id = origin;
        assert!(d.validate().is_err());
        assert_corrupt_load_preserves_bytes(&d);
    }
}

#[test]
fn retained_origin_must_be_v2_submit_new_not_another_kind_or_version() {
    for version in [1, 2] {
        let mut d = completed_origin_fixture();
        d.jobs[0].version = version;
        d.jobs[0].operation.kind = "submit".into();
        // A regular submit is valid by itself, but cannot establish an origin.
        let mut without_origin = d.clone();
        without_origin.conversations[0].origin_request_id = None;
        without_origin.validate().unwrap();
        assert!(d.validate().is_err());
        assert_corrupt_load_preserves_bytes(&d);
    }
}

#[test]
fn expired_submit_new_origin_remains_valid_without_retained_job() {
    let mut d = completed_origin_fixture();
    let origin = d.jobs.remove(0).operation.request_id;
    d.instructions.prune(&d.jobs, &d.conversations);
    d.expired_ids.push(origin.clone());
    d.validate().unwrap();
    let f = Fixture::new(Durable::default());
    f.app.commit(&mut f.app.inner.lock().unwrap(), d).unwrap();
    let (loaded, migrated) = Durable::load(&std::fs::read(&f.app.path).unwrap()).unwrap();
    assert!(!migrated);
    loaded.validate().unwrap();
    assert_eq!(
        loaded.conversations[0].origin_request_id.as_ref(),
        Some(&origin)
    );
}

fn assert_corrupt_load_preserves_bytes(d: &Durable) {
    let f = Fixture::new(Durable::default());
    let dir = f.dir.join("smith-next");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("app-state.json");
    let bytes = serde_json::to_vec(d).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let roots = Roots {
        data: f.dir.clone(),
        config: f.dir.clone(),
        runtime: f.dir.clone(),
    };
    assert!(Application::open(&roots, Arc::new(MaximalReply)).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
#[test]
fn assurance_legacy_maximum_escaped_results_migrate() {
    let mut old = LegacyDurable {
        history: vec![
            Message {
                role: "user".into(),
                content: "\u{0001}".repeat(MAX_TEXT),
            },
            Message {
                role: "assistant".into(),
                content: "\u{0002}".repeat(MAX_CONTEXT - MAX_TEXT),
            },
        ],
        ..LegacyDurable::default()
    };
    for n in 1..=MAX_JOBS {
        old.jobs.push(LegacyJob {operation:Operation {request_id:id(n),kind:"submit".into(),status:"succeeded".into(),phase:"complete".into(),config_revision:0,result:Some(json!({"assistant_text":"\u{0003}".repeat(MAX_CONTEXT),"audio":null,"speech_error":null})),error:None},fingerprint:"a".repeat(64)});
    }
    old.validate().unwrap();
    let bytes = serde_json::to_vec(&old).unwrap();
    assert!(bytes.len() <= STATE_LIMIT as usize);
    let (d, _) = Durable::load(&bytes).unwrap();
    d.validate().unwrap();
    old.config.revision += 1;
    assert_eq!(json!(d.legacy()), json!(old));
}

struct RevisionProvider;
impl Provider for RevisionProvider {
    fn inventory(&self) -> AppResult<Inventory> {
        let mut i = Inventory::default();
        i.models.push(crate::providers::Model {
            name: "local:exact".into(),
            digest: "a".repeat(64),
        });
        i.providers.llm.ready = true;
        i.providers.llm.error = None;
        Ok(i)
    }
    fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
        Ok("answer".into())
    }
}
fn req(version: u32, op: &str, args: Value) -> AppRequest {
    AppRequest {
        schema_version: version,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(900),
        op: op.into(),
        args,
    }
}
fn finish(app: &Application, n: usize) -> Value {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let v = app
            .call(req(
                2,
                "poll",
                json!({"request_id":id(n),"instance_id":app.instance_id}),
            ))
            .unwrap();
        if !["queued", "running"].contains(&v["status"].as_str().unwrap()) {
            return v;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn full_u64_config_revision_migration_wire_echo_and_exhaustion() {
    for revision in [
        MAX_REVISION - 1,
        MAX_REVISION,
        MAX_REVISION + 1,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let f = Fixture::new(Durable::default());
        let dir = f.dir.join("smith-next");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.join("app-state.json");
        let old = LegacyDurable {
            config: Config {
                model: Some("local:exact".into()),
                voice: None,
                revision,
            },
            ..LegacyDurable::default()
        };
        old.validate().unwrap();
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let roots = Roots {
            data: f.dir.clone(),
            config: f.dir.clone(),
            runtime: f.dir.clone(),
        };
        if revision == u64::MAX {
            let before = std::fs::read(&path).unwrap();
            let error = Application::open(&roots, Arc::new(RevisionProvider))
                .err()
                .expect("legacy migration must refuse configuration revision overflow");
            assert!(error.contains("Configuration revision exhausted"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            continue;
        }
        let app = Application::open(&roots, Arc::new(RevisionProvider)).unwrap();
        let revision = revision + 1; // The disk1–6 instructions migration bumps once.
        let wire = wire_config_revision(revision);
        assert_eq!(
            app.call(req(1, "state", json!({}))).unwrap()["config"]["revision"],
            json!(revision)
        );
        assert_eq!(
            app.call(req(2, "state", json!({}))).unwrap()["config"]["revision"],
            wire
        );
        let admitted=app.call(req(2,"submit_new",json!({"request_id":id(1),"text":"hello","output":"chat","expected_config_revision":wire}))).unwrap();
        assert_eq!(admitted["config_revision"], wire);
        assert_eq!(finish(&app, 1)["config_revision"], wire);
        app.call(req(2,"submit",json!({"request_id":id(2),"conversation_id":admitted["conversation_id"],"text":"again","output":"chat","expected_config_revision":wire,"expected_history_revision":1}))).unwrap();
        assert_eq!(finish(&app, 2)["status"], "succeeded");
        app.call(req(2, "models", json!({"request_id":id(3)})))
            .unwrap();
        assert_eq!(finish(&app, 3)["config_revision"], wire);
        app.call(req(2,"verify_model",json!({"request_id":id(4),"model":"local:exact","sha256":"a".repeat(64),"expected_revision":wire}))).unwrap();
        assert_eq!(finish(&app, 4)["status"], "succeeded");
        let state = app.call(req(2, "state", json!({}))).unwrap();
        for op in state["operations"].as_array().unwrap() {
            assert_eq!(op["config_revision"], wire);
        }
        let before = std::fs::read(&path).unwrap();
        let result = app.call(req(
            2,
            "configure_checked",
            json!({"model":"local:exact","voice":null,"expected_revision":wire}),
        ));
        if revision == u64::MAX {
            assert_eq!(result.unwrap_err().code, "CONFIG_REVISION_EXHAUSTED");
            assert_eq!(
                app.call(req(
                    1,
                    "configure_checked",
                    json!({"model":"local:exact","voice":null,"expected_revision":revision})
                ))
                .unwrap_err()
                .code,
                "CONFIG_REVISION_EXHAUSTED"
            );
            assert_eq!(std::fs::read(&path).unwrap(), before);
        } else {
            assert_eq!(
                result.unwrap()["revision"],
                wire_config_revision(revision + 1)
            );
        }
        drop(app);
        let reopened = Application::open(&roots, Arc::new(RevisionProvider)).unwrap();
        assert_eq!(
            reopened.call(req(1, "state", json!({}))).unwrap()["config"]["revision"],
            json!(revision.saturating_add(1))
        );
    }
}
#[test]
fn v2_configuration_revision_parser_requires_canonical_representation() {
    for value in [
        json!("0"),
        json!(MAX_REVISION.to_string()),
        json!(MAX_REVISION + 1),
        json!(-1),
        json!(1.0),
        json!("09007199254740992"),
        json!("+9007199254740992"),
        json!("18446744073709551616"),
        json!(null),
    ] {
        assert!(parse_config_revision(value.clone()).is_err(), "{value}");
        let f = Fixture::new(Durable::default());
        assert_eq!(f.app.call(req(2,"submit_new",json!({"request_id":id(1),"text":"x","output":"chat","expected_config_revision":value}))).unwrap_err().code,"INVALID_REQUEST");
        assert_eq!(
            f.app
                .call(req(
                    2,
                    "configure_checked",
                    json!({"model":"local:exact","voice":null,"expected_revision":value})
                ))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
        assert!(!f.app.path.exists());
    }
    for n in [0, MAX_REVISION, MAX_REVISION + 1, u64::MAX] {
        assert_eq!(parse_config_revision(wire_config_revision(n)).unwrap(), n);
    }
}
#[test]
fn assurance_legacy_valid_revision_must_migrate() {
    let old = LegacyDurable {
        config: Config {
            revision: MAX_REVISION + 1,
            ..Config::default()
        },
        ..LegacyDurable::default()
    };
    old.validate().unwrap();
    let (d, _) = Durable::load(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(d.validate().is_ok(), "valid legacy revision cannot migrate");
}

#[test]
fn assurance_last_sequence_and_late_completion_after_dispose() {
    let mut d = Durable {
        next_sequence: MAX_REVISION - 1,
        ..Durable::default()
    };
    let target = d.allocate(None, "Last".into()).unwrap();
    assert_eq!(d.next_sequence, MAX_REVISION);
    d.validate().unwrap();
    d.conversations.clear();
    assert!(d.allocate(None, "No reuse".into()).is_err());
    assert!(valid_id(&target));
    struct Late;
    impl Provider for Late {
        fn inventory(&self) -> AppResult<Inventory> {
            unreachable!()
        }
        fn chat(&self, _: &str, _: &[Message]) -> AppResult<String> {
            Ok("late".into())
        }
    }
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    let target = queued(&mut d, 1, 0);
    d.jobs[0].operation.status = "running".into();
    d.jobs[0].operation.phase = "generation".into();
    let work = Work {
        id: id(1),
        task: Task::Submit {
            text: "secret".into(),
        },
        config: d.config.clone(),
        history: vec![],
        conversation_id: Some(target),
        history_revision: Some(0),
        instructions: Some(d.instructions.preview()),
    };
    let f = Fixture::new(d);
    f.app.inner.lock().unwrap().owned = false;
    f.app.run(work, &Late);
    assert!(!f.app.path.exists());
    assert!(f.app.inner.lock().unwrap().durable.conversations[0]
        .history
        .is_empty());
}
