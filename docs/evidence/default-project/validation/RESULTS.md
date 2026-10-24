# LOCAL-SMITH-DEFAULT-PROJECT — parent-owned validation

**Observed validation completed; full quality/acceptance gate BLOCKED.** This is implemented-source evidence, not an accepted-risk decision, independent assurance, installed acceptance or human approval. No product/test/policy fixes, commits, source/index manipulations, GUI/product launch, installed-manager, provider/model/keyring, repository or remote actions were performed by this validation. Only the two new validation directories contain validator-authored files; isolated fixtures and configured Dagger pipeline ran as authorized. Existing Global Context inputs were preserved. No earlier owner/layout pass is counted as this run.

## Source and artifact binding

- Full executable core: **1275 records, 33 untracked**; HEAD/branch in [snapshot-binding.json](snapshot-binding.json). Core includes service/client/installer/dagger/scripts/CI and root build/config inputs. Generated caches/exports, parent-owned docs and target evidence are excluded by the inherited recorder's explicit policy.
- Before and final identity: `e51a718bbc9d990622ff64e3847e7bae095416d66f344fb48fb7e6be5e8d12d9`. **Stable at capture, throughout every command and final observation**; no changed live core paths. Full [before](core-manifest-before.json) and [final](core-manifest-final.json) manifests retain path/type/mode/size/content/tracked identities. This is bounded observation, not protection against future writes.
- Immutable copied candidate core archive: `target/default-project/validation/candidate-core.tar.gz`, SHA-256 `6290a3abceb7a4bba342a00818a5bc0886524b52622de48396d1c20bf536ce51`. [Final integrity](snapshot-final-integrity.json) finds no original candidate core file changed by validation and no control source changed since overlay.
- Newly rebuilt fixture: `target/default-project/validation/debug_service-fixture`, SHA-256 `79b38c181580772aec9c8d97ea20008f211787cbc68d85023b573b8eae6e7737`. `cargo test --manifest-path Cargo.toml --locked --offline --test debug_service --no-run` ran on the bound candidate snapshot with a fresh isolated target. No old/preseed executable was selected. [Fixture binding](fixture-binding.json).
- Baseline embedded manifest identity: `aeb1ce67ebb10c0af866313901b7060a921018be60fd267901323a4d69d56ca2`; baseline archive verified SHA-256 `6073eea510d781fb3acd83aa9afe11c93ce57851bf7fd6c07d1cc3e132a1c6b8`. Manifest byte hash and overlay hashes are separately retained in [negative-control-binding.json](negative-control-binding.json).

Current task and fixture source SHA-256:

- `service/src/app.rs`: `b90bc4b7414474a59bb6f13b970b3a5200a06d93162be6ed395e9636d8f3fa8b`
- `service/src/app_projects.rs`: `bc613eacb5e6d56c4d2551a9dfac29aa084f434ab262d6a4c4847a071e2e68e4`
- `service/src/app_projects_tests.rs`: `6f827d392dd1d8dac2fcd63f6951913a636ddd875aac71ad5236725a655d664a`
- `service/tests/debug_service.rs`: `e08425bc1f63b8c3dea35b4156cfce3b79bd7b520a3f126e60d89125ee3b88ec`
- `client/test/rust_projects_integration_test.dart`: `f6a1bef9d5f5a15e0ca4bec59df9ae9c1aa12f0c4c567ff607487d7c69569afc`

## Actual checks

Exact argv, working directories, sanitized allowlisted environment, start/end/timeouts/durations/exits, before/after core identities and logs: [COMMANDS.md](COMMANDS.md). Each invocation was finite; no test command timed out. Rust host toolchain1.98.0, Flutter3.47.2/Dart3.13.2; cached offline Cargo dependencies and cached local SDK/pub packages were used, not a clean-cache claim.

| Current check | Actual result |
|---|---|
| Rust `--lib projects_bootstrap` locked/offline | **PASS 7/7**, 0 ignored |
| Rust `--lib projects_` locked/offline | **FAIL 24 passed / 5 failed**, 0 ignored, 126 filtered |
| Production Rust `clippy --lib -- -D warnings` | **PASS** |
| Rust `clippy --all-targets -- -D warnings` | **FAIL** `app_instructions_tests.rs:21`, `field_reassign_with_default` |
| Full service `cargo fmt --all -- --check` | **PASS** |
| Actual current `debug_service` fixture rebuild | **PASS** |
| Exact fixture disk5 control and bootstrap/reset control | **PASS 1 + 1**, 0 ignored; 24 filtered per exact control |
| Six Projects view/navigation/layout/golden/controller/protocol Dart files | **FAIL 168 passed / 1 failed**, total169, no skips reported |
| UI foundation assets | **PASS 5/5**, no skips |
| Actual rebuilt Rust↔Dart Projects | **PASS 8/8**, no skips; fresh starter and existing-empty fixtures |
| Scoped Flutter analyzer (three Projects production files and compiled fixture) | **PASS**, no issues |
| Full Flutter analyzer | **PASS**, no issues |
| Dart format over `lib test` | **PASS**, 100 files, 0 changed |
| Once-only configured root `make … test build` | **FAIL exit2**, 31.696s, 900s limit; underlying service Clippy exit101 |
| Rebuilt-package verification and actual-package helper tests | **NOT RUN**, root rebuild did not succeed |
| `git --no-pager diff --check` | **PASS** |

The Dart169-case total is derived from this run's terminal168 passes +1 failure, not assumed from historical layout evidence. The failing `projects_navigation_test.dart` case is “Projects is lazy and explicit; tab switches retain mounted chat, draft, reading and project edits”. At line236 it expects no `TextField`, but observes three Global Context fields (`global-instructions`, `global-personality`, `global-guardrails`). See [raw failure](flutter-focused-projects.log). This is a current blocker; no historical baseline attribution for that UI failure was attempted or claimed. Production Flutter was not modified by validation.

