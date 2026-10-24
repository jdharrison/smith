# LOCAL-SMITH-DEFAULT-PROJECT — service self-check

Implemented service slice only; **not independent assurance or human approval**. Canonical documentation and compiled Dart/debug-service fixtures remain with their respective owners. No Git, host installation, GUI, live provider, repository or external network operation was performed.

## Implementation / traceability

- R1/AC1: explicit `projects::bootstrap(path)` creates exactly one ordinary Smith project, null repository Name, specified public URL, main/main, four null types and opencode. Request ID uses the ordinary generator; project ID uses its generated prefix and registry-revision suffix. Application and registry revisions are 1; one content-free receipt and metadata fingerprint are retained, with no expired IDs.
- R2/AC2: new tests exercise ordinary rename, repository Name/URL/Base/Work/all four type fields, nullable clearing, removal of all repositories, delete-last, fingerprint scrubbing, historical receipt queries and repeated restart without resurrection.
- R3/AC3: shared Registry/Durable defaults stay empty. Existing current empty/populated/full/tombstone-exhausted state is byte-stable on reopen. New disk1–6 cohort coverage verifies no bootstrap insertion or receipt/capacity change to the registry; existing migrations/instruction activation remain authoritative.
- R4/AC4: full validation, capacity preflight, encoding and STATE_LIMIT precede `filesystem::publish(path, bytes, 0600, false)`. Tests check publication before open returns, private mode, stable IDs/bytes, corrupt/empty/unsupported/mode/symlink/hardlink/directory preservation, deterministic staging obstruction, appeared-destination no-clobber, ordinary project capacity and revoked ownership.
- Only the `None` arm changed in `app.rs`; all other bytes match the pre-edit capture. No filesystem algorithm, migration, schema, API, provider, runner, instruction or production Flutter code changed. Existing tests remain intact; additions precede them.

## Validation evidence

All commands ran from repository root. Cargo commands used `--manifest-path service/Cargo.toml --locked --offline`. Full commands, timestamps, exit codes and raw output are in the linked logs, mirrored under `target/default-project/service/`.

| Gate | Actual result | Raw log |
|---|---|---|
| New `projects_bootstrap` tests | PASS: 7 passed, 0 failed | [focused-04.log](focused-04.log) |
| All Projects unit tests | BLOCKED: 24 passed, 5 failed; all seven new tests pass | [projects-02.log](projects-02.log) |
| Scoped rustfmt check, including app.rs without recursive formatting | PASS | [format-check-02.log](format-check-02.log) |
| Clippy `--lib -- -D warnings` | PASS | [clippy-lib-01.log](clippy-lib-01.log) |
| Clippy `--all-targets -- -D warnings` | BLOCKED by out-of-scope instruction-test lint | [clippy-02.log](clippy-02.log) |
| Locked/offline build | PASS | [build-01.log](build-01.log) |

**Quality Gate:** scoped formatting passes; production library warning-denied Clippy/build pass; broader test failures and all-target warning-denied Clippy remain blocking. No acceptance/release PASS is claimed.

### Precise remaining failures (unmodified existing tests)

From `projects-02.log`:

1. `projects_actual_disk5_migration_preserves_opaque_state_and_legacy_requests_never_replay`: `Corrupt v5 state; preserved` while loading its rich old fixture.
2. `projects_crud_receipts_and_empty_registry_survive_restart_without_provider_calls`: schema assertion actual 7, expected 6.
3. `projects_disk1_through_frozen_disk4_migrate_empty_preserving_all_existing_data`: legacy projection differs: config revision 8 vs 7, and the pending historical job is interrupted rather than running.
4. `projects_disk6_and_frozen_legacy_schemas_reject_corruption_without_rewriting_bytes`: `corrupt fixture accepted` at its existing assertion.
5. `projects_global_revision_is_independent_of_chat_mutations_and_pending_work`: fixture persistence rejects `Invalid instructions state; preserved`.

All-target Clippy: `service/src/app_instructions_tests.rs:21`, `clippy::field_reassign_with_default` under `-D warnings`. No out-of-scope remediation, test suppression, default/version-policy changes or discarded tests.

These failures occur in existing fixtures/assertions against the concurrent instruction/disk7 source. Their tests were not edited. No separate baseline test execution was performed, so this report does not claim experimentally established baseline outcomes.

### Preserved intermediate failures

- [focused-01.log](focused-01.log): initial compile rejected struct-update syntax accessing private Durable fields. Resolved by using the unchanged default constructor and assigning public fields.
- [focused-02.log](focused-02.log): 6/7 new tests passed; rich disk5 fixture exhausted configuration revision during existing migration.
- [focused-03.log](focused-03.log): 6/7 new tests passed; first fixture-headroom adjustment put configuration below historical job revisions. Resolved only in the new cohort test by choosing revision 10, above historical revision 7 with migration headroom.
- [projects-01.log](projects-01.log), [clippy-01.log](clippy-01.log): earlier broader failures retained unchanged. No logs/evidence were overwritten.

## Source binding

Baseline manifest identity supplied by the parent: `aeb1ce67ebb10c0af866313901b7060a921018be60fd267901323a4d69d56ca2`; archive SHA-256 verified as `6073eea510d781fb3acd83aa9afe11c93ce57851bf7fd6c07d1cc3e132a1c6b8` (1316 archived files; manifest 1994 records). All three owned files matched the archive at pre-edit capture: [baseline-binding.json](baseline-binding.json).

Final SHA-256:

- `service/src/app.rs`: `b90bc4b7414474a59bb6f13b970b3a5200a06d93162be6ed395e9636d8f3fa8b`
- `service/src/app_projects.rs`: `bc613eacb5e6d56c4d2551a9dfac29aa084f434ab262d6a4c4847a071e2e68e4`
- `service/src/app_projects_tests.rs`: `6f827d392dd1d8dac2fcd63f6951913a636ddd875aac71ad5236725a655d664a`

[final-source-validation.json](final-source-validation.json) records service source/test/Cargo identities before and after final Projects/format/all-target Clippy checks: no observed drift; `app_outside_exact_none_arm_unchanged=true`. [post-build-binding.json](post-build-binding.json) confirms no subsequent source drift through the build. This is a bounded service-source observation, not whole-worktree or future stability. [service-source.patch](service-source.patch) contains the exact three-file diff against the captured archive. [before-source.json](before-source.json) retains pre-edit identities.

## API / repository research notes

`persist()` validates and then publishes with replacement enabled, so it is deliberately not used for the initial publication. Existing `filesystem::publish(..., false)` stages/syncs in the same directory and uses hard-link creation for no-clobber. Errors propagate without adoption, retry or cleanup of a possibly published destination. No schema or receipt validator exceptions were introduced; the internal fingerprint uses the existing fingerprint encoding and is scrubbed by ordinary deletion.

## Remaining gates

Concurrent owner reconciliation of existing instruction/disk7 fixtures and all-target lint; parent-owned canonical documentation, compiled Rust–Dart fixtures and final combined-source gates; fresh independent assurance/adversarial review and explicit human signoff. Service self-check grants no approval or external-effect authority.
