# LOCAL-SMITH-DEV-NAVIGATION — fresh independent assurance closeout

## Validation scope

- **Implemented, bounded feature SOURCE PASS:** reviewed the approved inert selectable Dev root, independent disclosure, strict preferences, lazy keyed retention, focus and stale-modal fences. No actual feature defect identified in the reviewed source/evidence.
- **Overall assurance FAIL; combined quality BLOCKED; release NO-GO.** Source correctness is not passing combined validation, human approval, installed acceptance or a waiver.
- Fresh closeout assessment, not a resumed reviewer session. Previous reviewer `session6726f35d-f6fc-4338-bb60-5e3208da102a` executed the retained `review/` checks; this reviewer independently inspected source/probes, rehashed/reparsed their evidence, recomputed pixels and reran **only 57 preference cases**.
- Only new closeout artifacts and this report were written. No product/test/golden/prior-review edits, permission changes, commits, remote mutations, GUI/product launch, installation or live provider/model/manager/repository effects.

## Source and evidence identity

Current executable core: **1,296 records, including 54 untracked**, SHA256:

`b86b9d04d2867a23994b142d3850f1b4ea0a5a72c6df7f1d2321ec2f2a180777`

Verified baseline: **1,337 files**, canonical SHA256:

`8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f`

Baseline TAR SHA256: `968e688390737472076ac7ccef129bb8469e7a9ccc9b0627d0c74f837bc4802d`.

This reviewer independently regenerated Git tracked/untracked selector membership and full core records (bytes, sizes, modes, types, tracking state), compared with parent final manifest and prior reviewer source-before, and verified every baseline archive member. Exactly three production changes relative to the actual dirty baseline:

1. `client/lib/main.dart`: only `WorkspacePresentation.setPage` destination allowlist.
2. `client/lib/workspace_preferences.dart`: v4 writes; strict integer v1–v4 reads with exact version-specific destination sets and unchanged exact object shape.
3. `client/lib/ui/smith_workspace.dart`: root/disclosure/modal/focus/retention presentation changes.

No service/API, controller/editor internals, preference IO, theme/header or execution behavior change from this slice. Existing navigation test adjustments move disclosure actions to their distinct control and update the writer version; preservation assertions are retained/extended, not disabled. See `review-closeout/production.diff`, `source-binding.json`, `source-current.json` and final `seal.json`.

The executable-core selector deliberately excludes parent-owned documentation/evidence and generated outputs. Concurrent README/policy/ledger finishing is **not preapproved by this assessment**. A later change to selected source or required evidence invalidates its binding. This is not a clean-tree/release reconstruction claim.

## Source correctness assessment

**Meets the bounded target contract in implemented source**, under `docs/plans/LOCAL-SMITH-DEV-NAVIGATION.md` and the specific Architecture/Process/Design/Host requirements:

- Dev root selection only selects `dev`; separate 48×48 controls are stacked in the existing 56px scrollable rail. Selected semantics belong to the actual destination; disclosure exposes expanded state, not child selection.
- Root body is the exact static placeholder, with no Run/retry/service-dependent recovery content. Root/disclosure do not eagerly mount Projects/Agents; existing observation/work is not cancelled.
- Wide disclosure changes links only. Per-pane state starts collapsed and is not serialized. Compact disclosure is a real bounded `SimpleDialog`; expanded state tracks visibility. Epoch, origin page, mounted and inherited pane `TickerMode` guards reject stale results, including away/back and hide/show.
- Collapsing focused child navigation requests disclosure focus. Retained keyed Chat/Projects/Agents slots use Offstage/TickerMode/ExcludeFocus; stable keys avoid Agents-first identity replacement when Projects is first mounted later. Hidden pane focus exclusion remains in the unchanged split-workspace owner.
- Preferences retain exact destination history: v1 Chat/Settings, v2 adds Projects, v3 adds Agents, v4 adds Dev; no Dev in v1–v3, no v5, no disclosure/prompt/editor fields. No normalization-only load rewrite or session-only store construction is introduced.
- Prior corrected adversarial source covers 600 version/destination/slot/forbidden-field checks inside one test, keyboard focus, inert restore, compact enlarged modal and Agents-first retention. Prior retention probes exercise actual Agents/Projects draft state and uncertainty plus four Chat drafts/in-flight provisional work. These are prior executions corroborated by this review, **not reruns by this reviewer**.

