# LOCAL-SMITH-DEFAULT-PROJECT — compiled integration slice

**Implemented fixture changes; scoped self-check PASS against the retained snapshot. Not final combined-candidate acceptance or independent assurance.** Recorded 2026-10-17 UTC. No production Flutter, canonical policy, provider, runtime flag, or API changes belong to this slice.

## Scope and assertions

- `client/test/rust_projects_integration_test.dart`: one distinct absent-profile case takes the real Rust startup path, without existing-state preparation. It verifies the normal list record and persisted registry revision1, opaque identity, Name `Smith`, agent `opencode`, repository Name null, exact GitHub URL, Base/Work `main`, and all four explicit null type leaves. One retained bootstrap receipt and no expired IDs are checked.
- The same case reopens the unchanged persisted starter, updates its name/branches/repository name/URL/types and adds a second repository using ordinary update, persists those changes, clears repositories using another ordinary update, then deletes the last record. Repeated restarts preserve empty revision4 and byte-identical deleted state. Every unrelated durable field is compared, excluding only the ordinary application revision and Projects registry. This includes the current `instructions` sidecar.
- All **seven original compiled tests remain**, with their original revision0/count/replay/uncertainty/staleness/legacy guards. Their shared setup now explicitly constructs **existing empty current state**, preserving every generated JSON value except replacing `projects` with exactly `{revision:0,projects:[],receipts:[],expired_ids:[]}`. Saved-only bootstrap has no queued/running jobs (asserted). Restart occurs before Projects controller construction; reopened JSON must equal the prepared document. This is not a fresh-profile test or deletion through the API.
- `service/tests/debug_service.rs`: only `dart_cloud` fixture helper/test hunks change. Frozen disk5 construction expects current disk7 input and omits the synthetic unused instruction sidecar, which disk5 cannot represent; snapshots/grants/receipts/expired IDs must be empty. Its Rust control test explicitly prepares an existing empty registry. Disk7 reopen and its exact one-step configuration-revision migration are asserted, retaining exact comparison of other configuration/chat fields. No production migration contract was changed.
- The fresh case permits only catalog/list/update/delete API operations, asserts exactly two updates and one delete, and observes zero injected local inference, HTTP, credential reads, or messages. These are isolated synthetic fixture observations plus metadata-only call-path coverage, not live-host network/process monitoring or installed acceptance. Fixture setup itself performs existing synthetic catalog/configuration/fake-key preparation before resetting counters; no live provider/keyring/manager/repository is involved.

## Commands and outcomes

All commands have timestamped JSON records and logs, including intermediate failures. `run.py` records input hashes before/after; original raw records remain under `target/default-project/integration/`. Published text copies replace absolute workspace/home prefixes with `<workspace>`/`<home>` and therefore have separately recorded hashes.

| Record | Command/scenario | Result |
|---|---|---|
| 01 | `dart format test/rust_projects_integration_test.dart` | PASS; formatted changed fixture |
| 02 | `rustfmt --edition 2021 --check tests/debug_service.rs` | PASS |
| 03 | `cargo test --offline --locked --test debug_service --no-run`, first frozen snapshot | FAIL: E0451 private `Durable` fields in service-owner bootstrap |
| 04 | `flutter analyze --no-pub test/rust_projects_integration_test.dart` | FAIL: local helper referenced before declaration; fixture fixed |
| 05–09 | scoped analyzer and final Dart/Rust format checks | PASS; no analyzer issues, final format unchanged |
| 10 | `flutter test --no-pub --reporter expanded test/projects_protocol_test.dart test/projects_controller_test.dart` | PASS: 85 tests |
| 11 | same locked offline compile, second frozen service snapshot | PASS |
| 12 | `flutter test --no-pub --reporter expanded test/rust_projects_integration_test.dart`, `SMITH_TEST_CLOUD_SERVICE` bound to newly compiled binary | PASS: 8 tests, no skips |
| 13 | exact Rust `dart_cloud::disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects` | PASS: 1 test |
| 14 | exact Rust `dart_cloud::bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts` | PASS: 1 test |

The compile used isolated service source copies and the existing host Cargo.lock, **offline and locked**. No tracked lockfile/manifest modification was made. The second compile reused the first isolated Cargo target cache via `CARGO_TARGET_DIR=target/default-project/integration/build-source/service/target` (resolved absolute for Cargo). Actual successful source is `build-source-2/service`, not the first failed source snapshot. Tool versions: Flutter3.47.2/Dart3.13.2, Cargo1.98.0. No test was run against or counted from the old pre-seed binary.

## Identity and artifacts

Baseline embedded identity is the user-supplied `aeb1ce67ebb10c0af866313901b7060a921018be60fd267901323a4d69d56ca2`, count1994. The baseline JSON file's own byte hash is `fbcdd9d331cdc7f49ce8092b547daa0a74b77d7230f5b547499eca3fac0708d7`; these are different identity conventions, not a baseline overwrite.

| Artifact | SHA-256 |
|---|---|
| Retained `target/default-project/integration/debug_service-fixture` | `376b9e57e06a290a533dbf5ed0d5e264161a81336ba9e37c97f93e82f8cf8e9a` |
| Successful service source archive (same target directory) | `029c5612e64f3d347e608a3430a0f54488347b11935b8bc4907a6ccbd5ee7bc1` |
| Successful Dart source archive (same target directory) | `09c1b78e45b457a1a0872ba9c2aacb6961f90b365ce3c972f9f6c1b741132795` |
| Successful service source manifest | `535decc01494a8c269260ee0b7a277ea5b0567ff88f1f62ceae4003b452cc87d` |
| Final Dart fixture | `f6a1bef9d5f5a15e0ca4bec59df9ae9c1aa12f0c4c567ff607487d7c69569afc` |
| Final Rust fixture file | `e08425bc1f63b8c3dea35b4156cfce3b79bd7b520a3f126e60d89125ee3b88ec` |

`final-fixture-from-baseline.patch` isolates this slice from pre-existing dirty/untracked work. `capture-summary.json`, both service snapshot manifests, command records, and `SHA256SUMS` retain exact identities and failures. No previous evidence was overwritten. Original raw archives/binary/logs are local target artifacts; published documentation contains source identities and redacted textual evidence, not binary/source archives.

## Remaining parent gates

The retained service snapshot is immutable and the Dart inputs used by the successful run were unchanged. The **live shared worktree was not stable**: only `service/src/app_projects_tests.rs` changed during the compiled run and differed from the retained snapshot at capture. `compiled_source_stable_during_run:false` in the capture summary describes this live-worktree drift, not mutation of the isolated compiled source. This fixture result is not a final-current-worktree assertion. Parent must rebuild/rerun the compiled fixtures and required component/root/package gates once shared service/Global Context inputs settle, and obtain fresh independent review. Full service tests/Clippy, root gates, package checks, graphical/installed migration acceptance and human approval are not claimed here.

To replay the retained compiled candidate from `client/` without a rebuild:

```sh
SMITH_TEST_CLOUD_SERVICE=../target/default-project/integration/debug_service-fixture flutter test --no-pub --reporter expanded test/rust_projects_integration_test.dart
```

For final combined-candidate evidence, use a newly rebuilt binary bound to the settled source instead of this retained candidate.
