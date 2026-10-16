# Provider schema correction — independent assurance/adversarial review

## Validation scope

- Fresh independent review of `LOCAL-SMITH-NEXT-PROVIDER-SCHEMA-FIX`, repository `smith`, `feature/autonomy`, HEAD/base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` plus preserved preexisting dirty/untracked work. No builder reasoning transcript used.
- Read the required policy in order, `roles/assurance.md` (including its adversarial method; no separate adversary role exists), the plan, exact source/test delta and retained evidence. Review target is implemented source behavior, not an installed repair or a blanket assertion of target-policy compliance.
- Compared actual `cloud_keyring.before.rs` and `cloud_keyring_tests.before.rs` under `next/service/build/provider-schema-fix/`, **not HEAD**, because both cloud source files preexist as untracked work. Only this report has been created/updated; its parent and absence were verified before initial creation.
- No host bus/keyring, manager, real credential, vendor API, container operation, installation, commit, publication, cleanup or risk acceptance. The initial review reran only the existing private-socket test executable; the aggregate-failure follow-up uses read-only evidence/source inspection, with no new tests, builds or artifact execution.

## Findings, severity ranked

**Critical / high / medium: none identified in the bounded production/test delta.** The two actual aggregate test failures described below are nevertheless blocking broad-gate evidence; this scoped finding classification does not waive them.

### Low — L1: premature final-evidence pointer

`docs/SMITH_LINUX_ARCHITECTURE_REPORT.md:26` says schema-fix final package/gate and fresh-review outcomes “are recorded in `next/VALIDATION.md`.” At inspection the ledger begins with the earlier MIME correction and its different source inventory (`ad0cdd12…`); it has no schema-fix outcome section. The current plan correctly says final combined evidence is pending. This is an interim evidence-description mismatch, not a demonstrated security defect or an explicit false aggregate PASS.

**Reproduce:** read that sentence, then `next/VALIDATION.md:1–23` and the schema plan's Outcome. **Required disposition:** the parent must label the schema outcome pending or add the actual final source/artifact-bound results and this review at closure. Do not inherit the MIME gate's PASS. No other document was edited by this reviewer.

## Exact delta and adversarial assessment

**Implemented:** the only production hunk is `cloud_keyring.rs:212–218`: absent `xdg:schema` requires four attributes; the exact value `org.freedesktop.Secret.Generic` requires five; every other value conflicts. The unchanged comparison still requires all four exact application/provider/generation/save-request values. Given those four required keys and exact cardinality, no unknown extra can survive. There is no trimming, case folding, prefix/schema-family allowance or identity substitution.

- Outgoing `attrs()` remains exactly four fields (`:36–43`), used by `SearchItems` (`:193–198`) and `CreateItem` (`:306–339`). Schema is returned metadata, not a new lookup identity.
- The same predicate runs before `GetSecret` and again after decryption (`:244–267`); final unique search remains (`:269–272`). `Created == Modified`, before/after timestamp equality, lock/ambiguity checks, byte-exact readback and created-path equality remain unchanged. This does not promise rejection of a schema-only transition between the two individually permitted shapes when timestamps remain equal.
- Fixed login-bus path, socket/peer/owner UID checks, EXTERNAL authentication, unique-owner pinning, encrypted DH/AES session, exact two MIME forms, ciphertext/session bounds, printable-ASCII secret validation, NoAutoStart, original deadlines/cancellation and three-second budget are byte-unchanged. No activation/unlock/Prompt, plaintext fallback, replacement, adoption, deletion, retry or vendor authority was added.
- Publication is still marked attempted before the single `CreateItem`; the public save wrapper maps any later failure to `CREDENTIAL_OUTCOME_UNKNOWN` (`:332–340,379–397`). New tests call inner `Session::save`, so their raw `KEYRING_CONFLICT` assertions do **not** weaken or test away public uncertainty.
- At initial review, independently compared the preceding MIME source manifest with the reconstructed 1,267-entry component/build-input inventory: **only these two keyring files differed**. In particular `app_v3_worker.rs` was unchanged: one volatile secret is consumed, save must succeed before credential activation, and durable commit remains separate (`:249–276,375–421`). `tests/cloud.rs:997–1026` defines the unchanged failed-activation/old-generation/orphan test. Subsequent aggregate results and concurrent client-input drift are recorded below; the original inventory is not a current whole-worktree acceptance claim.

### Test quality and limits

All 15 prior test functions remain; three were added, with no disabled tests or relaxed assertions. Wire assertions are stronger: the fixture now constructs expected identity fields independently of production `attrs()` and checks full SearchItems/CreateItem maps plus label. The peer still independently derives the encryption key, decrypts the submitted canary and generates fresh response ciphertext.

- Positive matrix: two vendors × two accepted MIME forms × four/five returned attributes = **eight cases**, each save plus explicit read, two `GetSecret` calls and exactly one create.
- Negative maps: **32 cases** (five invalid schema values; for each schema-presence state one extra plus missing/substituted/changed versions of each identity field; schema-only map). Each conflicts on save/read with **zero secret retrievals**, publication attempted, item retained and exactly one create.
- Post-read mutations: **six cases** change schema, each required identity field, or add an unknown field during `GetSecret`; each conflicts after exactly one retrieval/create. Otherwise-valid ciphertext/metadata isolates the post-read predicate.
- These are not exhaustive D-Bus-map fuzzing. Post-read deletion variants and public-wrapper uncertainty are not newly exercised by these three tests; the same inspected predicate and unchanged wrapper support those conclusions. Existing MIME/negotiation/owner/UID/lock/prompt/deadline/no-replacement coverage was retained and passed. No blocking test weakening or coverage defect was found for this exact correction.

## Required Checks Evidence

Commands are from `next/` unless noted. Retained command records/log hashes and before/after inventory digests were independently recomputed, not accepted from summaries alone. Evidence root abbreviation **E** = `next/service/build/provider-schema-fix/`.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Exact prechange comparison: `git --no-pager diff --no-index -- <before-copy> <current>` for each file, repository root | Only optional-schema predicate and additive/stronger fixture coverage | Verified; diff exit 1 denotes differences, not a failed product test | Actual copies, current files, independent hashes below |
| Prechange `cargo test --manifest-path service/Cargo.toml --lib --locked --offline gnome_schema_metadata_round_trip -- --nocapture` | Expected red specifically after one create | Exit **101**, `KEYRING_CONFLICT; provider=openai; schema=true; creates=1` | E `schema-red.log`, `schema-red-status.json`, before/after inventories |
| `cargo test --manifest-path service/Cargo.toml --lib --locked --offline cloud::keyring::tests -- --nocapture` | All focused fixtures pass | Retained exit **0**, **18 passed / 0 failed / 0 ignored** | E `focused.log`, status and inventories |
| Reviewer execution, repository root: `next/service/target/debug/deps/smith_service-7c11b7ac37ef8548 cloud::keyring::tests --test-threads=1` (15-second tool bound) | Private socket-pair fixtures only, all pass | Exit **0**, **18 passed / 0 failed / 0 ignored**, 90 filtered, 1.85 seconds | Reviewer terminal; executable hash unchanged before/after, below. Cached executable rerun, not an independent fresh compile |
| `cargo fmt --manifest-path service/Cargo.toml --all --check` | No formatting errors | Retained exit **0**, empty log | E `format-status.json`, `format.log`, inventories |
| `cargo clippy --manifest-path service/Cargo.toml --all-targets --locked --offline -- -D warnings` | No warnings/errors | Retained exit **0** | E `clippy-status.json`, `clippy.log`, inventories |
| Source/log binding | Stable checked inputs and correct expected-red baseline | Reconstructed **1,267 entries**, membership/bytes/modes/hashes all match focused/fmt/Clippy inventories. Red production equals actual prechange copy; red tests equal current tests; red→focused changes only production | E inventories/statuses; read-only independent Python reconstruction |
| Isolated real GNOME final fixture: root command `python3 next/service/build/provider-schema-fix/gnome-integration/run.py` | Both schema reads and both fresh normal Save/reads succeed with exact canary/metadata | Retained **PASS**, schema/normal probe exits **0**, harness `overall_result=PASS`; not rerun by reviewer | E `gnome-integration/final/{integration.log,summary.json,commands.json,isolation.json,compiled-source.json}` |
| New `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build`, explicit package-test evidence and `python3 dagger/package.py --root installer/dist --verify` | Successful stable aggregate and bound rebuilt pair/package | Initially pending; now **two aggregate FAILs**, canceled GTK and build not reached. Separate source/package validation remains **PENDING** | Actual aggregate follow-up below; prior MIME/cloud gates are historical |

The red run is an expected regression demonstration, not a failing current required test. Hash consistency supports the source binding but does not independently authenticate execution chronology or turn owner-writable logs into signed provenance.

## Real GNOME isolation and binding

Inspected the Dockerfile, restrictive build context, harness, probe, runner, exact compiler/container commands, runtime inspection, cleanup and raw baseline/final logs. The adapter was not patched for this fixture. The harness alone prepares/unlocks its disposable collection; the production probe uses the fixed bus path and unchanged three-second budget.

- Final retained runtime: Ubuntu 24.04, GNOME Keyring **46.1-2ubuntu0.2**, D-Bus **1.14.10-4ubuntu4.1**, UID1000, network **none**, mounts **[]**, privileged **false**, capabilities **ALL dropped**, no-new-privileges, bounded CPU/memory/PIDs. Package acquisition belongs to dependency-image build, not runtime isolation. This is not host-session evidence.
- Rehashed all **27** copied service/Cargo entries: exact match to compiled manifest, recorded final before/after snapshots and current source. Rehashed the probe, service rlib, harness, runner, Dockerfile and **17** directly recorded dependency rlibs: all match. Direct `rustc` linkage to cached dependencies is not a clean/locked release build or complete transitive dependency attestation; aggregate gates remain necessary.
- Two encrypted bootstrap items carry exactly five fields and are read successfully, one per provider. Two fresh **normal** adapter saves/readbacks also succeed once each. Final collection has four unique unlocked identities and equal Created/Modified throughout observations. **Normal saved items had four attributes in this final run**; the real-daemon five-attribute path exercised reads of preseeded items, not five-attribute immediate Save readback. Private fixtures cover the latter composition. No stronger timing/version claim is justified.
- Baseline schema reads both conflict; normal Anthropic save is `CREDENTIAL_OUTCOME_UNKNOWN` and read conflicts. Baseline status remains **`UNEXPECTED_BASELINE_RESULT`**, not green. Recomputed preservation manifests match all **47** actual baseline files; the **19** earlier MIME evidence entries also remain byte-identical. No timestamp guard was weakened or failing run erased.
- Root `gnome-integration/summary.json` and `RESULTS.md` describe earlier blocked/baseline attempts, not final acceptance. The explicitly inspected `final/` evidence supplies the final result. Cleanup command/log supports removal of the owned container; this reviewer did not query the container engine. The dependency image remains cached per historical fixture notes.

## Source and evidence identities

SHA-256, independently computed. These define the uncommitted review target; HEAD alone does not.

| Input | Actual prechange copy | Reviewed current |
|---|---|---|
| `next/service/src/cloud_keyring.rs` | `93b2be987cdd6883d8d500269c7ec8be5c5576d3a85375b9e184d938ffdd3279` | `f6ac5c6ea536992ad7bf252d92ef06b4ab8085b6393dc2e1592c53f5628c20aa` |
| `next/service/src/cloud_keyring_tests.rs` | `e35cd783bbe4bae05f95b18dbb13572f89aae2be0fb5bada1dbc20fce5f036dd` | `b3c5816aa52f4b73e1ce2c55bd50bd8c7196f6ed9a136a6a96708f3c7981ec83` |

- Initial reviewed component/build-input inventory: `e38e6d85cee23d123b27a163d22c5fadd3ee7b27905d1a1f8741cf3707d3a513`. Expected-red inventory: `99b691d07db1711e59b6489ab75090ab82062de11be794a190452531922aa652`. Later whole-input snapshots differ as recorded below; both reviewed keyring hashes remain unchanged.
- Plan: `fe99c1d94a2d324a437e2d229f45fd0ba2fc7090cee72496e834ec50497f94ab`.
- Executed private-test binary: `cbe659a57b3fb17712c04153726502f2f4ef0f32a64210be68f43cba61959dd5`.
- E `schema-red.log`: `828ad62e72ff39a04fe71745253938b67922a98b603ab1018b4bba53c2ee2956`; `focused.log`: `cb79b4b433db4cbe076ba3cb290e76bab33fd774d4754ea6fc9539f6ae364f9f`.
- E `clippy.log`: `397f9a73e59a7d5af66bab21853cca873b127a1818033e687f41840b8734ff97`; empty `format.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
- Final GNOME `integration.log`: `e632bf5f530433b2594de3220dc798e918b999579ec9fb50f880580ed2510f0f`; `summary.json`: `56d18778a48d0cbfb622a5bbad921a933725f0c3ea0a1f0a6ed3092be4fc5515`.
- Final GNOME `compiled-source.json`: `a958ea97c928cc1a3e91b5cfa3ac8dd34262237d6afa0bea9073a906cebc10a1`; probe: `3df6c5d8af2898c751f57e4c174f19f8ebbe660f180550ab539f5d3c08e52bcf`; rlib: `d3638e8c260fca41fdb4b388c4214e1f0c7d88dee83465200b9be10eb2279c01`.
- Final GNOME `commands.json`: `902aea176d514952649930132ce46d289326776bc8ba52f7130ab00c9a7737a9`; `isolation.json`: `b120b41847bc0347c188559e52cf9f17d9933777dc803ae37207e480414c331e`.

