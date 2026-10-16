# Static cloud catalog and search — independent assurance/adversarial review

## Validation scope

- Fresh source/evidence review of `LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION`, repository `smith`, `feature/autonomy`, HEAD/base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, plus preserved dirty/untracked work. Read `AGENTS` → `SCOPE` → relevant Architecture/Process/Design/Host Profile → `roles/assurance.md` → plan, source/tests and evidence. No builder reasoning transcript used.
- Reviewed the actual **12-file implementation/test delta**, independently of HEAD: five backend/installer files and seven client files. Verified nine parent prechange copies against `next/service/build/cloud-catalog-expansion/initial.json`; regenerated both scoped patches in memory and matched their retained bytes exactly. HEAD alone cannot identify this change because much of the cloud implementation already existed untracked.
- Authority is static plausible-model additions and local filtering, with possible failure on an explicit send. It is **not** discovery/API validation authority or a waiver of readiness, consent, exact routing, no-replay or quality gates. Account access is deliberately unverified.
- Only this new report was written; parent directory and report absence were verified first. Reviewer actions were file/image inspection and read-only local Git/Python comparisons. No new test/build, application launch, installation, container/manager operation, host bus/keyring/credential read, network/vendor call, commit, cleanup or publication occurred. Combined gates were reported running elsewhere; this review neither waits for nor presumes their outcome.

## Findings, severity ranked

### High — G1: final acceptance evidence is not yet bound (gate blocker, not a demonstrated source defect)

CAT-R6 and `docs/PROCESS.md:106–116` require stable combined component/test/build/package evidence, paired artifact identities and fresh review binding. The retained focused results establish neither the full combined gate nor the final recipient package. At this review cutoff those final results have not been supplied or verified. The plan's Outcome and as-built catalog assessment correctly say pending; no false final PASS was found there.

**Reproduce:** compare the plan's CAT-R6/Validation/Outcome with the focused summaries under the evidence roots below and `next/VALIDATION.md:3–13`. The ledger still records the preceding schema task's two aggregate failures, not a completed catalog-expansion combined gate.

**Required disposition:** supply and independently inspect the final command statuses, raw logs, stable source inventories and actual package/artifact binding before closing this gate. Do not inherit a preceding task's PASS, infer success from a running command, or treat the operator's fail-on-send allowance as acceptance of failed/missing checks. No additional heavy execution or unrelated repair is requested by this review.

### Low — L1: direct unsent-chat / real split-pane picker regression — CLOSED

**Initial finding retained below; superseded by the test-only remediation closure at the end of this report.**

The new picker tests predominantly mount the already-saved chat returned by `readyCloud`. `FakeAppApi` seeds that chat (`next/client/test/app_fakes.dart:8–15`). The active/controller/disposal cases in `cloud_picker_test.dart:342–394` manipulate a small `_Host`; they do not drive the actual `SplitWorkspace` activation path. The existing `cloud_routing_test.dart:349–369` checks a new chat's local default and unavailable-route retention at controller level, not choosing from the new dialog on that unsent chat.

**Impact:** this is a narrow coverage limitation, **not a reproduced routing or new-chat bug**. Static inspection supports the implementation: `WorkspaceController.all` includes pane-owned blanks, `current()` checks object membership and selected-pane membership rather than requiring a saved ID, and the picker itself never creates a service conversation. Production split-pane state supplies the `active` property and invalidates the picker on update.

**Suggested regression scenario:** select New chat in one of two real panes; open/filter/clear/cancel and verify null chat ID, unchanged draft/queue/revisions and zero new calls; explicitly choose an enabled cloud route and verify only that blank's future route changes; reopen, deactivate/hide or replace the originating pane, then invoke a captured row callback and verify no selection. This would strengthen CAT-R3/R4 coverage without changing production behavior. It was not executed or added here. No meaningful existing test was weakened to create this gap.

**Critical: none. Other high/medium production or test defects: none identified in this bounded review.** G1 remains blocking regardless of that source finding disposition.

## Exact delta and adversarial assessment

### CAT-R1/R2 — membership, defaults, truthful verification and fail-on-send

**Implemented, source-limited:**

- `next/service/src/cloud.rs:59–153` contains the plan's exact ordered seven OpenAI/eight Anthropic identities, labels and contexts. OpenAI contexts are 1,050,000 and Anthropic 1,000,000. The original first entries remain `gpt-6-astra` and `claude-sonnet-5-5`. The 15 choices fit eight per provider and 256-local/272-total bounds. Independent expected lists in `service/tests/cloud_protocol.rs:25–95` and `client/test/cloud_catalog_expansion_test.dart:7–51` prevent a shared production-table error from passing solely through self-comparison.
- Rust outside `MODELS` and Dart `cloud_routing.dart` outside `cloudModels` are byte-identical to prechange copies. `cloud::model` remains exact provider+ID lookup. The unchanged Dart parser rejects unknown/wrong-provider IDs, missing generations outside catalog observations and inconsistent readiness/verification facts (`cloud_routing.dart:37–49,176–235`). No arbitrary ID, response alias learning or capacity increase was introduced.
- The installer test repair removes the invalid assumption that global model index 1 denotes Anthropic: `default_model`/`catalog_choice` select by provider, and explicit 7/8 counts plus literal Astra/Sonnet first-choice assertions remain. Production installer catalog parsing uses the shared exact membership predicate and existing bounds; GTK already selects the first **provider-filtered** choice (`installer/src/ui.rs:935–946`). No installer production edit was needed.
- Readiness still requires the exact current credential generation, current-instance observation and successful validation of that generation (`service/src/app_v3.rs:687–715`). Cached catalog construction reads those facts without keyring/API access (`:1043–1083`). Only the actually checked model receives `validated_model`; other enabled choices show **Not individually verified** in the new dialog. Added successful chat sends do not themselves mark every model validated. Unconfigured/saved-not-validated choices remain disabled.
- Every-entry protocol fixtures retain exact request parameters, 8192-token cap and final-text parsing; wrong response identities include **other compiled choices**, not just invented IDs. The unchanged parser still requires exact response `model` (`cloud_http.rs:499–535`). Added service tests exercise every entry under validated generations, first-destination consent and exact request/routing identities. Synthetic 403/503 failures on newly added unverified choices retain their exact routes, empty history, one attempt and zero local fallback (`service/tests/cloud.rs`, `unverified_catalog_model_failure_never_retries_or_falls_back`). Existing unknown-delivery/restart/replay/consent/generation tests remain.
- The unchanged worker constructs the admitted route's body, retrieves its exact generation, publishes the dispatch marker, then makes one HTTP attempt and parses against that model (`app_v3_worker.rs:125–186`). There is no new fallback/retry path. HTTP/keyring/domain/worker/controller/ChatPanel bytes inspected for this boundary match the initial manifest. This is not a fresh exhaustive audit of the surrounding cloud implementation.

