# LOCAL-SMITH-NEXT-WORKSPACE-ENTRANCE

## Bounded intent

Explicit user request, 2026-10-15: animate individual Smith application elements after the loader so the workspace, header and sidebar do not appear all at once. Local client source, tests, fixtures, canonical documentation and local validation only. Bound to repository Smith, existing `feature/autonomy`, base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` plus the existing dirty worktree; expires at completion of this task. Preserve all existing work, including the implemented loader. No commit, branch operation, install, live service/model/provider action, remote mutation, publication or deployment.

Risk: elevated client presentation flow. No readiness, service ownership, protocol, persistence, preference schema, dependency, permission or platform changes.

## Independent analysis and reconciliation

Independent producer, architect and designer contexts inspected the existing restored workspace and component boundaries before implementation. All require one shared finite timeline, starting only after Boot and preference hydration, stable widget/controller identity, hidden-control protection, immediate recovery, instant reduced-motion settlement and no replay.

- Use a dedicated presentation scope mounted under `ConversationScreen` after its existing restoration gate, not chat/service/preference controllers. This satisfies central ownership without adding animation state to persistence.
- Adopt the designer's 620ms choreography: shared header 0–220ms (4px from above); navigation 60–280ms (6px from left); wide sidebar 120–340ms (8px from left); title 180–400ms (4px from below); transcript/Settings 240–500ms (6px from below); composer/provider/queue controls 340–620ms (8px from below). Opacity plus `easeOutCubic`; no size changes, row/message staggering, bounce, new assets or packages.
- One shared schedule covers all initially visible panes. Content exposed after completion is settled; responsive content exposed during entrance joins the remaining shared schedule, never starts its own clock.
- Block workspace pointer, focus and actionable semantics until the short entrance completes; do not buffer/replay input or autofocus afterward. Existing hidden/inactive pane protections remain. Close/Quit remains outside this gate.
- Restoration warnings and actionable errors bypass decoration; reduced motion at launch or enabled mid-entrance settles permanently. Returning to normal motion cannot replay it.
- Existing loader timing and delayed-frame guarantee remain unchanged. No blocking disagreement remains; exact motion choices are task-local assumptions, reviewable by the operator.

## Traceability / implementation

| Requirement / blocking acceptance | Task | Required evidence |
|---|---|---|
| ENT-01: first eligible restored-workspace frame only; no pre-ready/default-layout flash | Add scope under existing restoration branch in `main.dart` | Independently delayed Boot/restoration tests |
| ENT-02/03: individual groups stagger and finish within 620ms, independent of tile/history count | Shared `ui/workspace_entrance.dart`; stable wrappers in header, workspace, chat panel | Explicit-duration progress/offset tests; desktop/intermediate/split fixtures |
| ENT-04: no replay/remount on status/messages/selection/Settings/layout/resize | Preserve scope and retained content topology | Editor/pane/draft/reading-state identity and hidden-pane expansion regressions |
| ENT-05/06: reduced motion settles immediately; hidden controls inaccessible; repair immediate; enlarged layouts fit | Scoped interaction exclusions and skip-on-recovery input | Semantics, Tab/Enter/pointer, setting-toggle, recovery/disposal and 400×400@2× tests |
| ENT-07: no new API effects, unchanged loader/readiness/observation/preferences | Presentation-only ownership; no backend edits | Existing Boot, restoration, shared-header, split, connection, exit, chat and full client gates |

Self-check → fresh independent assurance/adversarial review → record source identity/results. Required gates: focused Flutter tests, non-update visual comparisons, configured `make client/test`, `flutter build linux --release --no-pub`, scoped whitespace and source-stability checks. Historical PASS is not evidence for this snapshot. Source validation does not establish installed graphical acceptance or authorize release/deployment.

Rollback: remove only entrance-specific scope/wrappers, test/fixture additions and documentation hunks; never reset dirty files or remove the loader. No data migration is involved.

## Outcomes

**Implemented source:** one `WorkspaceEntrance` presentation scope after the existing restoration gate, with stable `WorkspaceReveal` regions in the shared header, workspace and chat panel. Explicit 620ms timing, immediate/permanent reduced-motion and recovery settlement, and scoped pointer/focus/semantics exclusion. No changes to the loader, API, service or preference contracts.

Self-check: `flutter test --no-pub test/workspace_entrance_test.dart` passed 19 tests. The nine-file focused run (entrance, Boot, Boot visuals, restoration, shared header, split workspace, chat presentation, connection polling and exit guard) passed 112 tests; log `next/client/build/workspace-entrance-regressions.log`. New visual fixture generation passed 3 tests; generation is not comparison. All eight desktop/mixed-grid/compact enlarged-text PNGs were opened and inspected. Compact enlarged text retains the existing scrollable minimum canvas. Initial integration work found and corrected a test-only overly broad status lookup by scoping it to the shared header; no production behavior was changed for that assertion.

The first configured `make client/test` passed formatting, analyzer and 500 tests; Linux release compilation passed. Independent assurance reproduced one medium finding: an unavailable selected route in routed mode did not bypass decoration when error/stale flags were clear. That blocked acceptance. Remediation limits pane-level recovery inspection to visible panes and includes selected-route availability (not unrelated optional destinations). Seven retained local/cloud route regressions now cover first-frame and mid-entrance recovery, permanent settlement, optional unavailable destinations and a hidden unavailable pane; the focused entrance suite passes 26 tests.

### Final validation and independent recheck — 2026-10-15

| Gate | Command / scenario | Result / evidence |
|---|---|---|
| Focused remediation + non-update visuals | `flutter test --no-pub test/workspace_entrance_test.dart test/workspace_entrance_visual_test.dart --reporter expanded` from `next/client/` | PASS, 29 tests; `next/client/build/workspace-entrance-remediation.log` |
| Configured complete client gate | `make client/test` from `next/` | PASS, 507 tests, no skips; locked dependency resolution, 76 files formatting-clean and analyzer clean; `next/client/build/workspace-entrance-client-gate-final.log` |
| Linux release compilation | `flutter build linux --release --no-pub` from `next/client/` | PASS; `next/client/build/workspace-entrance-linux-build-final.log` |
| Independent recheck | Reviewer separately reran the 29 contract/visual checks and five anonymous in-memory route/visibility probes | PASS; original medium finding closed; no new scoped findings |
| Whitespace / stable snapshot | Scoped `git --no-pager diff --no-ext-diff --check`; SHA-256 comparison of 91 client inputs after final gates/review | PASS; no input changed. Only `main.dart` and the entrance test changed between initial and remediated snapshots |

Independent assurance returned **PASS for scoped source-assurance closure**, with no remaining code remediation. Its original missing-selected-route reproduction now settles immediately; non-active visible panes in all split presets trigger recovery bypass, while unavailable hidden panes and unrelated optional destinations do not suppress a healthy entrance. No external approval is granted.

Reviewed source identity: `next/client/build/workspace-entrance-source-manifest.json`, SHA-256 `a7a6a6a8fd8ba6517ee456aed9a7e1a2e19997b415ce599f6b632d9f7d13d912`.

- `next/client/lib/ui/workspace_entrance.dart`: `2463f18f77ee882ea7d071e0c5908c216795bebf13ba11289a412020a488672c`.
- `next/client/lib/main.dart`: `786575ddda61886a8b7b63e4ad5259792dd6757e7d7960af51e3e046f20b5a50` (includes pre-existing unrelated work).
- Built Dart AOT `next/client/build/linux/x64/release/bundle/lib/libapp.so`: `e33de6fa391e0cf181c2ddf149dbea39e2def8db26d7509c0ec07b2cf06d17ae`.
- Built native launcher `next/client/build/linux/x64/release/bundle/smith_client`: `c61dc7ad8a59082c3317a968d7fe9e84584f68f2791a93550674d687285ab236`; this launcher alone does not bind Dart application code.

Existing unrelated dirty changes, including concurrent chat reading/scroll work, remain outside this task's ownership. Logs/manifests are local ignored/generated evidence, not durable release artifacts. PNGs are retained synthetic fixtures, not native frame-pacing, window-manager/IME/accessibility or installed graphical acceptance. The Linux compilation is not a newly verified paired recipient package. No commit, installation, live service/provider/model operation, publication or deployment occurred.
