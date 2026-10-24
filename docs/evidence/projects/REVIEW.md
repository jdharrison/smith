# Independent assurance — Projects metadata foundation

## Validation scope

- Review the approved **metadata-only** Projects list/summary-settings slice: stable project identity, name, optional repository-locator list and fixed project-owned OpenCode metadata. No project conversation, Dev/run, repository access, credential setup, clone, agent/container/tool/provider execution or permission expansion is approved.
- Review service registry/protocol/disk5 migration, replay/delete/publication recovery, existing chat projections/privacy, Dart protocol/controller/UI/navigation, presentation-preference compatibility and preservation of chat drafts/reading/queues.
- Independently falsify recovery invariants, re-review developer fixes and bind the final configured gates, current source, documentation and actual local artifacts. Parent tests are explicitly distinguished from reviewer-executed tests.
- Preserve unrelated runner and concurrent documentation work. This reviewer changed only `docs/evidence/projects/REVIEW.md` and reviewer-owned records/repros below this directory during finalization. No product/evidence-author file, Git branch/index/commit, installed state or external service was modified.

**Work:** `LOCAL-SMITH-PROJECTS`. **Review date:** 2026-10-17 UTC. This was an independent assurance context, not an implementation-owner approval. The reconciled plan and independent role inputs are retained in `docs/plans/LOCAL-SMITH-PROJECTS.md`; their existence is not a substitute for executable evidence.

## Candidate and evidence binding

| Identity | Bound value |
|---|---|
| Approved initial Projects base | `c1339f3670c70582db05a158b46821353662b87b` |
| Observed branch / final gate HEAD | `feature/autonomy` / `14d960975e76f906efb5e447ff3dece0e8e4385b` |
| Core inputs | **1,242**, including untracked Projects code/tests, not only tracked diff |
| Before/after/current canonical core SHA-256 | `5226a499e634e06d284f14b841c061d71c470cb903eedba95f228c40d9779d23` |
| Parent `results.json` SHA-256 | `c3c8c5b9719fb7e83d281429236f94958149fa2bd32124669f14385fec83d2b2` |
| Core JSONL file SHA-256 | `7fdd6bc07ad9b124486a145be6421d5b22c1d111d2ae37f3ac3178a20e4cdc48` |
| Parent artifact-record SHA-256 | `5feab40f1a28d507ad625bf1e57ceadb4ad986b461f7c333254f857a7c60043b` |
| Package manifest SHA-256 | `4c21ef724baba259e346962e64694be99c6e6edf02f7ad3c0a3a9c434e712d47` |

The final HEAD advance is the separately committed runner repair. Its source is included in the aggregate input identity but **excluded from Projects implementation/review ownership**. No runner correctness or publication approval is supplied here. Concurrent root/documentation cleanup is preserved and hash-bound for consistency, not claimed as Projects authorship or a comprehensive editorial review of unrelated historical/hosted claims.

The reviewer independently enumerated current core paths, modes, sizes and hashes and compared them with both raw gate manifests and the exported JSONL. All **59 parent raw records**, **27 supporting records**, and **189 service/client/package artifact records** matched their recorded identities. Supplementary canonical `SCOPE_DETAILS.md`, `ARCHITECTURE_DETAILS.md` and `HOST_PROFILE_DETAILS.md` were read and added to the reviewer snapshot, alongside the short entry documents. The supporting documentation is a post-aggregate snapshot, not a claim that every document was stable throughout testing.

`reviewer-records/review-inputs.jsonl` records **1,525 unique source, policy, evidence and artifact inputs**. `reviewer-records/review-bindings.json` seals the completed review hash, this input manifest, verification outputs and preserved repros after checking the inputs again. The review does not recursively hash itself. Parent-authored evidence was not rewritten to add the review.

### Key recovery source identities

