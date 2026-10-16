# Immediate-send UI validation

**Implemented source / developer self-check, not independent approval or deployment evidence.**

- Validation session: 2026-10-15 UTC; source hashes captured at `2026-10-15T04:26:06Z` (host clock).
- Branch: `feature/autonomy`; HEAD/base: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`.
- Existing dirty work retained. No commit, deployment, service/persistence change, or installed-model inference.
- Toolchain: Flutter **3.47.2 stable**, framework `d3b14c8769`; engine `1cf1c4773fb941c4c74a7f8bb144a8837596c0f4` (revision `a804b26164`); Dart **3.13.2**; DevTools **2.60.0**. Obtained with `flutter --version`.

## Implemented behavior and review remediation

Request-keyed process-local text appears and the composer clears immediately. Static accessible Sending / Queued / Thinking presentation resolves to verified answer or explicit failure/unknown status; history remains authoritative, newer drafts survive, and no request is automatically resent.

Review blocker fixed in `lib/chat_panel.dart`: compare the current request-key set with the previously observed set, rather than comparing the last surviving key. Removing successful B while failed A remains does not constitute a new send. Controller changes reset the observed set to that controller's receipts.

Added widget regression `reconciling B after failed A does not scroll back to A` in `test/optimistic_send_test.dart`: retain failed A; send B and verify its one-time scroll; scroll to the top; reconcile successful B into nonempty scrollable history; verify only A remains locally and the reading offset stays zero, including after another draft edit.

## Exact validation commands and results

Commands below ran from `next/client` unless stated otherwise. Counts are per invocation, not cumulative unique tests.

```sh
dart format lib/text_chat.dart lib/workspace_controller.dart lib/chat_panel.dart test/optimistic_send_test.dart test/assurance_regression_test.dart
flutter test test/optimistic_send_test.dart test/workspace_controller_test.dart test/multi_chat_widget_test.dart test/text_chat_test.dart test/connection_polling_test.dart test/assurance_regression_test.dart test/chat_presentation_test.dart --reporter expanded
flutter test test/optimistic_send_test.dart --reporter expanded
flutter analyze
flutter build linux --debug
```

| Check | Result |
|---|---|
| Formatting | PASS, exit 0 |
| Seven-file relevant test run | **78 passed, 1 failed**, exit 1 |
| Immediate-send regression suite | **8 passed, 0 failed**, exit 0 |
| Analyzer | PASS, no issues, exit 0 |
| Linux debug build | PASS, exit 0; `build/linux/x64/debug/bundle/smith_client` |
| Repository whitespace: `git --no-pager diff --check` from repository root | PASS, exit 0 |

Dependency resolution succeeded. Flutter reported seven newer package versions incompatible with current constraints; no dependency upgrade was made. This is not a full repository test run or installed graphical QA.

### Remaining test failure — preserved, not suppressed

`test/chat_presentation_test.dart:189`, scenario `production 400x400 at 2x preserves draft through Settings and failure`, calls `find.byTooltip('Retry observation')` while the UI is healthy. No such tooltip/control exists there.

Inspection of `lib/ui/smith_workspace.dart` shows a **text button** labeled `Retry observation` only inside the error/recovery-guidance conditional. `lib/workspace_controller.dart` uses a 30-second automatic observation timer in healthy state. There is therefore no equivalent healthy-state UI action for a tooltip-only replacement. Per approved scope, the test and production behavior were left unchanged rather than switching the coverage to timer-driven observation or adding a control. The broader test gate remains **FAIL** pending separately agreed test maintenance; analyzer/build success does not waive it.

## Source SHA-256 snapshot

Captured from repository root with:

```sh
date -u '+%Y-%m-%dT%H:%M:%SZ'
git --no-pager rev-parse HEAD
git --no-pager branch --show-current
sha256sum next/client/lib/text_chat.dart next/client/lib/workspace_controller.dart next/client/lib/chat_panel.dart next/client/test/optimistic_send_test.dart next/client/test/assurance_regression_test.dart next/client/test/chat_presentation_test.dart
```

Paths below are relative to `next/client`. Hashes cover immediate-send implementation/test files, including pre-existing dirty content; the final row is the inspected **unchanged** failing test. This evidence file is excluded from its own hash list.

| File | SHA-256 at 2026-10-15T04:26:06Z |
|---|---|
| `lib/text_chat.dart` | `19eaef7374260e144d793a9e1f2853f999dba16514b4013adc0f3a03136d050d` |
| `lib/workspace_controller.dart` | `34dc8e25794cb1a7d59a4f1d8d734c022bbdf8a8d85f6a7040c3c548e563dcc4` |
| `lib/chat_panel.dart` | `c6baa4d4142b8e49143052e2389b8a47f64990e5e6858b67ecd6068c0de72727` |
| `test/optimistic_send_test.dart` | `0417affe8d443062d168ab7fa52cbaf180dda732b458b4bfcbc56892ebf347c9` |
| `test/assurance_regression_test.dart` | `0a985cdd899e2b4e711bcf0091f7326c7f04c7b138dd574cab9c6c3a01723e4f` |
| `test/chat_presentation_test.dart` (unchanged) | `e97fc78f4dda29ee614a0b1da7d3f5e92325d48cc0ffcd4652fe5ebadf1446ea` |

**Quality gate:** formatting clean; analyzer warnings/errors zero; build passed; relevant tests retain one explicitly recorded failure. Fresh read-only independent review subsequently verified all six source hashes and whitespace checks, closed the scroll blocker, and found no remaining concrete immediate-send implementation blocker. The overall assurance/release gate remains no-go because of the broader presentation-test failure. See `docs/plans/LOCAL-SMITH-NEXT-IMMEDIATE-SEND.md` for the review disposition.
