# Settings chat visibility and title bounds

## Bounded intent

User-requested client-only implementation: hide chat/history controls in Settings while retaining the Chat return rail and mounted chat/draft; use at most 40 Unicode scalar values for all displayed chat titles; reject new outbound renames over 40 runes without silently shortening editor input. No dependencies or source changes outside `next/client`. Initial message-derived title generation belongs to the parallel service workstream, not the client. Existing parallel changes are preserved.

## Implemented

- `lib/chat_title.dart` supplies the shared display formatter and 40-rune limit. `ChatSummary.displayTitle` covers sidebar, archive, action tooltips and confirmations; the workspace heading, tooltip and semantics use the same formatter. No ellipsis is appended beyond the limit.
- Raw metadata and rename editor contents remain intact. Incoming conversation titles retain the existing 256-byte wire acceptance. Only outbound rename arguments acquire the 40-rune restriction.
- Rename editor reports a Unicode scalar count, explicit limit and over-limit error, disables submission and guards dispatch. It does not apply an input truncation formatter.
- Settings removes the sidebar slot contents/width, not the mounted chat slot. Compact navigation offers Return to Chat rather than history while Settings is active.

## Independent-review remediation: historical Unicode whitespace

**Implemented client-only correction:** incoming conversation title validation now checks the explicit Rust Unicode `White_Space` boundary set rather than Dart `trim()`. U+FEFF is not whitespace under this contract, including a title consisting only of U+FEFF. The title-specific byte check allows that value through without changing `bounded()` for other protocol fields. Empty titles, leading/trailing Rust whitespace (including U+0085), prohibited control/bidi characters and titles exceeding 256 UTF-8 bytes remain rejected. Outbound rename/editor rules are unchanged.

`test/chat_title_test.dart` covers leading/trailing/sole FEFF, historical detail and rename/archive/restore receipts (including terminal poll receipts), exact metadata preservation, every Rust White_Space code point at both boundaries, nearby non-whitespace values, allowed interior spaces and the 256-byte boundary.

Remediation self-check on 2026-10-14, from `next/client`:

- `dart format lib/app_v2.dart test/chat_title_test.dart`: PASS, two files / zero changes.
- `flutter test --no-pub test/chat_title_test.dart test/app_api_test.dart test/assurance_regression_test.dart`: PASS, 34 tests.
- `flutter analyze --no-pub lib/app_v2.dart test/chat_title_test.dart`: PASS, no issues.
- `git --no-pager diff --check -- lib/app_v2.dart test/chat_title_test.dart`: PASS.

These checks address the reported compatibility defect; fresh independent review and service integration verification are not claimed.

## Self-check evidence

Commands from `next/client` on 2026-10-14:

- `dart format --output=none --set-exit-if-changed lib/chat_title.dart lib/app_v2.dart lib/ui/chat_sidebar.dart lib/ui/smith_workspace.dart lib/ui/archived_chats_dialog.dart test/chat_title_test.dart`: PASS, six files / zero changes.
- `flutter analyze --no-pub lib/chat_title.dart lib/app_v2.dart lib/ui/chat_sidebar.dart lib/ui/smith_workspace.dart lib/ui/archived_chats_dialog.dart test/chat_title_test.dart`: PASS, no issues (after correcting one braces lint).
- `flutter test --no-pub test/chat_title_test.dart test/settings_view_test.dart test/shell_action_authority_test.dart test/multi_chat_widget_test.dart test/app_api_test.dart`: PASS, 49 tests.
- `git --no-pager diff --check -- lib test`: PASS.

Targeted tests cover astral Unicode, combining scalar counting, legacy wire compatibility, raw metadata preservation, 40/over-40 rename boundaries and feedback, archive/menu/confirmation/heading displays, and narrow/desktop Settings draft/state retention. Existing Settings tests also check retained reading position and no navigation-induced service calls.

This is client self-check, not independent assurance, service generation verification, aggregate build validation or deployment approval. Canonical documentation integration remains with the coordinating workstream because this task is explicitly restricted to `next/client`.
