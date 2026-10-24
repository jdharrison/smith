#[test]
fn assurance_bootstrap_receipt_eviction_and_delete_never_seed() {
    let mut f = fresh_fixture();
    let initial = f.disk()["projects"]["receipts"][0]["receipt"].clone();
    let target = initial["project_id"].clone();
    let rid = initial["request_id"].as_str().unwrap();
    let mut collision = create(&f.app, 9000, 1);
    collision["request_id"] = json!(rid);
    f.assert_rejected("project_create_v2", collision, "ID_CONFLICT");
    for revision in 1..=33 {
        call(
            &f.app,
            "project_update_v2",
            update(&f.app, 10000 + revision, revision as u64, &target),
        )
        .unwrap();
    }
    assert!(f.disk()["projects"]["expired_ids"]
        .as_array()
        .unwrap()
        .contains(&json!(rid)));
    assert_eq!(
        call(&f.app, "project_receipt", json!({"request_id":rid}))
            .unwrap_err()
            .code,
        "REQUEST_EXPIRED"
    );
    call(
        &f.app,
        "project_delete_v2",
        delete(&f.app, 11000, 34, &target),
    )
    .unwrap();
    for receipt in f.disk()["projects"]["receipts"]
        .as_array()
        .unwrap()
        .iter()
        .take(31)
    {
        assert!(receipt["fingerprint"].is_null());
    }
    let before = f.bytes();
    for _ in 0..3 {
        f.app = f.reopen().unwrap();
        assert_eq!(list(&f.app), json!({"revision":35,"projects":[]}));
        assert_eq!(f.bytes(), before);
    }
}

#[test]
fn assurance_whole_file_removal_only_creates_a_new_profile() {
    let mut f = fresh_fixture();
    let target = list(&f.app)["projects"][0]["project_id"].clone();
    call(
        &f.app,
        "project_delete_v2",
        delete(&f.app, 12000, 1, &target),
    )
    .unwrap();
    f.app = f.reopen().unwrap();
    assert!(list(&f.app)["projects"].as_array().unwrap().is_empty());
    fs::remove_file(&f.app.path).unwrap();
    f.app = f.reopen().unwrap();
    assert_eq!(list(&f.app)["revision"], 1);
    assert_eq!(list(&f.app)["projects"][0]["name"], "Smith");
    assert_ne!(list(&f.app)["projects"][0]["project_id"], target);
}

#[test]
fn assurance_dangling_symlink_and_appeared_symlink_are_not_adopted() {
    let f = Fixture::new(Durable::default());
    fs::remove_file(&f.app.path).unwrap();
    let missing = f.app.path.with_extension("missing");
    std::os::unix::fs::symlink(&missing, &f.app.path).unwrap();
    assert!(f.reopen().is_err());
    assert_eq!(fs::read_link(&f.app.path).unwrap(), missing);
    assert!(!missing.exists());
    assert!(bootstrap(&f.app.path).is_err());
    assert_eq!(fs::read_link(&f.app.path).unwrap(), missing);
    assert!(!missing.exists());
}

#[test]
fn assurance_appeared_valid_destination_is_preserved_without_adoption() {
    let f = Fixture::new(Durable::default());
    let before = f.bytes();
    assert_eq!(
        bootstrap(&f.app.path).err().unwrap(),
        "Destination exists or cannot publish"
    );
    assert_eq!(f.bytes(), before);
    let app = f.reopen().unwrap();
    assert_eq!(list(&app), json!({"revision":0,"projects":[]}));
    assert_eq!(f.bytes(), before);
}

#[test]
fn assurance_injected_post_link_failure_preserves_published_state() {
    let f = Fixture::new(Durable::default());
    fs::remove_file(&f.app.path).unwrap();
    filesystem::ASSURANCE_POST_LINK_FAILURE.with(|flag| flag.set(true));
    assert_eq!(
        bootstrap(&f.app.path).err().unwrap(),
        "ASSURANCE injected directory sync failure"
    );
    filesystem::ASSURANCE_POST_LINK_FAILURE.with(|flag| flag.set(false));
    let published = f.bytes();
    let (loaded, migrated) = Durable::load(&published).unwrap();
    assert!(!migrated);
    loaded.validate().unwrap();
    loaded.preflight().unwrap();
    assert_eq!(loaded.projects.revision, 1);
    let app = f.reopen().unwrap();
    assert_eq!(list(&app)["projects"].as_array().unwrap().len(), 1);
    assert_eq!(f.bytes(), published);
    assert!(bootstrap(&f.app.path).is_err());
    assert_eq!(f.bytes(), published);
}
