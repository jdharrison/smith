# LOCAL-SMITH-DEFAULT-PROJECT — independent ASSURANCE review

## Validation scope

- Fresh-context assurance, not the implementation owner; approved plan and absent-state-only, ordinary editable/deletable contract reviewed against the actual pre-task archive, not `git diff HEAD`.
- Read `AGENTS.md`, Scope, relevant Architecture/Process/Design/Host contracts, `roles/assurance.md`, and the approved plan. Review covers initialization, storage eligibility, ordinary identity/receipts, deletion and preservation, narrowly changed compiled fixtures, and evidence integrity.
- Independent locked/offline Rust checks used copied source and an initially absent assurance Cargo target. Additional adversarial tests and one synthetic fault hook changed **only the private copy**, not production source/tests/docs. No live profile, provider, model, keyring, manager, application launch, install or remote operation.
- Writes confined to `target/default-project/assurance/`, `docs/evidence/default-project/review/`, and this report. No source fixes, acceptance waivers, package promotion or external approval.

## Decisions

**Bounded feature-source assessment: PASS.** The reviewed implemented starter meets the approved fresh-only contract within the source/fixture checks below. No feature-owned correctness or authorization defect was found. This is not a whole-product quality verdict.

**Overall assurance verdict: FAIL. Combined quality gate: BLOCKED. Release Gate Decision: NO-GO.** Required combined-source tests/Clippy/root build fail and rebuilt-package evidence is missing. The scoped assessment cannot waive those gates or grant integration/ship acceptance. No residual risk is accepted.

## Exact source/evidence binding

Independently reconstructed the executable-core selection (1275 records;33 untracked):

`e51a718bbc9d990622ff64e3847e7bae095416d66f344fb48fb7e6be5e8d12d9`

It matches the parent's complete records, not merely its digest, and remained identical before/after independent commands and at final capture. See [core-before](review/core-before.json), [core-final](review/core-final.json), [binding](review/binding.json), and [source-identities](review/source-identities.json). This is observed stability, not a lock against future edits. Parent-owned README/ledgers updated later are outside the executable-core selection and are not pre-approved here.

Baseline:1994 records, canonical record SHA-256 `aeb1ce67ebb10c0af866313901b7060a921018be60fd267901323a4d69d56ca2`; archive SHA-256 `6073eea510d781fb3acd83aa9afe11c93ce57851bf7fd6c07d1cc3e132a1c6b8`. All archived file bytes agree with their baseline manifest records. The archive intentionally excludes evidence bytes; their recorded identities are not mistaken for archived content.

| Feature/fixture input | Current SHA-256 |
|---|---|
| `service/src/app.rs` | `b90bc4b7414474a59bb6f13b970b3a5200a06d93162be6ed395e9636d8f3fa8b` |
| `service/src/app_projects.rs` | `bc613eacb5e6d56c4d2551a9dfac29aa084f434ab262d6a4c4847a071e2e68e4` |
| `service/src/app_projects_tests.rs` | `6f827d392dd1d8dac2fcd63f6951913a636ddd875aac71ad5236725a655d664a` |
| `service/tests/debug_service.rs` | `e08425bc1f63b8c3dea35b4156cfce3b79bd7b520a3f126e60d89125ee3b88ec` |
| `client/test/rust_projects_integration_test.dart` | `f6a1bef9d5f5a15e0ca4bec59df9ae9c1aa12f0c4c567ff607487d7c69569afc` |

Parent's copied candidate archive independently rehashed to `6290a3abceb7a4bba342a00818a5bc0886524b52622de48396d1c20bf536ce51`; rebuilt fixture to `79b38c181580772aec9c8d97ea20008f211787cbc68d85023b573b8eae6e7737`. All167 entries in the parent validation checksum index verify; every command-record log digest verifies. See [parent-audit](review/parent-audit.json). Checksums establish consistency with retained evidence, not independent build provenance or human approval.

## Actual baseline delta and attribution

