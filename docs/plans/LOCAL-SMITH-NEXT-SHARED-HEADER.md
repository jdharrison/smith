# Shared app header and Layout pointer feedback

## Scope

User clarified that only workspace bodies should tile; one app header should remain above all workspaces. User also reports sticky hover on the Layout icon after menu selection or leaving the window edge. This local source/test follow-up supersedes the compact-navigation plan's per-tile app-header placement. Preserve existing dirty work; no commits, installation, live service/model execution or preferences migration.

## Independent analysis and reconciled approach

Architect and independent designer/producer reviews identify repeated SmithWorkspace headers and nested PopupMenuButton/child IconButton ownership. The production theme gives focus and hover similar fills, so retained focus can look like stuck hover; native lost-exit behavior is not yet reproduced. Do not claim a native cause without evidence.

Extract/reuse a controller-free app header once above the grid, outside both canvas scroll views: brand, factual status, 16px status-to-layout gap and one persistent Layout trigger. Tiles retain rail, compact chat title chooser/actions, sidebar, chat/composer and Settings. Maintain stable keyed pane/body identity, neutral borders (no active outline), hidden focus exclusion, pane-local navigation and shared serialized service ownership. Preserve standalone/legacy fixtures' header explicitly if needed, but production split has exactly one. No backend or persistence-schema change.

Use one interactive Layout trigger and one stable global focus node, not nested interactive controls or per-tile nodes. Global menu does not activate a tile. Hover fill and visible keyboard focus must be distinct. Selection/cancel/pointer-exit/removal clear transient hover/pressed state; keyboard access remains Enter/Space, arrows and Escape with visible focus. Shrinking hidden active pane returns focus safely to global control, not composer. Rebuild/disposal guards remain. Avoid hiding all focus indicators to mask mouse feedback.

## Acceptance

- R1 exactly one brand/status/Layout for every preset and mixed Settings pages, header outside canvas scroll; keep16px gap.
- R2 production-theme mouse gesture reproductions separate hover from focus, menu select/cancel and window-edge exit/removal; keyboard focus/navigation works without submissions.
- R3 retained body/widget identity, drafts, reading bookmarks, hidden slots and restoration through layout transitions.
- R4 menu lifetime safe across status rebuild/disposal; no duplicate callbacks or unintended active-pane persistence changes.
- R5 narrow400×400/enlarged-text geometry and existing compact navigation/authority regressions remain usable.

Run focused tests, formatting/analyzer, broader relevant regression files and Linux debug build. Update DESIGN/ARCHITECTURE. Fresh independent review. Native Ubuntu window-manager pointer-exit/deactivation validation remains separately reported if unavailable; source tests are not proof of native graphical symptom closure. Do not regenerate unrelated goldens.

## Evidence

Implemented `ui/smith_app_header.dart` and shared composition in `main.dart`, `ui/smith_workspace.dart`, `ui/split_workspace.dart`. New `shared_header_test.dart` plus compact/split test updates assert the superseding one-header contract. Pre-fix tests reproduced four headers in 2×2 and identical production focus/hover fills. Layout now uses the native PopupMenuButton icon path, with hover/pressed fills separate from keyboard focus outline. A stable global focus scope traverses to its parent rather than trapping Tab; shrinking hidden active panes focuses the shared control, not a composer.

Final parent checks from `next/client/`:
- PASS `flutter analyze --no-pub`, no issues.
- PASS `flutter test --no-pub --concurrency=1 test/shared_header_test.dart test/compact_workspace_navigation_test.dart test/split_workspace_widget_test.dart test/split_workspace_controller_test.dart test/workspace_restore_test.dart test/workspace_preferences_test.dart test/shell_action_authority_test.dart test/shell_pure_widgets_test.dart test/settings_view_test.dart test/boot_test.dart --reporter=expanded`: 141 tests.
- PASS `flutter build linux --debug --no-pub`.
- Implementer formatting and scoped whitespace checks passed on all seven touched code/test files.

Fresh independent source review found no concrete scoped defects, including no focus trap/steal path. Its evidence-completeness concern is addressed by recording the final parent checks here; it did not independently rerun them. Native Ubuntu window-manager pointer exit/deactivation and installed graphical symptom closure remain unverified. No host UI/service/model execution, installation or full recipient package rebuild. Full-suite historical golden/recovery failures were not rerun or reset. No persistence/controller/backend changes or new permissions.
