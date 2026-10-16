# Automatic connection observation — client-local evidence

Date: 2026-10-14

## Intent and boundary

**Implemented source, self-check only.** Explicit user scope: automatic SYSTEM ONLINE observation without refresh, limited to `next/client`. The supplied bounded architecture is the implementation contract. Parent owns canonical documentation and independent architecture/assurance review (reviewer session `1aeb48cc` was identified in the task; this coding context has no session messaging tool and does not claim that review occurred).

No service/installer/canonical-document edits, dependencies, lifecycle actions, installed-host probes, deployment, inference, model downloads, commits or remote effects were performed. Existing title, Settings, window and other concurrent changes were preserved. Test-generated Settings failure images were removed after the corrected baseline passed; other work was not cleaned or reverted.

## Implemented behavior

- `lib/workspace_controller.dart`: workspace-owned idempotent one-shot timer, first check 30 seconds after start and subsequent checks 30 seconds after the prior automatic reconciliation completes. It survives navigation, Settings and failures. Dispose cancels both observation timers, fences queued work and uses the existing single API disposal/owned-helper cleanup.
- `lib/boot.dart`: starts observation only after Boot publishes ready, never from widget build or retry loops. No automatic Core inspection or lifecycle operation is added.
- Shared validated snapshot path has separate automatic/manual modes. Automatic mode preserves pending request IDs, expired-request markers, mutation clocks and global-operation budgets. Newly discovered identities receive their own clocks; old identities are not replaced by unrelated global work. Explicit manual synchronization retains the pre-existing budget-reset recovery semantics.
- Whole workspace reconciliations are serialized, including selection detail, submission response/poll, lifecycle receipts, explicit inventory and snapshot observation. Same-mode concurrent snapshot requests coalesce. NativeAppApi remains the individual-exchange arbiter and owns helper timeouts/cleanup; no Future.timeout wrapper or orphaned helper is introduced.
- Automatic observations call only `state` and necessary `conversation_get`/existing-identity `poll`. The post-Boot operation timer cannot initiate deferred `models` work. Boot/explicit Retry retain inventory observation; an automatically recovered service with no current-instance inventory requires manual Retry or externally observed inventory before Send becomes eligible.
- Healthy checks retain online status and skip unchanged selected detail. Existing conversation objects/drafts and mounted UI are retained. Outages mark application state stale and gate actions; successful observation can restore connectivity without resend. Instance changes clear cached model/provider facts and invalidate saved detail before adopting current-instance facts.
- `lib/ui/settings_view.dart` and `lib/ui/smith_workspace.dart`: visible Settings disclosure and status accessibility hint distinguish periodically last-observed connection from continuous monitoring, model readiness and work completion. No status tooltip added. `README.md` describes the client behavior.
- `test/connection_polling_test.dart`: fake-time coverage for idempotent start, exact first/rearm timing, long-held calls, serialized/coalesced manual refresh, healthy status, outage/recovery, model-fact invalidation, request/mutation/global budget preservation, explicit expired-request recovery, operation/heartbeat serialization, interruption without replay, dispose with in-flight work, and Settings/draft retention. An injectable Stopwatch factory supplies monotonic production clocks and fake-time budget clocks without a dependency.
- `test/boot_test.dart`: held Boot observation verifies no app polling even after 30 fake seconds before ready.
- Only `test/goldens/ui-redesign/fixture-settings-1440x900.png` was regenerated for the intentional new disclosure and visually inspected. Other golden fixtures were not regenerated.

## Initial implementation validation evidence (superseded by review remediation below)

Commands below ran from `next/client`, without fetching dependencies or a live service candidate.

| Check | Command/scenario | Result |
|---|---|---|
| Formatting | `dart format lib/workspace_controller.dart lib/boot.dart lib/ui/settings_view.dart lib/ui/smith_workspace.dart test/connection_polling_test.dart` and subsequent formatting of `test/boot_test.dart`/changed tests | PASS; final formatter reported no changes |
| Focused controller/Boot | `flutter test --no-pub test/connection_polling_test.dart test/workspace_controller_test.dart test/boot_test.dart` | PASS, 34 tests at that checkpoint; two additional connection regression tests subsequently added and passed in focused/full runs |
| Final focused polling/Settings | `flutter test --no-pub test/connection_polling_test.dart test/settings_view_test.dart` | PASS, 11 tests |
| Intentional Settings baseline | `flutter test --no-pub --update-goldens test/ui_redesign_golden_test.dart --plain-name 'redesign golden fixture-settings-1440x900 bundled fonts real ChatPanel'` | PASS, one targeted baseline |
| Analyzer | `flutter analyze --no-pub` | PASS, no issues |
| Full suite | `flutter test --no-pub` | PASS, 233 passed, 4 skipped |

Earlier self-check failures were corrected: initial instance invalidation must not mark the unsaved blank draft stale; navigation uses the existing Return to Chat label; a braces lint was fixed; Settings wording retained the existing model-readiness disclaimer and its intended screenshot was updated. The final full run passed without broad baseline regeneration.

**Initial checkpoint quality gate:** analyzer warnings/errors: none; formatting clean; executed tests passed at that checkpoint. See the review-remediation results below for the current shared-worktree gate; the initial result is not a current full-suite pass.

## Independent-review remediation — 2026-10-14

**Implemented source following user-relayed review `1aeb48cc`; fresh review pending.** The user reported two blockers and explicitly approved the post-Boot app-state Retry design. Changes remain client-only and preserve concurrent optimistic-send/HUD work, including removal of header refresh.

### Fixes

