use super::*;

fn structured_legacy_disk(version: u64) -> Value {
    assert!((6..=8).contains(&version));
    let mut disk = json!(rich_durable());
    disk["config"]["revision"] = json!(41);
    disk["instructions"]["settings"] = json!({
        "instructions":"Retained global instructions\n exact ",
        "personality":"Careful",
        "guardrails":"No capability grant",
        "cloud_enabled":true
    });
    disk["instructions"]["receipts"] = json!([{
        "receipt":{"request_id":id(6100),"config_revision":40,"changed":true},
        "fingerprint":"d".repeat(64)
    }]);
    disk["instructions"]["expired_ids"] = json!([id(6101)]);
    disk["agents"]["agents"][0]["name"] = json!("Retained developer");
    disk["agents"]["agents"][0]["prompt"] = json!(" Exact saved agent prompt\n\t ");
    disk["agents"]["revision"] = json!(3);
    disk["agents"]["receipts"] = json!([{
        "receipt":{"request_id":id(6200),"agent_id":"developer","revision":3},
        "fingerprint":"e".repeat(64)
    }]);
    disk["agents"]["expired_ids"] = json!([id(6201)]);
    let mut durable: Durable = serde_json::from_value(disk).unwrap();
    durable.instructions.capture(&id(2));
    let conversation = durable.conversations[0].conversation_id.clone();
    durable
        .instructions
        .approve(&conversation, crate::cloud::Vendor::Openai, &id(700));
    durable.validate().unwrap();
    let mut disk = json!(durable);
    disk["schema_version"] = json!(version);
    let mut first_repo = repository();
    first_repo.as_object_mut().unwrap().remove("read_only");
    first_repo["name"] = Value::Null;
    first_repo["branch"]["work"] = json!("team/next");
    first_repo["branch"]["types"]["feature"] = json!("custom/team/features/*");
    first_repo["branch"]["types"]["fix"] = Value::Null;
    let mut second_repo = json!(Repository::unknown(
        "git@example.invalid:Other/Repo.git".into()
    ));
    second_repo.as_object_mut().unwrap().remove("read_only");
    let legacy_args = json!({"request_id":id(504),"instance_id":id(500),"expected_revision":3,
        "name":"Same legacy Name 🚀","repositories":[first_repo,second_repo],"agent":"opencode"});
    let receipt = |n: usize, project: usize, deleted: bool| {
        json!({
            "request_id":id(500+n),"project_id":id(project),"revision":n,"deleted":deleted
        })
    };
    disk["projects"] = json!({"revision":4,"projects":[
        {"project_id":id(502),"name":"Same legacy Name 🚀","repositories":[],"agent":"opencode"},
        {"project_id":id(503),"name":"Same legacy Name 🚀","repositories":legacy_args["repositories"],"agent":"opencode"}
    ],"receipts":[
        {"receipt":receipt(1,501,false),"fingerprint":null},
        {"receipt":receipt(2,501,true),"fingerprint":"a".repeat(64)},
        {"receipt":receipt(3,502,false),"fingerprint":"b".repeat(64)},
        {"receipt":receipt(4,503,false),"fingerprint":fingerprint("v3:projects:project_create_v2", &legacy_args).unwrap()}
    ],"expired_ids":[id(599)]});
    if version < 8 {
        disk.as_object_mut().unwrap().remove("agents");
    }
    if version < 7 {
        disk.as_object_mut().unwrap().remove("instructions");
    }
    disk
}

