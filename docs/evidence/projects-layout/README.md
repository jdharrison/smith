# Projects interior layout — local source evidence

**Implemented, configured local gates PASS, fresh independent source/layout assurance PASS.** This is the approved existing-field layout slice in [plan section0](../../plans/LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT.md#0-active-approved-slice), not the deferred full-field handoff or native/installed acceptance. The operator approved matching the reference's interior organization using current components, with no redesign or outer-UI changes.

## Scope and identity

- Projects interior only: Projects/New/search/dense saved-Name cards and derived repository counts; local editor actions → Basic Information (existing Name) → Agent/OpenCode → Repositories. Field pairs stack when needed. Compact panes drill in/Back and retain drafts, text selection, disclosures and independent scroll bookmarks.
- Return reopens a retained editor without a matching saved card, including new/missing/filtered/write-blocked cases. New remains a guarded replacement action; Keep editing returns to the retained form. Tab/Shift+Tab reveals focused controls through long-list wrap.
- Two newly authored local SVGs, exact wrapper/package/provenance updates;26 exact UI assets, not a wildcard. No editable ID, description, prompt, read-only, Duplicate or execution features.
- Shell/sidebar/header/navigation/theme/fonts/Chat/Settings, service/model/controller/API/preferences and App3/Core2/disk6 remain unchanged against the actual baseline. Prior uncommitted work was preserved; HEAD alone is not this task's baseline.
- Branch `feature/autonomy`, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`. No commit, branch/remote mutation, app launch, installation, live migration/provider/model probe or Dev/agent/container operation occurred. The existing pinned Dagger builder was used only for approved source validation.

| Binding | Identity |
|---|---|
| Actual pre-layout source/policy baseline | 1,459 files; canonical SHA256 `740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3` |
| Preserved baseline archive | `target/projects-layout/baseline-source.tar.gz`; SHA256 `80f9672a2707d25e27eb91c682eba4aa5064501fbbd95f7131ae9da68831ec6a` |
| Final core inputs | 1,265 files, including23 untracked and workflow/helper inputs; SHA256 `f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164` |
| Final rebuilt package tree | 99 files/20 directories; SHA256 `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725` |
| Official CLI | Dagger v0.21.9; SHA256 `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af` |

[Baseline manifest](baseline-source-manifest.json), [final core manifest](validation-rebound/core-manifest-final.json), [artifact/mode binding](validation-rebound/final-artifact-binding.json) and [independent review](REVIEW-CLOSEOUT.md) bind the dirty/untracked worktree, not clean-source release provenance. Canonical digests use each record's stated serialization/selector; different manifest shapes must not be equated. The review independently anchors the same final gate core and package digests.

## Final validation

[Authoritative results](validation-rebound/final-results.json), [fresh/cache accounting](validation-rebound/observed-gates.json) and [retained-file index](validation-rebound/evidence-index-final.json) provide exact sanitized argument vectors, environment/toolchain identities, timestamps, finite budgets, exits, logs and source/artifact hashes. The official CLI's checksum and existing v0.21.9 engine were checked; PATH's incompatible beta CLI was not used.

| Gate | Final outcome |
|---|---|
| Focused Projects/layout/navigation/controller/protocol/goldens |169 passed, no skips |
| Local assets / real compiled Rust↔Dart Projects |5 /7 passed, no skips |
| `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build` |PASS:295 service +283 installer +9 GTK +869 Flutter +41 native-package =1,497 aggregate coverage |
| Fresh/cached distinction |1,214 fresh +283 cached installer passes;3 fresh service +2 cached installer controlled fixture ignores; no other skips/failures |
| Quality/build |Formatting, analyzer, locked warning-denied Clippy and Linux client/package builds passed; interpreter-free CLI scenario was cached |
| `python3 -B dagger/package.py --root installer/dist --verify` |PASS on rebuilt candidate;26 exact assets, exported glyphs0644/native entrypoints0755; client exports and paired service copies agree |
| Helpers with `SMITH_TEST_PACKAGE=installer/dist` |34 fresh passes, including actual-package archive roundtrip; no skips |
| `git --no-pager diff --check` |PASS |
| Independent assurance |322 passing Flutter executions, unchanged original M2 repro, requirement-equivalent M1 Return probe and independent package verification; [source/layout gate GO](REVIEW-CLOSEOUT.md), not external approval |

Focused/reviewer executions overlap aggregate coverage; do not sum them as unique tests. Setup/dependency/runtime caches were used. Service tests retain their configured Bookworm environment, not an extra Ubuntu service proof. This is not a cold-cache/hermetic build. Actual-package archive tests use synthetic Git identity, not release provenance.

## Preserved failures and remediation

1. [First parent validation](validation/final-results.json) failed the unchanged service executable-writer fixture's internal two-second receive deadline. The canceled build left old24-asset exports; verifier and actual-package helper checks consequently failed. The original fixture and later full aggregate passed without deadline/assertion/concurrency changes. Their passes do **not** establish the historical failure's cause.
2. [Initial independent review](REVIEW.md) found **M1**, compact editors with no saved-card route becoming inaccessible after Back. Return and Keep editing now reopen the retained editor without reloading/rebinding or weakening write eligibility. The first corrective attempt changed ordinary saved-list geometry; original bookmark assertions failed, then passed after limiting Return to editors without a matching card.
3. [Follow-up review](REVIEW-FINAL.md) found **M2**, offscreen primary focus on Return after long-list Tab wrap. A traversal-group-only attempt did not cover parent-scope wrapping; its failed runs and temporary diagnostic trace are retained in [remediation](remediation/parent-checks-keyboard.json). The final local scroll-position override checks the opposite edge for directional reveals while preserving explicit alignment. Four bundled-font forward/reverse tests and the original independent M2 repro pass. A test's initial bottom-padding assumption was corrected to require focus reaching the last card, retaining every focused-geometry assertion. Temporary traversal-group/logging code is absent.
4. [Closeout attempt](validation-closeout/final-results.json) failed two analyzer brace findings in new tests before full tests/builds; prior exported bytes were not claimed as its candidate. Only those braces changed before the final [rebound](validation-rebound/final-results.json). The earlier [validation-final](validation-final/final-results.json) PASS remains bound to its preceding, pre-M2 candidate, not the final one.
5. Original M1 literal probes remain failed and unchanged: they demanded New reopen a form without its discard prompt. A separate probe changes only the post-Back action to Return and preserves every assertion, as explained in [closeout review](REVIEW-CLOSEOUT.md). The safety prompt was not removed or waived.

[UI-owner summary](ui-owner-validation.json) and [asset-owner summary](asset-owner-validation.json) preserve earlier local checks, intermediate fixture/capture failures and corrections. Some owner raw logs were not retained; these summaries are not independent/current parent gate evidence. Six native tests skipped in the owner's standalone synthetic package run later executed in full native gates; skipped tests are never counted as passes. The owner's31-golden record count is corrected to **28 unique pre-existing PNGs**; all remain unchanged, alongside127 prior Projects evidence files. Eight new Projects-only goldens were added, not unrelated images regenerated. Historical installer guard failures remain unexplained and are not fixed by this slice.

## Visual evidence and remaining boundaries

[Test-rendered wide interior](../../../client/test/goldens/projects-layout/wide-1536x1024.png) and the [eight fixture cases](../../../client/test/projects_layout_golden_test.dart) cover1536×1024,400×400 normal/2× and4200×2400. These are bundled-Regular-font widget captures, not a native GUI session or proof of production font-weight/IME/accessibility fidelity.

Native Ubuntu graphical/IME/accessibility/display acceptance, installed migration/restore, live model/provider/keyring/manager behavior, clean-host/source reconstruction, icon redistribution rights/SBOM/provenance, publication and deployment require their own evidence and human approval. The15 imported icons' rights remain unresolved; the two new glyphs do not clear that gate. No source-review pass grants release, deployment, publication or accepted-risk authority.
