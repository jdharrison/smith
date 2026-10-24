// Reviewer-only tests appended to the isolated copy of app_projects_tests.rs.
#[test]
fn assurance_duplicate_keys_at_every_nested_level_preserve_disk5_and_disk6_bytes() {
    let f = Fixture::new(Durable::default());
    let mut args = create(&f.app, 4242, 0);
    args["repositories"] = json!([repository()]);
    call(&f.app, "project_create_v2", args).unwrap();
    let current = f.disk();
    let (legacy, _) = frozen_disk5();
    for disk in [current, legacy] {
        let text = serde_json::to_string(&disk).unwrap();
        for key in [
            "schema_version",
            "projects",
            "revision",
            "project_id",
            "repositories",
            "receipt",
            "fingerprint",
            "expired_ids",
            "name",
            "url",
            "branch",
            "base",
            "work",
            "types",
            "feature",
            "fix",
            "release",
            "hotfix",
        ] {
            let needle = format!("\"{key}\":");
            if !text.contains(&needle) {
                continue;
            }
            // An invalid first occurrence must not disappear behind the valid last one.
            let bad = text.replacen(&needle, &format!("\"{key}\":false,{needle}"), 1);
            fs::write(&f.app.path, bad.as_bytes()).unwrap();
            assert!(f.reopen().is_err(), "accepted duplicate {key}");
            assert_eq!(f.bytes(), bad.as_bytes(), "rewrote duplicate {key}");
        }
    }
}
#[test]
fn assurance_null_migration_unrelated_save_nested_id_conflicts_and_legacy_erasure_refusal() {
    let f = Fixture::new(Durable::default());
    let (old, _) = frozen_disk5();
    f.write(&old);
    let app = f.reopen().unwrap();
    let before = list(&app);
    let target = &before["projects"][1]["project_id"];
    let mut edit = update(&app, 4243, 4, target);
    edit["name"] = json!("Unrelated rename");
    edit["repositories"] = before["projects"][1]["repositories"].clone();
    call(&app, "project_update_v2", edit.clone()).unwrap();
    assert_eq!(
        list(&app)["projects"][1]["repositories"],
        before["projects"][1]["repositories"]
    );
    let durable = f.bytes();
    for op in [
        "project_update",
        "project_delete",
        "project_create",
        "projects_list",
    ] {
        for args in [
            json!({}),
            json!({"request_id":id(4243),"repositories":[]}),
            edit.clone(),
            Value::Null,
        ] {
            assert_eq!(call(&app, op, args).unwrap_err().code, "UPGRADE_REQUIRED");
            assert_eq!(f.bytes(), durable);
        }
    }
    for path in [
        "/name",
        "/url",
        "/branch/base",
        "/branch/work",
        "/branch/types/feature",
        "/branch/types/fix",
        "/branch/types/release",
        "/branch/types/hotfix",
    ] {
        let mut changed = edit.clone();
        *changed["repositories"][0].pointer_mut(path).unwrap() = json!("different");
        assert_eq!(
            call(&app, "project_update_v2", changed).unwrap_err().code,
            "ID_CONFLICT"
        );
        assert_eq!(f.bytes(), durable);
    }
}
#[test]
fn assurance_branch_ascii_and_total_pattern_limit() {
    for b in 0u8..=127 {
        let s = format!("a{}z", char::from(b));
        let expected = b.is_ascii_alphanumeric() || b"._-".contains(&b) || b == b'/';
        assert_eq!(valid_branch(&s), expected, "byte {b}");
    }
    for n in 250..=258 {
        assert_eq!(valid_branch(&"a".repeat(n)), n <= 255);
        assert_eq!(valid_pattern(&format!("{}/*", "a".repeat(n))), n <= 253);
    }
}