## Aggregate failure follow-up — two failures, no new reviewer execution

Independently read both aggregate failures, isolated/full-runner results, statuses and source inventories. Recomputed log/status hashes and compact inventory digests; all four executed commands had equal before/after entries. Counts below were cross-checked against primary raw suite/completion records, excluding repeated Dagger failure-diagnostic output. These are retained validator executions, not new reviewer runs.

Both aggregate commands were `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` from `next/`, with a retained 1740-second runner budget. Neither timed out or was killed.

| Executed check | Expected | Actual / exit and timestamp UTC, 2026-10-16 | Retained evidence under E |
|---|---|---|---|
| First aggregate, original reviewed 1,267-input snapshot | All tests, GTK and paired build complete successfully | **FAIL, exit 2**, 02:51:27–02:53:32. `application_teardown_cancels_active_native_inference_and_joins_cleanup` panics at `next/service/tests/runner.rs:2787:84`, launching a new holder after teardown: `RUNNER_BUSY`. Runner suite **40 passed / 1 failed** | `combined-status.json`, `combined.log:10601–10620`, `failure-lines.log` |
| Isolated teardown: `cargo --locked --offline test --manifest-path service/Cargo.toml --test runner application_teardown_cancels_active_native_inference_and_joins_cleanup -- --exact --nocapture` | One test passes without source/test changes | **PASS, exit 0**, **1 passed**, 03:07:31–03:07:35 | `continuation-run/isolated-runner-{status,before,after}.json`, `isolated-runner.log` |
| Full runner: `cargo --locked --offline test --manifest-path service/Cargo.toml --test runner -- --nocapture` | All runner tests pass unchanged | **PASS, exit 0**, **41 passed / 0 failed / 0 ignored**, 03:07:52–03:08:15 | `continuation-run/runner-suite-{status,before,after}.json`, `runner-suite.log` |
| Second aggregate, continuation 1,272-input snapshot | All tests, GTK and paired build complete successfully | **FAIL, exit 2**, 03:08:27–03:10:31. Different test, `vulkan_startup_observes_full_window_and_accepts_only_complete_fragmented_evidence`, panics at `next/service/tests/runner.rs:636:13`: `gpu-window-full: a candidate must remain provisional for the full post-health window`. Runner suite **40 passed / 1 failed** | `continuation-run/combined-status.json`, `combined.log:11103–11113`, `failure-lines.log` |

