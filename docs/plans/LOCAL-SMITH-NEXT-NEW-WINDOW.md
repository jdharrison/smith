# Session-only New window

## Approval and bounded intent

User approved the scoped New window feature with “GOOOO” in this conversation after the explicit implementation handoff. This approval covers local source/docs and isolated tests/builds for this session only, on existing `feature/autonomy`, base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` plus its pre-existing dirty work. No commit, branch change, installation, publication, live service/model access or operator preference access. Preserve unrelated work. Risk: elevated client/operator-flow and process/persistence boundary.

## Reconciled implementation contract

Add one New window icon beside Layout in the fixed shared app header, including Settings. Tooltip: “Open a new window — layout isn’t saved.” Launch the current resolved executable using `dart:io`, detached, shell disabled, with exactly `--session-only-window`. No forwarded arguments/state, new dependencies, native runner or backend changes. Each production child receives the same action. Disable while spawn is pending, allow later deliberate launches, show sanitized failure with no automatic retry or claim of readiness. Fence late UI work after disposal.

`main(List<String>)` recognizes the exact flag and omits preference-store construction entirely; no load/save/lock or later ownership adoption. Normal startup keeps existing persistence. Test/embedded constructors do not implicitly launch processes. The child starts default 1×1/Chat with existing Boot-selected conversation (first nonarchived or blank), not a parent snapshot. Ordinary Boot/service observation and existing independently scoped explicit-Start boundary remain unchanged. Service chat history remains shared/durable; drafts, local queues, layout, reading positions and exit decisions remain process-local. Closing the primary does not adopt or stop descendants; existing Close/Quit warnings and service detach behavior remain.

Exclude cloning, monitor detection/placement, geometry/multi-window restoration, live draft/queue synchronization, new shortcuts, application-wide Quit and inference/admission changes.

## Independent analyses and reconciliation

Independent read-only producer (`07e1569f`), architect (`eaa9983a`) and designer (`b7235d32`) contexts inspected source/policy before implementation. All agree on exact detached launch, lazy no-store startup and unchanged service/native ownership. Architect requires production-only launcher injection to avoid spawning the Flutter test runtime. Designer requires Layout first in traversal and its existing focus scope untouched, header overflow checks, pending/error/disposal handling, and truthful spawn-versus-readiness distinction. Producer confirms session-only means presentation only, not temporary chats. No blocking product/design disagreement remains.

Producer requested actual process/graphical evidence; reconcile as follows: fake-only source tests and Linux build establish bounded source behavior, not native graphical acceptance. No operator desktop/service launch is authorized. Missing native multi-monitor/lifetime evidence remains an explicit acceptance limitation, never a release/deployment PASS.

## Traceability and implementation tasks

| REQ / AC | Task | Required evidence |
|---|---|---|
| R1 / AC1 Accessible one-per-window action | Header callback and button alongside existing Layout widget | Header count, Chat/Settings/layout, keyboard, tooltip, narrow/enlarged widget tests |
| R2 / AC2 Exact detached current-bundle launch | Injectable small launcher; production injection at main only | Exact executable/sole flag/detached/no-shell assertions; failure propagation |
| R3 / AC3 No preference IO; primary unchanged | Exact-flag startup factory selection, null store | Factory not called in session mode/near-match negative; no-store interaction/disposal; existing restoration/store regressions |
| R4 / AC4 No cloned local work or new service effects | Fresh standard shell, unchanged Boot and exit paths | Fake API call/state assertions, independent drafts/queues, Boot and close regressions |
| R5 / AC5 Pending/error/disposal safety | Screen-owned async state and sanitized feedback | One launch while pending, explicit later retry, no raw error or success claim, late completion tests |
| R6 / AC6 Independent recursive windows | Fixed launch mode on every production startup | Descendant action wiring; native lifetime/monitor acceptance explicitly not claimed by mocks |

## Validation and rollback

Run focused formatting/analyzer and new tests first; then relevant shared-header/split/restoration/preferences/Boot/exit/entrance regressions, broader Flutter tests and Linux debug build where available. Retain exact commands and results below, including existing failures and skipped checks. Obtain fresh independent assurance/adversarial source review after self-check. Do not update unrelated golden baselines or repair unrelated failures.

Rollback removes only this feature's scoped edits; no state migration exists and no operator file should be deleted/reset. Normal startup/store and existing dirty work must remain intact. Canonical Architecture/Design and the source assessment are updated alongside the feature. No source evidence constitutes release, installation or native multi-monitor acceptance.

## Evidence

Implemented in `next/client/lib/main.dart`, new `lib/window_launcher.dart` and `test/new_window_test.dart`; canonical Architecture/Design and the architecture report contain bounded source additions. No native runner, service, preference-store/schema or dependency changes were made by this task.

### Source binding

Final self-check at `2026-10-16T07:06:04Z`, Ubuntu/Linux with Flutter 3.47.2 / Dart 3.13.2, existing resolved dependencies and unchanged lockfile. Base remains `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`; no commit was created. SHA-256 of the dirty source snapshot (includes pre-existing edits in main):

| File, relative to `next/client/` | SHA-256 |
|---|---|
| `lib/main.dart` | `a99ef6cfe2c255531d31fab088e7853b4402aa51c2b6011e82a4f1c2108869ba` |
| `lib/window_launcher.dart` | `9b2f3aaa4a47d22df959764374b0fb058cc0a8d0c03163878780acc130361ebf` |
| `test/new_window_test.dart` | `53d6ae58c7f40ebf00927eed6d2be0bfabd0dc33742f96df782b9bbff8384d2c` |

### Commands and outcomes

Commands run from `next/client/` unless stated otherwise:

- PASS: `dart format --output=none --set-exit-if-changed lib test` — 81 files, zero changes.
- PASS: `flutter analyze --no-pub` — no issues.
- PASS: `flutter test --no-pub --concurrency=1 test/new_window_test.dart test/shared_header_test.dart test/split_workspace_widget_test.dart test/workspace_restore_test.dart test/workspace_preferences_test.dart test/boot_test.dart test/queue_exit_guard_test.dart --reporter=expanded` — **108 tests**, including **15 new-window tests**. Covers exact startup flag/factory omission, normal/near-match startup, executable/argument/mode/shell checks, shared saved-chat/accepted-operation observation without copying drafts/queues or submitting, all layouts/Settings, single-flight preserving parent state, sanitized failure/retry, late success/failure after disposal, fresh descendants after simulated primary disposal, empty service, keyboard/tooltip, 400×400 at 2× text and no implicit embedded launcher.
- PASS: tracked scoped `git --no-pager diff --check` for main and the three canonical documents; formatting covers the new untracked Dart files.
- FAIL: initial broader focused selection including `test/workspace_entrance_test.dart` — 130 passed, 3 failed. The initial standalone feature test had one test assertion failure because Flutter exposes tooltip semantics separately from the label; the assertion was corrected to check the tooltip, then all new tests passed. No production change or golden update was needed for that assertion.
- FAIL: `flutter test --no-pub --concurrency=1 --reporter=expanded` — **652 passed, 11 skipped, 3 failed**. All 15 feature tests passed. Skips require compiled fake-injected Rust fixtures/`SMITH_TEST_SERVICE`; those fixtures were not prepared. Complete log: `next/client/build/new-window-full-test.log` (SHA-256 `7c5e8edd04ce90f417a631eb42683104330f87097a456e45875049bb209b9001`). No broad PASS is claimed.
- PASS: `flutter build linux --debug --no-pub` — produced `build/linux/x64/debug/bundle/smith_client`, not installed or launched. Log: `next/client/build/new-window-linux-build.log` (SHA-256 `19766452c5527f1add2ba735b18d1d62e487864979e19e0da59a7c489968c1fc`).

### Baseline attribution of the three failures

All failures are in the existing `selected-route entrance recovery` group of `workspace_entrance_test.dart`:

1. `healthy selected local route still animates when an unselected optional destination is unavailable` — `PROTOCOL: Invalid v2 application data.` / `readyCloud` at `cloud_routing_test.dart:253`.
2. `selected openai route becoming unavailable mid-entrance settles permanently` — expected false, actual true at `workspace_entrance_test.dart:287`.
3. `healthy selected openai route still animates when an unselected optional destination is unavailable` — the same protocol/readyCloud failure as (1).

Independent validation context `fc41528e` copied client inputs into ignored `next/client/build/new-window-baseline/`, removed only this feature's additions from the copied main (restoring the original entrypoint/header and removing callback wiring/state/method), and rebased the copied package configuration to its copied library. All 198 original input-file hashes remained unchanged. From that fixture, `flutter test --no-pub --concurrency=1 --reporter=expanded test/workspace_entrance_test.dart` yielded **23 passed / the same 3 failures**, with matching names/errors/helper locations. This proves they do not require the new-window additions, not their root cause or a clean historical-checkout result. They were not fixed or waived here.

The first copied-fixture attempt stopped before testing due to missing package graph; copying the existing generated graph fixed setup without dependency/network resolution. Reconstruction operations/diff, package-root verification and source hashes are retained beneath that ignored fixture. Baseline log `workspace-entrance-baseline-test.log` has SHA-256 `b70d9ba297ad149bd5d9dc2f515523499b634188ec2e5668f4029a37e7aef09b`; `comparison-evidence.json` records comparison. Ignored build logs/fixtures are local evidence and may be removed by a future build clean; this work record retains commands, outcomes and hashes.

### Review and acceptance limits

Fresh independent assurance/adversarial context `94eebf66` inspected startup/argument delivery, launcher ownership, null-store lifetime, callback forwarding, header/focus, Boot/exit boundaries and tests. It found **no concrete implementation violation**. Its initial verdict withheld assurance because this evidence section was still pending and broader failures had not yet been attributed; final follow-up is recorded below. It correctly notes that fake startup composition is not an actual GTK/executable/production-protocol test.

No actual desktop window, real keyring/provider/model, operator preference or installed service was accessed or mutated. Native window visibility, OS-level descendant survival, focus/IME and multi-monitor graphical acceptance remain **unverified**. Whole-suite/release readiness remains **NO-GO** due to the recorded three baseline failures and missing acceptance/fixture evidence; scoped source completion is not acceptance of those risks or approval to deploy.

Final independent recheck (`94eebf66`) verified the three source hashes, full-suite/build/baseline log digests, reconstruction diff, copied package root, all 198 original input hashes and scoped whitespace. It found **no remaining scoped source or evidence defect** and marked **scoped source completion complete**. Overall assurance remains FAIL / release NO-GO for the unresolved aggregate failures, skipped fixtures and graphical gaps above. No further feature-source remediation was identified; no risk waiver or external-effect approval was granted.
