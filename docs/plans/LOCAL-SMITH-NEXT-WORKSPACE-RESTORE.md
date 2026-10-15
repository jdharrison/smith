# Workspace restoration

## Approval and scope

User requested remembering split setup/layout/pages between sessions, extending the local split experiment on `feature/autonomy`, base `2cc71a9` plus existing dirty work. This session covers bounded local source edits and isolated tests only, no commits, installation, live service or model actions. Preserve unrelated work. Persistence is elevated; independent architect and designer/producer analyses were obtained before implementation.

## Reconciled implementation contract

Persist version 1 JSON with layout (`1x1`, `2x1`, `1x2`, `2x2`), activeTile, and exactly four row-major slots containing conversationId (protocol ID or null) and page (`chat` or `settings`). Preserve hidden slots. No drafts, titles, transcript/editor/scroll state, pending operation identities, readiness, model/service preferences or credentials. Save selection promotion and repair, not every status/draft update.

Production store is wired only at main(), injectable constructors default to no persistence so tests do not touch host state. Destination is XDG_STATE_HOME/smith-next/client/workspace.json, falling back to HOME/.local/state/smith-next/client/workspace.json. Nonempty relative roots are invalid. Bounded 4 KiB load/encode, strict schema. Missing preferences retains existing first-use defaults. Invalid/unsupported/inaccessible preferences preserve bytes and disable saving for that session with nonmodal warning. Atomic same-directory replacement, serialized latest-snapshot saves, private files, exclusive session ownership. Implement race-resistant no-follow directory-relative filesystem access for Linux; do not claim path checks alone prevent symlink races. No directory fsync/power-loss durability guarantee unless specifically implemented/tested.

Restore once after successful authoritative Boot sync and before workspace interaction; suppress saves until loaded/applied. Existing nonarchived IDs select shared controllers; missing/archived IDs become pane blanks, preserving page, with concise notice. Transient detail failure is not deletion. Saved null overrides automatic initial chat choice. Restore all slots and visible detail through shared serialized owner, no mutation/replay/additional heartbeat. No auto-focus into restored composer. Failed save leaves UI usable and truthfully warns that arrangement may not survive restart. No implicit reset of corrupt preferences.

## Reconciliation

Architect recommends safe Linux directory-relative primitives and ownership locking rather than race-prone path checks: required storage implementation boundary. Designer recommends explicit reset UI for corrupt data: defer reset for this small slice; preserve data and report session-only operation, never silently overwrite. Both require one-shot restoration before interaction and no persistence of unsent content. First-use default differs from explicit saved blank; test both.

## Traceability

| Requirement | Change | Blocking evidence |
|---|---|---|
| R1 Restore all layouts/active tile/pages | Injected preferences + controlled grid/page inputs | Round-trip/restart widget tests |
| R2 Hidden slots and shared saved-chat identity | Four-slot model and post-Boot selection restore | Shrink/restart/expand, duplicates, null and missing IDs |
| R3 Safe bounded local storage | Linux private atomic store + exclusive owner | Size/schema/corruption/symlink/competing owner/order tests |
| R4 No authority or content persistence | Presentation-only snapshot and read-only hydration | No-submit/config/lifecycle assertions, schema tests |
| R5 Race/failure resilience | Restore/write gating, serialized saves, warning | Delayed load/save, disposal, failure tests |

## Validation and rollback

Run formatting, analyzer, focused store/restoration tests, existing split tests and Linux debug build. Record broader existing suite limits honestly. Fresh independent assurance is required. Rollback removes only this slice's code; do not delete operator preferences or reset existing dirty work. No source test is installed-host or graphical acceptance.

## Evidence

Implemented in `next/client/lib/workspace_preferences.dart`, `workspace_preferences_io.dart`, `main.dart`, `workspace_controller.dart`, `ui/split_workspace.dart` and `ui/smith_workspace.dart`, with new storage and restoration test files. No dependency/backend changes or live preference/service access were performed; storage tests use temporary fixtures.

Post-remediation validation from `next/client/` in this session:

- PASS: `dart format --output=none --set-exit-if-changed lib/main.dart lib/workspace_preferences.dart lib/workspace_preferences_io.dart lib/workspace_controller.dart lib/ui/split_workspace.dart lib/ui/smith_workspace.dart test/workspace_preferences_test.dart test/workspace_restore_test.dart`.
- PASS: `flutter analyze --no-pub`, no issues.
- PASS: `flutter test --no-pub --concurrency=1 test/workspace_preferences_test.dart test/workspace_restore_test.dart test/split_workspace_controller_test.dart test/split_workspace_widget_test.dart test/workspace_controller_test.dart test/boot_test.dart test/settings_view_test.dart --reporter=expanded`, 96 tests. Includes 26 store and 15 restore tests covering actual widget disposal/recreation, all layouts, hidden slots/pages, shared selections, explicit blanks, repairs, failure/late loading, write coalescing, draft exclusion, symlink/permissions/corruption and competing owners.
- PASS: `flutter build linux --debug --no-pub`. This is a Flutter debug build, not paired packaging, installed-host or native graphical acceptance.

Fresh independent assurance found startup repairs were initially not saved and canonical docs still excluded persistence. The repair regression failed before remediation; valid loaded snapshots now compare against canonical restored state and queue changed repairs after hydration, without writing unchanged/missing/failed loads. Updated restart tests verify a missing/archived chat becoming available again does not resurrect a repaired selection. Canonical ARCHITECTURE/DESIGN now reflect the extension. Fresh re-review cleared both findings and identified no remaining concrete scoped blocker; this is not release/deployment approval.

The full client suite was not rerun for this extension. Broader presentation/recovery/golden failures recorded in the split experiment remain outstanding; no baseline changes were made here. Adversarial same-UID races, native window-manager termination and installed graphical/IME acceptance were not validated. File fsync plus atomic rename is not directory-fsync/power-loss durability, and abrupt process exit may lose the latest preference. Native filesystem operations do not have hard deadlines. No installed app was changed.
