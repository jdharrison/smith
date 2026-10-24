# LOCAL-SMITH-DEV-NAVIGATION — UI self-check

**Implemented; scoped self-check passed. Combined acceptance remains blocked/not established.** This is T1 evidence, not independent assurance, native graphical acceptance, approval or release evidence. Parent owns canonical documentation and combined closeout; the other implementer owns preferences/main allowlist and their tests.

## Implemented scope

- `client/lib/ui/smith_workspace.dart`: existing 56px rail and header/theme geometry retained; selectable inert Dev root and a separate 48px disclosure directly below it. Root never opens a chooser or toggles links.
- Wide disclosure starts false per retained pane, changes links only and survives resizing/hiding without persistence. Compact disclosure uses the existing scrollable labelled `SimpleDialog`; its expanded state follows modal visibility rather than the wide flag. Compact options expose selected semantics for the current child destination.
- Dev title/tooltip/semantics and exact static placeholder, without unrelated error/guidance/retry content or a service action.
- Stable keys on every destination stack slot, with existing lazy Projects/Agents and Chat Offstage/TickerMode/ExcludeFocus retention preserved. Collapsing focused navigation returns focus to the disclosure. Modal results are invalidated when the origin page changes or its pane becomes hidden, including hide/show and away/back races.
- Existing Agents/Projects navigation tests retain guards, API counts and recovery checks. Old disclosure helpers now use `Dev navigation`, not the Dev root; preference assertions change only the current write version 3→4. No old-version read contract is changed here.
- Added seven Dev behavior tests and three bundled-Regular-font Dev visual fixtures. Existing Agents and Projects editor-only goldens and all existing shell goldens remain byte-for-byte unchanged.

## Final validation evidence

All commands run from `client`, using the cached SDK's `bin/cache/dart-sdk/bin/dart` and `bin/cache/flutter_tools.snapshot --no-version-check`, through `target/dev-navigation/ui/run_check.py`. The recorder applies a 180-second timeout, retains each attempt in a new directory and sanitizes repository/home prefixes. Exact argument vectors, exit codes and source hashes are in each `result.json`; full output is adjacent. No command timed out.

| Check/scenario | Expected | Actual/status | Record |
|---|---|---|---|
| `flutter test --no-pub test/dev_navigation_test.dart test/dev_navigation_golden_test.dart test/agents_navigation_test.dart test/projects_navigation_test.dart test/compact_workspace_navigation_test.dart test/agents_golden_test.dart --reporter expanded` | Navigation, laziness/no-API deltas, critical Agents-first→Dev→first Projects→Agents, draft/text-selection/scroll identity, per-pane disclosure, focus fences, compact400×400/2× keyboard/modal bounds, stale modal rejection and real-font images | **33 passed, zero failed/skipped** | `20261018T010129540323Z/` |
| `flutter test --no-pub test/split_workspace_widget_test.dart test/shared_header_test.dart test/shell_pure_widgets_test.dart test/shell_action_authority_test.dart test/chat_scroll_anchor_test.dart test/workspace_entrance_test.dart test/queue_exit_guard_test.dart test/new_window_test.dart --reporter expanded` | Existing retained panes, chat reading/editing, authority/close guards, shared header, entrance and session-only behavior preserved | **121 passed, zero failed/skipped** | `20261018T010130040331Z/` |
| `flutter analyze --no-pub lib/ui/smith_workspace.dart test/dev_navigation_test.dart test/dev_navigation_golden_test.dart test/agents_navigation_test.dart test/projects_navigation_test.dart test/compact_workspace_navigation_test.dart` | No diagnostics | **PASS, no issues** | `20261018T010130585909Z/` |
| `dart format --output=none --set-exit-if-changed lib/ui/smith_workspace.dart test/agents_navigation_test.dart test/projects_navigation_test.dart test/dev_navigation_test.dart test/dev_navigation_golden_test.dart` | No formatting changes | **PASS, five files/zero changed** | `20261018T010454468479Z/` |

No source drift occurred during these four final checks. Earlier format records intentionally report their own formatting mutations, not concurrent contract drift. `check-index.json` retains every attempt, including failures.