**Not verified:** published vendor pages, real account entitlement, current model availability, real inference or billing. No external documentation fetch or API probe was performed by this reviewer. The plan/API contract's public-research claims are retained inputs, not independently reproduced live evidence; the explicit operator allowance makes account/model rejection on send an expected possibility, not by itself a finding.

### CAT-R3/R4 — search, origin fencing and future-only selection

- Filtering is a trimmed lowercase substring over label, model ID, provider and LOCAL/CLOUD group, using only the cached catalog (`cloud_selector.dart:237–252`). Search edits/clear use private controller state. Current selection is separately rendered even when it does not match; missing/empty catalogs differ from zero results. Disabled reasons and unverified labels wrap rather than ellipsize.
- Opening/filtering/dismissal contains no API/keyring/configuration call. Application of a result requires a non-current, currently available exact route. The explicit same-route check avoids `selectRoute`'s route-revision increment and queue latch (`:131–135`; `workspace_controller.dart:26–35`). Snapshot tests include API calls/arguments, accepted requests, draft/history/configuration, route and queue revisions (`cloud_picker_test.dart:73–231`).
- Row activation and the awaiting trigger each recheck authority. Captured origin object, origin revision, route revision, service instance, configuration revision, saved/null conversation identity, archive/exit-hold state and membership are fenced; exact route equality includes credential generation (`cloud_selector.dart:98–146,227–233`; `cloud_routing.dart:57–63`). A replaced generation is not silently rebound. Removed/disabled/missing-catalog callbacks, changed origins, disposed routes and result-before-commit races have explicit tests.
- `didUpdateWidget` invalidates/dismisses on controller or active-state changes, and disposal invalidates the captured revision before deferred route removal. Membership guards also refuse an origin removed from the workspace. These conclusions are limited to the observed widget/controller state; the initial direct real-pane regression limitation L1 is closed by the follow-up below, not a claim of native split-window acceptance.
- Existing controller semantics remain: blanks start from the saved local default; accepted requests retain their immutable route; an actual future-route change pauses retained unsent text for explicit review. Selection does not send or grant destination consent. The original queued-cloud review action and transfer dialog are unchanged. Historical unavailable selections remain readable and replaceable without deleting history or forcing a new chat.

### CAT-R5 — keyboard, IME and bounded layout

- The search field's Enter submission only refocuses search; it does not choose even one match. Dialog/trigger focus wrappers suppress repeated Enter/key-up activation; explicit Tab-to-row plus Enter chooses once. Escape/barrier/Cancel dismiss without route/draft/queue effects and focus returns to the trigger rather than the composer. Captured callbacks during/after pop cannot act (`cloud_picker_test.dart:396–543`). The real `ChatPanel` isolation test confirms picker Enter does not reach its Send handler.
- The retained composition test sets an active composing range and verifies Enter has no routing/send effect. This is synthetic Flutter event coverage, **not installed Linux IME acceptance**, and does not independently prove native IME Escape handling.
- The dialog is viewport constrained, with a scrollable central list and reachable search/Cancel. Inspected actual bundled-font 400×400 1x and 2x captures (initial, scrolled unverified and disabled-reason states). At 2x the initial viewport only partially shows the first matching row; scrolling reveals its full identity/status. That is bounded scrolling, not evidence that all content fits simultaneously. Geometry/reachability tests cover the final catalog choice and no Flutter overflow exception (`cloud_picker_test.dart:570–624`; `cloud_picker_visual_test.dart`). No installed graphical/accessibility guarantee follows.

## Existing-test preservation

Independently regenerated and inspected all scoped existing-test differences, not only the added tests:

- Service hardcoded three-entry assertions are replaced by stronger per-entry identity/label/generation/readiness/verification/no-effect assertions. Independent literal count/order/context tests replace the removed isolated `MODELS.len() == 3` check. Existing parser negatives remain; cross-entry response rejection is additive.
- Installer index-based assertions/mutations now address the intended provider/model, preserving the negative case rather than accidentally mutating another OpenAI choice. Availability checks generally expand from one model to all provider choices. Real service-domain round trips gain catalog and no-extra-read/post assertions.
- The existing client pending-work selector test changes only the UI interaction to filter/scroll/click the exact row; its accepted-local-route assertion remains. The real Rust↔Dart journey only adds independent full-catalog parsing assertions; consent, expiry, reattach, unknown delivery, queue retry and counters remain unchanged.
- `cloud_selector_widget_test.dart` is byte-identical to its parent baseline. No scoped test removal, new skip/ignore, timeout increase, fallback enablement or meaningful assertion relaxation was identified. Existing controlled Rust fixture entrypoints and optional direct-Dart skip conditions are not newly introduced by this delta.

## Required Checks Evidence