**Actual partial outcomes:** each aggregate has service **252 passed / 1 failed / 3 controlled helper ignores**, including **18 passing keyring tests**; installer **279 passed / 0 failed / 2 controlled helper ignores**; package tests **41 passed**. Flutter completed **456** in the first and **470** in the continuation, with no failed/skipped tests. **GTK was canceled in both**, not passed. `make test` failed before the requested build target; exported-package verification was not run by either aggregate. The 41 synthetic package tests are not verification of a freshly exported recipient package. Evidence: E `final.json`, `continuation-run/final.json` and their hash-matched raw logs.

**Source binding and scope separation:**

- The first executed aggregate is bound to the original `e38e6d85…` 1,267-entry inventory above. The isolated test, full runner suite and second aggregate all used the same stable **1,272-entry** inventory, SHA-256 `7c718307e6a6f825b238ef539c00b9d0415b4e8338cd38506c38d287e2011bfc`.
- Both keyring hashes still equal this review's source identities in both command snapshots and the current files. `next/service/src/runner.rs` is SHA-256 `34ccf8da4da7edc262f8df8d4673b815b74de6ea3cd7d2ed659346d9aded05c7`; `next/service/tests/runner.rs` is `b0661d713bd63c5bd5ae3eaa80db9e347ae55e9c9d1251b6e41a07026d07c931`. Both equal the prior MIME manifest, both aggregate snapshots and current bytes: no runner production/test relaxation is present.
- The first-to-second input delta is confined to **10 client/Boot code/test/golden paths**, not schema or runner files. Initial three-file Boot drift and later additions/edits were preserved. The continuation report then captures three further post-command changes (`boot.dart`, `boot_test.dart`, `goldens/boot/outro.png`), giving report inventory `8f5cc5e4f6bbb01872054b7e0c6e31e3860a839a1733d1a055ab5b7c1b7c94c3`, **not** the tested snapshot. See both `postrun-drift.json` records and `continuation-run/source-manifest.json`. No semantic review or acceptance of those independent client changes is implied.
- The teardown failure concerns reacquiring the native-runner holder after cleanup. The second failure concerns the unchanged helper's measured post-health interval of at least 900 ms (`tests/runner.rs:636–638`). Neither failing assertion invokes the changed keyring predicate. This establishes distinct failing paths and unchanged bytes, **not** their root causes, harmlessness, or absence of all indirect timing/resource interaction. An isolated/full-suite pass does not repair, supersede or explain either aggregate failure. No flakiness, scheduler-only cause or accepted-risk conclusion is established.

