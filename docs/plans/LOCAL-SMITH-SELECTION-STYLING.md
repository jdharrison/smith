# LOCAL-SMITH-SELECTION-STYLING

## Approved scope

**LOCAL_PLAN_APPROVED:** operator explicitly requested purely UI styling: global selection should be a slightly brighter base surface, not an unreadable neon background. Scope is shared Flutter focus/text-selection fills, focused theme regression assertions and matching design documentation. Preserve all other dirty/untracked work on existing `feature/autonomy`; no commits, deployment, live service effects, behavior or capability changes. Approval expires on completion or material drift.

## Implementation and acceptance

Use existing `SmithColors.surfaceSelected` for shared `ThemeData.focusColor` and text-selection highlight. Preserve accent cursor/handles and explicit focus borders, existing selected-row styling, geometry, interaction and model/service state. This small cosmetic correction needs no architecture/capability change or workflow redesign.

Acceptance: selected/focused backgrounds are lifted teal, primary/secondary foregrounds retain at least4.5:1 contrast (existing contrast regression), cursor/focus-border cues remain visible, formatting and focused foundation/client analysis pass. No golden refresh or unrelated fixture repair. Source tests are not native graphical acceptance.

## Evidence

**Implemented source; focused validation PASS.** From `client/`: `dart format --output=none --set-exit-if-changed lib/ui/smith_theme.dart test/ui_foundation_assets_test.dart` passed with0 changes; `flutter test --no-pub test/ui_foundation_assets_test.dart --reporter expanded` passed5 tests; `flutter analyze --no-pub` reported no issues. Logs are retained under `docs/evidence/selection-styling/`.

Fresh independent read-only source assurance inspected the theme diff, test assertions, documentation and logs; no scoped blockers found. It did not independently rerun tests or perform native graphical QA. No installed changes or external effects occurred or are authorized.