## Negative baseline control: experimentally bounded blame

The control is a **new current-service snapshot**, with ONLY full `app.rs`, `app_projects.rs`, `app_projects_tests.rs` replaced by exact pre-task archive bytes. Current unrelated Global Context, instruction/schema inputs and `tests/debug_service.rs` remain byte-identical; all overlay/source hashes and empty mismatch lists are recorded. It is deliberately not a pristine historical source checkout.

The first control attempt shared the candidate Cargo target. Inspection exposed artifact reuse: it executed29 tests including seven bootstrap cases absent from the overlaid source. **That attempt is invalid baseline evidence.** Its logs/records remain unchanged in [initial-attempt-invalid.json](negative-control-initial-attempt-invalid.json); no pass or blame conclusion is taken from it. The optional Clippy result in that shared-target attempt is also excluded from baseline evidence.

A separate recorded replacement uses an **initially absent, distinct control Cargo target**, unchanged control sources, the same locked/offline `--lib projects_` arguments, and the same all-target Clippy arguments. It really compiles the overlaid control and runs22 tests: **17 pass / 5 fail**, 0 ignored. All five failure names and full assertion/error/projection bodies are identical to candidate failures after removing only thread/PID/source-line locations and the backtrace note. [Comparison](negative-control-comparison.json): `same_failure_names=true`, `exact_normalized_failure_bodies_equal=true`.

- `projects_actual_disk5_migration_preserves_opaque_state_and_legacy_requests_never_replay`
- `projects_crud_receipts_and_empty_registry_survive_restart_without_provider_calls`
- `projects_disk1_through_frozen_disk4_migrate_empty_preserving_all_existing_data`
- `projects_disk6_and_frozen_legacy_schemas_reject_corruption_without_rewriting_bytes`
- `projects_global_revision_is_independent_of_chat_mutations_and_pending_work`

The separate control also reproduces the same instruction-test `field_reassign_with_default` lint at line21. Thus these five Rust failures and this lint are experimentally reproduced **without the default-project service changes, while retaining the current unrelated inputs**. That establishes independence from this slice under this controlled source comparison, not a general claim about all historical commits or a waiver of the failing gates. No failures were fixed, suppressed or ignored.

## Root attempt, cache/cancellation and stale exports

Exactly once, from the actual repository root:

```sh
make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build
```

Official CLI SHA-256 was verified against `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af`. The complete explicit environment sets `_EXPERIMENTAL_DAGGER_RUNNER_HOST=docker-container://smith-installer-pipeline-dagger-v0.21.9`, disables cloud-token/analytics inheritance and does not use PATH Dagger. Client/server both reportedv0.21.9. Docker read-only observations show the same already-running engine and start timestamp before/after; no restart/change was performed. Dagger's “starting engine” connection label is not evidence of a container restart.

The real configured pipeline had cached vertices and executed work. [Observed gates](root-observed-gates.json) enumerates27 cached vertex IDs and five `context canceled` stdout vertices. Setup included cached Flutter clone vertices; actual crates.io index/crate-download and apt traffic occurred within the configured pipeline. No new image/network authority or configuration was added. Source tests are not cold-cache/hermetic build evidence.

Service Clippy failed. Its unit tests were not reached after Clippy. Parallel installer UI, installer, client, package and no-Python CLI runtime stdout results were **context canceled**, not passes or independent test failures. No completed root test-suite totals were observed. `make build` was **not reached**. Graph-construction `DONE` labels are not suite-pass results. The root command was not retried.

Both downstream commands were explicitly recorded **not run**, because rebuild did not succeed:

```sh
python3 -B dagger/package.py --root installer/dist --verify
SMITH_TEST_PACKAGE=installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
```

Exports were hashed before/after and remain unchanged/stale for this candidate:

- `installer/dist`: `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725` before and `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725` after; identical: `True`.
- `client/dist`: `9864da238d469c2f43afe639880f231c8df558a5825282eb3b241d9990b48141` before and `9864da238d469c2f43afe639880f231c8df558a5825282eb3b241d9990b48141` after; identical: `True`.
- `service/dist`: `37795320c22f09ebf877ec6f737a852571d2a6f24c13dfb9618b5c90669edc86` before and `37795320c22f09ebf877ec6f737a852571d2a6f24c13dfb9618b5c90669edc86` after; identical: `True`.

Hashes above are canonical tree-manifest identities including path/type/mode/size/content, not archive hashes. Full file records are in the `*-dist-before.json`/`*-dist-after.json` files. **No exported package is claimed current.** Existing export asset/helper contents are not promoted to a fresh package-verification pass.

## Requirement/gate disposition

- R1–R4: seven fresh/bootstrap/preservation service cases and the fresh compiled case passed on the stable bound candidate. Broader Rust failures remain blocking even though controlled comparison reproduces them without this slice.
- R5: eight compiled cases and assets pass; Projects/layout regression group has the one blocking navigation assertion above. No installed graphical or live-host acceptance is inferred from synthetic fixtures.
- R6/T5: source binding and current parent commands are recorded; root/package combined gate is **BLOCKED**. Fresh independent assurance and human signoff remain separate parent work. No accepted risk is granted.

**Quality Gate: BLOCKED — formatting and scoped/full analysis pass; production Rust warnings-denied Clippy passes; all-target Clippy, five Projects Rust tests, one Flutter navigation test and incomplete root/package gates block full acceptance.**