**B** = `next/service/build/cloud-catalog-expansion/backend/`; **C** = `next/client/build/cloud-catalog-expansion/`. These are retained developer executions independently inspected, **not reviewer-executed tests**. Backend raw-log SHA-256 values were checked against their command records. All seven final backend command records agree on unchanged before/after hashes for nine inputs (five files plus both Cargo manifests/lockfiles), which match current bytes. All six before/after inventories for the three settled client checks agree on 208 current inputs and the digest below.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Reviewer read-only Python: baseline hashes, exact regenerated patches and catalog-only production comparisons | Exact scoped delta; no dependency/transport/controller expansion | Verified, exit 0 | Parent `baseline/`, B `scope-audit.json`, B `scoped.diff`, C `before/`, `client-delta.json`, `client-scoped.patch`, inventories |
| `rustfmt --edition 2021 --check --config skip_children=true service/src/cloud.rs service/tests/cloud.rs service/tests/cloud_protocol.rs service/tests/debug_service.rs installer/tests/providers.rs` from `next/` | Format clean | Retained exit 0 | B `format.{json,log}` |
| `cargo clippy --manifest-path service/Cargo.toml --locked --offline --test cloud_protocol --test cloud --test debug_service -- -D warnings` from `next/` | No warnings/errors | Retained exit 0 | B `service-clippy.{json,log}` |
| `cargo test --manifest-path service/Cargo.toml --locked --offline --test cloud_protocol --test cloud` from `next/` | Exact catalog and neighboring cloud regressions pass | **30 cloud + 12 protocol passed**, 0 failed/ignored; exit 0 | B `service-cloud-tests.{json,log}` |
| `cargo clippy --manifest-path installer/Cargo.toml --locked --offline --test providers -- -D warnings` from `next/` | No warnings/errors | Retained exit 0 | B `installer-clippy.{json,log}` |
| `cargo test --manifest-path installer/Cargo.toml --locked --offline --test providers` from `next/` | Strict catalog/provider integration preserved | **18 passed**, 0 failed/ignored; exit 0 | B `installer-providers-tests.{json,log}` |
| `cargo test --manifest-path service/Cargo.toml --locked --offline --test debug_service app_v3_cached_catalog_and_preparation_use_the_actual_socket_without_cloud_effects -- --exact` from `next/` | Real private socket reads catalog with no cloud effects | **1 passed**, 22 filtered, 0 failed/ignored; exit 0 | B `debug-catalog-socket-test.{json,log}` |
| `flutter test --no-pub --reporter expanded test/cloud_catalog_expansion_test.dart test/cloud_picker_test.dart test/cloud_picker_visual_test.dart test/cloud_routing_test.dart test/cloud_selector_widget_test.dart test/cloud_protocol_test.dart test/cloud_retry_test.dart test/rust_cloud_integration_test.dart test/rust_socket_integration_test.dart` from `next/client/` | Focused tests, actual Rust catalog/cloud journey and four ordinary sockets run, not skip | **54 passed**, 0 failed/skipped; exit 0 | C `compiled-focused-settled.{json,log}`, matching source inventories and `fixture.json` |
| `flutter analyze --no-pub` from `next/client/` | Analyzer clean | No issues; exit 0 | C `analyze-settled.{json,log}` |
| `dart format --output=none --set-exit-if-changed` on the seven changed Dart files from `next/client/` | No changes | 7 files, 0 changed; exit 0 | C `format-check-settled.{json,log}` |
| Scoped whitespace and source/capture binding | No whitespace diagnostics; retained bytes match reviewed inputs | Backend corrected check exit 0; client summary reports scoped whitespace clean; reviewer independently verifies source/patch/capture hashes | B `whitespace-corrected.{json,log}`, C `summary.json`, source inventories and six PNG hashes |
| Required combined `make service/test installer/test client/test`, `make test build`, package tests/export verification as configured | Successful, stable final-source and paired-artifact evidence | **Pending final evidence binding; not verified by this review** | Parent-owned final records and `next/VALIDATION.md` follow-up |

Backend focused total is **61** (42 + 18 + 1), not a full backend acceptance count. Retained backend checks are dated 2026-10-16 UTC, 04:59:36–05:01:48. The settled Flutter run is 05:07:52–05:07:57 UTC as recorded. Owner-writable logs and hash consistency are evidence, not signed provenance or independent authentication of chronology.

Earlier expected-red catalog, whitespace-wrapper status error and initial client failures remain retained; they are not current passes or erased history. The reviewer also corrected one read-only audit's path prefix (`service/...` is relative to `next/`); its initial `FileNotFoundError` was an audit-command error, not a product/build/test result.

## Review identities — initial snapshot

Independently recomputed SHA-256; these bind the initial uncommitted source review, not final packaged artifacts. The follow-up below supplies superseding test-only bindings; the original identities and focused results remain historical evidence.