| File | SHA-256 |
|---|---|
| `client/lib/projects_controller.dart` | `5bede0ef793034c75486e5721971657b1e2a3493168dcf615b7d54191ce9f8f2` |
| `client/lib/ui/projects_view.dart` | `db5b4a831a07ae7ce5be31e97a2f42ecb48b0fd271a376c7195e5f72faa634fd` |
| `client/test/projects_controller_test.dart` | `6aa30154ca1dcd4e94c7c7e82cf6ebaf33d4c40ca5c8a797550ae69179a9ff68` |
| `client/test/projects_view_test.dart` | `efe396b465360b8e9646362010a577db7f20226aa8998974c0731e5eb397f039` |

All 34 production library files matched the review mirror. The previously passing 82-case independent focused run remains applicable to those identical library/test bytes; the final five unchanged adversarial cases were also executed again after final bundle verification.

## Requirements and source assessment

**Implemented source, not installed guarantees:** the approved metadata slice is present and consistent with its bounded contract.

| Requirement | Reviewed implementation / evidence |
|---|---|
| P1 — durable bounded registry and safe writes | `service/src/app_projects.rs`, `app_v2.rs` and 17 Projects Rust tests cover CRUD/restart, independent registry revisions, instance/revision rejection, replay/conflict/expiry, 32 receipts/1,024 non-evictable tombstones, saturation, races, state/frame headroom and ownership/publication fencing. |
| P2 — inert repositories and per-project fixed agent | Strict Rust/Dart validation: 32 projects; 16 distinct credential-free restricted HTTPS/Git SSH locators each; 2,048 UTF-8 bytes/locator; trimmed nonempty 40-Unicode-scalar names; stable 32-lowercase-hex IDs; exactly `opencode`. Locators are metadata, not connection/access authorization. No execution/readiness control is added. |
| P3 — compatibility, privacy and migration | Frozen disk1–4 migrations initialize an empty registry and preserve existing histories/configuration/full-u64 values/cloud/provenance/timing/receipts. Corrupt/unsupported bytes are preserved. App3-only Projects operations use a separate receipt family, not chat work/provider context. Legacy reply shapes remain; the existing global application commit revision advances on a metadata commit, while registry revision is independent. Deletion scrubs old metadata-derived fingerprints and leaves only bounded content-free/opaque replay evidence. |
| P4 — recoverable UX | Explicit Save/Cancel, exact-target delete, dirty-input preservation, keyboard and compact/enlarged forms; captured instance plus revision fences; query-only uncertainty resolution/observation. Historical acknowledgements never substitute for current membership. Three independent blocking findings below were reproduced and fixed. |
| P5 — neighboring workflows and preferences | Lazy Projects loading, one desktop registry/API owner, retained per-view selection/editors, preserved chat drafts/reading/queues, shared panes and independent windows. Preferences strictly read v1/v2, write v2 and contain presentation only—not project records, selection or edits. Navigation/preference fixtures passed independently and in the final aggregate. |
| P6 — documentation and truthful evidence | Current policy/details, plan, report, validation summaries, evidence README and structured records distinguish implemented/source facts, deferred execution, uncertainty, installed migration and release limits. The final packaged guide equals current source. Failures, superseded snapshots and exporter limitations remain recorded. |

No source path added repository probing, cloning, credential/keyring access, provider/model dispatch, process/container/agent execution or automatic project-to-chat context transfer to Projects CRUD. Compiled fixture counters for synthetic local inference, HTTP and key retrieval stayed zero. These facts do not establish an OS egress sandbox or hostile-same-UID isolation.

## Findings and remediation history

### Critical / high

No unresolved finding in this bounded Projects review.

### Medium — all three blocking findings closed

**F1 — editor and delete-confirmation origin silently rebound after a same-revision restart. CLOSED.**

Initial behavior: refresh replaced the controller instance while a dirty editor/open confirmation tracked revision only. Independent tests observed an old form dispatching `project_update` and an old confirmation dispatching `project_delete` into the refreshed instance. This was client-origin rebinding, not the Rust service accepting a raw stale instance ID.

