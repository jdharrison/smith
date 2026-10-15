# Compact workspace navigation

## Scope

User-approved local UI changes: move Layout beside system status in the existing header (remove separate toolbar space); expose compact chat-title chevron navigation/context menus plus New chat and Archived chats on the right of the existing chat header. Current feature/autonomy dirty checkout is preserved; no commits, installation, live service, backend/storage changes or new capabilities.

## Reconciled plan

Independent producer, designer and architect analyses agree on UI-only reuse. Every tile, including Settings, exposes the same global layout choice beside status; one SplitWorkspace still owns grid/persistence callbacks. Remove outer toolbar, preserve stable tile/widget identity and hidden focus exclusion. Each menu has its own focus resources; hiding its tile returns focus to a visible control, never a composer.

Use the existing tile-width/text-scale compact threshold. In compact Chat only, title plus down-chevron is an accessible bounded chooser; reuse live ChatSidebar/ChatActionsMenu and revision guards. New chat and Archived chats are directly accessible at the chat header right; archive icon opens archive browser, not archive-current mutation. Suppress redundant New/archive toolbar in the compact chooser. Preserve wide sidebar behavior, Settings hiding chat controls, draft/reading-state retention and authority revalidation. The existing rail history entry may remain as an additional accessible route, not the only route. Wrap within existing headers only if required by enlarged text; no dedicated extra global row.

## Acceptance / traceability

- R1: layout is beside status inside each existing header, all four presets retain one shared owner and persistence; no outer row. Geometry/tile-scoped tests.
- R2: compact title chooser selects only originating tile, includes active chat context actions, handles empty/long-title states. Live notifier and stale-action tests.
- R3: compact New and Archived controls reuse existing callbacks; Settings excludes chat controls; wide layout unchanged except header Layout. Breakpoint/Settings tests.
- R4: focus, mounted drafts/scroll and hidden slots survive transitions; no unsolicited submissions/mutations. Existing split/restore/authority regressions.
- R5: narrow 400×400 and enlarged-text controls usable with no overflow. Widget geometry/semantics evidence.

Update DESIGN in same change. Run format/analyze, focused/new and regression widget tests, Linux debug build, then fresh independent assurance. Record exact failures without rewriting unrelated golden baselines. Native installed graphical acceptance and broader release gates remain separate.

## Header-spacing follow-up

User requested more space between system status and the layout icon. Added a 16-logical-pixel gap in the existing header; no behavior changes. Formatting and all 7 `compact_workspace_navigation_test.dart` tests passed. Source-only change, not installed.

## Active-outline follow-up

User requested removal of the unreliable/unnecessary green workspace outline. Scoped cosmetic change: every tile now uses the existing subtle neutral border; active-pane focus, composer eligibility and persisted active selection are unchanged. No additional behavior or permissions. Validation: formatting check passed; `flutter test --no-pub --concurrency=1 test/compact_workspace_navigation_test.dart test/split_workspace_widget_test.dart` passed all 14 tests. No installed app change.

## Evidence

Implemented in `next/client/lib/ui/split_workspace.dart`, `smith_workspace.dart` and `chat_sidebar.dart`. The inherited layout scope preserves the public tile builder and shared persistence callback. Added `compact_workspace_navigation_test.dart`; updated split menu lookup to be tile-scoped and a pure-shell archive lookup to target its open sidebar (the header now also has that shortcut). No main/controller/storage/backend changes.

Final parent validation from `next/client/`:

- PASS: `dart format --output=none --set-exit-if-changed lib/ui/split_workspace.dart lib/ui/smith_workspace.dart lib/ui/chat_sidebar.dart test/compact_workspace_navigation_test.dart test/split_workspace_widget_test.dart test/shell_pure_widgets_test.dart`.
- PASS: `flutter analyze --no-pub`, no issues. An intermediate missing test import after scoping the archive finder was corrected before this final pass.
- PASS: `flutter test --no-pub --concurrency=1 test/compact_workspace_navigation_test.dart test/split_workspace_widget_test.dart test/split_workspace_controller_test.dart test/workspace_restore_test.dart test/workspace_preferences_test.dart test/shell_action_authority_test.dart test/shell_pure_widgets_test.dart test/settings_view_test.dart test/boot_test.dart --reporter=expanded` — 132 tests.
- PASS: `flutter build linux --debug --no-pub`.

New tests cover shared header menus/no outer toolbar, compact chooser and current membership, context-action freshness, originating-tile New/archive access, Settings/breakpoints, hidden-anchor focus and small enlarged-text geometry. Fresh independent source review found no concrete scoped defects; it did not independently rerun validation and is not release approval.

An earlier broader agent run had three failures: the introduced archive-finder ambiguity (fixed and now passing) and two already-recorded stale-dialog wording expectations at `multi_chat_widget_test.dart:126` (unchanged). Full-suite goldens/recovery limitations from the split plan remain; no baselines were rewritten. No screenshots/native graphical acceptance, paired packaging, installation or live service/model actions were performed. Local debug build and widget evidence are not deployment readiness.
