# Split workspace experiment

## Scope and authorization

Local source/test experiment requested and approved in the current conversation ("lets try this out"), on the existing `feature/autonomy` checkout based on `2cc71a9`, including its pre-existing dirty multi-chat/UI work. Approval covers this bounded editing/isolated validation session only; no commit, branch change, installation, live service/model access or publication. Existing work is preserved. This is elevated client/operator-flow work, not a backend concurrency change.

## Reconciled proposal

Duplicate the complete existing SmithWorkspace shell, not the app/Boot/service owner. Fixed equal columns × rows layouts: 1×1, 2×1, 1×2, 2×2. One window-level labelled layout selector. Four stable process-local slots retain selection, Chat/Settings navigation and per-conversation scroll through hiding/rearrangement. Each tile retains the existing compact-history behavior. Minimum usable tile bounds with scrollable grid on small windows; no shrinking controls/text to fit.

One shared WorkspaceController/API/reconciliation queue, Boot coordinator and heartbeat. Saved conversation IDs share one controller and draft. Each slot has its own unsent New chat, promoted in place on accepted submission. Only the focused tile's composer can edit/send; clicking a composer activates it. Duplicate views synchronize text/history but retain independent scroll; a send scrolls its originating view only. Hidden tiles exclude focus. Layout changes do not submit, cancel, dispose shared controllers or create saved chats. Archive/delete repairs all affected selections, including hidden slots. Service admission (four outstanding, one per chat, one inference) and protocol remain unchanged.

No Dev/terminal execution, arbitrary splits, docking, draggable sizing, durable layout preferences, new dependencies, inference parallelism or service lifecycle changes.

## Independent analysis and reconciliation

Independent producer, architect and designer source analyses were obtained in this session before implementation.

| Question | Views | Resolution |
|---|---|---|
| New chat | Producer/designer prefer shared blank for smallest scope; architect recommends slot-owned blanks for independent instances. | Slot-owned bounded blanks: independent New chat is essential to duplicated workspace behavior; saved chats still share drafts. Test promotion and unknown acknowledgements. |
| Small window | Existing single-shell 400×400 tests do not prove four tiny tiles usable. | Minimum-size scrollable grid; preserve requested preset and reachable controls. |
| State ownership | All identify coupled global selection and controller-owned scroll. | Tile-local selection/view state; shared service and saved-chat controllers. |
| Layout reduction | Discarding hidden tiles could lose state. | Retain four stable slots for process lifetime; hide/exclude focus rather than delete. |
| Same-chat sends | Multiple observers currently auto-scroll. | Origin-only send scroll and synchronous shared send reservation; focused composer. |

## Traceability and blocking acceptance

| Requirement | Implementation | Evidence |
|---|---|---|
| R1 Complete shells in all four layouts | Outer grid reusing SmithWorkspace | Widget tests for grid presets and complete navigation |
| R2 Independent selection, Settings and reading positions | Slot handles and tile-local view state | Same/different-chat navigation and layout-transition tests |
| R3 Shared same-chat draft; no duplicate submission | Shared saved controller, focused editors, serialized reservation | Shared draft/send/focus tests; API max-active assertion |
| R4 Independent unsent chats with safe promotion | Slot-owned blanks, generalized correlation and synchronization | Controller tests for promotion, rejection/uncertainty and mutation repair |
| R5 No lifecycle/ownership changes | One owner and no backend edits | Shared access/disposal tests; existing recovery suites |
| R6 Small windows and enlarged text | Minimum tile canvas + scrolling | Overflow/focus tests; visual fixture evidence where available |

## Validation / rollback

Run available Flutter formatting, analysis and targeted tests first; broader client suite/build using repository tooling where feasible. Never substitute installed-host inference for fake validation. Record exact results and missing evidence below. Obtain fresh independent assurance after changes. Rollback is removal of this experiment's scoped edits only, not resetting the dirty checkout. No source test proves installed graphical/IME acceptance or authorizes deployment.

## Evidence

Implemented in `next/client/lib/ui/split_workspace.dart`, `main.dart`, `chat_panel.dart` and `workspace_controller.dart`, with two new split-specific test files. No backend or dependency changes were made for this experiment.

Validation in this local session (2026-10-14), from `next/client/`:

- PASS: `dart format --output=none --set-exit-if-changed lib/main.dart lib/chat_panel.dart lib/workspace_controller.dart lib/ui/split_workspace.dart test/split_workspace_controller_test.dart test/split_workspace_widget_test.dart`.
- PASS after remediation: `flutter analyze --no-pub` — no issues.
- PASS final: `flutter test --no-pub --concurrency=1 test/split_workspace_controller_test.dart test/split_workspace_widget_test.dart --reporter=expanded` — 22 tests.
- PASS final: `flutter build linux --debug --no-pub` — debug Flutter bundle built, not installed or launched; not a packaged paired-service release.
- PASS: scoped `git diff --check` for experiment Dart files (new files also formatted/analyzed).
- Earlier seven-file regression selection passed 88 tests before the two bookmark regressions were added. The final same selection (split controller/widget, workspace controller, multi-chat widget, optimistic send, connection review regression, shell action authority) yielded 88 passed / 2 failed. Both failures at `multi_chat_widget_test.dart:126` expect `Action unavailable.`; the current unchanged dialog instead displays `This action is no longer available. No change was sent.` The preceding no-dispatch assertion passes. These are recorded rather than changing another workstream's dialog/tests.
- Full `flutter test --concurrency=1 --reporter=expanded` run before final bookmark remediation: 256 passed, 4 skipped, 17 failed. Failures include presentation goldens and legacy recovery setups looking for absent `Retry observation` tooltips (`conversation_test.dart:592`, `chat_presentation_test.dart:189`). Independent triage reproduced the two expired recovery setup failures and sampled a golden with obsolete version/refresh header content, which bypasses the split grid. No complete clean-baseline comparison was performed; not all failures are attributed. No golden baselines were updated.

Independent assurance identified a real split-specific reading-position defect: hidden/rearranged panes could lose offsets when temporarily clamped by a larger viewport. Two added tests reproduced expected 84.6 versus actual 0 before remediation. The implementation now retains desired bookmarks independently of viewport correction; both regressions pass. Fresh source re-review found that remediation satisfactory and no remaining concrete split-specific defect, but correctly withheld overall validation/release signoff because the broader suite remains failing.

Limits: no native graphical/IME manual acceptance, split visual captures, Dagger paired build, installed-host changes or real inference. This is a locally built experiment with passing focused checks, not full acceptance, release readiness or deployment approval. Existing dirty work and generated test-failure artifacts were not reset.