## Required Checks Evidence

Expected outcome for executable gates is exit 0 with required assertions passing; required missing/failing gates block acceptance. Exact sanitized argv, environments, times, exits and raw log hashes are retained in the referenced metadata; `review-closeout/raw-gates-rechecked.json` contains this reviewer's parsed summaries and verified hashes.

| Check / command or scenario | Actual outcome / exit | Evidence and execution ownership |
|---|---|---|
| Independent baseline/core/membership audit: `python3 -B docs/evidence/dev-navigation/review-closeout/verify.py` | PASS / 0; baseline and current core verified; three production edits | **This reviewer**, `review-closeout/source-binding.json`, `source-current.json`, `production.diff` |
| Independent exact RGBA comparison, baseline TAR vs current shell PNGs | PASS; 23 images, 159,815 changed pixels, zero outside disjoint 56px rail masks (four masks for mixed panes); 13 editor PNGs unchanged | **This reviewer recomputed**, `review-closeout/pixel-recheck.json`; prior geometry mask provenance explicitly retained |
| Full `dart format --output=none --set-exit-if-changed lib test`; Flutter analyzer | PASS / 0; 111 formatted files, zero changes; analyzer no issues | **Parent executions**, `validation/full-format.*`, `full-analyzer.*`; this reviewer verified log hashes/terminals. Prior reviewer narrower static checks also PASS (`review/format.*`, `analyzer.*`) |
| Focused Flutter navigation/prefs/retention/golden/queue/authority suites | Parent PASS / 0: 305 passed, 0 skipped. Prior reviewer FAIL / 1: 291 passed, 14 preference-store fixture failures | `validation/focused-safe-temp.*`, `review/focused.*`; **not rerun here**. Unsafe ancestor rejection is retained, not converted into a passing prior invocation |
| Corrected independent adversarial / retention Flutter probes | PASS / 0: 5 tests / 2 tests respectively | **Previous reviewer**, `review/adversarial-attempt002.*`, `retention.*`; probe source inspected here, raw logs hash-verified. First adversarial attempt remains FAIL / 1 (3 passed, 2 failed): leaked SemanticsHandle and omitted existing Settings `instructions_get` expectation |
| Strict preferences and Linux store: `python3 -B docs/evidence/dev-navigation/review-closeout/preferences.py` → Flutter `test --no-pub --reporter expanded test/workspace_preferences_test.dart` | **PASS / 0: 57 passed, 0 failed, 0 skipped**, 6.133s | **This reviewer reran**, new fixture, `TMPDIR=/tmp`; 43 copied source/pubspec/test files hash-bound; `review-closeout/preferences.json`, `preferences.log`. No host permission changes |
| Full cached-SDK Flutter suite | FAIL / 1: 978 passed, 4 failed, 22 explicit compiled-fixture skips | **Parent**, `validation/full-native-tests-safe-temp.*`; this reviewer rehashed/reparsed |
| Configured `make client/test` with compiled fixtures | FAIL / Make exit 2: 989 passed, 15 failed, 0 skipped | **Parent**, `validation/client-test-configured-module.*`; this reviewer rehashed/reparsed |
| Configured Linux `make client/build` | PASS / 0; private desktop/helper export, not a full package | **Parent build**, `validation/client-build-configured-module.*`, `linux-client-export.json`; this reviewer verifies exported file identities/modes and snapshot archive in `review-closeout/seal.json` |
| Actual repository-root `make test build`, official CLI, one real attempt | FAIL / 2, 91.45s: **one service unit vertex, 143 passed, 20 failed, 2 controlled ignores**; identical summary printed twice by failure replay, not 286/40/4. Build not reached | **Parent**, `validation/root-original-results.json`, `root-original-test-build.*`; this reviewer independently parses/deduplicates raw terminals in `review-closeout/raw-gates-rechecked.json`, `root-distinction.json` |
| Earlier copied-module root discovery harness | FAIL / 2 before product checks; preserved separately | `validation/root-test-build.*`; actual-root result supersedes **only this earlier root-status claim**, not other gate records |
| Full-package verification / actual-package helper tests | **NOT RUN**, exit null; stale exports unchanged, no current full-package acceptance artifact | `validation/root-original-results.json` conditional commands and export identities; `review-closeout/root-distinction.json` |
| Final source/selector, prior-evidence preservation and artifact seal: `python3 -B docs/evidence/dev-navigation/review-closeout/seal.py` | Final result recorded in `review-closeout/seal.json`; hashes bind this report and closeout evidence | **This reviewer**, not prior reviewer/parent approval |

