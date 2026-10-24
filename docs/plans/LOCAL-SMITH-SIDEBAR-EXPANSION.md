# LOCAL-SMITH-SIDEBAR-EXPANSION

## Authority and bounded intent

**LOCAL_PLAN_APPROVED:** the operator explicitly requested a sidebar expanded with text, vertically top-anchored, with expansion part of the Dev tab rather than its own row. This approves local shell presentation, directly related tests/docs and focused isolated validation. Preserve all existing dirty work. No service, job runtime, preferences/schema, install, app launch, Git/remote or host effects are included.

This supersedes the fixed56px/vertically separate disclosure restrictions in the original Dev-navigation plan and the unchanged-compact restriction in the earlier pending sidebar-reference proposal. Earlier independent sidebar-reference producer/architect/designer inputs are inspected, not claimed newly executed: retain measured label fit, existing glyphs/colors,48px targets, stable mounted pages and no effect from disclosure. Reconciliation uses responsive automatic expansion rather than adding another preference or collapse mode.

## Requirements and implementation

- SIDE01: wide panes show icon+text Chat, Dev, Settings in a full-height top-aligned navigation column.184px minimum width grows for scaled text; narrow panes retain56px icon rail.
- SIDE02: wide Dev is one visual row with separately keyboard-operable48px route and trailing disclosure targets; child Projects/Agents links are indented beneath it. Root selects Dev; arrow changes links only. No standalone disclosure row.
- SIDE03: compact Dev is one48px control with code/chevron glyph, opening a bounded labelled chooser containing Dev, Projects, Agents. Selection alone navigates. No second expand tab/control row.
- SIDE04: retain route identities, lazy keyed editors/Dev form, drafts, hidden focus/tickers, per-pane/session-only disclosure, compact chat chooser, and current service authority. No new persistence or runtime work.

Write scope: `client/lib/ui/smith_workspace.dart`, one focused sidebar widget test file, canonical Design summary/details and this record. No theme/header/editor/backend changes or golden rebaselines. Focused formatting/analyzer and widget checks, then a bounded independent source review. Native graphical acceptance, whole-client/golden/build gates and installed acceptance are not established by this small iteration.

## Reference alignment refinement

**LOCAL_PLAN_APPROVED:** the subsequent operator screenshot refines the same presentation slice; exact icon matching is explicitly not required. Preserve the existing hierarchy/actions/responsiveness, while matching the visual grid: Chats label, one selected Dev surface including its chevron,2px left accent, consistent icon/text tint and columns, indented children/guide, and an8px group gap before Settings. Keep48px actionable targets, primary-foreground text, separate focus/route/disclosure semantics, and no service/persistence changes. This is a layout refinement, not pixel-exact icon or font reproduction.

**Refinement implemented; focused checks PASS.** The same exact formatting/analyzer/three-file test command recorded below was rerun on2026-10-18: formatting2 files/0 changes, analyzer no issues,27 tests passed (including the fifth shared-highlight/grid/spacing test). Scoped tracked-file `git diff --check` also passed. The independent reviewer re-inspected this refinement and found no concrete introduced UX/accessibility/reference-alignment blocker; it did not execute checks or approve graphical/installed/release behavior. Preceding26-test evidence below remains historical to the earlier snapshot. No live app, backend/runtime, persistence or host action occurred.

## Evidence

**Implemented source; focused self-check PASS and bounded independent source review PASS.** No installed/graphical/release acceptance is claimed.

- `client/lib/ui/smith_workspace.dart`: wide labelled184px-minimum navigation, measured scaled-label growth, full-height top alignment, Dev root/chevron in one row, indented children, single compact Dev chooser control. Visible text is excluded from duplicate semantics. Existing keyed lazy pages and focus/route guards remain.
- `client/test/sidebar_expansion_test.dart`:4 focused tests for geometry/semantics, disclosure-only action plus retained draft/focus, compact keyboard chooser and2× label fit.
- Canonical Design summary/details updated; no other production file changed for this presentation request. Preferences and backend/runtime are untouched.

### Exact focused checks — 2026-10-18

From `client/`, against the actual dirty worktree with existing caches and no dependency download:

1. `dart format lib/ui/smith_workspace.dart test/sidebar_expansion_test.dart` — PASS, final run2 files/0 changes.
2. `flutter analyze --no-pub lib/ui/smith_workspace.dart test/sidebar_expansion_test.dart` — PASS, no issues.
3. `flutter test --no-pub test/sidebar_expansion_test.dart test/shell_pure_widgets_test.dart test/dev_jobs_test.dart` — PASS,26 tests.

The initial same-command run passed analyzer but failed tests: navigation's compact glyph overflowed by1px because the56px width also included a1px border, and the new semantics test disposed its handle too late. Fixed the content padding to reserve the border while preserving48px targets, and disposed the test handle before verification. The final invocation above passed. No baseline failures were erased or unrelated tests edited.

A fresh independent reviewer inspected the scoped source/tests and current contracts without commands, found no introduced blocker, and passed this bounded source review. This is not independent execution of the parent checks or approval of graphical/installed/release behavior. Full navigation/golden/component/build gates were not run or rebaselined for this minimal iteration; native graphical acceptance remains pending. No app launch, installation, service/container/model action or Git mutation occurred.
