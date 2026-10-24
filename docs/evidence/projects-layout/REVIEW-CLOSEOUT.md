# Projects layout — independent bounded-source closeout

- **Scope:** approved `LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT` section0 only; close M1/M2, review the reveal-policy fix and four keyboard regressions, rerun affected behavior/goldens, and independently verify rebound source/log/package bindings.
- **Independence/preservation:** no product code, source tests, goldens, parent ledgers or exports were edited. New artifacts are under `review-closeout/` and ignored scratch paths. All 458 previously snapshotted files remain byte/mode identical. This report and its attribution metadata have a documented factual correction; their pre-correction bytes and identities are preserved separately below.
- **Verdict:** **PASS for the bounded source/layout assurance gate.** No blocking finding remains in the approved slice. This is not native/installed acceptance, release/publication authorization, a risk waiver, or final binding of later outside-core narrative/ledger edits.

## Attribution correction — documentation only

The pre-correction report/summary incorrectly attributed a parent-supplied technical interpretation to the operator. That interpretation was **not a specific human instruction, additional approval, or accepted-risk decision**. This correction distinguishes the already-approved layout requirements, the parent's interpretation, and assurance's independent requirement-coverage judgment. The bounded source verdict remains based on the requirements and retained test/source evidence, not the parent's authority. No tests were rerun and no source/test/golden files changed for this correction.

[Preserved pre-correction report/summary identities](review-closeout/attribution-correction/preserved-identities.json) retain the original report SHA256 `944b295624dd00098639b37fd549658abcb8d3b44fd874d50a2e7dad6bd3db4d`, original summary SHA256 `9bff8551629cad8985b49878f8bcafd2ca0fa70936a5df812f8eb148782b361e`, and preceding evidence anchor `5889bffad2f6432bbf49931e1c28fd2700110546ef82c7cf650b9bf1bc3c216f`. Those preserved versions are historical correction evidence, not current approval evidence.

## Exact reviewed candidate

| Identity | Independently checked value |
|---|---|
| Work / slice | `LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT` / section0 existing-field layout only |
| Branch / HEAD | `feature/autonomy` / `14d960975e76f906efb5e447ff3dece0e8e4385b` |
| Gate-shaped executable core | **1,265 files**, SHA256 `f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164` |
| Reviewer-shaped core before/after | **1,262 files**, SHA256 `d6545cff27abe5dea2b5009f0d992a28603701265cd15d3d3db243bd11e420e5` |
| Shape reconciliation | **1,261 shared paths, zero byte/size/mode conflicts**. Gate-only: `.gitattributes`, `dagger.lock`, `dagger.toml`, `rust-toolchain.toml`; reviewer-only: `AGENTS.md`. The gate's tracked/type/mode metadata and digest were separately reconstructed, not equated with the reviewer shape. |
| Actual exported package tree | **99 regular files + 20 directories**, SHA256 `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725` |
| UI assets / client pairing | Exact **26** nonempty assets, source = client export = package bytes; complete exported client subtree equals packaged client |

The core includes tracked and untracked inputs; HEAD alone is not the layout delta. Gate manifest references were resolved and their referenced file hashes verified before reconstructing current records. Package hashes were recomputed from actual bytes/modes, not merely copied from a checksum statement. The verifier reports **97 inventory files**; adding `manifest.json` and `SHA256SUMS` gives the full **99-file** tree.

Evidence: [source/safety binding](review-closeout/source-and-safety-binding.json), [before core](review-closeout/before-core-manifest.json), [after core](review-closeout/after-core-manifest.json), [stability](review-closeout/source-stability.json), [artifact-byte verification](review-closeout/artifact-bytes-verified.json).

## Findings and closure

### Critical / High

None identified within the reviewed source/layout scope.

### Medium — prior M1: closed by independently justified requirement coverage

**Current source:** `client/lib/ui/projects_view.dart:170–196`, `:748–760`.

The parent supplied the technical interpretation that **Return to project editor** satisfies R2/section0 retained-editor navigation, while **New Project** remains the R5 replacement/create action with its discard guard. The user did not give that specific mapping instruction; the parent's interpretation is not human approval. Assurance independently checked it against the already-approved requirements: R2/AC2 requires state-preserving, reachable navigation, and R5/AC5 retains explicit draft actions and dirty-replacement guards. The tested Return route satisfies the former without weakening the latter. No safety prompt was removed or bypassed, and no new approval is inferred.

A separate ignored copy of the original requirement probe was created at `target/projects-layout/assurance/independent_layout_probe_m1_return_test.dart`. It differs from the immutable original by **exactly one post-Back action**:

```diff
-      await projectTap(tester, 'new-project');
+      await projectTap(tester, 'resume-project-editor');
```