Additional SHA-256 evidence anchors:

- First aggregate `combined.log`: `bf979ae6070728afb0a21b7c7ae262062564fae4d101e9eb5fcfb2f8a8a72996`; `combined-status.json`: `eb65e8ea4d1ac5f284d6b5c96835c6d6f31880af6c943d006654ea0efa457bdc`.
- Continuation `combined.log`: `46f2fd928ac4ec2ca7dd0026bc0643e37cf5950c45c6e4100edcffca1b5a1631`; `combined-status.json`: `f569629e3f71643ca5b7289a193ec1469d40b2b63c106bb70c806f68c9aa95b6`.
- Continuation `isolated-runner.log`: `3653becf65fbfa33cf247150fc226af383b42bc7dd1f58a42ffba7333763d9fb`; `runner-suite.log`: `b0f4df7b9672c68fa3e609994f5e8d17e1f1449aedf66dd3e5a8fd6bea792d2f`.

## Evidence Gaps and decision

**Source-limited verdict: no scoped blocker identified in the exact schema-source correction.** The implemented predicate meets SCHEMA-R1–R4 and preserves the inspected boundaries; its source/test identities remain unchanged. No schema-source remediation is requested on present evidence. This is not an overall `pass`/`pass-with-risk`: SCHEMA-R5/final acceptance is incomplete and the broad aggregate gate has actually failed twice.

