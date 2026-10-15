# LOCAL-SMITH-NEXT-SETTINGS

## Approved bounded intent

User approval on 2026-10-14: execute the outlined minimal Settings page, using the parallel UI redesign's existing separate page and gear below Chat. Repository `smith`, existing branch `feature/autonomy`, inspected base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`. Approval covers this local source/test/documentation task only, through its implementation and review; no commits, installation, service/model actions or publication. Recheck concurrent edits before integration. Risk: elevated operator navigation, with no new configuration or runtime capability.

The independent producer/architect/designer analyses and reconciliation in [UI redesign](LOCAL-SMITH-NEXT-UI-REDESIGN.md#independent-analyses-and-reconciliation) supply the read-only Settings architecture/UX contract. This task adopts that contract, not authority over the broader redesign. Fresh independent assurance follows self-check.

## Scope and ownership

Reuse the already-present `next/client/lib/ui/settings_view.dart` and `smith_workspace.dart` navigation; do not introduce a duplicate destination. Keep three read-only facts: last-observed service connection, observed model status, and build-derived version. Retain bounded-history, close-detaches and external installer guidance. No preferences, toggles, credentials, model/GPU controls, lifecycle actions, new polling, dependencies or persistence.

This task owns only minimal page accessibility, new `next/client/test/settings_view_test.dart`, this record, and a narrow additive Settings paragraph in `docs/DESIGN.md`. Existing shared shell, main/controller files, fixtures, goldens, build configuration and parallel redesign changes remain untouched. Existing page/navigation code is parallel work, not newly authored by this task.

## Requirements and evidence

| Requirement / acceptance | Task | Required evidence |
|---|---|---|
| REQ-01 / AC-01: gear directly below Chat; functional separate page | Adopt existing workspace navigation | Geometry/round-trip widget tests; selected semantics retained by existing navigation tests |
| REQ-02 / AC-02: minimal, factual read-only service/model/version display | Retain standalone page and accessible heading | Exact observed values, changed/unavailable values, no editable controls; no invented readiness |
| REQ-03 / AC-03: navigation preserves chat and has no service effects | Add isolated regression tests | Same chat widget state, draft and scroll offset; no extra app/control calls or hidden-composer submission |
| REQ-04 / AC-04: accessible, scrollable narrow/enlarged page | Add focused layout tests | 360x800 and 400x400 at 200% text; all disclosure content reachable; no overflow |

Run scoped Dart formatting/analyzer/widget tests, then existing client test/build gates from `next/` with cloud trace sharing disabled. These may use development caches and pinned build/dependency downloads, never model weights or live host service operations. Record exact results below; existing tests or other sessions' results alone are not validation of this task. Fresh independent review must inspect the bounded diff and evidence. Failed/missing required checks block release claims. Revert only this task's changes for rollback; never reset parallel work.

## Evidence

### Developer self-check — 2026-10-14

Observed by this session on the dirty parallel worktree at the base above; no install, live service/model operation or publication. The standalone page/navigation predates this task. Initial test-harness mistakes (semantics handle disposal, selecting the outer scrollable and handling a paragraph taller than the viewport) were corrected before the passing runs below.

| Command / working directory | Actual result |
|---|---|
| `dart format --output=none --set-exit-if-changed lib/ui/settings_view.dart test/settings_view_test.dart` from `next/client` | PASS, exit 0: `Formatted 2 files (0 changed)` |
| `flutter analyze --no-pub lib/ui/settings_view.dart test/settings_view_test.dart` from `next/client` | PASS, exit 0: `No issues found!` |
| `flutter test --no-pub test/settings_view_test.dart test/shell_pure_widgets_test.dart test/conversation_test.dart` from `next/client` | PASS, exit 0: `+49: All tests passed!` |
| `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/test client/build` from `next` | Initial FAIL, exit 2: parallel `ui_redesign_golden_test.dart` recovery-details fixture cast `EditableText` to `SelectableText`; 148 passed, 1 failed, build not reached. This task did not modify that test. |
| `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/build` from `next` | PASS, exit 0: built `build/linux/x64/release/bundle/smith_client`, exported `client/dist`. Dependency resolution reported seven newer incompatible package versions, not analyzer/compiler errors. |
| `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/test` from `next` | PASS, exit 0 after the parallel owner corrected its fixture: `+149: All tests passed!`. Includes configured formatting/analyzer and compiled Rust↔Dart fixture gates. |
| `git --no-pager diff --check` from repository root | PASS, exit 0, no whitespace findings |

Source SHA256 recorded for this task and the reused integration seams:

```text
b8381a58f5e00f7dfa2a7935729160b59654618ef1c228060a29092d79e21887  next/client/lib/ui/settings_view.dart
7625e35c0802df298dcf0800446600c5d818d2db53b425d2ff52156e41f936bd  next/client/test/settings_view_test.dart
55c93d6d61b8b3e5d9df2093ff5b36b900f998fc54f23696d48fffab489e97b0  next/client/lib/ui/smith_workspace.dart
c0856325f07a91a85b186fd396f25f12a454ff0c88e0d118432bf23dfe6f72a4  next/client/lib/main.dart
```

These hashes identify scoped source, not the entire concurrently edited candidate. Full client gate results apply to their respective Dagger source snapshots. Installed graphical QA, release/deployment and the broader redesign acceptance remain separate.

### Independent assurance

Fresh reviewer session `6adfb627-09c3-4db5-8176-ee651ce3542b` found no blocking code defect in the bounded change. Initial verdict was evidence-incomplete pending retention of the above results. Nonblocking notes: the new round-trip test exercises idle rather than in-flight work; keeping the same mounted controller/panel supports pending-work preservation but is not a dedicated pending-navigation test. The parallel untracked UI tree prevents reconstructing this task's tiny heading-only delta from Git alone; pre-edit inspection and this ownership record identify it. After inspecting the retained commands/results and independently matching all four scoped hashes, the same reviewer closed the evidence finding: **PASS-WITH-RISK for the bounded local Settings implementation; no remaining blocking findings or required remediation in this scope**. The nonblocking limitations above remain explicit, not accepted on the operator's behalf. No release/deployment approval.
