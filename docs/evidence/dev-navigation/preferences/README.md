# LOCAL-SMITH-DEV-NAVIGATION — PREFS / T2 / DEV06 self-check

**Implemented; developer self-check only, not independent assurance, approval, combined-gate acceptance or installed/native acceptance.** Authority: the user-scoped PREFS assignment and `docs/plans/LOCAL-SMITH-DEV-NAVIGATION.md`. Parent owns canonical documentation and broader gates; UI owner owns root/disclosure/navigation tests/goldens.

## Source scope

Exactly two production files and two test files edited:

- `client/lib/workspace_preferences.dart`: in-memory `dev`; write v4; exact version-specific v1/v2/v3/v4 destination sets. v3 still excludes Dev. Exact four-slot shape and equality are unchanged.
- `client/lib/main.dart`: only `WorkspacePresentation.setPage` allowlist, including formatter-required wrapping. No hydration, snapshot comparison, startup, session-only injection, controller or IO change.
- `client/test/workspace_preferences_test.dart`: 20 independently named version/destination cases, including isolated invalid destinations in each hidden/visible slot; exact missing/extra field rejection; legacy read preservation and explicit v4 saves; existing Linux IO regressions retained. Rich v3 fixture preserves IDs/nulls and Projects/Agents/Settings through reopen and explicit Dev save. Legacy v3 duplicate/escaped-duplicate fixtures retained alongside v4 cases. Unsupported v5 and old-version Dev files are preserved and fence writes.
- `client/test/workspace_restore_test.dart`: v1/v2/v3 hydration does not save defaults/normalize-only changes; next explicit Dev selection saves v4. Mixed four-pane hidden Dev restoration preserves conversation sharing with no duplicate detail reads; Dev-only restoration performs no eager registry reads. Existing restore fixtures and assertions remain.

No disclosure, selection/editor/prompt field was added. Four slots, layout, active tile and saved-conversation-ID/null remain the complete preference shape. Linux 4KiB/path/permissions/locking/publication/preservation machinery is untouched.

## Final validation evidence

Direct cached Dart SDK and Flutter tools snapshot; no dependency fetch (`--no-pub`), app/GUI/install/live provider/manager/keyring/repository operation. Test APIs are fakes; Linux preference tests use temporary private fixture roots. Every runner subprocess has a 180-second bound. Logs retain exact commands, working directory, UTC timestamps, exit status and sanitized output. `<cached-flutter-sdk>`, `<repo>` and `<home>` replace local paths. `run_checks.py` and `check_identity.py` are retained copies; executable originals are under `target/dev-navigation/preferences/`.

| Check | Scenario / expected | Actual |
|---|---|---|
| `format-check.log` | `dart format --output=none --set-exit-if-changed` on all four owned files; no changes | PASS, 4 files / 0 changed |
| `analyze.log` | `dart analyze` all four owned files; no issues | PASS, no issues |
| `preferences.log` | direct Flutter tools `test --no-pub --reporter expanded test/workspace_preferences_test.dart` | PASS, 57 tests |
| `restore.log` | same, `test/workspace_restore_test.dart` | PASS, 21 tests |
| `session-only.log` | same, unchanged `test/new_window_test.dart` | PASS, 15 tests |
| identity records | compare before/after final test source and frozen core | PASS, four owned hashes stable; exact main-only hunk; IO unchanged; 1,069 baseline service/installer/Dagger files unchanged |

Total final tests: **93 passed**, zero final failures. **Quality gate:** focused formatting clean, analyzer warnings/errors absent. Broader component/Linux-build/package/UI/independent assurance gates are parent-owned and not claimed here.

## Retained failures and boundary clarification

- `restore-first-failure.log`: 19 passed / 1 failed. An initial new assertion incorrectly required zero registry reads even when **other retained hidden panes themselves restore to Projects/Agents**. Existing retained-pane initialization performs one `projects_list_v2` and one `agents_list_v1` read. The approved promise is that Dev root/disclosure alone does not open/load registries, not a change to initialization of restored child destinations. Final tests separately prove zero eager reads for all-Dev and Dev/Chat/Settings panes, and exact single initial child reads with zero duplicate calls on Dev reselection for mixed Dev/Projects/Agents/Settings. No production loading behavior was changed or hidden.
- `format-check-first-failure.log`: check-only formatting reported the enlarged `setPage` set required multiline wrapping. Applied only that allowlist hunk after rereading it; no whole-file formatting rewrite. Final check passes.
- Initial formatting output is retained in `format-first.log`; no failure is erased or represented as a final pass.

## Identity and concurrent edits

Baseline manifest: 1,337 files, canonical source SHA-256 `8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f`. Its archive SHA-256 is separately `968e688390737472076ac7ccef129bb8469e7a9ccc9b0627d0c74f837bc4802d` (archive bytes are not the canonical source digest).

`before-hashes.json` records actual pre-edit owned and client production bytes. `owned-source.diff` compares those actual source bytes, not HEAD. `final-before-checks.json` and `final-after-checks.json` bind the final stable owned snapshot and core checks. Raw owned source snapshots are in `target/dev-navigation/preferences/before-owned.tar.gz` (SHA-256 `9b4ae63ed3a81bcf935c6bf956a295b4103181c4bb55bb74a1c3ad78bf5a59f8`) and `final-owned.tar.gz` (SHA-256 `cd9614c8cb04f213d69abe67fd8e0d24953934f54259ee0bb20e5c03df3110f3`).

Observed client production drift outside ownership: only `client/lib/ui/smith_workspace.dart`, the expected concurrent UI-owner root/disclosure implementation. The root was absent at first inspection and present before restore validation. Never edited that file. No conflicting external edit to owned files was observed. Main's entire remaining byte content matches its pre-edit capture. Core baseline comparison covers all 1,069 existing manifest entries under service/installer/Dagger; it does not claim no concurrent new files anywhere in the worktree.

## Compatibility and remaining boundaries

Valid legacy preferences are loaded without rewriting for normalization alone; an actual presentation save writes v4. Existing readers may reject v4; no downgrade-readability promise. Missing/archived chat repair can still legitimately save repaired presentation state, as before. Session-only startup construction/access tests remain unchanged and pass. No self-approval, commit, remote mutation, migration or live-host effect was performed. Canonical policy updates, UI-owner gates, stable combined-source gates and fresh independent assurance remain with the parent workflow.