| Reviewed input | SHA-256 |
|---|---|
| `next/service/src/cloud.rs` | `7e7faba6ae9a841c3c157368c557dc8004d1f5dc2b0de7d316519b3a4f5d0aa0` |
| `next/service/tests/cloud.rs` | `cb7595b6a5e12c3cf9ff0ca23e934809b61e8ae5c18fdf2207e9ac3711a53529` |
| `next/service/tests/cloud_protocol.rs` | `2a7723410f451fec85fa4c37403038c7d21669ccca85329438cd16917fdccb25` |
| `next/service/tests/debug_service.rs` | `20385c7304731ff338fa419afb47995ac204a04b28a6a5493780f104a97c9c2a` |
| `next/installer/tests/providers.rs` | `21108bdc8e83b78040f7b8774dd05912977c8ac906692e1153e79121a25794a6` |
| `next/client/lib/cloud_routing.dart` | `da3c07661f90913521ca86d4c41ee5004d21857253d9166c1c5c944ffbb897ad` |
| `next/client/lib/ui/cloud_selector.dart` | `6ec389a4e7186f5966bcc6bc84578c210097a7a460e423647935fd2d10561835` |
| `next/client/test/cloud_catalog_expansion_test.dart` | `86cffb18d06f74fd798165b08d4c617c9d5d30f97f36ae6eca3621e03c846bf2` |
| `next/client/test/cloud_picker_test.dart` | `e5d44b94332fd9229f8efdd18c4d1dd36fe0deeaaf64eac905ac4b12c1ce058b` |
| `next/client/test/cloud_picker_visual_test.dart` | `4bf1e5506a5f1f6e5403e8524ac15b7f98befe863398acf094e6f565c6cf9ad9` |
| `next/client/test/cloud_routing_test.dart` | `0cdd660e5259dbfbf4d97bdfe181bbfdd9214b6b0a982cbd51f7cb63aa7d9b29` |
| `next/client/test/rust_cloud_integration_test.dart` | `3a9b98228ff2f37266eae670cc8e11e4f75e2b27ff38125f7fbc57dab3777273` |

- Twelve-file mapping digest: `5e8d994c2dddc8a3cc56e2a78494da90533404f0a5683ec06626aab2433e8c83` (SHA-256 of Python `json.dumps({project_relative_path: file_sha256}, sort_keys=True).encode()`, default separators).
- Settled 208-input inventory digest: `3dad0cc405cf0ad6e4c6d88db16988a058c05a0f657f67bc2ec46b204c1d3dc4`, same encoding. This focused inventory is not the full aggregate/package input set.
- B `scoped.diff`: `ebf8f6ceea8c5e8eac038f30d64a3fc3b6afb2f6333bdf9f702230fa701ac2f8`; C `client-scoped.patch`: `dd2b81f8dadfb5ed8660937fa8f8fd7cc8cafe391d9b737c464280be710db7dc`.
- Plan: `9542957ef72c9b0d553629676eb4735368f1a33dea7b276008f96b7169c36c7b`; API contract: `047dcfa59a65dbc8a03cb94d0310f07e09b1da84a399fef114c2e696cf8871a4`.
- C `compiled-focused-settled.log`: `47c9fa0c04a68907760cd59206cbb6ea34dfb8d9bbad532b6cefba28afadb83a`; command record: `ca66c2ae22eae2e0867bb89b06eaee120dd90c56fca5b1a2e2f3f62bfef3420e`.

## Evidence Gaps and pending final-gate binding

1. **G1 is open.** Final combined/build/package results, full input stability, final artifact equality/checksums and ledger closure have not been reviewed. Follow-up must bind actual results to this source set and distinguish controlled Rust fixture ignores from Dart fixture execution; skipped required journeys are not acceptance. Review any source drift, not just HEAD equality. A documentation-only follow-up does not itself authorize new execution or external effects.
2. **L1 is closed at source/retained-widget-evidence level.** The appended test-only remediation demonstrates pane-local blank interaction and actual split-pane deactivation fencing without production changes. Native graphical/IME acceptance remains unverified.
3. **Historical failures remain historical failures.** `next/VALIDATION.md:11` and the preceding schema review record aggregate `RUNNER_BUSY` teardown and Vulkan full-window timing failures. This catalog delta did not repair those paths or establish their root causes/harmlessness. A later genuine PASS can establish that execution's result, not erase prior failures, prove their resolution or confer blanket release readiness.
4. No new installed-host/keyring/vendor/model test was authorized or performed. Real exact-model access/charges, normal-session behavior, native graphical/IME/accessibility acceptance, installed migration/restore and release/deployment remain separate unverified gates. Strict older catalog-aware readers can reject new IDs; use rebuilt paired artifacts rather than asserting mixed-version compatibility. Existing schemas/storage rollback requirements remain unchanged.

**Source-limited verdict:** no concrete blocking defect or meaningful existing-test relaxation identified in the bounded production/test delta. The inspected implementation follows CAT-R1–R5 at source/synthetic-evidence level, subject to the stated native/live limits; L1 is closed by the follow-up below. CAT-R6/final acceptance remains incomplete. This is not an overall `pass` or `pass-with-risk`, exhaustive target-policy compliance, human approval or acceptance of residual risk.

- **Formal assurance verdict: FAIL — required final combined/artifact verification remains pending; the initial aggregate failure is retained in the follow-up below.**
- **Release Gate Decision: NO-GO.**
- **Required remediation/follow-through:** close G1 using supplied final source/artifact-bound evidence and independent review; retain all historical failures and distinguish current results from root-cause claims. L1 is closed by the appended test-only evidence. No production fix is demanded on present evidence, and no approval for commits, live probes, installation, publication or deployment is granted.

## Test-only remediation follow-up — L1 closed, G1 still pending

### Scope and independent checks

Reviewed the supplied two-test-file delta and retained evidence, without new tests/builds, host/runtime calls or inspection/waiting for the new final aggregate. Only this report was updated. **R** = `next/client/build/cloud-catalog-expansion/review-remediation/`; remediation execution records/logs are one directory above R with the `review-remediation-*` prefix. **A** = `next/service/build/cloud-catalog-expansion/`.

Read the actual prechange copies, current tests, R `delta.patch`/`summary.json`, raw red/targeted/final logs and command records. Independently regenerated the patch in memory; it exactly matches R `delta.patch`. Verified both before-copy hashes and the parent `A baseline/workspace_entrance_test.dart` copy. Compared the prior 208-input reviewed snapshot with all six final remediation before/after inventories: **only `test/cloud_picker_test.dart` and `test/workspace_entrance_test.dart` differ**, and all final inventories agree with current bytes. R's separate 200-client-input inventory also matches current bytes: its other 198 entries, including production, assets and goldens, remain unchanged. The five previously reviewed backend/installer files still match their initial review hashes. These are inspected-inventory claims, not a new whole-repository acceptance claim.