Fix: all controller mutation methods require `expectedInstanceId` and `expectedRevision`; `projects_controller.dart:135–157` rejects mismatched origins and sends captured values. `projects_view.dart:56–60` includes instance in stale detection. Save, review and delete closures retain instance/revision/epoch eligibility and revalidate after asynchronous dialogs. Dirty input is not silently rebased. Current controller tests cover create/update/delete, and widget cases cover dirty/new editors, clean/dirty confirmations and stale captured callbacks.

Original three-case fixture: `reviewer-repros/assurance_recovery_test.dart`, first two named tests. Initial failures are preserved in `reviewer-records/initial-recovery-failures.log` and the earlier control run in `initial-instance-controls.log`. Final result: both cases pass unchanged.

**F2 — unknown/expired receipts dead-ended authoritative list observation. CLOSED.**

Initial behavior: failed receipt lookup stopped Resolve before the list read, while uncertainty disabled Refresh. All views could remain on stale metadata with no working list-observation action.

Fix: `projects_controller.dart:259–285` observes the current list on definitive unknown/expired receipt errors while retaining the pending identity and uncertainty. `projects_view.dart:603–608` allows serialized read-only Refresh during uncertainty. A fresh list cannot clear the original outcome or enable writes. Controller/widget cases cover both errors, list outage, repeated/coalesced reads, disposal and preservation of edits/original IDs.

Original fixture: third named test in `assurance_recovery_test.dart`. It initially failed and now passes unchanged. This fixture accepts either successful Resolve list observation or an available Refresh action; the owner regressions additionally execute Refresh and verify query-only behavior and continued write blocking.

**F3 — confirmed deletion resolved after restart retained a dirty deleted selection. CLOSED.**

The first recovery fix returned on instance change before deletion reconciliation. The independent clean-form control passed but the dirty-form case failed even though receipt/list reconciliation had cleared uncertainty and proved target absence.

Fix: `projects_view.dart:281–294` handles the confirmed exact-target deletion plus authoritative absence before cross-instance dirty preservation. It clears selection, never chooses another project and does not replay. Added owner regressions distinguish absence from a historically acknowledged target still present in current state.

Unchanged fixture: `reviewer-repros/assurance_delete_restart_test.dart`. Before-fix evidence is preserved in `reviewer-records/dirty-delete-before-fix.log`; both clean and dirty cases now pass. The two original repro-file hashes remain respectively:

- `b395fb737e3452339f5f87dc625e293a50c1310cf46fc6c260df312145252579`
- `07aa62ba88bbc95197ebb905d07564b990e4de235b9ce6a1926e34e718fb6761`

### Low — nonblocking documentation wording

The parent evidence README's historical setup paragraph says “both recorded gate attempts” although the complete record now retains three aggregate attempts. The structured records and later paragraph correctly identify the final third attempt. This review explicitly verifies all three identities and uses only the third for final aggregate acceptance; no count, product behavior or authority conclusion depends on that stale wording. Parent evidence is preserved as requested.

## Required Checks Evidence

The parent ran the full configured gates; this reviewer inspected their source binding, scripts and raw logs rather than claiming to have independently rerun the aggregate.

**Final parent window:** 2026-10-17 **06:07:39.547286–06:13:16.217321 UTC**. All four recorder children exited 0 without timeout.