All draft/controller/selection/no-dialog/no-write assertions, imports, other actions, test names and comments are byte-identical. The obsolete “New is the only route” comment remains unchanged solely to make that one-action proof exact; it is not adopted as current product truth. Both normal/2× requirement cases now pass. [Mapping proof](review-closeout/m1-action-mapping.json), [one-line diff](review-closeout/m1-action-mapping.diff), [run](review-closeout/mapped-m1-one-action.json), [log](review-closeout/mapped-m1-one-action.log).

The original two failed literal New/no-dialog cases and their logs remain immutable historical evidence. They are **not relabelled as passes**. This is a requirement-to-control mapping independently justified by the existing approved requirements and retained coverage—not a newly human-authorized mapping, accepted risk, removal of an assertion, or a waiver of preservation/recovery requirements.

The unchanged broader independent fixture also passes all **12** new/missing/uncertain/filtered/restarted-filtered return and Keep editing cases. It checks retained view/`EditableText`/controller/focus identities, selections, disclosures/input, list/editor bookmarks, hidden-focus exclusion, captured stale Save/Delete callbacks, API counts, and continued origin/write blocking. Keep editing after a replacement prompt returns to the same retained form without reload or mutation. [Run](review-closeout/retained-return-guards.json), [log](review-closeout/retained-return-guards.log).

### Medium — prior M2: closed by unchanged reproduction and policy checks

**Current source:** `client/lib/ui/projects_view.dart:1151–1181`.

`_ProjectsScrollPosition.ensureVisible` preserves explicit alignment. For a directional keep-visible policy it asks the base implementation to check the opposite edge too, so a traversal wrap can reveal a control lying beyond the other edge. It forwards the reveal parameters; it does not add a traversal group, modify global focus/shell behavior, or add production logging. Already-visible controls remain stationary in the independently checked scenarios.

The **original bundled-font M2 repro was executed unchanged** and passes both cases, including actual Tab traversal, primary-focus checks, visible geometry, Enter return and inert API assertions. The previously offscreen Return now has these rectangles:

| Scale | Current focused Return rectangle | Result |
|---|---|---|
| 1× | `(16, 48)–(244, 96)` | Visible; original repro passes |
| 2× | `(16, 48)–(384, 128)` | Visible; original repro passes |

[Run](review-closeout/original-m2-unchanged.json), [raw log](review-closeout/original-m2-unchanged.log). Its original file bytes are covered by the prior-file preservation snapshot.

**Four added source keyboard cases reviewed:** `client/test/projects_layout_test.dart:623–711` covers forward/reverse × normal/2× with bundled Regular font. Every traversal step asserts the focused rectangle's top/bottom are in the 400px viewport; completion requires actual **last-card primary focus** to have occurred before Return, then checks Return hit testing/48px dimensions, keyboard activation, retained Name/controller/selection and zero writes. Last-card focus is the correct progress criterion: reaching bottom padding or `maxScrollExtent` is not required for the last card to be visible. This is not a weakened visibility assertion.

The four-case group is additive: removing only that group reproduces the complete prior reviewed layout test file's SHA256. Existing bookmark/focus/authority assertions are unchanged and pass. [Additive delta](review-closeout/m2-delta.json).

**Additional independent policy checks:** ten bundled-font scenarios pass at normal/2×, checking explicit 0/0.5/1 alignment, both directional policies from above and below, animated reveals, horizontal/vertical target visibility, and no movement on repeated already-visible reveals. [Run](review-closeout/reveal-policy.json), [log](review-closeout/reveal-policy.log). The original editor-keyboard/filter-bookmark probes also pass unchanged.

### Low

None additionally reported. No unresolved source/layout blocker was found.

## Scope, guard and evidence-integrity review