#[test]
fn projects_v3_scalar_key_and_text_validation_is_exact() {
    for key in ["a", "0", "smith", "project-123-a", &"a".repeat(40)] {
        assert!(valid_key(key), "{key:?}");
    }
    for key in [
        "",
        "A",
        "a_B",
        "-a",
        "a-",
        "a--b",
        "a.b",
        " a",
        "a ",
        "é",
        "a\n",
        &"a".repeat(41),
    ] {
        assert!(!valid_key(key), "{key:?}");
    }
    assert!(valid_description(""));
    assert!(valid_description("  preserved spaces  "));
    assert!(valid_description(&"🚀".repeat(60)));
    assert!(!valid_description(&"🚀".repeat(61)));
    assert!(valid_description(&"e\u{0301}".repeat(30)));
    assert!(!valid_description(&"e\u{0301}".repeat(31)));
    assert!(valid_prompt(""));
    assert!(valid_prompt(" \n\tExact lines\n "));
    assert!(valid_prompt(&"🚀".repeat(1000)));
    assert!(!valid_prompt(&"🚀".repeat(1001)));
    for code in 0..=159 {
        let c = char::from_u32(code).unwrap();
        if c.is_control() {
            assert!(!valid_description(&c.to_string()));
            assert_eq!(valid_prompt(&c.to_string()), matches!(c, '\n' | '\t'));
        }
    }
    assert!(!valid_description("a\u{2028}b"));
    assert!(!valid_description("a\u{2029}b"));

    let f = Fixture::new(Durable::default());
    for (field, invalid) in [
        ("key", json!("Bad-key")),
        ("key", json!("a".repeat(41))),
        ("description", json!("🚀".repeat(61))),
        ("description", json!("a\u{2028}b")),
        ("description", json!("a\rb")),
        ("prompt", json!("🚀".repeat(1001))),
        ("prompt", json!("a\rb")),
        ("prompt", json!("a\0b")),
    ] {
        let mut args = create(&f.app, 1, 0);
        args[field] = invalid;
        f.assert_rejected("project_create_v3", args, "INVALID_REQUEST");
    }
    let mut args = create(&f.app, 1, 0);
    args["name"] = json!("a".repeat(40));
    args["key"] = json!("a".repeat(40));
    args["description"] = json!("🚀".repeat(60));
    args["prompt"] = json!("🚀".repeat(1000));
    call(&f.app, "project_create_v3", args.clone()).unwrap();
    let app = f.reopen().unwrap();
    let saved = &list(&app)["projects"][0];
    for field in ["name", "key", "description", "prompt"] {
        assert_eq!(saved[field], args[field]);
    }
}

#[test]
fn projects_v3_repository_ids_validate_new_writes_atomically() {
    let f = Fixture::new(Durable::default());
    let mut args = create(&f.app, 1, 0);
    args["repositories"] = json!([repository()]);
    let first = call(&f.app, "project_create_v3", args).unwrap();
    let mut revision = 1;
    for name in ["0".to_owned(), "a".repeat(40)] {
        let mut args = update(&f.app, 1 + revision, revision as u64, &first["project_id"]);
        let mut row = repository();
        row["name"] = json!(name);
        row["read_only"] = json!(false);
        args["repositories"] = json!([row]);
        call(&f.app, "project_update_v3", args).unwrap();
        revision += 1;
        assert_eq!(list(&f.app)["projects"][0]["repositories"], json!([row]));
    }
    for name in [
        Value::Null,
        json!(""),
        json!("Backend"),
        json!("two words"),
        json!("a_b"),
        json!("a--b"),
        json!("é"),
        json!("a".repeat(41)),
    ] {
        let mut row = repository();
        row["name"] = name;
        let mut args = create(&f.app, 10, revision as u64);
        args["repositories"] = json!([row]);
        f.assert_rejected("project_create_v3", args, "INVALID_REQUEST");
        let mut args = update(&f.app, 11, revision as u64, &first["project_id"]);
        args["repositories"] = json!([row]);
        f.assert_rejected("project_update_v3", args, "INVALID_REQUEST");
    }
    let mut other = repository();
    other["url"] = json!("https://example.invalid/other");
    let mut args = update(&f.app, 12, revision as u64, &first["project_id"]);
    args["repositories"] = json!([repository(), other]);
    f.assert_rejected("project_update_v3", args, "INVALID_REQUEST");
}