### L1 disposition and existing-test preservation

**L1 CLOSED.** `next/client/test/cloud_picker_test.dart:451–594` now mounts actual `ConversationScreen`/`SplitWorkspace`, chooses the side-by-side layout and New chat through UI actions, and edits distinct drafts in two pane-owned blanks. It verifies null IDs, saved-local defaults and separate identities. Open/search/clear/Cancel preserve both blanks' and the saved chat's snapshots, including calls, draft/history/configuration and queue/route revisions. Explicit choice changes only the second blank's future route, leaves it unsaved/send-ready, and creates no accepted request, service chat or job.

The scenario captures a picker-row callback, requests focus on the other actual composer, then verifies the real `ChatPanel`/`SplitWorkspace` activation path deactivates the origin and dismisses its dialog. The old callback is inert both after deactivation and after pointer-driven reactivation. The inactive trigger is disabled, other-pane/saved-chat state stays unchanged, and final assertions require no additional API/control calls. Programmatic focus drives the first activation deliberately; this is an actual production-widget callback path under synthetic Flutter testing, **not native pointer-through-modal, window-manager or installed graphical acceptance**. Deactivation covers the L1 alternative originally requested; an additional hide/replace variant is not necessary to close this narrow finding.

The new picker coverage is additive: three imports plus one scenario (**+148/-0 lines**). Removing those additions reconstructs the prior picker test file byte-for-byte. The historical entrance repair (**+44/-6 lines**) is confined to the original local/cloud unavailable-route loop in `workspace_entrance_test.dart:1090–1187`; prefix/suffix outside that loop are byte-identical. The old `DropdownButton<ChatRoute>` cast and direct `selectRoute` action are replaced by opening the actual picker, filtering, checking retained historical/unavailable selection, and clicking the enabled exact local replacement. The former enabled-control intent is exercised through a successful real selection rather than merely dropped.

Initial unavailable/readiness/history checks and subsequent entrance-settled/no-remount/no-replay/read-only assertions remain. New assertions preserve route/draft/queue revisions and call counts while filtering, then require the exact explicit selection and one route-revision increment without rewriting historical routing. `_expectRouteState` and `_expectCloudReadOnly` remain unchanged. No skip/ignore, timeout increase, golden update, readiness relaxation or meaningful assertion removal was identified. **No production defect or production remediation is established by these test corrections.**

### Required Checks Evidence — remediation only

These are retained implementer executions inspected by the reviewer, not newly executed reviewer tests. Times below are recorded UTC on 2026-10-16.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Initial `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` from `next/` | Required full gate passes | **FAIL, exit 2**, 05:13:41–05:16:04; Flutter **535 passed / 2 failed** with obsolete TextButton-to-DropdownButton casts | A `combined-status.json`, hash-matched `combined.log`, `failure-excerpt.log` |
| Pre-edit `flutter test --no-pub --reporter expanded test/workspace_entrance_test.dart --name 'historical (local|openai) route unavailable'` | Reproduce the specific stale test assumptions | **0 passed / 2 failed, exit 1**, both casts at original line 1128; 05:26:37–05:26:41 | `review-remediation-historical-before.{json,log}` and inventories |
| Targeted repaired historical cases plus `real split-pane picker`, using the recorded `--name` expression | Three scenarios pass | **3 passed / 0 failed/skipped, exit 0**, 05:28:13–05:28:17; intermediate test snapshot | `review-remediation-targeted.{json,log}` and inventories |
| `flutter test --no-pub --reporter expanded test/workspace_entrance_test.dart test/cloud_picker_test.dart test/cloud_routing_test.dart test/cloud_selector_widget_test.dart test/cloud_catalog_expansion_test.dart test/cloud_protocol_test.dart test/cloud_retry_test.dart` from `next/client/` | All affected focused files pass on stable current tests | **74 passed / 0 failed/skipped, exit 0**, 05:29:05–05:29:12 | `review-remediation-focused-final.{json,log}` and matching inventories |
| `flutter analyze --no-pub` from `next/client/` | Analyzer clean | **No issues, exit 0**, 05:29:05–05:29:08 | `review-remediation-analyze-final.{json,log}` |
| `dart format --output=none --set-exit-if-changed test/workspace_entrance_test.dart test/cloud_picker_test.dart` from `next/client/` | Format clean without writes | **2 files / 0 changed, exit 0**, 05:29:05–05:29:06 | `review-remediation-format-check-final.{json,log}` |
| Reviewer read-only hash/diff reconstruction | Exact two-file delta; old tests preserved; no production/golden drift in inspected inventories | Verified, exit 0 | Actual files/copies, R inventories/patch, six final command inventories, previous review bindings |

The pre-edit reproduction's before/after inventory exactly equals the original reviewed 208-input snapshot (`3dad0cc4…`), so its failures are not counted as passes or inferred from narrative. The targeted 3-test run used intermediate inventory `30798fe9…`; the final 74-test/analyzer/formatter checks share current inventory `25f0b0e9…`. Only the picker test changed between those two post-remediation snapshots. Current closure relies on the final 74-test run, not on incorrectly rebinding the intermediate targeted PASS. This seven-file remediation run **does not include compiled Rust socket/cloud journeys or the visual capture test**, unlike the earlier distinct 54-test run; no fresh result for those scenarios is claimed here.

The initial aggregate's raw log hash matches its status record, which reports equal before/after inventory `13546eede8149526d73982f1444a8f3c40f66514b065c571cede8ecf793b5a81` (1,289 inputs), exit 2 and no kill. The raw diagnostics and focused reproduction localize these two observed failures to stale test casts, not a demonstrated production failure. **The aggregate remains a failed historical execution.** Focused remediation does not turn it into a full aggregate PASS, establish final package acceptance, or close G1. The new `final-validation/` execution is not inspected or presumed successful in this follow-up. Older schema-task runner failures also remain unchanged historical evidence.