- Reverified the actual **1,459-file** baseline archive/manifest, canonical SHA256 `740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3`, including exact names/content/sizes/modes and safe unique regular-file membership. This is not `git diff HEAD` attribution. [Baseline check](review-closeout/baseline-verification.json).
- Relative to the last reviewed executable core, only `client/lib/ui/projects_view.dart` and the additive `client/test/projects_layout_test.dart` group changed. The production delta is the reveal override and a blank line. [Exact view diff](review-closeout/m2-view.diff).
- Seven safety sections are byte-identical to the previous reviewed view: registry change handling, discard, Refresh/Resolve, Save, immutable submission reconciliation, revision review and Delete. Epoch/origin/write guards were not relaxed. Existing executed tests cover same-revision restart, old/current callbacks, nested immutable capture/newer edits, unknown/expired query-only uncertainty and deletion reconciliation after restart. [Section hashes](review-closeout/source-and-safety-binding.json).
- Production service/model/controller/API/preferences/global shell/sidebar/header/navigation/theme/font code remains baseline-frozen. Search remains saved-Name-only/local, with no mutation or selection substitution; current cards/counts, local action/section ordering, optional exact Work/Base semantics and four independent type fields remain unchanged. No new ID/description/prompt/read-only/Duplicate/schema/API or Dev/repository/Git/provider/agent/container effect was introduced.
- All **127** historical Projects/structured-repository evidence files, **15** imported icon bytes and **28** pre-existing golden PNGs remain unchanged. The eight Projects layout PNGs are also unchanged from the previously reviewed candidate; no golden was regenerated. The exact wrapper/package allowlists remain 17 glyph names / 26 UI files, not a wildcard. Local SVG structure/source-provenance consistency and packaged source equality were rechecked. [Static assets](review-closeout/static-checks.json), [artifact bytes](review-closeout/artifact-bytes-verified.json).
- All **458** snapshotted prior reports/evidence/repro files remain byte/mode identical. The two earlier independent evidence indexes were also verified against their retained digest anchors. [Before preservation](review-closeout/prior-preservation-before.json), [after preservation](review-closeout/prior-preservation-after.json).
- Parent remediation attempts, keyboard trace and padding-versus-last-card harness correction remain preserved. The failed `validation-closeout` analyzer result remains a failure in its original bundle. Independent comparison finds only the layout test file changed from that candidate; removing exactly the two reverse-modifier brace pairs reconstructs its failed-candidate file hash. No assertion, deadline or concurrency adjustment was used to clear that analyzer failure. The rebound analyzer/format results are passing current evidence, not rewritten earlier outcomes.

## Required Checks Evidence

Exact sanitized commands/working directories/environment overrides/timestamps/durations/exit codes and raw-log hashes are in the linked records. Independent Flutter executions use cached Dart/`flutter_tools.snapshot`, analytics/version suppression and `test --no-pub`. No app/GUI/live manager/provider/model/installation/repository effect or Git mutation was performed by the reviewer.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Unchanged original M2 bundled-font repro | Normal/2× wrap reveals focused Return and allows safe keyboard return | **2 passed**, exit0 | [record](review-closeout/original-m2-unchanged.json), [log](review-closeout/original-m2-unchanged.log) |
| M1 copied requirement probe; only post-Back action mapped to Return | Same original preservation/no-dialog/no-write assertions pass through approved resume route | **2 passed**, exit0; exact one-action byte proof | [mapping](review-closeout/m1-action-mapping.json), [record](review-closeout/mapped-m1-one-action.json), [log](review-closeout/mapped-m1-one-action.log) |
| Five-file Projects layout/view/navigation/controller/protocol run | Existing guard/recovery coverage and all 26 layout cases pass | **161 passed**, exit0 | [record](review-closeout/focused.json), [log](review-closeout/focused.log) |
| Projects + existing UI/entrance/Boot golden entrypoints | Current images match without updates | **30 test cases passed**, exit0; all eight new and 28 prior PNGs exercised through their entrypoints | [record](review-closeout/visual.json), [log](review-closeout/visual.log) |
| API/shell/entrance/compact/split/restore/queue neighboring regression | No adjacent behavior regression | **102 passed**, exit0 | [record](review-closeout/neighboring.json), [log](review-closeout/neighboring.log) |
| Unchanged broad return/focus/origin fixture | New/missing/uncertain/filtered/restarted states and Keep editing retain state and guards | **12 passed**, exit0 | [record](review-closeout/retained-return-guards.json), [log](review-closeout/retained-return-guards.log) |
| Independent explicit/directional/no-jump reveal-policy fixture | Alignment respected; opposite-edge target revealed; already-visible target stays fixed | **10 passed**, exit0 | [record](review-closeout/reveal-policy.json), [log](review-closeout/reveal-policy.log) |
| Original unchanged editor traversal/filter bookmark probes | Existing reachability/48px and bookmark behavior preserved | **3 passed**, exit0 | [record](review-closeout/original-nondefect-probes.json), [log](review-closeout/original-nondefect-probes.log) |
| `python3 -B target/projects-layout/assurance/closeout/verify_closeout_evidence.py` | Reconstruct actual source/artifact records and verify parent index/command/log/cache binding | **PASS**, exit0; 11 command records and 38 raw result/cache/build excerpts checked | [record](review-closeout/binding-verifier.json), [log](review-closeout/binding-verifier.log), [parent observations](review-closeout/parent-rebound-gates-observed.json) |
| Independently run `python3 -B dagger/package.py --root installer/dist --verify` | Current strict package verifies | **PASS**, exit0; 97 inventory / 99 full-tree files | [record](review-closeout/independent-package-verify.json), [log](review-closeout/independent-package-verify.log) |
| Parent rebound full make/test/build, package/helper and compiled Projects gates | Complete stable-source configured gate results, exact export and no required missing/ordinary skipped checks | **Observed PASS**, not reviewer executions: **1,497 aggregate = 1,214 fresh + 283 cached installer**, plus **34 helpers**. Five controlled ignores, no ordinary skips. Standalone seven compiled Projects cases are supplemental, not added again to the aggregate. | [independent observation/binding](review-closeout/parent-rebound-gates-observed.json); parent [root record](validation-rebound/root-gates.json), [raw root log](validation-rebound/root-gates.log), [final results](validation-rebound/final-results.json) |

