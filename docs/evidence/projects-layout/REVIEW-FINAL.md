# Projects editor layout — final-source independent follow-up

- **Scope:** reassess M1 remediation and the approved section0 layout-only slice, including retained state, origin/write guards, keyboard access, affected regressions/goldens, and final validation/artifact binding.
- **Independence/preservation:** no production code, source tests, goldens, parent records, or earlier review records were edited. Original `REVIEW.md`, its indexed evidence, and original scratch fixtures remain unchanged; new records are under `review-final/`.
- **Binding:** current dirty/untracked working tree on `feature/autonomy`, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`; independently verified gate-core SHA256 `d27c35e900fea8b00345cb5633fd43e71a8c0ba6706b7d0a01648cb5d46daaec` and actual exported package bytes/modes.
- **Final-source verdict: FAIL / NO-GO.** M1's missing return route is remediated through the new Return/Keep editing paths, but independent keyboard testing found **M2**, a reproducible medium-severity visibility failure. The original literal M1 probe also still fails and is not represented as passing. Parent final gates genuinely record PASS; those executions are distinguished below from reviewer executions.

## Findings by severity

### Critical / High

None identified in the bounded follow-up. No native, installed, or release-wide guarantee is inferred.

### Medium — M2: Tab-wrapped Return action receives focus without being revealed

**Affected source:** `client/lib/ui/projects_view.dart:717–760` (retained Projects list/new Return control), especially `:747–759`; retained scroll positioning is implemented at `:1121–1174`. These are affected code locations, not an assertion that a particular lower-level fix has been isolated.

**Requirement:** approved plan R2/AC2 and responsive/focus requirements; `docs/DESIGN.md:14`, `:74`, `:178`: visible keyboard focus and reachable narrow/enlarged controls, without destroying retained reading state.

**Minimal independent reproduction:**

1. Mount the real `ProjectsView` at **400×400**, with 20 valid saved projects and the current theme.
2. Use the real **New Project** control, enter an unsaved Name, then use the real **Back to projects** control. The new **Return to project editor** action is present.
3. Focus the visible **Search projects** field. Use actual Tab events through the saved-project list until traversal wraps to Return. The probe performs no `ensureVisible` call on Return and substitutes no callback for the return operation.
4. Return has **primary focus**, but the list remains scrolled toward its bottom and the focused action is far above the viewport. The assertion requiring the focused action to be visibly reachable fails.

**Observed with bundled JetBrains Mono Regular:**

| Text scale | Focused Return rectangle | List offset | Expected |
|---|---|---:|---|
| 1× | `(16, -877)–(244, -829)`, center Y **-853** | 941 | Focused action revealed in the 0–400 viewport |
| 2× | `(16, -1090)–(384, -1010)`, center Y **-1050** | 1210 | Same, with enlarged text |

The normal test-font version independently fails at both scales too. This is not merely an oversized label or missing focus ownership: the bundled-font run confirms primary focus on the actual button while its center is outside the visible viewport. It does **not** establish that Enter is inoperative or that input is lost; the blocker is invisible focus/recovery navigation.

**Evidence:** [bundled-font command/result](review-final/keyboard-wrap-bundled-font.json), [raw sanitized log](review-final/keyboard-wrap-bundled-font.log); [minimal original-font run](review-final/keyboard-wrap.json), [log](review-final/keyboard-wrap.log). The larger initial return probe also exposed this in eight scenarios; its [log](review-final/return-paths.log) and a [primary-focus/geometry trace](review-final/return-paths-trace.log) are retained, rather than discarded as harness noise.

Ignored, independent repro: `target/projects-layout/assurance/final/keyboard_wrap_probe_test.dart`. Run from the repository root:

```sh
python3 -B target/projects-layout/assurance/final/final_check.py run reproduce-m2 --concurrency=1 --reporter=expanded ../target/projects-layout/assurance/final/keyboard_wrap_probe_test.dart
```

**Required remediation:** make traversal reveal the focused Projects control, including forward wrap from a long list, while preserving list/editor bookmarks across reflow, filtering, Back and hidden panes. Keep the fix inside the approved presentation boundary, preserve authority/uncertainty guards, and add source regressions covering this traversal boundary at normal/2× text with bundled fonts. No production patch or waiver was made by this reviewer. The precise focus-reveal/retained-scroll interaction needs isolation before choosing a fix.

### Low

None additionally reported.

## M1 remediation and original-probe disposition

**Implemented/source verified:**

- `client/lib/ui/projects_view.dart:180–183` reopens the retained editor after declining replacement, subject to the existing mounted/saving/epoch checks.
- `:747–759` adds Return only for a compact retained editor with no matching saved-card route. Its callback sets `_editorOpen` only; it does not `_load`, rebind, mutate, select a substitute, or relax a write fence.
- A matching saved card retains the existing route without inserting the extra action. This avoids changing the saved-list geometry in the preserved bookmark scenarios.

**Independent current-path evidence:** all **12** pointer/state cases pass: new, missing, uncertain, filtered, and same-revision restarted+filtered editors at normal/2× text, plus the real New → discard prompt → Keep editing return at both scales. They check retained view/`EditableText`/controller/focus-node identities, Name/type selections, disclosure/input, editor/list scroll state, hidden-focus exclusion, query retention, stale captured Save/Delete callbacks, unchanged API counts, and continuing write blocking. See [command](review-final/return-pointer.json) and [log](review-final/return-pointer.log).

The pointer checks are a **separate fixture**, not a weakened replacement for the keyboard requirement. The failing keyboard fixture and its assertions remain retained, and M2 remains blocking.

**Original M1 requirement repro was rerun UNCHANGED:** [command/result](review-final/original-m1-unchanged.json), [log](review-final/original-m1-unchanged.log). Its file hash was verified against the original review before execution and at close. Both text-scale cases still fail at original line86 because they click **New Project** and require **no discard dialog**; they do not use the newly added Return action or then choose Keep editing. The old comment that New is the only list route is no longer true.

Thus, the underlying **retained-editor navigation gap is remediated for the reviewed Return/Keep editing behavior**, but **the unchanged literal probe did not pass**. No test expectation was edited, no failure was hidden, and no approval/risk waiver is inferred. The plan owner must reconcile that literal New/no-dialog assertion with the preserved replacement/discard guard; removing a safety prompt merely to satisfy that old path is not recommended. M2 independently prevents a bounded-source pass regardless of that test-mapping mismatch.

## Scope, safety and preservation checks

- Reverified the **1,459-file** canonical baseline/archive (`740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3`) and frozen source against it, without treating HEAD as the layout baseline. All 127 earlier Projects/structured-repository evidence files, 15 imported icons, and 28 pre-existing golden PNGs remain unchanged. Eight new layout PNGs are also unchanged from the initial reviewed candidate.
- Relative to the initial independently reviewed executable core, **only** `client/lib/ui/projects_view.dart` and `client/test/projects_layout_test.dart` changed. The latter is **purely additive**: removing only the new four-state/two-scale block reproduces the complete initial test file's SHA256. Existing assertions were not weakened. [Remediation delta](review-final/remediation-delta.json).
- Independently reconstructed the initial reviewed view from the verified source archive plus the immutable first-review diff, verified its recorded hash, and produced the small [corrective view diff](review-final/remediation-view.diff). Seven safety sections remain byte-identical: registry change handling, discard, Refresh/Resolve, Save, submission reconciliation, revision review and Delete. [Safety/source binding](review-final/source-and-safety-binding.json).
- Parent remediation evidence preserves the first corrective run's two saved-list bookmark failures and its second passing run. Its test SHA256 is identical in both attempts; source changed to suppress Return when a saved-card route exists. Those original bookmark assertions pass in the independently executed current suite. Historical failures are retained, not converted to passes.
- Production service/model/controller/API/preferences/global shell/navigation/theme/font behavior remains frozen. Existing regressions cover old/current callbacks, same-revision instance changes, immutable nested submissions/newer input, uncertain writes remaining query-only, and deletion reconciliation after restart. No new schema, repository/Git/provider/agent/container or lifecycle effect was found.
- The original initial review/report/indexed evidence and scratch identities match their independently retained hashes: [before preservation](review-final/before-prior-preservation.json), [after preservation](review-final/after-prior-preservation.json). The initial report remains the original failing assessment of the earlier core, not a mutable final verdict.

## Final source and artifact identities

| Identity | Independently verified value |
|---|---|
| Gate-shaped core | **1,265 files**, SHA256 `d27c35e900fea8b00345cb5633fd43e71a8c0ba6706b7d0a01648cb5d46daaec` |
| Reviewer-shaped core before/after | **1,262 files**, SHA256 `f4d41081455f331ab29e69183aa4213d73f666b50c029e1338eb3319b4403ea0` |
| Shape reconciliation | **1,261 shared files; zero byte/size/mode conflicts**. Gate-only: `.gitattributes`, `dagger.lock`, `dagger.toml`, `rust-toolchain.toml`; reviewer-only: `AGENTS.md`. The gate's tracked/type/mode metadata and canonical digest were separately reconstructed, not equated with the reviewer shape. |
| Actual `installer/dist` tree | **99 regular files + 20 directories**, canonical tree SHA256 `94ad66ae1857abe580bdf94704fbf829933c3998858ced909f97bc2f02c21531` |
| Package `manifest.json` bytes | SHA256 `39844736cb75d408c17e05b132211eb3c5e6efa9e445b91a700cb569712dca1d` |
| Package `SHA256SUMS` bytes | SHA256 `c849eeb10bc2507a69a913c97501414db387fa70b25e3e81c2acd478cc507679` |
| Paired service/helper bytes | Four exports match bytes/mode, SHA256 `30202135a35e4b188aa2ffd7f332d552c969585f0cf8a050ce23eb806e262d65` |
| UI assets | Exact **26**, all nonempty and source = `client/dist` = packaged bytes; complete client export also equals the packaged client subtree |

The reviewer recomputed every current gate-core record, actual package/client tree record and these digests. The 99-file full tree includes `manifest.json` and `SHA256SUMS`; the package verifier's **97 inventory files** are not a contradictory count. Source binding is to the frozen dirty/untracked candidate, not HEAD-only or clean-source release provenance. See [source binding](review-final/source-and-safety-binding.json), [artifact-byte verification](review-final/artifact-bytes-verified.json), [before](review-final/before-core-manifest.json), [after](review-final/after-core-manifest.json), and [stability](review-final/source-stability.json).

## Required Checks Evidence

Exact sanitized argv, cwd, environment overrides, timestamp, duration, exit code and log hashes are retained in each independent command record. Flutter uses cached Dart/`flutter_tools.snapshot` with analytics/version checks suppressed and `test --no-pub`; no application/GUI or live host effect was used as a shortcut.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Original M1 repro, unchanged file, normal/2× | Observe exact old requirement probe without changing its assertions | **2 failed**, exit1; New still opens the replacement prompt; no unchanged-probe PASS claimed | [record](review-final/original-m1-unchanged.json), [log](review-final/original-m1-unchanged.log) |
| Independently executed five-file Projects layout/view/navigation/controller/protocol regression | Existing safety plus eight additive resume cases pass | **157 passed**, exit0, including all 22 layout cases | [record](review-final/focused.json), [log](review-final/focused.log) |
| Independent Projects/UI/entrance/Boot golden entrypoints | Current images match without regeneration | **30 test cases passed**, exit0; all eight layout PNGs and all 28 old PNGs covered by their entrypoints | [record](review-final/visual.json), [log](review-final/visual.log) |
| Independent API/shell/entrance/compact/split/restore/queue neighboring regression | No adjacent regressions | **102 passed**, exit0 | [record](review-final/neighboring.json), [log](review-final/neighboring.log) |
| Original unchanged non-M1 scratch checks | Existing editor Tab/48px and filter/reflow bookmarks still pass | **3 passed**, exit0 | [record](review-final/original-nondefect-probes.json), [log](review-final/original-nondefect-probes.log) |
| New independent pointer/state and Keep editing cases | Resume without identity/scroll/selection/focus/origin loss or writes | **12 passed**, exit0 | [record](review-final/return-pointer.json), [log](review-final/return-pointer.log) |
| Minimal long-list Tab-wrap visibility, normal/2×, including bundled-font confirmation | Focused Return is revealed | **2 failed** in each minimal run, exit1; M2 independently confirmed | [bundled-font record](review-final/keyboard-wrap-bundled-font.json), [log](review-final/keyboard-wrap-bundled-font.log) |
| `python3 -B target/projects-layout/assurance/final/verify_final_evidence.py` | Current source/artifact and parent log/index bindings match | PASS assertions; gate-core/package digests recomputed; 87 parent index entries and 48 raw root-log excerpts checked | [source result](review-final/source-and-safety-binding.json), [artifact result](review-final/artifact-bytes-verified.json), [parent observation](review-final/parent-final-gates-observed.json) |
| Independently run `python3 -B dagger/package.py --root installer/dist --verify` | Exact current exported package passes verification | **PASS**, exit0; 97 inventory/99 full-tree files | [record](review-final/independent-package-verify.json), [log](review-final/independent-package-verify.log) |
| Parent final `make … test build`, package verify and actual-package helper gates | Stable-source full gates/build/export pass | **Observed PASS**, not reviewer executions: **295 service + 283 installer + 9 GTK + 865 Flutter + 41 package = 1,493 aggregate**, plus **34 helpers**; five controlled fixture ignores, no ordinary skips | [independent observation/digests](review-final/parent-final-gates-observed.json); parent [root record](validation-final/root-gates.json), [final results](validation-final/final-results.json), [raw root log](validation-final/root-gates.log) |
| Parent original service fixture reproduction and aggregate recurrence | Original test passes without deadline/concurrency/assertion changes | Observed **focused1 passed** and unchanged full aggregate passed. Earlier timeout cause remains **unestablished**; no flaky/concurrency explanation inferred | Parent [inspection](validation-final/original-fixture-inspection.json), [focused log](validation-final/original-fixture-reproduction.log); [independent observation](review-final/parent-final-gates-observed.json) |

Independent **repository** Flutter runs total **289 passing test executions**. The separately listed unchanged three scratch checks and 12 pointer/state checks are additional observations; failed probes and duplicated investigative runs are not hidden in a passing total. The broader failed return probe, trace, minimal first-font failure, and bundled-font failure all remain retained.

### Parent gate observation: freshness is bounded, not cold-cache proof

The reviewer verified the parent final evidence index and every command/log digest; all 12 command records are exit0, stable-input, non-timeout/non-skip records bound to the exact gate-core SHA256. Raw result excerpts independently support the aggregate counts, analyzer/format results, native package cases, build outputs and original fixture pass. The actual package/client bytes were then checked independently; no parent PASS was merely copied from a narrative.

Counting uses **invocation + vertex**, because Dagger reuses vertex IDs on later calls. The five test vertices in the aggregate invocation—service21, installer73, GTK65, Flutter95, package125—show fresh terminal results and are not marked CACHED. All six native package cases are recorded `ok`, including no-interpreter/display-free/dry-run cases; the actual-package archive helper also ran rather than skipped.

Setup/dependency/build caches **were** reused (recorded cached vertex counts by invocation: 27/12/78/53). In particular the service release export reused a cached build. Both Linux client build invocations have current build output. This is not a cold-cache/hermetic reconstruction claim or evidence of effective image-lock enforcement. The official Dagger CLI bytes were separately verified against their recorded SHA256; logged engine/client version is v0.21.9. Parent fixture/native/GTK executions remain builder/source gates, not installed operator acceptance.

## Evidence Gaps and remaining limits

1. **M2 is an unresolved required keyboard/visible-focus failure.** Passing parent suites and positive pointer checks do not waive it. No bounded-source PASS is issued.
2. **The original M1 literal probe remains failing.** Its New/no-dialog expectation and the approved Return/replacement-guard behavior require explicit test/requirement mapping reconciliation; original records remain immutable. The two new-path return mechanisms have direct positive evidence, not an invented original-probe pass.
3. Parent full gates and final exports are now verified as passing observations of the new source. They close the earlier aggregate/stale-export evidence gaps, but do not identify the preserved timeout's historical cause or supersede this independent keyboard finding.
4. Native Ubuntu window/IME/composition/accessibility-service/fractional-display/font-weight fidelity, installed migration/restore, live manager/keyring/provider/model/repository/worker/container behavior, clean-source release provenance, SBOM and publication/redistribution rights remain unverified or separately gated. Bundled Regular-face fixtures do not attest every production font weight.
5. Parent narrative/policy/validation ledgers outside the executable core may continue changing. This assessment binds the source/core, artifacts and evidence identities above, not a later whole-tree snapshot, merge, release, deployment or accepted-risk decision.

## Decision

- **Verdict: fail.** M1's missing navigation route is remediated in the current path-specific evidence, but the original literal probe has not passed and M2 is a confirmed current-source keyboard blocker.
- **Release Gate Decision: no-go.** No source acceptance, external effect, release/deployment or risk waiver is granted.
- **Required remediation:** resolve M2 within the Projects presentation boundary, add long-list traversal-wrap regressions, reconcile the original-probe contract without erasing or falsifying it, then rerun and rebind affected/full gates and fresh independent review to the changed source/artifacts. Preserve the now-passing parent evidence and all earlier failures as historical records.