| Required check | Exact command / scenario | Expected | Actual / status | Evidence |
|---|---|---|---|---|
| Rust service | Root Make invokes `service/scripts/test.sh`: locked formatting/Clippy/tests | No errors/warnings/failures | **289 passed**, 3 controlled ignores | `target/projects-validation/root-gates.log`; output vertex 22 |
| Native installer | `installer/scripts/test.sh`: locked formatting/Clippy/tests | Pass independent workspace | **283 passed**, 2 controlled ignores | Same log; vertex 72 |
| GTK | `installer/scripts/test-ui.sh`: warning-denied GUI Clippy plus isolated Xvfb tests | All cases pass without normal desktop/manager | **9 passed**, no ignores/skips | Same log; vertex 63 |
| Flutter | `client/scripts/test.sh`: format/analyze/all tests | Clean formatting/analyzer, no skipped compiled fixtures | **773 passed**; 89 files formatted with 0 changes; no analyzer issues | Same log; vertex 94 |
| Actual native package | Native CLI/GUI inputs supplied to builder package tests | No placeholder ELF substitution/skips | **41 passed** | Same log; vertex 124 |
| Builds | `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build` | Service, Linux client and complete paired package build/export/verify | Exit **0**; final local outputs match artifact manifest | Parent `results.json`, raw root log, `artifacts.json` |
| Package verification | `python3 -B dagger/package.py --root installer/dist --verify` | Current package valid | Parent and reviewer exit **0**, **95 manifest-listed files** | Raw package log and `reviewer-records/package-verify.json/log` |
| Actual-package helpers | `SMITH_TEST_PACKAGE=installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v` (recorder resolves absolute package path) | Pass real-package archive/config helpers | **34 passed**, exit **0**, no skips | Raw `pipeline-helpers.log` |
| Compiled Rust↔Dart Projects | Cached SDK `test/rust_projects_integration_test.dart` with actual `SMITH_TEST_CLOUD_SERVICE` | Four real compiled round trips, no provider effects | **4 passed**, exit **0**; executable/input/log hashes verified | `compiled-projects.json/log`; parent bundle copies exact record |
| Independent focused regression | `python3 -B target/projects-assurance/recovery-rereview-2/run_checks.py` | Original repros and current owner suites pass | **3 + 77 + 2 = 82 passed**, exit **0**; 34 runtime files and tests stable | Raw record hash-bound by parent; preserved `reviewer-records/focused-results.json` and focused logs |
| Final independent recheck | `python3 -B docs/evidence/projects/reviewer-records/verify_bundle.py` | All bindings valid; both unchanged repro files pass; actual package verifies | Exit **0**; **5 repro cases passed** at 06:20:03–06:20:07 UTC; package and diff check exit **0** | `reviewer-records/verification.json`, command JSONs/logs and input manifest |

Aggregate: **289 + 283 + 9 + 773 + 41 = 1,395 passing tests**, plus 34 separate helpers. There are exactly **five controlled Rust fixture ignores**, no other skips or final failures. Their identities/reasons are retained in `verification.json`: isolated proxy child, captured-manager helper, installer launch-parent child, signal-probe child and explicitly launched Dart cloud fixture. Standalone compiled/focused/repeated runs are corroboration, not additional unique aggregate coverage.

ANSI was removed only in the review parser's in-memory input. Raw bytes/hashes were preserved. Log-emitting execution vertices 22/72/63/94/124 are all non-CACHED in the first aggregate session. Literal `Container.stdout` consumer vertices are 23/73/64/95/125. Later build sessions reuse numeric IDs and contain cached vertices; they were not mistaken for cached test runs. Dependencies/build layers used cache; no clean-cache/hermeticity claim is made.

The standalone compiled run occurred before the final documentation-only rebuild. Its complete recorded source/fixture-binary hashes still match current inputs, and compiled coverage is also required by the final all-tests Flutter gate. The successful standalone count is not added to 1,395.

## Artifact and documentation consistency

The reviewer verified complete output path sets, modes, lengths and hashes: **1 service export + 91 client files + 97 package files = 189 records**. All four service placements match SHA-256 `9a4f05f55f25d3a8661a2e1d83992589d343c7ef0a8f8fb15a168b0b9fb41b3a`. The exported client and packaged client match byte-for-byte and mode-for-mode.

`installer/dist/README.txt` equals current `dagger/INSTALLER.md`, **3,236 bytes**, SHA-256 `cb51d39e8e244c5db6c542fba46355e426bcbf76a70ae4df3ff6fffcb21f2990`. The 97 package regular files include 95 manifest-listed entries plus manifest/checksum control files. These are ignored local exports, not published or clean-source-provenanced releases.