**Pending separate source/package deliverable closure:** the validator's separately planned single `service/test`, `installer/test`, `make build` and package verification under E `package-validation/` require their actual results, including GTK disposition, exact commands/statuses/timestamps/log hashes, stable source inventories, paired service equality and package/binary hashes. No result is awaited or presumed here. Such evidence may complete the requested bounded source/package deliverable; even if all pass, it does **not** erase the two aggregate failures or establish global acceptance. New concurrent client inputs require their own binding/review, not inheritance of the first aggregate's snapshot.

Resolve L1 and retain sanitized actual results plus a schema-specific ledger entry without rewriting prior failures. The sanitized evidence directory was empty at initial inspection; final population remains parent-owned. No further aggregate retries, unrelated runner fixes, test changes, risk acceptance or deployment are requested or authorized by this report. Disposition of the broad failures remains separate from this source/package work.

Installed-host behavior, real-key/API access, entitlement/charges, migration/restore and deployment are neither verified nor authorized here. The known separate timestamp-timing concern remains unchanged and is not accepted as residual risk by this review. Metadata-only host observations in the plan were not repeated and do not establish secret contents or active-generation validity.

- **Formal assurance verdict: FAIL — two required aggregate test failures, plus pending separate package evidence.** No blocker has been attributed to the exact schema delta, but that does not exempt required failed gates.
- **Release Gate Decision: NO-GO.** Required follow-through is truthful evidence/documentation closure, renewed review for changed inputs, and separate disposition of the unresolved broad-gate failures before any broader readiness claim. This is not a request for more retries or out-of-scope fixes. This review grants no human approval, release/deployment authority or accepted-risk decision.

