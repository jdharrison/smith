# Chat vertical scroll anchoring

## Bounded intent

User-requested local client bugfix on `feature/autonomy`, base `7f6a483`, preserving the pre-existing dirty worktree. Scope: transcript Y offsets only; no layout, service, routing, persistence, host or external effects. This supersedes the one-time initial-scroll and Send-forced-scroll behavior, not submission authority. No commit, install or deployment is authorized.

## Plan and acceptance

- AC1: Each view defaults to following the bottom through message, status and render/viewport extent changes, including an initially empty chat.
- AC2: Scrolling away preserves desired absolute Y through updates and explicit/queued sends. Returning within one logical pixel of the bottom resumes following.
- AC3: Per-pane/per-chat bookmarks and temporary geometry-clamp recovery remain; programmatic layout corrections do not change the reader's intended anchor.
- AC4: Only transcript scroll Y changes; composer selection/focus, message layout and service behavior remain unchanged.

Implementation: generalize the existing settled-geometry bookmark/restoration path to ordinary and independent views. Encode follow-bottom as an infinite desired bookmark, always clamp to finite extents before applying, and never use infinity as an initial scroll offset. Remove one-time/Send-specific overrides. Update canonical Design and focused tests.

Independent pre-implementation architecture/designer review found no blocker, conditional on finite applied offsets, desired/rendered position separation, deferred callback identity guards, and tests for extent changes/navigation/clamping. Absolute Y intentionally does not anchor a message identity when history is trimmed or text reflows. No disagreement remains in this bounded approach.

## Validation and rollback

Observed local developer checks on 2026-10-15, Flutter 3.47.2 / Dart 3.13.2, Ubuntu/Linux:

- `flutter pub get --offline` from `next/client`: PASS; no dependency additions.
- `dart format --output=none --set-exit-if-changed next/client/lib/chat_panel.dart next/client/test/chat_scroll_anchor_test.dart next/client/test/optimistic_send_test.dart next/client/test/split_workspace_widget_test.dart` from root: PASS, four files unchanged. An initial invocation from root using client-relative paths found no files; the corrected command above is the actual formatting evidence.
- `flutter analyze --no-pub` from `next/client`: PASS, no issues.
- `flutter test --no-pub test/chat_scroll_anchor_test.dart test/chat_scrollbar_test.dart test/chat_presentation_test.dart test/optimistic_send_test.dart test/split_workspace_widget_test.dart test/message_queue_widget_test.dart --reporter expanded`: PASS, 57 tests, independently rerun with the same result.
- `flutter test --no-pub --reporter expanded`: 472 passed, six skipped. Skips require compiled fake-injected Rust schema3/migration/socket fixtures (`SMITH_TEST_SERVICE`/`make client/test`); they are not integration acceptance. Output was retained locally at `/tmp/smith-chat-anchor-client-tests.log` (ephemeral, not a committed artifact).
- `flutter build linux --debug --no-pub`: PASS, built desktop bundle; not run/installed.
- Scoped `git diff --check` for changed tracked client/test/Design files: PASS.

AC mapping: new `chat_scroll_anchor_test.dart` covers AC1–AC3 in ordinary and independent views (empty/grow/shrink/reflow, absolute Y, bottom threshold, chat navigation, pending callbacks and finite remount). Existing scrollbar/presentation/split/queue/send tests cover interaction, clamping, submission/reconciliation and AC4 focus/selection/composer stability. Send-specific expectations were intentionally updated to preserve locked positions.

Fresh independent assurance found no concrete scroll-code blocker; it independently reran the 57 tests and whitespace checks. Its initial formal verification verdict remained incomplete while parent analyzer/full-suite/build were pending; parent results are recorded above. Six compiled integration fixtures and installed native graphical wheel/trackpad/resize behavior remain unverified. This bounded source self-check and scoped code review are not aggregate paired-package, release or deployment approval.

Rollback only this work item's edits; do not revert existing dirty changes. No persisted schema changes.