[Complete inspected baseline diff](review/baseline.diff) and [delta inventory](review/baseline-delta.json):14 changed existing files, no new executable-core paths. These are the five feature/fixture paths above plus nine canonical docs. The approved plan/evidence are separately bound additions outside executable core.

- `app.rs`: **only** `None => (Durable::default(), true)` becomes `None => (projects::bootstrap(&path)?, false)`; loaded/migrated/current-state arms and instruction activation remain untouched.
- `app_projects.rs`: only explicit bootstrap factory added; shared empty defaults, validators, ordinary CRUD/replay, limits, schema and wire API unchanged.
- `app_projects_tests.rs`: new helper and seven tests; existing test bodies/assertions untouched.
- Dart: seven prior cases explicitly start from existing empty Projects state, preserving every unrelated durable field and asserting exact post-restart equality. A distinct eighth case exercises truly absent-state startup, exact nested starter values, edits/removal/deletion/restarts and exact operation/effect counters. No old behavioral assertion is silently removed. Two schema expectations become7, and the disk5 projection expects exactly the existing disk7 config revision increment rather than dropping config comparison.
- `debug_service.rs`: changes confined to `dart_cloud` fixture compatibility/control. Synthetic disk5 drops the disk7 instruction sidecar only after asserting its unused collections; migration asserts exact expected config increment and restored instruction sidecar, preserving other fields. No production migration change.
- Production Flutter, `app_v2`, provider/instruction/rendering files and filesystem primitive do **not** differ from this baseline. Global Context/disk7/API/instruction/UI behavior is concurrent/external work already present in the baseline, not this starter feature. No historical Git blame conclusion follows from that observation.

## Implemented invariants / adversarial result

1. Safe read `None` is the sole production bootstrap callsite. Existing current empty/populated/full/exhausted and supported legacy states do not enter it. Corrupt/empty/unsupported bytes, wrong modes, symlinks, hardlinks and directories are errors, not fresh profiles.
2. Factory constructs exact Smith metadata, normal generated opaque project/request IDs, ordinary monotonic project-ID suffix, application/registry revision1, one internal content-free receipt/fingerprint and no tombstones. Full durable validation, preflight and encoded-state bound precede publication.
3. Publication uses existing `filesystem::publish(path,bytes,0600,false)`: same-directory secure staging and no-clobber hard-link, not replacement-capable `persist`. An appeared invalid, valid or symlink destination is preserved and the helper errors; no adoption/retry loop. An existing destination is never deleted by the factory. Readiness follows successful publication.
4. Bootstrap does not bypass ordinary capacity, receipt eviction/conflict, ownership or delete-fingerprint scrubbing. The internal receipt is evictable/queryable history, not a durable reseed marker. Edits/clears/deletion survive repeated reopen; even eviction followed by last-project deletion stays empty. Only whole-state-file removal makes another new profile and obtains a different project ID.
5. Additional private-copy fault hook returns an error **after successful link and staging unlink, before directory sync**. The helper returns failure while valid published bytes remain; later observation does not reset/reseed them, and another helper call fails no-clobber. This is a synthetic uncertain-outcome test, not an actual ENOSPC/fsync/hardware-failure experiment. Production code has no recovery retry/adoption/rollback block after `publish?`.
6. No new provider/network/Git/repository/tool/agent/context behavior appears in the inspected delta. Existing Rust fixture provider counters remain zero; compiled Dart case additionally checks fake key/provider/inference counters and an exact operation allowlist. No claim is made that concurrent Global Context itself lacks capabilities. Ordinary UI selection/forms remain unchanged because no production Flutter input changed.

## Required Checks Evidence