## Final component/package evidence closure — 2026-10-16 UTC

### Disposition and method

**The separate component/build/package results are independently verified PASS; no concrete blocker is identified in the exact schema-source correction. Overall assurance remains FAIL / Release Gate NO-GO.** This append-only closure supersedes the earlier pending-package and open-L1 statements, not either aggregate failure, the source-scope limits, or the absence of broader acceptance.

Only this review was appended. The reviewer inspected retained files, recomputed source/artifact/log hashes and inventories, and checked manifest/checksum contents. No new build, test, package-verifier execution, artifact execution, container/host service call, keyring/API access or installed mutation occurred. HEAD/branch still match the recorded base and `feature/autonomy`.

Durable evidence root **D** = `docs/evidence/provider-schema-fix/`; raw sequential evidence root **P** = `next/service/build/provider-schema-fix/package-validation/`. These are validator executions independently checked here, not reviewer-executed gates.

### Required Checks Evidence — actual separate sequential results

All commands ran from `next/`, prefixed with `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain`. Each completed once, exit **0**, without timeout/killing; all used the same stable source inventory. Times below are retained UTC on 2026-10-16.

| Check / command | Expected | Independently verified actual | Raw evidence under P |
|---|---|---|---|
| `make service/test` | Formatting, warning-denied Clippy and locked service tests pass | **253 passed / 0 failed / 3 controlled helper ignores**, including **18 keyring tests**; 03:16:43–03:18:06 | `service-test-status.json`, `service-test.log` |
| `make installer/test` | Rust quality/tests, isolated GTK and interpreter-free CLI smoke pass | **279 Rust + 9 GTK passed / 0 failed**, two controlled Rust helper ignores; configured no-Python Ubuntu help/version/artifact dry-run completed; 03:18:14–03:20:11 | `installer-test-status.json`, `installer-test.log` |
| `make build` | Standalone service, complete client and recipient package export successfully | **PASS**, all three exports completed, including build-time 95-file package verification; 03:20:34–03:24:10 | `build-status.json`, `build.log` |
| `python3 dagger/package.py --root installer/dist --verify` | Verify the actual newly exported package | **PASS**, explicitly verified **95 manifest-listed files**, distinct from **97 total recipient files**; 03:24:18–03:24:19 | `package-verify-status.json`, `package-verify.log` |
| `make package/test` | Native package-entrypoint/builder tests pass | **41 passed**, using configured compiled installer ELFs; 03:24:38–03:27:21 | `package-test-status.json`, `package-test.log` |

