# LOCAL-SMITH-DEV-NAVIGATION — parent validation

**Implemented validation result: focused PASS; combined BLOCKED.** No acceptance, waiver, independent assurance, release or installed/native graphical claim. No product source, test, golden, canonical policy or prior evidence was edited.

## Observed gates

| Check | Actual outcome |
|---|---|
| One focused invocation: Dev/prefs/restore/navigation/current Dev and rebased shell goldens/session-only/retention/focus/queue/authority | **305 passed, 0 failed, 0 skipped**, exit 0 (`focused-safe-temp.*`). Counts are not added across retries or prior owner suites. |
| Full Flutter formatting | **PASS**, 111 files, 0 changed (`full-format.*`). |
| Full Flutter analysis | **PASS**, no issues (`full-analyzer.*`). |
| Full native cached-SDK tests | **978 passed, 22 skipped, 4 failed** (`full-native-tests-safe-temp.*`). Missing compiled fixtures were explicitly skipped, not acceptance. |
| Configured `make client/test`, current source and compiled fixtures | **989 passed, 0 skipped, 15 failed**, Make exit 2 (`client-test-configured-module.*`). No old host binary supplied. |
| Required Linux `make client/build` | **PASS**, exit 0, bounded 600 seconds; actual ELF desktop and paired helper exported privately (`client-build-configured-module.*`, `linux-client-export.json`, `linux-artifact-file.*`). Standalone client bundle is not a full package. |
| Root `make test build`, once only | **BLOCKED before component execution**: private copied module discovery exposed example functions and rejected `test`; Make exit 2. No root rerun. Neither prior instruction-lint failure nor any backend pass is inferred. |
| Full-package verification / actual-package helpers | **NOT RUN**: no completed current full-package build. Old host exports are stale for this validation. |
| `git diff --check` | **PASS**. |

## Exact remaining failures

Native full tests fail:

- `projects_layout_golden_test.dart`: `wide-1536x1024`, `compact-editor-400x400`, `ultrawide-4200x2400`.
- `multi_chat_widget_test.dart`: `multi-chat small 400x400 2x Settings and drawer preserve draft without overflow`; finder/`ensureVisible` failure. No baseline attribution is made.

The compiled full-client gate additionally fails all eight Projects integration cases and three Rust socket migration cases (configuration revisions `1`, `9007199254740992`, `18446744073709551615`). Exact names, skip reasons, authoritative terminal counts and cache observations are in `observed-gates.json`; raw logs preserve detailed expected/actual values. The Projects logs include expected disk version 7 / actual 8 and fixture exit 101; socket cases include revision drift. These failures were not fixed, suppressed or called baseline failures merely because source files were unchanged. No Agents integration failure or skip was reported by this completed full-client invocation.

### Frozen visual proof

`visual-binding.json` records baseline and current hashes for **all 13 frozen Agents/Projects editor PNGs**. All remain unchanged. Each of the three Projects golden failures differs by **865 pixels** and its actual PNG hash exactly matches the preserved UI handoff failure archive. This establishes that those renders were already present at handoff, without claiming that arbitrary earlier baseline execution passed or failed. No editor goldens were regenerated.

All 23 current rebased shell PNG hashes bind to the owner's navigation-only pixel-proof ledger and to their old baseline hashes. Their original suites and the three Dev goldens passed in the single focused invocation using the normal comparator. The owner's geometry proof is referenced, not relabelled as independent assurance.

## Execution, isolation and preserved attempts

- Exact sanitized argv, complete allowlisted environment, UTC start/end, timeout, exit, elapsed time, log SHA256 and core before/after manifests accompany every recorded attempt. Raw combined stdout/stderr is retained with only local repository/home prefixes sanitized. `final-results.json` indexes outcomes.
- Native commands used cached `dart` and `flutter_tools.snapshot --suppress-analytics --no-version-check`, with `--no-pub`; no native SDK/pub download. Initial private-cache tests stopped for missing generated `package_graph.json`; separately named retries copied that metadata only. The next attempts exposed unsafe ancestor permissions in repository-rooted private preference fixtures; final native attempts used isolated system temporary fixtures. Every earlier failure remains intact. No product permission check was weakened or host directory permission changed.
- Root and initial component attempts used a byte-bound private source copy to contain writes. Read-only discovery proved copied-module example functions versus the expected Smith functions from the original configured module. The final component checks used the original Makefile/module/source and an evidence-only `DAGGER` wrapper that rewrites only the client export destination into the new validation directory. Exact original CLI invocation is printed into the raw log. The root attempt was not repeated.
- Official Dagger v0.21.9 CLI hash matched `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af`; existing engine running/image checks passed. No PATH Dagger, beta fallback or engine/configuration change. Configured isolated dependency downloads occurred. Debug fixture compilation and full tests executed; release-helper dependencies used content-addressed cache where logged. Fresh invocation does not mean cold-cache execution. Internal debug fixture was not exported, so no independent ELF hash is claimed for it.
- No timeouts or cancellations occurred. Root component stages and all root build prerequisites were not reached. No full package was produced. No app/GUI launch, install, live manager/provider/model/keyring/repository effect, commit or remote mutation.

## Stable source and artifact binding

`final-source-binding.json` and `core-manifest-final.json` bind **1,296 executable-core inputs, including 54 untracked files**, before and after all checks:

`b86b9d04d2867a23994b142d3850f1b4ea0a5a72c6df7f1d2321ec2f2a180777`

No core drift; the private snapshot core matches exactly. Parent-owned changing policy/evidence is excluded from executable core and the copied policy bytes are separately bound in `snapshot-binding.json`. Relative to the verified 1,337-file baseline (`8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f`), production deltas are exactly `client/lib/ui/smith_workspace.dart`, `client/lib/workspace_preferences.dart`, and `client/lib/main.dart`. No service/API/platform-controller edit follows from this slice.

`target/dev-navigation/validation/source-snapshot.tar.gz` retains the validated source/policy snapshot. `current-visual-failures.tar.gz` retains current failure PNGs. The Linux bundle remains under `target/dev-navigation/validation/source/client/dist/`; every exported artifact is hashed in `linux-client-export.json`. `evidence-index.json` binds this new bundle's reports, recorders, logs and source/failure archives. These are parent-owned validation digests, not independent assurance or human approval.

**Quality gate:** focused tests, formatting, analysis and Linux build PASS; required full-client tests FAIL; root/full-package gates incomplete. Combined validation remains BLOCKED with no waiver.
