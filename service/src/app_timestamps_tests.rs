use super::*;
use crate::cloud::{Clock, Dependencies};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn request(version: u32, op: &str, args: Value) -> AppRequest {
    AppRequest {
        schema_version: version,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(9999),
        op: op.into(),
        args,
    }
}
fn history(pairs: usize) -> Vec<Message> {
    (0..pairs)
        .flat_map(|i| {
            [
                Message {
                    role: "user".into(),
                    content: format!("old user {i}"),
                },
                Message {
                    role: "assistant".into(),
                    content: format!("old assistant {i}"),
                },
            ]
        })
        .collect()
}
fn durable(pairs: usize) -> Durable {
    let mut d = Durable::default();
    d.config.model = Some("local:exact".into());
    let target = d.allocate(None, "Chat".into()).unwrap();
    d.legacy_conversation_id = Some(target);
    d.conversations[0].history = history(pairs);
    d
}
struct TestClock {
    wall: AtomicU64,
    samples: AtomicUsize,
}
impl Clock for TestClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wall_ms(&self) -> u64 {
        self.samples.fetch_add(1, Ordering::SeqCst);
        self.wall.load(Ordering::SeqCst)
    }
}
struct Reply {
    answer: AppResult<String>,
    contexts: Mutex<Vec<Value>>,
}
impl Reply {
    fn new(answer: AppResult<String>) -> Self {
        Self {
            answer,
            contexts: Mutex::new(vec![]),
        }
    }
}
impl Provider for Reply {
    fn inventory(&self) -> AppResult<Inventory> {
        panic!("unexpected inventory")
    }
    fn chat(&self, _: &str, messages: &[Message]) -> AppResult<String> {
        self.contexts.lock().unwrap().push(json!(messages));
        self.answer.clone()
    }
}
struct Fixture {
    app: Application,
    roots: Roots,
    base: PathBuf,
    clock: Arc<TestClock>,
}
impl Fixture {
    fn new(d: Durable) -> Self {
        let base = std::env::temp_dir().join(format!("smith-timestamps-{}", new_id().unwrap()));
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        for p in [
            &base,
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
        ] {
            fs::create_dir(p).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let clock = Arc::new(TestClock {
            wall: AtomicU64::new(2000),
            samples: AtomicUsize::new(0),
        });
        let path = roots.data.join("smith-next/app-state.json");
        persist(&path, &d).unwrap();
        let app = Application {
            inner: Arc::new(Mutex::new(Inner {
                durable: d,
                inventory: Inventory::default(),
                fault: false,
                owned: true,
                inventory_observed: false,
                cloud: v3::Volatile::default(),
            })),
            owner: None,
            dev_jobs: Arc::new(dev_jobs::Owner::open(
                roots.data.join("smith-next/dev-jobs.json"),
                Arc::new(crate::dev_runtime::DockerRuntime::new(
                    roots.runtime.clone(),
                )),
            )),
            path,
            instance_id: new_id().unwrap(),
            cloud: Arc::new(Dependencies::disabled().with_clock(clock.clone())),
        };
        Self {
            app,
            roots,
            base,
            clock,
        }
    }
    fn pending(pairs: usize, accepted: u64) -> (Self, Value) {
        let mut d = durable(pairs);
        let target = d.conversations[0].conversation_id.clone();
        let args = json!({"request_id":id(1),"conversation_id":target,"text":"new","output":"chat","expected_config_revision":0,"expected_history_revision":0});
        d.jobs.push(Job {
            operation: Operation {
                request_id: id(1),
                kind: "submit".into(),
                status: "running".into(),
                phase: "generation".into(),
                config_revision: 0,
                result: None,
                error: None,
            },
            fingerprint: v2::request_fingerprint(2, "submit", &args).unwrap(),
            version: 2,
            conversation_id: Some(target),
            history_revision: Some(0),
            task: Some(Task::Submit { text: "new".into() }),
            accepted_ms: accepted,
        });
        (Self::new(d), args)
    }
    fn target(&self) -> String {
        self.app.inner.lock().unwrap().durable.conversations[0]
            .conversation_id
            .clone()
    }
    fn times(&self) -> Value {
        times(&self.app, &self.target())
    }
    fn run(&self, reply: &Reply) {
        let work = self.app.next_work().unwrap().unwrap();
        self.app.run(work, reply);
    }
    fn write(&self, value: &Value) -> Vec<u8> {
        let bytes = serde_json::to_vec(value).unwrap();
        fs::write(&self.app.path, &bytes).unwrap();
        bytes
    }
    fn reopen(&self) -> Result<Application> {
        Application::open_with_cloud(
            &self.roots,
            Arc::new(Reply::new(Err(invalid()))),
            Dependencies::disabled().with_clock(self.clock.clone()),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
fn times(app: &Application, target: &str) -> Value {
    app.call(request(
        3,
        "conversation_timestamps",
        json!({"conversation_id":target}),
    ))
    .unwrap()
}

#[test]
fn timestamps_local_completion_is_atomic_exact_and_stable_on_replay_restart_and_job_eviction() {
    let (f, args) = Fixture::pending(1, 1000);
    assert_eq!(
        f.times()["turns"],
        json!([{"user_at_ms":null,"assistant_at_ms":null}])
    );
    let provider = Reply::new(Ok("answer".into()));
    f.run(&provider);
    let expected = json!({"conversation_id":f.target(),"history_revision":1,"turns":[
        {"user_at_ms":null,"assistant_at_ms":null}, {"user_at_ms":1000,"assistant_at_ms":2000}
    ]});
    assert_eq!(f.times(), expected);
    assert_eq!(f.clock.samples.load(Ordering::SeqCst), 1);
    let bytes = fs::read(&f.app.path).unwrap();
    f.clock.wall.store(9000, Ordering::SeqCst);
    assert_eq!(
        f.app.call(request(2, "submit", args)).unwrap()["status"],
        "succeeded"
    );
    for _ in 0..3 {
        assert_eq!(f.times(), expected);
    }
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    let reopened = f.reopen().unwrap();
    assert_eq!(times(&reopened, &f.target()), expected);
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    drop(reopened);
    let mut d = f.app.inner.lock().unwrap().durable.clone();
    for n in 2..=MAX_JOBS {
        let mut job = d.jobs[0].clone();
        job.operation.request_id = id(n);
        d.jobs.push(job);
    }
    d.make_room().unwrap();
    assert!(d.jobs.iter().all(|j| j.operation.request_id != id(1)));
    f.app.commit(&mut f.app.inner.lock().unwrap(), d).unwrap();
    let reopened = f.reopen().unwrap();
    assert_eq!(times(&reopened, &f.target()), expected);
    assert_eq!(f.clock.samples.load(Ordering::SeqCst), 1);
}

#[test]
fn timestamps_leave_all_existing_projections_and_provider_messages_unchanged() {
    let (f, _) = Fixture::pending(1, 1000);
    let p = Reply::new(Ok("answer".into()));
    f.run(&p);
    let mut context = history(1);
    context.push(Message {
        role: "user".into(),
        content: "new".into(),
    });
    assert_eq!(p.contexts.lock().unwrap()[0], json!(context));
    let mut without = f.app.inner.lock().unwrap().durable.clone();
    without.timestamps.clear();
    let untimed = Fixture::new(without);
    for version in 1..=3 {
        for (op, args) in [
            ("state", json!({})),
            ("conversation_get", json!({"conversation_id":f.target()})),
        ] {
            if version == 1 && op == "conversation_get" {
                continue;
            }
            assert_eq!(
                f.app.call(request(version, op, args.clone())),
                untimed.app.call(request(version, op, args))
            );
        }
    }
    for version in [2, 3] {
        let poll = |app: &Application| {
            app.call(request(
                version,
                "poll",
                json!({"request_id":id(1),"instance_id":app.instance_id}),
            ))
            .unwrap()
        };
        assert_eq!(poll(&f.app), poll(&untimed.app));
    }
    assert_eq!(
        json!(f.app.inner.lock().unwrap().durable.legacy()),
        json!(untimed.app.inner.lock().unwrap().durable.legacy())
    );
}

#[test]
fn timestamps_old_disk_versions_migrate_to_unknown_without_rewriting_history_or_cloud() {
    let legacy: Value =
        serde_json::from_slice(include_bytes!("../tests/fixtures/voice-v1.json")).unwrap();
    let mut d = durable(2);
    d.config.revision = u64::MAX;
    for version in 1..=3 {
        let f = Fixture::new(Durable::default());
        let mut old = if version == 1 {
            legacy.clone()
        } else {
            json!(d)
        };
        if version != 1 {
            old["schema_version"] = json!(version);
            old.as_object_mut().unwrap().remove("timestamps");
            old.as_object_mut().unwrap().remove("projects");
            if version == 2 {
                old.as_object_mut().unwrap().remove("cloud");
            }
            if version == 3 {
                old["cloud"]["providers"][0]["credential"] =
                    json!({"provider":"openai","generation":id(20),"save_request_id":id(21)});
                old["cloud"]["providers"][0]["revision"] = json!(7);
            }
        }
        f.write(&old);
        let app = f.reopen().unwrap();
        let saved: Value = serde_json::from_slice(&fs::read(&f.app.path).unwrap()).unwrap();
        assert_eq!(saved["schema_version"], 6);
        assert_eq!(saved["timestamps"], json!({}));
        assert_eq!(saved["config"], old["config"]);
        assert_eq!(saved["expired_ids"], old["expired_ids"]);
        if version == 3 {
            assert_eq!(saved["cloud"], old["cloud"]);
        }
        if version != 1 {
            assert_eq!(saved["conversations"], old["conversations"]);
            assert_eq!(saved["jobs"], old["jobs"]);
        } else {
            assert_eq!(saved["conversations"][0]["history"], old["history"]);
        }
        let c = &saved["conversations"][0];
        let count = c["history"].as_array().unwrap().len() / 2;
        assert_eq!(
            times(&app, c["conversation_id"].as_str().unwrap())["turns"],
            json!(vec![TurnTimes::default(); count])
        );
    }
}

#[test]
fn timestamps_strict_current_disk_and_frozen_disk3_preserve_malformed_bytes() {
    let (f, _) = Fixture::pending(1, 1000);
    f.run(&Reply::new(Ok("answer".into())));
    let good = json!(f.app.inner.lock().unwrap().durable);
    let target = f.target();
    let mut bad = vec![];
    for version in [1, 2, 3, 4, 7] {
        let mut v = good.clone();
        v["schema_version"] = json!(version);
        bad.push(v);
    }
    let mut v = good.clone();
    v.as_object_mut().unwrap().remove("timestamps");
    bad.push(v);
    for pair in [
        json!({"user_at_ms":null}),
        json!({"assistant_at_ms":null}),
        json!({"user_at_ms":1,"assistant_at_ms":2,"extra":3}),
    ] {
        let mut v = good.clone();
        v["timestamps"][&target][0] = pair;
        bad.push(v);
    }
    for key in ["user_at_ms", "assistant_at_ms"] {
        for time in [
            json!(0),
            json!(-1),
            json!(MAX_TIMESTAMP_MS + 1),
            json!(1.5),
            json!("1000"),
            json!(true),
        ] {
            let mut v = good.clone();
            v["timestamps"][&target][0][key] = time;
            bad.push(v);
        }
    }
    for turns in [
        json!([]),
        json!([{"user_at_ms":1,"assistant_at_ms":2}]),
        json!(vec![TurnTimes::default(); 17]),
    ] {
        let mut v = good.clone();
        v["timestamps"][&target] = turns;
        bad.push(v);
    }
    let mut v = good.clone();
    v["timestamps"][id(123)] = json!([]);
    bad.push(v);
    let mut v = good.clone();
    v["conversations"][0]["history"][0]["user_at_ms"] = json!(1000);
    bad.push(v);
    let mut v = good.clone();
    v["schema_version"] = json!(3);
    v.as_object_mut().unwrap().remove("timestamps");
    v.as_object_mut().unwrap().remove("cloud");
    bad.push(v);
    for v in bad {
        let bytes = f.write(&v);
        assert!(f.reopen().is_err());
        assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    }
    let duplicate = serde_json::to_string(&good).unwrap().replacen(
        "\"user_at_ms\":",
        "\"user_at_ms\":1,\"user_at_ms\":",
        1,
    );
    fs::write(&f.app.path, duplicate.as_bytes()).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(fs::read(&f.app.path).unwrap(), duplicate.as_bytes());
}

#[test]
fn timestamps_query_is_strict_read_only_and_schema3_only() {
    let f = Fixture::new(durable(2));
    let bytes = fs::read(&f.app.path).unwrap();
    assert_eq!(f.times().as_object().unwrap().len(), 3);
    for args in [
        json!({}),
        json!({"conversation_id":null}),
        json!({"conversation_id":"bad"}),
        json!({"conversation_id":f.target(),"extra":0}),
    ] {
        assert_eq!(
            f.app
                .call(request(3, "conversation_timestamps", args))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }
    assert_eq!(
        f.app
            .call(request(
                3,
                "conversation_timestamps",
                json!({"conversation_id":id(123)})
            ))
            .unwrap_err()
            .code,
        "CONVERSATION_NOT_FOUND"
    );
    for version in [1, 2, 4] {
        assert_eq!(
            f.app
                .call(request(
                    version,
                    "conversation_timestamps",
                    json!({"conversation_id":f.target()})
                ))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    assert_eq!(f.clock.samples.load(Ordering::SeqCst), 0);
}

#[test]
fn timestamps_bounds_and_backward_wall_clock_never_rewrite_dates() {
    for (accepted, completed, expected) in [
        (1000, 999, json!({"user_at_ms":1000,"assistant_at_ms":999})),
        (
            1,
            MAX_TIMESTAMP_MS,
            json!({"user_at_ms":1,"assistant_at_ms":MAX_TIMESTAMP_MS}),
        ),
        (
            0,
            MAX_TIMESTAMP_MS + 1,
            json!({"user_at_ms":null,"assistant_at_ms":null}),
        ),
        (
            MAX_TIMESTAMP_MS + 1,
            0,
            json!({"user_at_ms":null,"assistant_at_ms":null}),
        ),
    ] {
        let (f, _) = Fixture::pending(0, accepted);
        f.clock.wall.store(completed, Ordering::SeqCst);
        f.run(&Reply::new(Ok("answer".into())));
        assert_eq!(f.times()["turns"], json!([expected]));
        assert_eq!(f.clock.samples.load(Ordering::SeqCst), 1);
        f.app.inner.lock().unwrap().durable.validate().unwrap();
    }
}

#[test]
fn timestamps_trim_with_pair_and_byte_bounds_and_delete_atomically() {
    for answer in ["answer".into(), "a".repeat(MAX_CONTEXT)] {
        let (f, _) = Fixture::pending(16, 1000);
        {
            let mut inner = f.app.inner.lock().unwrap();
            let target = inner.durable.conversations[0].conversation_id.clone();
            inner.durable.timestamps.insert(
                target,
                (1..=16).map(|n| TurnTimes::sampled(n, n + 100)).collect(),
            );
        }
        f.run(&Reply::new(Ok(answer.clone())));
        let expected = if answer.len() == MAX_CONTEXT { 0 } else { 16 };
        assert_eq!(f.times()["turns"].as_array().unwrap().len(), expected);
        if expected == 16 {
            assert_eq!(f.times()["turns"][0]["user_at_ms"], 2);
            assert_eq!(f.times()["turns"][15]["user_at_ms"], 1000);
        }
        f.app.inner.lock().unwrap().durable.validate().unwrap();
        let target = f.target();
        f.app.call(request(3, "conversation_archive", json!({"request_id":id(20),"conversation_id":target,"expected_metadata_revision":0}))).unwrap();
        assert_eq!(f.times()["turns"].as_array().unwrap().len(), expected);
        f.app.call(request(3, "conversation_delete", json!({"request_id":id(21),"conversation_id":target,"expected_metadata_revision":1,"expected_history_revision":1}))).unwrap();
        let saved: Value = serde_json::from_slice(&fs::read(&f.app.path).unwrap()).unwrap();
        assert_eq!(saved["timestamps"], json!({}));
        assert_eq!(
            f.app
                .call(request(
                    3,
                    "conversation_timestamps",
                    json!({"conversation_id":target})
                ))
                .unwrap_err()
                .code,
            "CONVERSATION_NOT_FOUND"
        );
    }
}

#[test]
fn timestamps_failure_interruption_and_publication_fence_never_add_a_pair() {
    let (f, _) = Fixture::pending(1, 1000);
    f.run(&Reply::new(Err(AppError::new(
        "TEST_FAILURE",
        "Synthetic failure",
    ))));
    assert_eq!(f.times()["history_revision"], 0);
    assert_eq!(
        f.times()["turns"],
        json!([{"user_at_ms":null,"assistant_at_ms":null}])
    );
    assert_eq!(f.clock.samples.load(Ordering::SeqCst), 0);
    let (f, _) = Fixture::pending(0, 1000);
    let app = f.reopen().unwrap();
    assert_eq!(times(&app, &f.target())["turns"], json!([]));
    assert_eq!(
        app.inner.lock().unwrap().durable.jobs[0].operation.status,
        "interrupted"
    );
    assert!(app.inner.lock().unwrap().durable.timestamps.is_empty());
    let (f, _) = Fixture::pending(0, 1000);
    let work = f.app.next_work().unwrap().unwrap();
    f.app.inner.lock().unwrap().owned = false;
    let bytes = fs::read(&f.app.path).unwrap();
    f.app.run(work, &Reply::new(Ok("late".into())));
    assert_eq!(fs::read(&f.app.path).unwrap(), bytes);
    assert_eq!(f.clock.samples.load(Ordering::SeqCst), 0);
}

#[test]
fn timestamps_failed_publication_preserves_history_and_restart_does_not_invent_completion() {
    let (f, _) = Fixture::pending(0, 1000);
    let before = fs::read(&f.app.path).unwrap();
    let saved = f.base.join("before.json");
    fs::rename(&f.app.path, &saved).unwrap();
    fs::create_dir(&f.app.path).unwrap();
    f.run(&Reply::new(Ok("unpublished".into())));
    assert!(f.app.inner.lock().unwrap().fault);
    assert!(f.app.inner.lock().unwrap().durable.timestamps.is_empty());
    assert_eq!(fs::read(&saved).unwrap(), before);
    fs::remove_dir(&f.app.path).unwrap();
    fs::rename(saved, &f.app.path).unwrap();
    let app = f.reopen().unwrap();
    assert_eq!(times(&app, &f.target())["turns"], json!([]));
    assert_eq!(
        app.inner.lock().unwrap().durable.jobs[0].operation.status,
        "interrupted"
    );
}

#[test]
fn timestamps_preflight_reserves_unknown_prefix_and_maximum_completion_encoding() {
    for pairs in 0..=16 {
        let (f, _) = Fixture::pending(pairs, MAX_TIMESTAMP_MS);
        let d = f.app.inner.lock().unwrap().durable.clone();
        let before = d.reserved_bytes().unwrap();
        let mut reserved = d.clone();
        reserve(&mut reserved.timestamps, &f.target(), pairs);
        let added =
            serde_json::to_vec(&reserved).unwrap().len() - serde_json::to_vec(&d).unwrap().len();
        assert!(added > 0);
        assert!(before >= serde_json::to_vec(&d).unwrap().len() + added);
        f.clock.wall.store(MAX_TIMESTAMP_MS, Ordering::SeqCst);
        f.run(&Reply::new(Ok("answer".into())));
        let completed = f.app.inner.lock().unwrap().durable.clone();
        assert!(completed.reserved_bytes().unwrap() <= before);
        completed.validate().unwrap();
    }
}