1. **Immutable queued Send authority.** `submit` synchronously captures request ID, text, original draft revision, service instance, conversation identity, config revision and history revision into an immutable intent. A per-conversation queued-send reservation closes the repeat-click/Enter window. Dispatch checks current readiness, membership, archive state and captured identity/revisions without rebinding to new authority. Original payload/revisions are sent only if still authorized. Rejection retains the parallel optimistic local receipt as Not sent; subsequent drafts are not cleared. Disposal releases an undispatched reservation without a service call. The existing optimistic clear-on-send/local receipt presentation is preserved.
2. **Serialized post-Boot manual observation.** `main.dart` routes WorkspaceController Retry through `synchronize()` and explicitly permitted `refreshModels()`, both using the shared reconciler. It performs no Core inspect and does not set healthy state unavailable or CHECKING SYSTEM merely because a manual observation started. The controller publishes actual observation failure. Legacy non-workspace Retry and Boot lifecycle handling remain unchanged. Status semantics/Settings/README now define last verified app service transport/state, not an ongoing enabled-at-login guarantee.
3. **Disposal fence.** A snapshot checks disposal again after selected detail completes, before reconciling operations. Held detail/poll replies and queued sends/mutations cannot publish or dispatch after disposal.

### Regression evidence

New `test/connection_review_regression_test.dart` covers:

- Held heartbeat with edited/repeated Send for both existing and unsaved conversations: exactly one original payload, newer draft preserved.
- Config revision, history revision, instance, archive and membership changes while Send is queued: no submission or revision rebinding.
- Delayed manual-before-automatic and automatic-before-manual observations: max one API call, healthy status retained while waiting, stale on actual failure, recovery without another Core inspect.
- Disposal with held detail and held poll plus queued Send/mutation: no late publication, follow-up call, timer rearm or duplicate API disposal.

`test/connection_polling_test.dart` gains a held-detail fake seam. Two existing `conversation_test.dart` workspace Retry tests now use the existing callback and app-state outage facts, asserting only the Boot Core inspection; this does not restore removed HUD controls. Other concurrent HUD tests/baselines were not rewritten. Only the two Settings fixtures were regenerated for the changed disclosure, preserving the current concurrent HUD appearance, and inspected.

### Current validation results

All commands ran from `next/client`; no live host/service action or package fetch.

| Check | Command | Actual result |
|---|---|---|
| Focused review/heartbeat/optimistic/controller/Boot/Settings | `flutter test --no-pub test/connection_review_regression_test.dart test/connection_polling_test.dart test/optimistic_send_test.dart test/workspace_controller_test.dart test/boot_test.dart test/settings_view_test.dart` | PASS, 58 tests at this checkpoint |
| Existing model recovery regression | `flutter test --no-pub test/conversation_test.dart --plain-name 'model configuration stays in the separate installer'` | PASS, 1 test |
| Existing outage regression | `flutter test --no-pub test/conversation_test.dart --plain-name 'outage preserves visible history and editable draft; Retry never sends'` | PASS, 1 test |
| Settings baselines only | `flutter test --no-pub --update-goldens test/ui_redesign_golden_test.dart --plain-name 'redesign golden fixture-settings-'` | PASS, 2 tests |
| Analyzer | `flutter analyze --no-pub` | PASS, no issues |
| Formatting | `dart format --output=none --set-exit-if-changed lib/workspace_controller.dart lib/main.dart lib/ui/settings_view.dart lib/ui/smith_workspace.dart test/connection_polling_test.dart test/connection_review_regression_test.dart`; subsequent `dart format test/conversation_test.dart` | PASS, no changes |
| Full shared-worktree suite | `flutter test --no-pub --reporter json > build/connection-review-test-results.json` | **FAIL: 236 passed, 17 failed, 4 skipped** in the latest run |

Current full-suite failures (not silently accepted):

- 13 non-Settings golden mismatches: historical single-conversation; chat 400x400, 1440x900, 1280x720, 1440x900 IBM, 4200x2400, 400x400 text2; archive 1440x900 and 400x400 text2; overflow 1440x900 and 400x400 text2; history 400x400 text2; recovery 400x400 text2.
- Four missing-control expectations: production 400x400/2x draft/Settings/failure; enlarged expired recovery at 360x800 and 800x600; recovery-details 400x400 text2 (`Needs attention` absent).
- These remaining results are associated with the concurrent HUD/control changes. Removed controls were not restored and unrelated golden baselines were not regenerated. The parent/HUD worker must reconcile those tests/baselines and rerun the full suite. The latest JSON output and test failure images remain local diagnostic artifacts.
- The four skips are Rust socket fixture tests without `SMITH_TEST_SERVICE`.

**Current quality gate: BLOCKED on the full shared-worktree suite and fresh independent review.** Focused tests pass, analyzer has no warnings/errors, formatting is clean. This does not claim a full-suite pass, independent approval, installed-host validation or release readiness.

## Remaining review / risks

- Parent must reconcile canonical policy and obtain independent architecture/assurance review; self-check is not approval. No tool available here can contact the named reviewer session.
- Four Rust socket fixture tests explicitly skip without `SMITH_TEST_SERVICE`; no paired Rust candidate validation is claimed in this record.
- Last-observed status can remain stale between checks, and the next check waits for queued reconciliation plus existing bounded helper cleanup. Thirty seconds is an idle interval after completion, not a strict wall-clock freshness SLA.
- Missing inventory after restart is intentionally not repaired by automatic model work. Manual Retry remains available; connection online is not model-ready or inference-verified.
- Installed desktop behavior, real service restart, user-manager behavior and real inference remain unverified and require separately approved validation. No service contract or helper cleanup policy was changed.
