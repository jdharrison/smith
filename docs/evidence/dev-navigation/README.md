# Dev root and collapsible child navigation

**Implemented. Bounded feature SOURCE PASS; overall assurance FAIL / combined quality BLOCKED / release NO-GO.** The [approved additive plan](../../plans/LOCAL-SMITH-DEV-NAVIGATION.md) implements the requested hierarchy without completing or replacing the separate Agents work:

```text
Chat
Dev
  Projects
  Agents
Settings
```

## Behavior and scope

- Dev selects a static root: “Development workspace placeholder. Use Projects or Agents from Dev navigation.” No Run, dashboard, readiness/error/retry action or service-dependent state.
- A separate48px chevron controls child navigation. Existing56px icon-rail width, theme/fonts/header and content geometry remain. Controls are stacked vertically, not squeezed into overlapping targets. Wide disclosure shows/hides Projects/Agents links; compact disclosure opens the labelled bounded chooser. Root activation never toggles disclosure.
- Collapse changes links only, not the current page, drafts, selection, scroll, uncertain writes or accepted work. Wide disclosure begins collapsed, stays per-pane/session-local and is not persisted. Selected semantics identify the actual destination; disclosure owns expanded semantics. Hidden links/pages cannot retain focus; stale chooser results cannot revive a changed/hidden origin.
- Stable keyed lazy Chat/Projects/Agents page slots preserve editor identity, including Agents-first → Dev → first Projects → Agents. Dev/disclosure alone does not load registries or cause an incremental API effect. Existing background observation/work and ordinary Settings reads remain unchanged.
- Presentation preferences retain their exact shape and IO machinery. Write v4; strictly read v1 Chat/Settings, v2 adds Projects, v3 adds Agents, v4 adds Dev. Reject Dev in older versions and unknown v5. No disclosure/editor/prompt field, normalization-only load rewrite or session-only preference access. Older clients may reject v4; no downgrade-readability promise.
- Exactly three production files changed: `client/lib/ui/smith_workspace.dart`, `client/lib/workspace_preferences.dart`, and only the destination allowlist in `client/lib/main.dart`. No service/API/disk8, controller/editor, preference IO, native dependency, host-root or execution change.

## Identity and preservation

| Record | Binding |
|---|---|
| Worktree / HEAD | Existing dirty `feature/autonomy`; `14d960975e76f906efb5e447ff3dece0e8e4385b`, not HEAD alone as baseline |
| Actual pre-edit source/policy baseline |1,337 files; canonical SHA256 `8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f` |
| Baseline archive | `target/dev-navigation/baseline-source.tar.gz`; SHA256 `968e688390737472076ac7ccef129bb8469e7a9ccc9b0627d0c74f837bc4802d`; prior evidence excluded |
| Final executable core |1,296 records/54 untracked; SHA256 `b86b9d04d2867a23994b142d3850f1b4ea0a5a72c6df7f1d2321ec2f2a180777` |

[Baseline manifest](baseline-manifest.json), [source binding](validation/final-source-binding.json), [independent report](REVIEW.md) and [independent seal](review-closeout/seal.json) distinguish source/evidence selectors and dirty/untracked inputs. Observed stability is not protection against later edits or clean-source release provenance. Prior Agents/Projects/default-project reports remain immutable historical evidence, not passes for this combined candidate.

Twenty-three existing shell PNGs were updated only after exact pixel checks:159,815 changed pixels, all within the existing56px navigation rails, with four disjoint masks in mixed panes and **zero outside**. The requested hierarchy composes with the in-progress Agents grouping; not every old expected rail mismatch is attributed solely to the new button. Initial mismatch/control evidence is preserved. All13 Agents/Projects editor-only PNGs remain unchanged; three Dev-only bundled-font fixtures were added. [Per-image ledger and proof](ui/visual-rebase/RESULTS.md), independently recomputed in [pixel recheck](review-closeout/pixel-recheck.json).

## Actual validation

Exact sanitized commands/environments/timestamps/exits/budgets/log digests, source/failure archives and artifact modes are retained in [initial parent report](validation/RESULTS.md), [observed component gates](validation/observed-gates.json), and the **later actual-root corrective record** [root-original-results.json](validation/root-original-results.json). The corrective record supersedes only the initial report's copied-module root-status statement; earlier records were not rewritten.