## Preserved failures and limitations

1. Initial navigation run: two **new-test finder errors** (matching Projects' title tooltip instead of a navigation control, and hidden retained panes in a semantics finder). Both corrected; no guard/assertion was removed. `20261018T005650077492Z/`; subsequent scoped runs pass.
2. Initial analyzer: two deprecation infos for new `TickerMode.of` calls. Changed to the cached SDK's supported `TickerMode.valuesOf(context).enabled`; final analyzer clean. `20261018T005821127128Z/`.
3. Existing shell visual suite: **1 passed, 19 failed** in `ui_redesign_golden_test.dart` and `workspace_entrance_visual_test.dart`. `20261018T005847434533Z/`. Failure renders preserved in `target/dev-navigation/ui/shell-golden-initial-failures.tar.gz`.
   - An isolated reconstruction uses captured pre-edit `smith_workspace.dart` with only imports redirected to current canonical dependencies; no shared production file is swapped. The first reconstruction had incorrect import paths/type identities; compile failure retained in `20261018T005953044981Z/`.
   - Corrected reconstruction also **fails** the existing `fixture-chat-1280x720` golden (`20261018T010020207792Z/`). Its actual render versus this task's render differs only inside rail rectangle `(11,284)–(64,492)`; see `shell-golden-comparison.json` and the reconstructed harness/failure images under `target/dev-navigation/ui/`. This demonstrates an inherited shell-fixture mismatch for that case, not a baseline PASS for every existing visual fixture.
   - Existing shell images were **not regenerated**, because doing so would also accept inherited rail-position changes beyond the new navigation control.
4. Existing Projects editor-only goldens: **three failures**, each865 changed pixels: `wide-1536x1024`, `compact-editor-400x400`, `ultrawide-4200x2400`. `20261018T005953782996Z/` records38 passes/3 failures across that combined run. These tests do not instantiate the shell; their implementation, tests and baseline images remain frozen by the source audit. Failure renders preserved in `target/dev-navigation/ui/editor-golden-failures.tar.gz`. No unrelated editor golden was changed.
5. Not run: full client suite, Linux build, Rust/compiled integrations, root/package gates, installed/native graphical/IME/accessibility checks, independent assurance. No live service, manager, model, provider, repository, keyring, install or graphical app launch was used. No Git mutation, remote effect or commit was performed. These omissions and existing visual failures prevent full acceptance claims; none are waived here.

## Baseline and source identity

- Verified1337-file canonical baseline identity: `8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f` (SHA256 of compact sorted-key JSON of `baseline-manifest.files`). Archive SHA256 also matches its manifest. See `baseline-verification.json`.
- `pre-edit.json` binds captured owned files to baseline hashes. Original bytes remain under `target/dev-navigation/ui/`; current UI diff is `workspace.diff`.
- `final-source-audit.json` compares every baseline file: none missing, no unexpected/frozen-file changes, no existing golden modifications. Deltas are exactly the three existing UI-owned files plus the five parallel-owner preference/main/test files. New UI tests/fixtures are separately hashed there. Production services/controllers, AgentsView/ProjectsView, theme, header and dependency inputs remain unchanged from baseline.
- Final `client/lib/ui/smith_workspace.dart` SHA256: `5138cc15c7fd97d1f3e68710649c5912a0c78315c84126e16672e0545673490d`.
- `hash-bindings.json` binds final UI source, new golden bytes, baseline identity, evidence and preserved target artifacts. These are owner-writable self-check digests, not independently retained assurance bindings.

**Quality gate:** scoped analyzer warnings/errors none; formatting clean;154 final focused tests pass. Existing shell/Projects visual gates remain failed; full combined/native/independent gates unverified. No accepted-risk decision is made.

## API/repository research notes

No dependency or custom icon assets were added. The existing Flutter `SimpleDialog` supplies its bounded scrollable children. Cached SDK source confirms `TickerMode.valuesOf` is the replacement for deprecated `TickerMode.of`. New fixtures load the bundled Material font and JetBrains Mono Regular explicitly; they do not establish production font-weight fidelity or native accessibility/IME acceptance.