#[test]
fn projects_v3_repository_ids_preserve_legacy_reads_and_replay_before_admission() {
    let mut f = Fixture::new(Durable::default());
    let mut original = create(&f.app, 1, 0);
    original["name"] = json!("Smith");
    original["key"] = json!("smith");
    let mut unnamed = repository();
    unnamed["name"] = Value::Null;
    unnamed["read_only"] = json!(false);
    unnamed["branch"]["types"]["feature"] = json!("custom/features/*");
    let mut freeform = repository();
    freeform["name"] = json!("Legacy Repo 🚀");
    freeform["url"] = json!("https://example.invalid/legacy");
    original["repositories"] = json!([unnamed, freeform]);
    let mut stored = original.clone();
    for field in ["request_id", "instance_id", "expected_revision"] {
        stored.as_object_mut().unwrap().remove(field);
    }
    stored["project_id"] = json!(id(50));
    let historical = json!({"request_id":id(1),"project_id":id(50),"revision":1,"deleted":false});
    let mut disk = f.disk();
    disk["projects"] = json!({"revision":1,"projects":[stored],"receipts":[{
        "receipt":historical,
        "fingerprint":fingerprint("v3:projects:project_create_v3", &original).unwrap()
    }],"expired_ids":[id(99)]});
    let bytes = f.write(&disk);
    f.app = f.reopen().unwrap();
    assert_ne!(json!(f.app.instance_id), original["instance_id"]);
    assert_eq!(list(&f.app)["projects"], json!([stored]));
    assert_eq!(f.bytes(), bytes);
    assert_eq!(
        call(&f.app, "project_create_v3", original.clone()).unwrap(),
        historical
    );
    assert_eq!(receipt(&f.app, 1).unwrap(), historical);
    let mut conflict = original.clone();
    conflict["repositories"][0]["name"] = json!("corrected");
    f.assert_rejected("project_create_v3", conflict, "ID_CONFLICT");
    let mut expired = original.clone();
    expired["request_id"] = json!(id(99));
    f.assert_rejected("project_create_v3", expired, "REQUEST_EXPIRED");
    let mut correction = update(&f.app, 2, 1, &historical["project_id"]);
    correction["repositories"] = original["repositories"].clone();
    f.assert_rejected("project_update_v3", correction.clone(), "INVALID_REQUEST");
    correction["repositories"][0]["name"] = json!("explicit-first");
    correction["repositories"][1]["name"] = json!("explicit-second");
    call(&f.app, "project_update_v3", correction.clone()).unwrap();
    stored["repositories"] = correction["repositories"].clone();
    assert_eq!(list(&f.app)["projects"], json!([stored]));
    f.app = f.reopen().unwrap();
    assert_eq!(list(&f.app)["projects"], json!([stored]));
    let bytes = f.bytes();
    assert_eq!(
        call(&f.app, "project_create_v3", original).unwrap(),
        historical
    );
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_v3_readonly_and_independent_type_clears_preserve_hidden_values() {
    let f = Fixture::new(Durable::default());
    let before = f.disk();
    let mut args = create(&f.app, 1, 0);
    args["name"] = args["key"].clone();
    args["description"] = json!("  exact single line  ");
    args["prompt"] = json!(" \n\tExact project context — not chat\n ");
    let mut row = repository();
    row["branch"]["work"] = json!("topic/team/*");
    row["branch"]["types"]["feature"] = json!("custom/features/*");
    row["branch"]["types"]["release"] = json!("versions/*");
    args["repositories"] = json!([row]);
    let first = call(&f.app, "project_create_v3", args.clone()).unwrap();
    for field in ["key", "description", "prompt"] {
        let mut conflict = args.clone();
        conflict[field] = json!("different");
        f.assert_rejected("project_create_v3", conflict, "ID_CONFLICT");
    }
    let mut conflict = args.clone();
    conflict["repositories"][0]["read_only"] = json!(false);
    f.assert_rejected("project_create_v3", conflict, "ID_CONFLICT");
    let mut revision = 1;
    for read_only in [false, true, false] {
        let app = f.reopen().unwrap();
        let mut args = update(&app, 10 + revision, revision as u64, &first["project_id"]);
        row["read_only"] = json!(read_only);
        args["repositories"] = json!([row]);
        call(&app, "project_update_v3", args).unwrap();
        revision += 1;
        drop(app);
        let app = f.reopen().unwrap();
        assert_eq!(list(&app)["projects"][0]["repositories"], json!([row]));
        assert_eq!(
            list(&app)["projects"][0]["prompt"],
            " \n\tExact project context — not chat\n "
        );
    }
    for kind in ["feature", "fix", "release", "hotfix"] {
        let app = f.reopen().unwrap();
        let mut args = update(&app, 10 + revision, revision as u64, &first["project_id"]);
        row["branch"]["types"][kind] = Value::Null;
        args["repositories"] = json!([row]);
        call(&app, "project_update_v3", args).unwrap();
        revision += 1;
        assert_eq!(list(&app)["projects"][0]["repositories"], json!([row]));
    }
    let after = f.disk();
    for (key, value) in before.as_object().unwrap() {
        if !matches!(key.as_str(), "projects" | "revision") {
            assert_eq!(after[key], *value, "{key}");
        }
    }
}

#[test]
fn projects_v3_work_patterns_never_widen_base_or_skip_readonly_validation() {
    let f = Fixture::new(Durable::default());
    for work in [
        "main".to_owned(),
        "team/next".to_owned(),
        "topic/*".to_owned(),
        format!("{}/*", "x".repeat(253)),
    ] {
        assert!(valid_work(&work));
        let mut args = create(&f.app, 1, 0);
        args["repositories"] = json!([repository()]);
        args["repositories"][0]["branch"]["work"] = json!(work);
        let parsed: Create = decode(args).unwrap();
        assert!(parsed.repositories[0].valid());
    }
    for work in [
        "",
        "*",
        "topic*",
        "topic/**",
        "topic/*/next",
        "refs/topic/*",
        "topic.lock/*",
        "a..b/*",
        "HEAD/*",
    ] {
        assert!(!valid_work(work));
        for read_only in [true, false] {
            let mut args = create(&f.app, 1, 0);
            args["repositories"] = json!([repository()]);
            args["repositories"][0]["read_only"] = json!(read_only);
            args["repositories"][0]["branch"]["work"] = json!(work);
            f.assert_rejected("project_create_v3", args, "INVALID_REQUEST");
        }
    }
    for path in [
        "/branch/base",
        "/branch/types/feature",
        "/branch/types/fix",
        "/branch/types/release",
        "/branch/types/hotfix",
    ] {
        let mut args = create(&f.app, 1, 0);
        let mut row = repository();
        *row.pointer_mut(path).unwrap() = if path == "/branch/base" {
            json!("topic/*")
        } else {
            json!("topic/**")
        };
        args["repositories"] = json!([row]);
        f.assert_rejected("project_create_v3", args, "INVALID_REQUEST");
    }
}

#[test]
fn projects_v3_key_uniqueness_name_preservation_and_copy_identity_are_atomic() {
    let f = Fixture::new(Durable::default());
    let mut args = create(&f.app, 1, 0);
    args["name"] = args["key"].clone();
    let first = call(&f.app, "project_create_v3", args.clone()).unwrap();
    let mut copy = create(&f.app, 2, 1);
    copy["name"] = args["name"].clone();
    copy["key"] = args["key"].clone();
    f.assert_rejected("project_create_v3", copy.clone(), "INVALID_REQUEST");
    copy["key"] = json!("copied-project");
    let second = call(&f.app, "project_create_v3", copy).unwrap();
    assert_ne!(first["project_id"], second["project_id"]);
    assert_eq!(list(&f.app)["projects"][1]["name"], args["name"]);
    let mut rename = update(&f.app, 3, 2, &first["project_id"]);
    rename["key"] = json!("copied-project");
    f.assert_rejected("project_update_v3", rename.clone(), "INVALID_REQUEST");
    rename["key"] = json!("renamed-project");
    rename["name"] = json!("would erase old compatibility name");
    f.assert_rejected("project_update_v3", rename.clone(), "INVALID_REQUEST");
    rename["name"] = args["name"].clone();
    call(&f.app, "project_update_v3", rename).unwrap();
    assert_eq!(list(&f.app)["projects"][0]["name"], args["name"]);
    assert_eq!(list(&f.app)["projects"][0]["key"], "renamed-project");

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (10..12)
        .map(|n| {
            let app = f.app.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut args = create(&app, n, 3);
                args["key"] = json!("same-concurrent-key");
                barrier.wait();
                call(&app, "project_create_v3", args)
            })
        })
        .collect();
    let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| r.as_ref().is_err_and(|e| e.code == "STALE_REVISION"))
            .count(),
        1
    );
    let mut retry = create(&f.app, 20, 4);
    retry["key"] = json!("same-concurrent-key");
    f.assert_rejected("project_create_v3", retry, "INVALID_REQUEST");
    let app = f.reopen().unwrap();
    assert_eq!(list(&app)["projects"].as_array().unwrap().len(), 3);
}