### Superseding test/evidence identities and final disposition

SHA-256 independently recomputed:

- `next/client/test/cloud_picker_test.dart`: initial `e5d44b94332fd9229f8efdd18c4d1dd36fe0deeaaf64eac905ac4b12c1ce058b` → current `ba1c95cc191a6644c5e2a21365191eeae23f08c26e3a1fe6d68e9941f1e7d7d9`.
- `next/client/test/workspace_entrance_test.dart`: prechange `817a14d70831ba2540f4f146be9b1128f0cfc067eb5f8eb93d3326b9249d9fd3` → current `650a580e0d22b3ecacab051abf8ab773820e1c5c925491fd16c5b0fff83bdf15`.
- Current final 208-input inventory: `25f0b0e93ce9f07e22032ac2e6f6e4984bc82aef1cf050e65342a883faf3b68a` (same sorted-JSON encoding as the initial inventory).
- R `delta.patch`: `616934a7168a3cbb27a3b54eb571ca188f99a88a748717dc9afc88ed914dc2c1`; R `summary.json`: `2dc6bd736920ade5a963e3e9dea127bb10d289730ff0871a8a38d8c48e1e7a13`.
- Final remediation focused log: `fcade0b83a6516324d34d77ec81cc6413ab384212496d8d3dbbef5c73364e0e9`; command record: `054cd06a3e9f46c5f02a1d965abe1d3b4e57b00ad303cd35afd93b99a0e58b14`.
- Pre-edit red log: `582b62a6b0a4fd513dbe593b1647c056ec0b8f439176b72e3632c7d7e3eb5040`.
- Initial aggregate raw log: `1a7cb6db381cb773896494add7e7fa7952103e35fbcd09a2b1916167f44c6676`; status: `7829280d95e1e84770e81ab04ca159d3031d48fa6f65ab7e7b9f980b2b49b4d0`.

**Source-limited follow-up verdict:** L1 is closed; no meaningful test relaxation or new production blocker identified in this two-test-file delta. The initial source review's production conclusions remain unchanged within their stated limits.

- **Formal assurance verdict: FAIL — G1 final combined/build/package evidence binding remains pending; the initial aggregate failure is retained.**
- **Release Gate Decision: NO-GO.**
- **Required follow-through:** supply final source/artifact-bound results for independent G1 disposition, preserving the initial failed execution and earlier historical failures. No additional L1 remediation is requested. This report authorizes no new execution, live probe, installation, commit, publication, deployment or accepted-risk decision.

## Final separate component/package evidence closure — 2026-10-16 UTC

### Scope and disposition

**The previously missing source/component/package binding is now independently verified. The scoped separate component/build/package deliverable has passing bound evidence; broad aggregate acceptance remains FAIL / Release Gate NO-GO.** This append-only closure supersedes earlier *pending component/package evidence* statements, not either aggregate failure, the initial review history, native/live limitations or the lack of human approval.

Only this report was appended. Reviewer work was retained-file/source inspection, independent read-only inventory reconstruction and hashing, raw-log checks and package manifest/checksum comparisons. No tests, builds, package-verifier invocation, artifact execution, container/manager/keyring/API call, installation or other host/runtime operation was performed. The configured verifier's successful execution is retained evidence, not a new reviewer run.

**P** = `next/service/build/cloud-catalog-expansion/component-package-validation/`; **D** = `docs/evidence/cloud-catalog-expansion/`; **A2** = `next/service/build/cloud-catalog-expansion/final-validation/`. Inspected P `final.json`, statuses/raw logs, before/after inventories, source/artifact/count records, bookkeeping scripts, D's index and copies, actual exports, current plan/ledger/as-built descriptions, and the unchanged Dagger/component gate definitions. HEAD and branch remain `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` / `feature/autonomy`.

### Final findings, severity ranked

- **High — G1-A, broad aggregate gate remains failed/unresolved.** The second aggregate failed `provider_prefers_full_vulkan_and_falls_back_once_only_before_completion_dispatch` at `next/service/tests/runner.rs:2130:51`, with `RUNNER_BUSY: Model verification could not acquire or retain its bounded runner lease`. Raw source inspection confirms the failing call and the mapping at `service/src/providers.rs:311–317`. Independent summation of primary raw summaries gives service **257 passed / 1 failed / 3 ignored**, installer native **279 passed / 2 ignored**, and package **41 passed**. Flutter reached **529 observed passes without full-suite completion**; GTK was canceled and build was not reached. A2's status is exit **2**, not killed, 05:33:28–05:36:01 UTC. This remains a failed required aggregate on the same final source as the later separate passes. No causal repair, harmlessness, equivalence to earlier runner failures or accepted risk has been established.
- **G1-C, missing component/package evidence — CLOSED.** The sequential commands, complete current source set, rebuilt exports and explicit package verification are bound and passing as detailed below. This closes the missing-evidence portion of G1, **not** its failed aggregate requirement. A separate full component PASS does not equal `make test build` aggregate acceptance.
- **Low — L1 remains CLOSED.** Both test-remediation hashes still match the previously reviewed closure. No additional L1 fix or production change was needed.
- **Critical / additional high / medium / low scoped source findings: none identified.** The final thirteen-file target matches the implementation and test-only remediation already reviewed; no new source defect or meaningful test weakening is identified in that bounded delta. This does not assert the whole codebase is bug-free or that the runner failure has no indirect interaction with combined load.

### Required Checks Evidence — actual separate results

All five commands ran once sequentially from `next/`, on identical 1,289-entry inventories, with retained exit **0**, no timeout/killing and no command-time source drift. Make commands use `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain`. Dates/times below are retained UTC on 2026-10-16. The reviewer verified each raw log SHA-256 against its actual status, each status against P/D `final.json`, and the chronological non-overlap of commands.

