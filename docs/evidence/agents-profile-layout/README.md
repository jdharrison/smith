# Agents profile layout — bounded UI evidence

## Scope and source identity

**Implemented; broader acceptance BLOCKED.** [Approved plan](../../plans/LOCAL-SMITH-AGENTS-PROFILE-LAYOUT.md): existing profile UI only. No runtime, containers, new fields/CRUD, controller/API/storage/schema or navigation changes. Developer/Assurance and their existing validators, single-draft replacement guard and query-only recovery remain.

Baseline is `feature/autonomy` at `14d960975e76f906efb5e447ff3dece0e8e4385b` plus existing modified/untracked work, not HEAD alone. Pre-edit UI/test copies and raw local logs remain in `target/agents-profile-layout/`. Final reviewed source SHA256:

- `client/lib/ui/agents_view.dart`: `0d9c0ff2a915662c4f75332e1bc17e8272f07c3ef579040d4cd22521751a3c62`
- `client/test/agents_view_test.dart`: `4d7877c243041323f883d87dade34285267153f9b992966a0c3018161b61add2`
- `client/test/agents_golden_test.dart`: `e3660ede7ba7c855ec9c37e968ed26d2d3492661838a47d147278bda7061260b`

The local `final-source-sha256.txt` also binds the seven Agents goldens. This is a bounded UI identity, not a clean-source product/package manifest. No commit, branch mutation, install, app launch, live service/model/provider invocation or publication occurred.

## Self-check results

Commands ran from `client/` unless noted; Flutter3.47.2/Dart3.13.2. Validation date: 2026-10-17 local.

| Check | Actual result | Local log |
|---|---|---|
| Pre-edit `flutter test test/agents_view_test.dart test/agents_navigation_test.dart test/dev_navigation_test.dart --reporter expanded` | 16 passed, 1 failed: Dev navigation expected `projects_list_v2`, actual `projects_list_v3` | `baseline-tests.log` |
| `dart format --output=none --set-exit-if-changed lib/ui/agents_view.dart test/agents_view_test.dart test/agents_golden_test.dart` | PASS, 3 files, 0 changed | `format-final.log` |
| `flutter test --no-pub --update-goldens test/agents_golden_test.dart` | 7 presentation fixtures generated; not itself comparison evidence | `final-golden-update.log` |
| `flutter test --no-pub test/agents_view_test.dart test/agents_navigation_test.dart test/agents_controller_test.dart test/agents_protocol_test.dart test/agents_golden_test.dart --reporter expanded` | PASS, 50 tests, including 7 golden comparisons without updates | `final-focused.log` |
| `flutter analyze --no-pub` | PASS, no issues | `full-analyze.log` |
| `flutter test --no-pub --reporter expanded` | FAIL, 892 passed, 22 skipped, 102 failed | `full-flutter-tests.log` |
| Root configured `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger client/test` | FAIL: formatting111 files/0 changed and analyzer passed; compiled-fixture suite908 passed/108 failed | `configured-client-test.log` |
| `flutter build linux --release --no-pub` | PASS, Linux client bundle built; no launch or full recipient-package claim | `linux-build.log` |

Configured gate used the existing `docker-container://smith-installer-pipeline-dagger-v0.21.9` engine and checksum-verified official CLI (`609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af`), with cloud token empty, tracking disabled and plain progress. PATH resolves a beta CLI; it was queried for version only and not used for validation. Existing build/dependency caches were used; no cold-cache/hermetic claim. Dagger error-output replay is not counted as additional tests.

Focused coverage: top-left anchor at2400px, fixed gutter/reading bound, compact row/selected styling, paired/stacked field element identity and IME composition, explicit fonts including IBM Plex Mono inheritance, >1000-character exact multibyte prompt save,400px/enlarged/DPR2 access, no unsupported actions, stale instance/revision review, lost acknowledgement resolution, later typing, and retained multi-pane navigation/focus/scroll. Reviewed wide and enlarged compact images; only Agents goldens changed.

## Failures and limits

The full local suite reports failures in Projects protocol/layout/golden/view/navigation tests, workspace restore, Dev navigation and multi-chat tests. Only the Dev `projects_list_v2` versus `v3` failure was reproduced in the pre-edit control. Do not label every broader failure baseline-proven.

Configured compiled tests additionally expose compatibility failures. In particular `rust_agents_integration_test.dart:127` expects disk schema8 but receives9. That test imports the Agents data/controller layer, not the changed UI; neither its expectation nor service persistence is changed in this slice. This is a real unresolved gate failure, not a passed/skipped fixture. No unrelated test or golden was changed to mask these results.

Root service/installer/package/release gates were not rerun for this presentation-only slice. Existing product-wide acceptance remains blocked. Source widgets/goldens do not prove native Ubuntu IME/display/accessibility or installed acceptance. No persistence rollout or rollback is authorized.

## Independent assurance

Fresh reviewer session `581cf687-e5e8-4d4b-8548-fd0198935ea6` compared the exact baseline copies to final UI/tests and reviewed scope/docs/logs. It independently reran the same focused50-test suite successfully, including all seven goldens without updates. It found **no blocking defect in the approved UI delta**, confirming unchanged save/discard/review/recovery fences, stable field ancestry, retained focus/scroll and no invented execution capability.

The reviewer explicitly returned **overall assurance FAIL / release NO-GO** because broader required gates fail. It did not grant native, installed or release approval. Follow-up owner: operator, before any product acceptance/release; reconcile the broader compatibility failures in separately scoped work and rerun stable-source gates. This UI completion does not accept those risks.