Independent repository Flutter total: **293 passing executions** (161 + 30 + 102). Additional scratch checks: **29 passed**, for **322 independent Flutter executions** overall. These are not combined with parent or historical results to inflate a unique-test count. Every current reviewer-run selected check passed; historical failures remain retained.

## Rebound gate observation — fresh versus cached

The reviewer verified every indexed rebound evidence file and all 11 command record/log digests against the stable core. The actual source records, package/client tree, all 26 source-equal assets, four paired helper bytes/modes, native entrypoint headers and package verification were independently checked. This is more than accepting the parent's summary, but it is **not an independent execution of its full Make/Dagger/GTK/native/helper gates**.

Raw counting is by **invocation + vertex**. In aggregate invocation0:

- **Fresh:** service vertex21, **295** cases / 3 controlled ignores; GTK vertex60, **9**; Flutter vertex125, **869**; native package vertex103, **41**, including six native cases actually `ok`. Total **1,214 fresh cases**.
- **Cached:** native installer quality vertex72, **283** cases / 2 controlled ignores. Its CACHED event is verified. The complete **1,065-file installer/service/Dagger input projection** is byte/mode/metadata-identical to the preserved successful prior component; its canonical digest `8993531727a8fc09fb197d3f1be83a15f485f7f39e0ad75e954befd349caba8a` was independently recomputed. Those prior raw suite results were checked; they are not presented as 283 new executions.
- **Interpreter-free CLI scenario:** cached and explicitly not a fresh scenario. Setup/runtime/build caches were also reused; observed cached vertex counts by invocation are **86/12/41/54**. Logged current Linux build/export output and rebuilt package bytes are bound separately.
- **Helpers:** all **34** current helper cases, including actual-package archive round trip, pass without skips. The archive round-trip printed digest is an observed helper result, not an independently retained archive/provenance attestation.

The official Dagger CLI bytes match the recorded pinned digest; compiled fixture/helper bytes match their binding and the service inputs are unchanged. Fresh format/analyzer and warning-denied quality results are recorded, with native installer quality correctly identified as cached. No cold-cache/hermetic or clean-source reconstruction guarantee follows. The historical service timeout cause remains unestablished; it was not “fixed” by changing deadlines/concurrency or waived.

## Evidence Gaps and limits

1. **No unresolved required source/layout gate or blocking finding remains for the exact core/artifacts above.** This is a bounded as-built source/fixture verdict against the approved layout requirements, not a guarantee for all future policy or every native timing interleaving.
2. Native Ubuntu graphical/window-edge, IME/composition, accessibility-service, fractional-display and all-weight font fidelity remain separately unverified. Bundled Regular-face tests are not full native QA.
3. Installed migration/restore, live manager/keyring/provider/model/repository/worker/container behavior, clean-source/cold-cache release provenance, SBOM, imported-icon redistribution rights and publication approvals remain separate gates. No installation or external effect is authorized here.
4. Parent narrative/ledger updates outside the frozen core have now been written, but this report does **not** claim final whole-tree documentation binding. Final documentation anchoring is awaiting the parent-generated `closeout-binding.json` and a subsequent independent check that those documents preserve the exact reviewed core/artifacts and accurately represent findings, attribution, counts and limits. Any core/artifact change needs renewed validation/review binding.

## Decision

- **Verdict: pass — bounded source/layout assurance only.** M1 is closed through independently verified R2/R5 requirement coverage, using the parent's technical interpretation without treating it as human approval, with all original assertions retained in the one-action probe; M2 is closed by the unchanged failing repro now passing, four source keyboard regressions, and independent reveal-policy checks.
- **Source-review gate: go** for the exact approved section0 candidate identified above; no in-slice remediation is required by this reviewer.
- **Release Gate Decision: no-go for release/deployment/publication.** Separate native/installed/provenance/rights evidence, final document binding as applicable, and exact human effect authorization are still required. This source PASS grants no external approval and accepts no residual risk on the operator's behalf.