Recomputed each log hash and checked every original status field against D `final.json`. Independently checked all **21 primary Rust suite summaries**, the package unittest completion and no-Python completion against raw logs. The service/installer/GUI scripts still enforce formatting, warning-denied Clippy and locked tests. The **582 counted passing tests / five helper ignores** exclude earlier aggregates and Flutter; they are not a combined acceptance count. The controlled ignores remain fixture entrypoints, including the Dart cloud helper launched by its separate cross-language gate, not new product-test exclusions.

### Final source, evidence and artifact binding

- Reconstructed the current filesystem inventory using the retained traversal/exclusions, symlink treatment, byte counts and modes: **all 1,273 entries exactly match** D `source-manifest.json`. Compact sorted-JSON inventory SHA-256 is **`2f083f0e5556c35faf7b1f0696c55e27e86073e52790f5545709c071b20f85c0`**. All **ten** before/after manifests for the five commands have those same entries; their SHA-list files also match. No later source drift was found at closure. Root canonical docs, this review and `next/VALIDATION.md` are outside the compiled-input inventory; their outcome updates are not claimed as tested inputs.
- Reviewed keyring hashes are unchanged in current bytes and final inventories: `cloud_keyring.rs` **`f6ac5c6ea536992ad7bf252d92ef06b4ab8085b6393dc2e1592c53f5628c20aa`**; `cloud_keyring_tests.rs` **`b3c5816aa52f4b73e1ce2c55bd50bd8c7196f6ed9a136a6a96708f3c7981ec83`**. Against the preceding MIME inventory, these remain the **only service-input deltas**. Runner production/tests retain their previously recorded hashes; no assertion, timing threshold or runner code was changed for this correction.
- D `final.json`, `artifacts.json` and `source-manifest.json` are byte-identical to their raw P counterparts. Both durable aggregate reports equal the original failed reports; their raw aggregate logs retain the checked failure identities. Final/baseline GNOME logs, summaries and isolation copy exactly. The durable expected-red log is the exact raw-log derivative replacing only the repository absolute path with `<repository>`. Independently rehashed all **192 protected prior evidence files**: no byte/size/mode changes.
- Rehashed all **189 exported files**: standalone service **1**, standalone client **91**, recipient package **97**. Actual path sets, bytes, sizes and modes exactly match D `artifacts.json`. Combined export inventory SHA-256 is **`f5444865bc0bdd99c8b45f79e10d0336922e5443cdb1dafcb111181d0afcc1c2`**; recipient-only inventory is **`4530e813f628e08e57af87aa81c675b30877ee65410163b2cd091ed086e4bb3e`**. Actual files also match every retained post-build/pre-post-verification/pre-post-package-test export snapshot.
- All **four** service copies equal SHA-256 **`56e617c12fba1dba98b7d55c05b9c4efaaae0dbbda05432736cc50b51eeb9777`**: `next/service/dist/smith-service`, `next/client/dist/smith_backend/smith-service`, `next/installer/dist/payload/service/smith-service`, and `next/installer/dist/payload/client/smith_backend/smith-service`. All **91** standalone/packaged client relative paths, contents and modes agree.
- Independently compared all **95** package-manifest members to the actual recipient file set and checked their hashes/modes. The checksum file exactly matches its configured **96-record** format: 95 sorted payload records followed by the manifest record. Reconstructed the client release-manifest digest, including directory modes, and matched `227a780606296b53e71f9185f67c0fa5e30131c0b8ad9f34fdb1803ef53a54b4`. Manifest SHA-256 is **`397f3dd8eeebd4228e4cf6810d5cd1f0ac453e170de8d5bee0809d5118b38d7c`**; checksum-file SHA-256 is `89f6e061c5636d80fdc168a830c56b762fe063489eff6d1c7e47f4711ee151b8`. These are read-only integrity comparisons, not a rerun of the verifier or installer.
- Exported service/client/installer executable headers are native Linux x86_64 ELF with expected executable modes. The complete rebuilt candidate is **`next/installer/dist/`**, not the separately excluded developer helper under `next/installer/build/`; no rebuild/validation of that stale optional helper is inferred.
- The final GNOME fixture's **27** copied service/Cargo inputs still match current service inputs. Its baseline copy instead matches the actual prechange keyring copies, with the other service inputs equal. The real-daemon five-attribute reads, four-attribute normal saves, synthetic-only scope and retained `UNEXPECTED_BASELINE_RESULT` remain exactly the previously reviewed evidence, not newly run tests or installed-session proof.