#[test]
fn projects_v3_disk6_through_disk8_migrate_losslessly_without_backfill() {
    for version in 6..=8 {
        let f = Fixture::new(Durable::default());
        let old = structured_legacy_disk(version);
        let bytes = f.write(&old);
        let (next, migrated) = Durable::load(&bytes).unwrap();
        assert!(migrated);
        next.validate().unwrap();
        let converted = json!(next);
        for (key, value) in old.as_object().unwrap() {
            if !matches!(key.as_str(), "schema_version" | "projects" | "config") {
                assert_eq!(converted[key], *value, "disk{version} {key}");
            }
        }
        let mut config = old["config"].clone();
        if version == 6 {
            config["revision"] = json!(42);
        }
        assert_eq!(converted["config"], config);
        for key in ["revision", "receipts", "expired_ids"] {
            assert_eq!(converted["projects"][key], old["projects"][key]);
        }
        for (index, project) in old["projects"]["projects"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let mut expected = project.clone();
            expected["key"] = json!(format!(
                "project-{}",
                project["project_id"].as_str().unwrap()
            ));
            expected["description"] = json!("");
            expected["prompt"] = json!("");
            for row in expected["repositories"].as_array_mut().unwrap() {
                row["read_only"] = json!(true);
            }
            assert_eq!(converted["projects"]["projects"][index], expected);
        }
        let app = f.reopen().unwrap();
        let published = f.disk();
        assert_eq!(published["schema_version"], 10);
        assert_eq!(published["revision"], old["revision"].as_u64().unwrap() + 1);
        assert_eq!(published["projects"], converted["projects"]);
        if version >= 7 {
            assert_eq!(published["instructions"], old["instructions"]);
        }
        if version == 8 {
            assert_eq!(published["agents"], old["agents"]);
        }
        assert_eq!(
            receipt(&app, 504).unwrap(),
            old["projects"]["receipts"][3]["receipt"]
        );
        assert_eq!(receipt(&app, 599).unwrap_err().code, "REQUEST_EXPIRED");
        drop(app);
        let bytes = f.bytes();
        let app = f.reopen().unwrap();
        assert_eq!(f.bytes(), bytes);
        assert_eq!(list(&app)["projects"].as_array().unwrap().len(), 2);
    }
    let f = Fixture::new(Durable::default());
    for version in 6..=8 {
        let mut old = structured_legacy_disk(version);
        old["projects"] = json!({"revision":0,"projects":[],"receipts":[],"expired_ids":[]});
        f.write(&old);
        let app = f.reopen().unwrap();
        assert_eq!(list(&app), json!({"revision":0,"projects":[]}));
    }
}