| Check / command | Expected | Independently verified actual | Evidence under P |
|---|---|---|---|
| `make service/test` | Formatting, warning-denied Clippy and locked service tests pass | **258 passed / 0 failed / 3 controlled fixture ignores**; all 41 runner tests passed in this separate execution; 05:45:21–05:46:47 | `service-test-status.json`, `service-test.log`, before/after inventories |
| `make installer/test` | Native quality/tests, isolated GTK and no-interpreter Ubuntu runtime checks pass | **279 native + 9 GTK passed / 0 failed**, two controlled native fixture ignores; configured no-Python help/version/artifact-only dry-run completed; 05:47:00–05:48:51 | `installer-test-status.json`, `installer-test.log`, inventories |
| `make client/test` | Dependency resolution, formatter/analyzer and complete Flutter suite with mandatory compiled fixtures pass | **538 passed / 0 failed / 0 skipped**, 79 Dart files formatting-clean and analyzer clean; 05:48:59–05:50:11 | `client-test-status.json`, `client-test.log`, inventories |
| `make build` | Standalone service, complete client and full native recipient package exported | **PASS**, including Linux release client and build-time 95-file package verification; 05:50:23–05:54:13 | `build-status.json`, `build.log`, inventories |
| `python3 dagger/package.py --root installer/dist --verify` | Verify the actual rebuilt recipient export | **PASS**, explicitly **95 manifest-listed files**, distinct from 97 recipient files total; 05:54:26 | `package-verify-status.json`, `package-verify.log`, inventories |
| Reuse of second aggregate's package subtask, **not a fresh `make package/test` run** | Prior package subtask passes on exactly matching complete source | **41 passed**, raw unittest `Ran 41 tests` followed by `OK` on task128; source matches every final component/build/verify snapshot | A2 `combined.log:9841–9851`, P/D `suite-counts.json` and `final.json` package-reuse records |
| Reviewer read-only reconstruction/hash/content checks | Source and actual exports match all retained bindings | **Verified, exit 0**; 1,289 current inputs, thirteen-file reviewed delta, 189 exports, manifest/checksums and durable copies all match | Current source/exports, P/D records, previous review identities |

Independently checked **21 primary Rust suite completion summaries** (eight service, twelve native installer and one GTK), including counts and raw line/task references, excluding duplicated untagged Dagger stdout. The five ignores are the retained controlled subprocess fixture definitions, not newly excluded product tests. The full Flutter completion and zero skips are verified; the unchanged Dagger recipe supplies both the compiled service and fake-cloud fixture, and its mandatory test guards do not silently skip them. Compact concurrent output does not expose every journey name, so no separately visible per-scenario timing/result is invented.

The fresh separate component totals are **1,084 passing tests / five controlled ignores**. The reused41 is explicitly separate, not a fresh execution, a unique grand total across overlapping focused gates, or overall aggregate acceptance. The package fixture subtask consumes actual native installer ELFs through the unchanged Dagger producers; it reports synthetic **88-file** fixture verifications, **not** verification of the final95-file candidate. Its logs do not emit separate hashes for those tested ELFs: reuse is bound by identical full source and native-builder recipe, not an independent byte-for-byte proof that every fixture ELF equals a later export. The subsequent explicit verifier and independently rehashed exports bind the rebuilt candidate's contents. No clean/hermetic/reproducible-build claim follows.

### Source and durable-evidence binding

- Independently enumerated the current service/installer/client/Dagger trees and three root build inputs using the recorded exclusions, without executing the bookkeeping scripts. Recomputed path membership, file/symlink kind, bytes, modes and SHA-256 for **all 1,289 entries**. The resulting compact sorted-JSON inventory equals **`205bfa82434e1a4ecbd23f2f9c5455c7d61274c1b96c888b9e3a948453724b03`**, P/D source manifests, and all ten final command before/after snapshots. The second aggregate before/after entries also exactly match this set, establishing the package41 reuse's source equality.
- Exactly **thirteen source/test files** differ from the retained initial inventory. Their current hashes exactly equal the initial twelve-file review plus the two reviewed test-only changes (one overlaps the original twelve). The only production changes remain Rust `cloud.rs` `MODELS`, Dart `cloud_routing.dart` `cloudModels`, and `ui/cloud_selector.dart`; all three production hashes still equal the initial review. Current thirteen-file path→SHA-256 mapping digest is **`9345edc7d6ff1e7191a141e5321371c8c8630fb4eab507a62c010a35c372333d`**, using the initial review's default-separator sorted-JSON mapping encoding. This differs intentionally from the compact full-inventory encoding.
- `service/src/runner.rs`, `service/tests/runner.rs` and `service/src/providers.rs` remain byte/mode-identical to the initial inventory. Their respective hashes are `34ccf8da4da7edc262f8df8d4673b815b74de6ea3cd7d2ed659346d9aded05c7`, `b0661d713bd63c5bd5ae3eaa80db9e347ae55e9c9d1251b6e41a07026d07c931`, and `978f9885735e8c261c8e50eea801c37681a0b3e74442ff082560767b91d952fc`. No runner assertion relaxation or provider repair is present. Unrelated accumulated Boot/header/logo/scroll work is included in the tested inventory, not newly authored or independently approved by this catalog review.
- Independently verified byte-identical P→D copies of `final.json`, `artifacts.json`, `source-manifest.json` and `suite-counts.json`; both durable aggregate reports equal their original raw reports. D `picker-2x.png` equals the original client capture and was visually inspected. All **93 protected prior evidence files** still match their pre-continuation hashes/bytes. Owner-writable consistency is not signed provenance or authentication of execution chronology.
- The exclusion list omits generated/cache roots and keeps the source inventory separate from root policy/review/validation documents. Current plan Outcome, ledger and evidence index truthfully separate sequential source/package PASS from aggregate FAIL, distinguish reused41 from fresh execution, retain both failed attempts and do not assert deployment. This report append does not change the tested build-input set.