Current Projects policy, plan, architecture report, root validation summary/detail and evidence README consistently describe metadata only, disk5 compatibility, query-only uncertainty, captured-origin review and logical—not secure—deletion. Parallel concise documentation is preserved; authoritative linked details are included in the final reviewer input snapshot. Hosted PR/merge statements in the concurrent validation summary were not independently queried and are not used as Projects evidence.

## Preserved failures and export limitations

1. **Aggregate attempt 1 remains failed**, exit 2/inner Rust exit 101: cloud test `save_is_local_validation_is_exact_synthetic_and_no_secret_is_durable` asserted actual disk5 against expected4 at `service/tests/cloud.rs:713`. The expected version was corrected for the approved migration; final cloud suite is 34/34. Cancelled/unexecuted first-attempt stages are not passes.
2. **Attempt 2 remains passing for its original snapshot**, core SHA `6c89d33c02397569aebc206644e8211c5f98ffbacf682c1a92bfe7c317d2457d`. Subsequent concurrent guide edits made it insufficient for final packaging. The exporter rejected that drift; only `dagger/INSTALLER.md` changed among 1,242 core inputs. Root README changes were supplementary, outside that core selection. The third full gate/build binds the retained current guide.
3. The mistyped CLI-digest setup command failed before gates; verified official v0.21.9 was used afterward, not a beta replacement.
4. The ANSI export-parser setup failure and post-gate drift refusal are evidence-harness outcomes, **not product test failures**. The exported failed-parser source is explicitly labelled reconstructed rather than an atomically retained pre-failure script; its failure timestamp is unavailable. No stronger historical-provenance claim is supplied. Parent records accurately preserve these limits.

## Evidence gaps and boundaries

- This is **source-limited assurance for the approved implemented metadata slice and configured source/build gates**, not proof that every normative target policy is implemented. In particular service quality ran on configured Rust1.98 Bookworm; this is not the additional Ubuntu24.04 service execution described by target policy. Other component/GTK/Flutter/runtime Ubuntu evidence remains distinct. This review does not waive that target-environment requirement for later qualification.
- No native graphical/IME/accessibility/multi-monitor acceptance, normal-session service/keyring/provider/model/GPU operation, installed disk5 migration/restore, real recipient or clean-host acceptance was performed. These are separately authorized stages, not implied by widget/private-socket evidence.
- Old binaries reject disk5. Later rollback needs approved pre-migration state, compatible paired binaries and exact keyring-generation relationships, not a binary-only downgrade. Logical deletion does not erase backups, RAM, swap or provider copies.
- Clean-source reconstruction/provenance, dependency-image locking/hermeticity, license/icon rights, SBOM, publication and deployment remain release obligations. Actual-package helper archive coverage uses a synthetic source identity, not clean-source release provenance.
- Raw parent logs/exports under ignored `target/` remain local retained evidence, not a Git backup. Original independent repros and selected before/fail/after logs are additionally preserved here. Retain the required raw evidence before any later cleanup. Owner-writable hashes detect drift, not adversarial authenticity or approval; independently retain the final review digest outside this bundle.
- No missing configured source check is waived, and no remaining blocking Projects code finding was found. Broader stage limitations above are expressly not accepted as completed or granted accepted-risk authority.

## Recommendation and verdict

**Verdict: PASS — bounded local Projects source candidate and verified configured gates only.** The three medium blockers are closed with unchanged independent repros and regression coverage. There is no required code remediation remaining for this slice. The low historical wording note is nonblocking and parent evidence was not edited.

Recommend human consideration of source acceptance for exactly the recorded candidate and documentation/artifact bindings. This recommendation is not human signoff and does not broaden the approved scope.

**Release Gate Decision: NO-GO for release, installation, migration, publication or deployment.** Those stages retain their separate evidence and exact human approval requirements. No commit, push, PR, merge, release, installed/external effect or accepted-risk decision is authorized by this review.