Expected outcomes are successful completion, preserved state and exact assertions unless stated otherwise. Full sanitized argv/environment/time/exit/log SHA/core identity are in each linked JSON; commands below omit only the recorded sandbox prefix. Local commands are finite (240s, toolchain20s); no timeout occurred.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Baseline archive/manifest and final executable core reconstruction | exact identity | PASS; parent records exactly match; no archive mismatch | [binding](review/binding.json), [delta](review/baseline-delta.json), [final](review/core-final.json) |
| `cargo test --locked --offline --lib projects_bootstrap -- --test-threads=1` on unmodified copied source |7 pass | **7 pass /0 fail /0 ignored**, exit0 | [record](review/bootstrap-final-sandbox.json), [log](review/bootstrap-final-sandbox.log) |
| `cargo test --locked --offline --lib projects_ -- --test-threads=1` on unmodified copied source | all pass | **24 pass /5 fail**, exit101; blocks combined gate | [record](review/projects-final-sandbox.json), [log](review/projects-final-sandbox.log) |
| Five private adversarial tests: receipt conflict/eviction/delete, whole-file removal, dangling/appeared symlink, valid destination refusal, post-link uncertainty | all pass | **5 pass /0 fail /0 ignored**, exit0 | [record](review/adversarial-projects-exact-scope.json), [log](review/adversarial-projects-exact-scope.log), [test source](review/adversarial_tests.rs), [instrumentation binding](review/probe-binding.json) |
| Parent negative-control overlay and independent normalized failure comparison | exact three pre-task overlays; other service files current | PASS evidence consistency; all five failure bodies identical to independent candidate run and valid isolated control | [audit](review/parent-audit.json), [parent control](validation/negative-control-isolated-projects.log) |
| Parent real rebuilt Rust↔Dart Projects |8 pass with no fixture skip | **8 pass**, exit0; evidence audited, not independently rerun | [record](validation/flutter-compiled-projects.json), [log](validation/flutter-compiled-projects.log), [fixture binding](validation/fixture-binding.json) |
| Parent two compiled Rust fixture controls |2 pass | **1+1 pass**, exit0; audited | [command ledger](validation/COMMANDS.md#fixture-control-disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects) |
| Parent Rust production Clippy / full Rust format | warnings-denied clean / unchanged format | PASS, exit0; audited | [Clippy](validation/rust-production-clippy.json), [format](validation/rust-format.json) |
| Parent all-target warning-denied Clippy | clean | **FAIL**, exit101, instruction-test lint; also reproduced in isolated pre-task control | [log](validation/rust-all-target-clippy.log), [control](validation/negative-control-isolated-all-target-clippy.log) |
| Parent focused Flutter Projects regression / assets | all pass | **168 pass /1 fail**, exit1; assets5 pass separately | [regression](validation/flutter-focused-projects.log), [assets](validation/flutter-assets.log) |
| Parent scoped/full Flutter analyzer + Dart format | clean | PASS, exits0; no analyzer issues;100 Dart files/0 format changes | [ledger](validation/COMMANDS.md) |
| Parent configured root `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build` | tests and build complete | **FAIL exit2**, service Clippy101; parallel work canceled, build not reached. Not rerun by assurance | [root record](validation/root-gates.json), [raw log](validation/root-gates.log) |
| Rebuilt-package verify and actual-package helper tests | current built package passes | **NOT RUN**, no successful root rebuild/current package | [verify](validation/package-verify.json), [helpers](validation/pipeline-helpers.json) |

### Retained exploratory failures (not passes)

All attempts remain in `review/` with command records and full sanitized logs:

- `bootstrap-original` / `projects-original`: fixtures under host `target/` fail safe ancestor permissions;0/7 and3/29 pass respectively. The group-writable ancestors are outside this review's write authority; no chmod or validator weakening was used.
- `bootstrap-sandbox` / `projects-sandbox`: initial read-only-root namespace omitted usable `/dev/urandom`; setup fails. `*-sandbox-devices` supplies devices but host `/` maps to unmapped uid65534 and correctly fails ownership checks. `*-private-root` uses namespace-owned root but initially lacks `/proc/self/exe`, so rustup exits1.
- Final sandbox has private namespace-owned tmpfs `/`, `/proc`, devices, read-only toolchain/source parents, writable assurance tree only, network namespace isolation, and assurance-owned host fixture directory bound at `/tmp`. This fixes test-environment construction without changing production validation.
- Initial adversarial filter `assurance_` unintentionally also selects seven pre-existing `app_v2` tests: **10 pass /2 fail**, exit101. All five new Projects probes pass there. The two extra failures are `assurance_exact_byte_reservation_must_allow_dispatch` and `assurance_legacy_maximum_escaped_results_migrate`; retained [log](review/adversarial.log). These are additional observed combined-source failures, not starter-probe failures, and were neither repaired nor suppressed. Subsequent explicit `app::projects::tests::assurance_` runs exactly the intended five. No pre-task control for these two extra failures was run; causal blame is unverified. `app_v2` inputs themselves are unchanged by this delta.

## Findings by severity

### Critical

None found in the bounded feature delta. This is not proof of absence across the product.

### High — blocking combined acceptance

**H1: required service gates fail.** Independent unmodified copied-source Projects run reproduces five failures (same complete normalized assertion bodies as candidate and valid negative control):

- `projects_actual_disk5_migration_preserves_opaque_state_and_legacy_requests_never_replay`
- `projects_crud_receipts_and_empty_registry_survive_restart_without_provider_calls`
- `projects_disk1_through_frozen_disk4_migrate_empty_preserving_all_existing_data`
- `projects_disk6_and_frozen_legacy_schemas_reject_corruption_without_rewriting_bytes`
- `projects_global_revision_is_independent_of_chat_mutations_and_pending_work`

All-target Clippy fails at `app_instructions_tests.rs:21` (`field_reassign_with_default`), also without the three starter service changes in the valid separate-target control. The root command stops on this lint. These are not starter-owned changes, but required gates remain failed. Additional two pre-existing-test-name runtime failures were observed in the exploratory broad filter above; no causal attribution or waiver.

**H2: required Flutter regression fails.** Parent navigation test at `projects_navigation_test.dart:236` expects no `TextField` but finds `global-instructions`, `global-personality`, `global-guardrails`. Production Flutter is unchanged relative to the task baseline. The failure is real current combined behavior; historical blame has not been demonstrated. Do not label the169-case group PASS.

**H3: no current successful root build/package evidence.** Root command fails; canceled branches are not passes. Current-package verification/helpers were NOT RUN. No old export/package is promoted by this review.

### Medium

**M1: combined canonical schema descriptions lag concurrent disk7 implementation.** Architecture/Host detail text still describes current disk6 while the actual baseline/current service and fixture implement disk7. This discrepancy predates the starter delta; the feature adds no schema change. Coordinate reconciliation with the Global Context owner before combined acceptance; this review does not edit externally owned schema policy or call historical disk6 descriptions a current guarantee.

### Low

None requiring starter-source remediation. Reviewer sandbox setup failures are documented harness limits, not product regressions.

## Evidence Gaps / limits

- Full component/root build and current-package gates remain failing/missing as above; successful source-scoped tests cannot replace them. Parent's valid negative control establishes only the bounded three-file comparison, not a pristine historical checkout or general historical blame.
- Dart/compiler fixture results were independently inspected/rehash-bound, not independently re-executed in this review. Rust original/probe results were freshly executed with a separately populated assurance target and cached offline dependencies; no clean-cache/hermetic claim.
- Real directory-sync/media failure, hostile-same-user filesystem races, installed application/native graphical behavior, deployed migration/restore, real provider/keyring/model and clean-source package/release provenance were not tested or authorized. Fault hook models one post-publication error boundary only. Existing same-user trust limitations are unchanged.
- No test here proves future contribution workflows, repository availability or permission, Git access, OpenCode readiness, or runtime-agent/context capabilities. Those are not implemented by this feature.

## Required remediation / closeout

Relevant external/concurrent owners must resolve the failing service/Flutter/component gates and reconcile current schema documentation, then bind a stable combined candidate and rerun required tests/build/package verification/helpers with fresh assurance where source changes warrant it. Preserve existing failed/skipped evidence. No unrelated fixes are requested of the starter owner by this report; no gate is waived.

**Verdict: FAIL (overall); bounded feature-source PASS only.**

**Release Gate Decision: NO-GO.** No integration/ship acceptance, accepted-risk decision, installation, launch, remote mutation or deployment is authorized.