## Findings by severity

### Critical

None identified in the bounded navigation implementation. This is not a whole-product security assurance claim.

### High — blocking combined validation

1. **Required full-client tests fail.** Three frozen Projects label goldens; one compact multi-chat finder/`ensureVisible` failure; eight compiled Projects cases (expected disk7 vs actual disk8/fixture exit101); three Rust socket migration revision cases. Reproduce using the exact recorded configured `make client/test` command/environment; expected all pass, actual 15 fail. Native full-suite fixture skips do not replace compiled coverage. No arbitrary baseline blame or waiver is asserted.
2. **Actual root gate fails and package evidence is absent.** The single real root attempt fails service unit tests (143/20/2); aggregate build is not reached. Full package verification and actual-package helpers remain NOT RUN. A passing standalone Linux client build does not clear this blocker. Root's failure-replayed service summary is counted only once.

### Medium — preserved harness limitations, not demonstrated product defects

- Previous reviewer focused run used repository-rooted TMPDIR with a group-writable ancestor and triggered 14 safe-store permission refusals. The prior temp directory is mode0775; product IO was not weakened. This reviewer's separate `/tmp` fixture run passes all 57 preference tests; it does **not rewrite the historical failed invocation**.
- First adversarial probe leaked its own SemanticsHandle and assumed no Settings instruction read. The corrected independently retained probe passes; both attempts remain visible.

### Low

No additional bounded-feature defect identified. Three Projects failure renders each differ by **865 pixels**; current actual hashes match the preserved UI-handoff failure archive, with baseline expected editor bytes unchanged. This establishes handoff equality only, not that an arbitrary historical baseline run failed. This reviewer recomputes these deltas and verifies archive equality in `review-closeout/seal.json`.

## Evidence Gaps

- No passing stable-source full-client/root/full-package gate. No rerun of 1,000-test/backend suites was authorized or performed in this closeout.
- No installed/native Ubuntu graphical, real IME/window-edge/accessibility, migration/rollback, live-host or provider acceptance. Widget evidence is not graphical acceptance.
- Prior runs are assessed through immutable source/log/artifact bindings, not represented as this reviewer's executions. Dagger cache execution is not cold-build/hermeticity proof; internal unexported debug fixture has no independently verified ELF hash.
- Current parent-owned documentation finishing outside the selected core is not approval-bound by this report. Any promotion needs fresh source/evidence/review binding and explicit human scope.

## Decision and required remediation

**Verdict: FAIL. Release Gate Decision: NO-GO. Combined quality: BLOCKED.**

The implemented, narrowly scoped Dev-navigation source assessment is **PASS**, with no feature fix requested. This is neither overall acceptance nor permission to waive failures. Responsible owners must reconcile the exact full-client/root failures, then obtain passing required stable-source component/root/build/package evidence and fresh independent assessment. Native/installed checks and any release/deployment effects require separate exact approval. No fixes, waivers or external effects were performed or authorized here.