#[test]
fn projects_v3_frozen_legacy_shapes_and_work_grammar_reject_without_rewriting() {
    let f = Fixture::new(Durable::default());
    for version in 6..=8 {
        let good = structured_legacy_disk(version);
        for (path, value) in [
            (
                "/projects/projects/1/repositories/0/branch/work",
                json!("topic/*"),
            ),
            (
                "/projects/projects/1/repositories/0/branch/base",
                json!("topic/*"),
            ),
            (
                "/projects/projects/1/repositories/0/branch/types/feature",
                json!("topic/**"),
            ),
            ("/projects/projects/1/repositories/0/name", json!("")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(path).unwrap() = value;
            let bytes = f.write(&bad);
            assert!(f.reopen().is_err(), "accepted disk{version} {path}");
            assert_eq!(f.bytes(), bytes);
        }
        for (parent, field, value) in [
            ("/projects/projects/1", "key", json!("not-an-old-field")),
            ("/projects/projects/1", "description", json!("")),
            ("/projects/projects/1", "prompt", json!("")),
            (
                "/projects/projects/1/repositories/0",
                "read_only",
                json!(true),
            ),
        ] {
            let mut bad = good.clone();
            bad.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), value);
            let bytes = f.write(&bad);
            assert!(f.reopen().is_err());
            assert_eq!(f.bytes(), bytes);
        }
        let duplicate = serde_json::to_string(&good).unwrap().replace(
            "\"work\":\"team/next\"",
            "\"work\":\"team/next\",\"work\":\"topic/*\"",
        );
        fs::write(&f.app.path, duplicate.as_bytes()).unwrap();
        assert!(f.reopen().is_err());
        assert_eq!(f.bytes(), duplicate.as_bytes());
    }
    let mut bad = structured_legacy_disk(8);
    bad.as_object_mut().unwrap().remove("agents");
    let bytes = f.write(&bad);
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_v3_old_operations_refuse_before_replay_even_empty_or_faulted() {
    for populated in [false, true] {
        let f = Fixture::new(Durable::default());
        let args = create(&f.app, 1, 0);
        if populated {
            call(&f.app, "project_create_v3", args.clone()).unwrap();
        }
        for faulted in [false, true] {
            f.app.inner.lock().unwrap().fault = faulted;
            let before = f.bytes();
            for op in [
                "projects_list",
                "project_create",
                "project_update",
                "project_delete",
                "projects_list_v2",
                "project_create_v2",
                "project_update_v2",
                "project_delete_v2",
            ] {
                for input in [
                    json!({}),
                    json!([]),
                    Value::Null,
                    args.clone(),
                    json!({"request_id":id(1),"repositories":[]}),
                ] {
                    assert_eq!(
                        call(&f.app, op, input).unwrap_err().code,
                        "UPGRADE_REQUIRED",
                        "{op}"
                    );
                    assert_eq!(f.bytes(), before);
                }
            }
            f.app.inner.lock().unwrap().fault = false;
        }
        if populated {
            assert_eq!(receipt(&f.app, 1).unwrap()["revision"], 1);
            assert_eq!(
                call(&f.app, "project_create_v3", args).unwrap()["revision"],
                1
            );
        }
    }
    let f = Fixture::new(Durable::default());
    f.write(&structured_legacy_disk(8));
    let app = f.reopen().unwrap();
    for op in [
        "project_create_v2",
        "project_update_v2",
        "project_delete_v2",
    ] {
        assert_eq!(
            call(&app, op, json!({"request_id":id(504)}))
                .unwrap_err()
                .code,
            "UPGRADE_REQUIRED"
        );
    }
    assert_eq!(receipt(&app, 504).unwrap()["revision"], 4);
    assert_eq!(
        call(&app, "project_create_v3", create(&app, 504, 4))
            .unwrap_err()
            .code,
        "ID_CONFLICT"
    );
}