### Rebuilt candidate, export manifest and checksums

Independently streamed hashes for **all 189 actual exports**: standalone service1, client91 and recipient97. Actual membership, regular-file kind, bytes, modes and mtimes match P/D `artifacts.json`; no export changed during hashing. Export content inventory SHA-256 is **`03cb4849dfed693b6c16457f0db67db5825634bb88ff75039d2af409055887aa`** using the recorded compact path/kind/mode/bytes/hash projection.

All **four service copies** match **`e5b80a4d1c2a4e687385280a91aa44bec1cabeb7caf59d4387476a10223a708f`**: standalone service, client helper, recipient client helper and recipient standalone service. All **91 client/recipient-payload pairs** have identical membership, kinds, sizes, modes and contents. No mixed/stale export identities from the failed aggregate reports are substituted for this rebuilt candidate.

The current `next/installer/dist/manifest.json` exactly matches the retained manifest record. Its **95 declared paths** equal all recipient files except the manifest and `SHA256SUMS`; every listed mode/hash matches the current file. Reconstructed the client tree's declared digest, including directory modes, and checked the service identity against the manifest. `SHA256SUMS` exactly equals the sorted95-file list plus the manifest hash (**96 checksum entries**, 97 recipient files including the checksum file). Key executable headers are native x86_64 ELF; no binary was executed by the reviewer.

| Candidate identity | SHA-256 |
|---|---|
| Recipient `manifest.json` | `d6ab1d2d8392cfa242cd3b66a8689f69a2e382f26de773d214bbb2b0097839cf` |
| Recipient `SHA256SUMS` | `06927f173b01e7942fae2e8a8e54a8e36bd6e52df0bccfb24d4420b61fdfeb64` |
| Client `smith_client` | `b12601aa3bf99ee6bbc66c857282b0d8c111d78929b69f83ea9db1b45dba551c` |
| Client `lib/libapp.so` | `ebe9cda70a863f3509f0e98033254f70be64e7a1c68886bbec18eb167674b069` |
| Recipient `smith-installer` | `f1eab2258b07c417b8ed1dde22da749f59ce33ca2dd5e189775fae170e4767e3` |
| Recipient `smith-installer-ui` | `74634996da42b4356cac9faca7f44d42f793ab4b3d2f77b678b413db551711c3` |

The complete local candidate is `next/installer/dist/`, not a preceding partial/stale export. Hash agreement establishes local artifact/source-record consistency, not authenticity, installed acceptance, license/SBOM clearance, provider entitlement or permission to publish/install it.

### Durable evidence anchors

SHA-256 independently recomputed; D records preserve raw log/status paths and hashes:

| Evidence under D | SHA-256 |
|---|---|
| `final.json` | `0512b9b07b70a884c1ce699ac4b322eb41072e91c6f966b16bbf6e3edd0e273a` |
| `source-manifest.json` | `83fdc67c56bea2925bd21937bb34b146405461f6e994d3fd6c451cee95c3cc0c` |
| `artifacts.json` | `1b83628c33c64be4c498dbc57838e3fbe2a68ec5270b1d0cf68b98e2091b6992` |
| `suite-counts.json` | `8508d375c074666b75890fde83e94b6d5ee06d850181dfd3e7eb9af62bfd286b` |
| `aggregate-attempt-1.json` | `8e599817df66daf705e99f18a720ef0c253b706bcdbd6b8ef177dd2f80095772` |
| `aggregate-attempt-2.json` | `c919880f5a5891d9c2bb2ab22ff4e98a028d75eb9e67521e1a5b5313eeca2bea` |
| `picker-2x.png` | `c1860ad4012cd0a43e50c228832fef9a33f81bfd0ee06f7c33f260ca51eb879d` |

Both aggregate raw log hashes/status bindings remain intact: first `1a7cb6db381cb773896494add7e7fa7952103e35fbcd09a2b1916167f44c6676`, second `8757064f1e53f0bcbf99d8b3b72fbbe12228fadeef974688eacf2781eda791ec`. The first remains **535 Flutter passes/two stale-cast failures** on its original snapshot; those test defects were subsequently remediated and reviewed, not relabeled green. The second remains the unresolved bounded-lease failure on final source. No third aggregate or fresh package-test run is recorded in this continuation. Separate passing executions establish neither the runner failure's cause nor its resolution, nor equivalence to earlier schema-task teardown/lock/full-window failures.

### Evidence Gaps and final decision

**No missing component/build/package binding remains for this bounded local deliverable.** G1-C is closed and L1 stays closed. The independently inspected source and test delta has no known concrete blocking defect; the separate source-bound component/package evidence is verified PASS within its declared limits.

**G1-A remains failed, not merely missing:** the required broad aggregate has an actual unresolved failing test. Clean-clone/hermetic provenance, full redistribution/license/SBOM review, installed/native graphical/IME/accessibility acceptance, real credentials/keyring/model access/billing, installed migration/restore and release/deployment remain unverified separate gates. No accepted-risk decision is made. Later source or artifact changes invalidate this binding until reviewed; older strict catalog readers still require the paired rebuild.

- **Scoped source/component/package disposition: VERIFIED PASS — local deliverable only; no additional catalog/picker source remediation requested.**
- **Formal overall assurance verdict: FAIL — unresolved required aggregate failure.**
- **Release Gate Decision: NO-GO.**
- **Required follow-through:** preserve both aggregate failures and independently bound separate passes; any investigation, remediation or eventual disposition of the broad runner gate requires its own bounded authorization and evidence. This closure requests no third aggregate, runner fix, live probe or deployment, grants no publication/installation/commit authority, and accepts no residual risk.
