# LOCAL-SMITH-HIDE-AGENTS-NAVIGATION

## Approved local scope

The operator clarified: “ignore the dev/run thats being handled by another. just hide agents option/tab, temporarily. well be back to this soon.” This authorizes only reversible local navigation presentation edits, affected tests/docs and isolated source checks. Work stays on the existing `feature/autonomy` bootstrap branch (HEAD `14d9609`) and preserves all prior dirty/concurrent work. No commits, remote actions, app launch, deployment, host/container actions or accepted risk are authorized.

## Reconciliation and acceptance

Earlier independent producer, architect and designer analyses agreed to preserve Agents definitions/editor/routes/preference compatibility and hide both wide and compact navigation entry points. Their run-admission questions are superseded by the explicit exclusion of Dev/Run; no implicit Developer choice or admission change is made. Parent reconciliation selects a single private, nonpersisted visibility constant in the shell. Existing/restored `agents` pages remain valid and visible if already selected; there is no redirect or preference write.

- AC1: Expanded sidebar and compact Dev chooser omit Agents, including keyboard/semantics entry points; Projects and Dev navigation remain usable.
- AC2: Agents page, saved profiles, controllers, keyed retention and route/preference compatibility remain intact. Re-enable both entry points by changing one constant.
- AC3: No Dev/Run, agent selector, service, protocol, runtime, persistence or provider changes.

Owner: parent agent. Risk: bounded UI presentation only. Evidence: focused formatting/analyzer/widget checks and fresh independent source review; no golden refresh, deep suites or graphical/runtime acceptance claim. Full candidate/release gates remain separate.

## Evidence

**Implemented temporary hiding; focused source self-check PASS.** On 2026-10-18, from `client/`, the following exact command completed successfully against the existing dirty working tree:

```sh
dart format lib/ui/smith_workspace.dart test/sidebar_expansion_test.dart test/agents_navigation_test.dart && flutter analyze --no-pub lib/ui/smith_workspace.dart test/sidebar_expansion_test.dart test/agents_navigation_test.dart && flutter test --no-pub test/sidebar_expansion_test.dart test/agents_navigation_test.dart
```

Retained tool output (excluding the shell's unrelated pyenv initialization notice):

```text
Formatted 3 files (0 changed) in 0.07 seconds.
Analyzing 3 items...
No issues found! (ran in 2.0s)
00:04 +10: All tests passed!
```

All 10 selected widget tests passed. They cover wide/compact hiding, remaining navigation/focus/draft behavior, programmatic Agents access, four-pane editor retention and v3 restoration/session-only isolation. No live service/container action, provider call, graphical launch, dependency download or golden refresh was performed.

Source SHA256 binding after checks and before independent review:

```text
fef14bd33f86bd5ae89aa70ae8a1766d9bc81674443f28d3ebb20b95005a35d0  client/lib/ui/smith_workspace.dart
4b56b71a6d3e502e3a85d46b9d2142511ebf1ccc1e6a88d15d25199608117e10  client/test/sidebar_expansion_test.dart
484cf27b454f66f4a9c094788b5648dcccc84676864c63339230f31c4b8a84a0  client/test/agents_navigation_test.dart
```

From repository root, `git --no-pager diff --check -- client/lib/ui/smith_workspace.dart client/test/sidebar_expansion_test.dart client/test/agents_navigation_test.dart docs/DESIGN.md docs/DESIGN_DETAILS.md docs/plans/LOCAL-SMITH-HIDE-AGENTS-NAVIGATION.md` produced no diagnostics. Existing tracked/untracked work means the full Git diff is not this slice's diff.

Fresh independent read-only assurance (`8594d872-e302-4226-8a96-bd5ab2a03ef8`) found no blocking source defect and confirmed both entry-point guards, retained route/editor/preference support and independence from Dev/Run. Its initial formal evidence-completion finding was that this record still said pending; this section now retains the exact observed output and source binding. Review did not independently execute tests. Native graphical/accessibility acceptance, unrelated legacy/golden/full suites and whole-candidate/release gates remain unverified; no release or deployment approval is claimed.