#[test]
fn projects_v3_current_required_fields_and_hidden_corruption_are_preserved() {
    let f = Fixture::new(Durable::default());
    let mut args = create(&f.app, 1, 0);
    args["repositories"] = json!([repository()]);
    call(&f.app, "project_create_v3", args).unwrap();
    let good = f.disk();
    for (parent, fields) in [
        ("/projects/projects/0", vec!["key", "description", "prompt"]),
        ("/projects/projects/0/repositories/0", vec!["read_only"]),
    ] {
        for field in fields {
            for remove in [false, true] {
                let mut bad = good.clone();
                let object = bad.pointer_mut(parent).unwrap().as_object_mut().unwrap();
                if remove {
                    object.remove(field);
                } else {
                    object.insert(field.into(), Value::Null);
                }
                let bytes = f.write(&bad);
                assert!(f.reopen().is_err(), "{parent}/{field}");
                assert_eq!(f.bytes(), bytes);
            }
        }
    }
    for (path, value) in [
        ("/projects/projects/0/key", json!("Upper")),
        ("/projects/projects/0/description", json!("a\u{2029}b")),
        ("/projects/projects/0/prompt", json!("x".repeat(1001))),
        ("/projects/projects/0/repositories/0/read_only", json!(1)),
        (
            "/projects/projects/0/repositories/0/branch/work",
            json!("topic/**"),
        ),
        (
            "/projects/projects/0/repositories/0/branch/types/feature",
            json!("HEAD/*"),
        ),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(path).unwrap() = value;
        let bytes = f.write(&bad);
        assert!(f.reopen().is_err(), "{path}");
        assert_eq!(f.bytes(), bytes);
    }
    let surrogate = serde_json::to_string(&good)
        .unwrap()
        .replace("\"prompt\":\"\"", "\"prompt\":\"\\ud800\"");
    fs::write(&f.app.path, surrogate.as_bytes()).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), surrogate.as_bytes());
    f.write(&good);
    let app = f.reopen().unwrap();
    let mut args = create(&app, 2, 1);
    args["repositories"] = json!([repository()]);
    for invalid in [Value::Null, json!("true"), json!(1), json!([])] {
        args["repositories"][0]["read_only"] = invalid;
        assert_eq!(
            call(&app, "project_create_v3", args.clone())
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }
    args["repositories"][0]
        .as_object_mut()
        .unwrap()
        .remove("read_only");
    assert_eq!(
        call(&app, "project_create_v3", args).unwrap_err().code,
        "INVALID_REQUEST"
    );
    let mut duplicate = good.clone();
    let mut second = duplicate["projects"]["projects"][0].clone();
    second["project_id"] = json!(id(987));
    duplicate["projects"]["projects"]
        .as_array_mut()
        .unwrap()
        .push(second);
    let bytes = f.write(&duplicate);
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), bytes);
}