Before/after manifests and owner-writable hashes establish retained consistency and review binding, not continuous monitoring, signed provenance, clean-clone reproducibility or hostile-same-user protection.

### Findings and documentation disposition

- **Critical / high / medium / low scoped source findings: none open.** No new source, log, source-inventory, artifact, copy or manifest binding mismatch was found in this closure.
- **L1 CLOSED:** `next/VALIDATION.md:3–13` now records schema-specific component/package outcomes and both failed aggregates; `docs/SMITH_LINUX_ARCHITECTURE_REPORT.md:26`, the plan's Outcome and D `README.md` agree. They explicitly distinguish component/package PASS from aggregate FAIL/NO-GO and accurately state the real-GNOME fixture's limits. The former premature ledger pointer is now backed by actual evidence.
- Concurrent Boot/client changes are retained and included in the final build inventory, **not independently reviewed/approved by this schema review**. There is still **no final-current-snapshot Flutter test/analyzer gate claimed here**. Earlier 456/470 Flutter passes bind only their earlier snapshots. Build success and matching package copies do not close that gap.
- The two aggregate failures remain unresolved actual failures. The later separate service253 pass neither demonstrates their root causes nor establishes harmlessness, remediation or accepted risk. This closure does not satisfy or waive the original combined-acceptance condition.

### Final evidence anchors and decision

Independently computed SHA-256 for D files:

- `final.json`: `2a4f805bf4783f25320e6cc7d8edfdcd21ffbdc2fa01434f77a27defaa920181`.
- `artifacts.json`: `1143578892ce0f9bc1f296c26328270af75db37e30aadf0080f6973382cab580`.
- `source-manifest.json`: `549070b6bda779f9dd15518162b605e5bfb6fdb383db9bf38e9a6222a3a11148`.
- `README.md`: `2864bb324ba37dbb7238fda42dc2c5c794b4c6559460afda9ff17cd65eed7c75`.

**Scoped conclusion:** no concrete schema-source blocker; the separate source/component/build/package evidence is now verified and no longer pending. This is a bounded evidence closure, not a product-wide assurance pass.

- **Formal assurance verdict: FAIL — unresolved required aggregate failures remain.** Missing final-current Flutter/independent Boot assurance also limits any broader claim. No schema-source remediation is requested on present evidence; broader failure/gap disposition requires its own bounded work and review, not retries or unrelated edits under this task.
- **Release Gate Decision: NO-GO.** No human approval, accepted-risk decision, real-key/API validation, installed upgrade/migration, publication or deployment authority is supplied. Source/package delivery is not installed repair or deployment readiness.