| Gate | Observed result |
|---|---|
| One focused navigation/preferences/restoration/retention/golden/queue/authority run |305 passed, zero failures/skips |
| Full formatting / analyzer |PASS;111 files/zero format changes, no analyzer issues |
| Configured Linux client build |PASS; private desktop/helper export, not a full package |
| Full native cached-SDK Flutter suite |978 passed /4 failed /22 explicit compiled-fixture skips |
| Configured `make client/test`, current-source compiled fixtures |989 passed /15 failed /zero skips |
| Actual repository/original-module `make test build`, one real attempt |FAIL exit2 in91.45s: one service unit vertex143 passed /20 failed /two controlled fixture ignores; build not reached |
| Full-package verify / actual-package helpers |NOT RUN; no completed current full-package build |
| Diff whitespace check |PASS |

The actual-root Rust summary appears twice through Dagger failure replay; it is counted **once**, not doubled. Earlier copied-module root discovery exposed example functions and stopped before product gates; that preserved isolation/harness failure is not an additional real-root test execution. The corrected attempt used the actual source/module and official checksum-verified Dagger v0.21.9, existing pinned engine and finite budget. Setup/dependency/build caches and configured isolated egress were used; no cold-cache/hermetic claim. No graphical/product launch, installation, commit/branch/remote mutation, live manager/provider/model/keyring/repository operation or accepted risk occurred.

Required full-client failures are three frozen Projects label goldens (865 differing pixels each), one compact multi-chat finder case, eight compiled Projects cases with disk7 expectations/fixture exits under disk8, and three Rust socket migration revision cases. Current Projects actual-image hashes match preserved handoff failures; this establishes handoff equality, not arbitrary historical baseline blame. Real root failures concern current service migration/capacity/state fixtures; they were not fixed or suppressed here. Root build was not reached, and original exports remained unchanged/stale. A private standalone Linux client export is not a current paired installer/package acceptance artifact.

## Independent assurance and retained failures

[Independent closeout](REVIEW.md) verifies the three production deltas, full core selector membership/bytes/modes/tracking state, tests/probes, retained gate logs, exact image masks and93 Linux export file identities. No feature-owned correctness defect was found. It retains separate execution ownership:

- The first fresh reviewer executed keyboard/mouse, compact/modal, strict-version, editor/uncertainty and four-chat/provisional-work probes. Corrected adversarial5 cases and retention2 cases pass. Its focused run had291 passes/14 safe-store ancestor-permission fixture failures; its initial semantic-handle leak and omitted Settings-read expectation also remain recorded.
- A fresh closeout context inspected/rehashed those artifacts, independently recomputed pixels and source membership, audited parent results, and reran **only57 preference cases**, all passing using routine private `/tmp` fixtures. No host permissions or product IO checks were weakened. Earlier failed invocations remain failed, not relabelled.
- UI and preferences owner evidence retains initial finder/format/analyzer/restore assertion failures and fixes: [UI self-check](ui/RESULTS.md), [preferences self-check](preferences/README.md). Earlier owner statements about untouched shell images describe their pre-rebase phase; the later targeted rail-only ledger binds the final images.
- Two reviewer/validator sessions reached model context limits after recording work. Their retained outputs were inspected and closed out by a fresh independent context; no duplicate backend root run or fabricated final message is inferred from those interruptions.

**Source-review PASS is not overall assurance or ship acceptance.** Overall review remains FAIL / combined BLOCKED / release NO-GO. Responsible owners must reconcile full-client/root failures, then obtain fresh stable combined-source component/root/build/package evidence and independent assessment. Native Ubuntu graphics/IME/accessibility, installed migration/restore, live-host/model/provider acceptance, clean-source provenance, SBOM/imported-icon rights, publication/deployment and exact human signoff remain separate gates.

## Visual fixtures

[Test-rendered wide Dev root](../../../client/test/goldens/dev-navigation/wide-expanded.png), [compact enlarged root](../../../client/test/goldens/dev-navigation/compact-root-text2.png), and [compact enlarged chooser](../../../client/test/goldens/dev-navigation/compact-chooser-text2.png) use bundled Regular fonts. They are widget captures, not a native GUI session or font-weight/accessibility fidelity proof.