#[test]
fn projects_v3_migration_publication_failure_preserves_disk8_bytes() {
    let f = Fixture::new(Durable::default());
    let old = structured_legacy_disk(8);
    let bytes = f.write(&old);
    let staging = f.app.path.parent().unwrap().join(format!(
        ".smith-next-app-state.json-{}-tmp",
        std::process::id()
    ));
    fs::write(&staging, b"migration blocker").unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(f.bytes(), bytes);
    assert_eq!(fs::read(&staging).unwrap(), b"migration blocker");
    fs::remove_file(staging).unwrap();
    let app = f.reopen().unwrap();
    assert_eq!(list(&app)["revision"], 4);
    assert_eq!(f.disk()["agents"], old["agents"]);
    assert_eq!(f.disk()["instructions"], old["instructions"]);
}

#[test]
fn projects_v3_new_text_fields_are_in_encoded_capacity_and_reply_bounds() {
    let mut maximal = Durable::default();
    maximal.projects = maximal_registry();
    maximal.validate().unwrap();
    let maximal_bytes = maximal.reserved_bytes().unwrap();
    let reply = json!({"revision":maximal.projects.revision,"projects":maximal.projects.projects});
    assert!(reply_fits(&reply));
    assert!(serde_json::to_vec(&reply).unwrap().len() < FRAME_LIMIT);
    for project in &mut maximal.projects.projects {
        project.description.clear();
        project.prompt.clear();
    }
    assert_eq!(
        maximal_bytes - maximal.reserved_bytes().unwrap(),
        MAX_PROJECTS * 4 * (MAX_DESCRIPTION_SCALARS + MAX_PROMPT_SCALARS)
    );
}
