# Projects layout — independent final docs/evidence-binding review

- **Scope:** final parent narrative/ledgers, `closeout-binding.json`, referenced gate/review evidence, and unchanged source/artifact identities for `LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT` section0 only.
- **Method:** read/hash/mode and retained-log inspection, baseline archive inspection without extraction, scope/link reconciliation, and manual requirement/approval-attribution review. **No tests, builds, package programs, native executables, app launches, remote operations or installations were run in this docs review.**
- **Preservation:** writes are confined to this new `review-docs/` directory. No bound file, old report/summary/index, source/test/golden or export was edited. All **480** pre-existing layout evidence files remain byte/size/mode identical during this check.
- **Verdict:** **PASS for the exact 27-file documentation/evidence binding and its unchanged reviewed layout core/package.** One nonblocking scope-inventory discrepancy is reported below: two later, unbound Global Context documents prevent extending this result to the entire current checkout. This is neither human acceptance nor a risk waiver.

## Exact anchors independently checked

| Identity | Observed value |
|---|---|
| Branch / HEAD | `feature/autonomy` / `14d960975e76f906efb5e447ff3dece0e8e4385b` |
| Bound final documents/evidence/source list | **27 regular files**, canonical SHA256 `d7a32b500cbaf874f1f8ea840cd102f71406be7bd6a3f7b653a31c1a7d26c7a8` |
| Raw `closeout-binding.json` bytes | SHA256 `a5bf0e6e5eab220f6eb2b08023449969ce2f3d5682dac232a4c28d50e964c16d`, 5,553 bytes, mode0664 |
| Actual baseline | **1,459 files**, canonical SHA256 `740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3` |
| Baseline archive | SHA256 `80f9672a2707d25e27eb91c682eba4aa5064501fbbd95f7131ae9da68831ec6a`; all 1,459 safe, unique regular-file members match baseline content/sizes/modes |
| Final gate-shaped core | **1,265 files**, SHA256 `f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164`; no byte/size/mode/type/tracked-status drift or unlisted selected core files |
| Actual package tree | **99 regular files / 20 directories**, canonical SHA256 `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725`; actual bytes and modes recomputed |
| Prior corrected review evidence | 45 indexed files, canonical SHA256 `b2a0fdffbbc2800f838d86ad6db44a030d67a69f0bcfca63aec5aab88e666db6` |
| Parent rebound evidence index | 77 indexed files, canonical SHA256 `b4823f54e9a845f40cbb4f46c0ec3e1b541fe33f9fc7e633e2d02e5a6f1b1141` |

Canonical-list hashes use `json.dumps(list, sort_keys=True, separators=(",", ":"))` encoded as UTF-8, preserving each manifest's list order and record shape. Raw-file SHA256 is a different identity. The final core reference was resolved and its referenced manifest-file hash verified; no assumption that the reference itself contains `files` was made. The earlier reviewer-shaped 1,262-file core is not equated with the gate-shaped core.

Evidence: [technical verification](technical-verification.json), [raw check log](verification.log), [supplemental verification](supplemental-verification.json), [before](bound-files-before.json), [after](bound-files-after.json), [current core](current-core-manifest.json), [current package](package-recomputed.json).

## Findings by severity

### Critical / High / Medium

No blocking inconsistency was found in the 27 bound files, final layout narrative, corrected approval attribution, referenced final results/reviews, or reviewed core/package anchor.

### Low — D1: the current whole-checkout inventory has two additional unbound documents

**Location:** parent `scope-audit.json:625` records an empty `out_of_scope_changes`; its generator's closed-set assertion is `record_closeout.py:66`. That audit is a dated snapshot, not a current whole-worktree certificate.

Read-only enumeration now finds these additional paths outside both the 1,459-file baseline and the declared layout additions/evidence namespace:

- `docs/plans/LOCAL-SMITH-GLOBAL-CONTEXT.md`
- `docs/evidence/global-context/BASELINE.json`

Their observed modification times are 22:50:57 and 22:51:03 UTC, after the parent binding at 22:45:33 UTC. Their contents identify Global Context planning/baseline material, not the Projects layout. **Modification times and document claims do not authenticate ownership or human approval.** This reviewer does not validate that task or infer its authority.

**Reproduction:** read-only `git --no-pager --no-optional-locks ls-files --cached --others --exclude-standard -z`; subtract the baseline paths, the five separately HEAD-checked baseline omissions, the 12 declared new layout source paths, and `docs/evidence/projects-layout/`. The two paths remain. Their exact identities and timestamps are retained in [supplemental verification](supplemental-verification.json).

**Impact/disposition:** the 13 existing baseline modifications, 12 layout source additions, frozen core, historical bytes and all bound documentation remain correct. The stronger claim that these are *all* present checkout additions is no longer reproducible. Keep this final PASS limited to the exact bound candidate; do not apply it to these documents, subsequent Global Context implementation, or the entire checkout. No bound file was edited to hide this discrepancy. No waiver or accepted-risk decision is made.

## Required Checks Evidence

| Check / command or scenario | Expected | Actual / status | Evidence |
|---|---|---|---|
| `python3 -B docs/evidence/projects-layout/review-docs/verify_binding.py` — every bound record and canonical identity | 27 safe regular paths; actual bytes/size/mode and canonical list equal the supplied binding, before/after | **PASS** for all binding/core/artifact/index checks. Initial verifier exit1 is preserved: two overly strict local expectations and the real D1 inventory difference are explained below. | [record](technical-verification.json), [log](verification.log) |
| Baseline archive/manifest, scoped delta and frozen inputs | Actual baseline, not `git diff HEAD`; no forbidden production changes | **PASS:** 1,459 archive entries; exactly13 allowed baseline modifications, 12 layout source additions;127 historical evidence files,28 old PNGs and15 imported icons unchanged. D1 excludes two new unrelated documents from any whole-tree claim. | [record](technical-verification.json), [documentation delta](baseline-documentation.diff) |
| Final core reference and current selector | Exact final gate core, including tracked/untracked inputs and workflow/helper roots | **PASS:** all1,265 inputs and selected membership match;1,242 tracked/23 untracked. | [core](current-core-manifest.json), [record](technical-verification.json) |
| Package actual-byte/mode comparison | No artifact drift; exact assets, full client subtree and paired helpers | **PASS:**99 files/20 directories;26 exact nonempty assets byte-equal source/client/package; exported assets0644, paired helpers0755, package root0700 and complete exported client subtree equal. No package/ELF was executed here. | [package](package-recomputed.json), [record](technical-verification.json) |
| Gate/review indexes, command references, raw-log hashes and counts | Current reproducible evidence, no count/cache inflation | **PASS:**77 parent-indexed,45 corrected-review-indexed and458 previously snapshotted records match; all11 parent command/log references verified. Parent and earlier reviewer executions are observations in this docs review, not reruns. | [observations](gate-and-review-observations.json), [record](technical-verification.json) |
| `python3 -B docs/evidence/projects-layout/review-docs/supplemental_verification.py` | Resolve overly strict verifier expectations from exact bytes/raw results, retain original failures, recheck binding/core/preservation | **PASS**, exit0. Process clause and multiline helper result independently reconciled; focused169/assets5/compiled7 terminals checked; D1 remains explicitly reported, not silently allowlisted. | [record](supplemental-verification.json), [log](supplemental-verification.log) |
| Parent Markdown file-link audit | Same claimed130 local file links resolve in the same8 documents | **PASS:**130/130 exist; list equals parent audit. No anchor validation or remote fetching claimed. | [links](links-rechecked.json) |
| Manual narrative/requirement/approval review | Final prose matches retained results, failures, approved section0 scope and corrected attribution | **PASS** for bound prose; reasoning below. | Bound `README.md`, `VALIDATION.md`, `docs/VALIDATION_DETAILS.md`, Design/Process/assessment/plan, `results.json`, corrected review/report and retained failure bundles |

### Initial verifier results are retained, not rewritten

1. **Process:** the first verifier expected only literal24→26 substitution. The actual complete delta also says “including the two locally authored Projects layout glyphs”. Supplemental byte comparison proves that this single factual asset-count/provenance clause is the **only** Process change. The strict26-path/no-wildcard requirement and all authority/gate text are unchanged. This descriptive clause does not introduce new policy or expand approval scope.
2. **Helpers:** the first verifier incorrectly required the test name and `... ok` on one line. Python unittest prints the annotation-check test name on line12 and its descriptive docstring/result on line13. An independent pending-case parser reconciles all34 names/results and the final `Ran 34 tests`/`OK`. No parent log, assertion or test was modified.
3. **Inventory:** D1 is a genuine later-worktree difference, not a parser failure. It remains in both the initial and supplemental records. The passing bounded binding does not turn the closed-set whole-checkout check into PASS.

## Narrative and evidence reconciliation

**Implemented and observed final source coverage:** the authoritative rebound contains **1,497 aggregate coverage =1,214 fresh +283 cached installer**. Fresh component counts are295 service +9 GTK +869 Flutter +41 native-package. There are34 separate fresh helpers,3 fresh service/2 cached installer controlled fixture ignores and no ordinary skips/failures in the final component results. The native-package41 and helpers34 case tables agree with actual raw `ok` lines, including the multiline helper case. Focused169 includes the eight layout golden cases; standalone compiled Projects7 and asset5 are supplemental/overlapping, not extra unique aggregate tests.

**Cache truth:** native installer quality is explicitly cached, not283 new executions. Its actual1,065-file installer/service/Dagger input projection matches the prior successful projection exactly, canonical SHA256 `8993531727a8fc09fb197d3f1be83a15f485f7f39e0ad75e954befd349caba8a`. Raw CACHED events and the prior result identity agree. Interpreter-free CLI is cached too. Service tests remain configured Bookworm, not an extra Ubuntu service run. Setup/build/runtime caches do not establish cold-cache or hermetic provenance.

**Independent-review accounting:** the eight retained reviewer run records/logs reconcile to **322 Flutter executions =293 repository +29 scratch**, not322 new tests run here. Prior repository groups are161 focused +30 visual +102 neighbors. All26 layout cases are part of the retained focused run;28 unique old PNGs and eight new layout PNGs are distinct from test-case/owner-record counts. The parent/owner gates are not presented as independent reviewer Make/native/helper executions.

**Failures and fixes:** final narrative accurately preserves the first unchanged service fixture timeout and consequent stale24-asset verifier/helper failures; intermediate M1 bookmark failures; M2 traversal-only/diagnostic and last-card-versus-padding harness attempts; two analyzer brace failures; and both original failed literal M1 New/no-dialog probes. Later passes do not establish the service timeout's cause, erase failed runs, or prove historical installer failures fixed. Owner summaries without all original raw logs are disclosed as summaries, not fresh independent gate evidence. Prior bundles and corrected/pre-correction review evidence were verified and left unchanged.

**M1 attribution/requirement mapping:** the **parent supplied the technical New/Return interpretation**, not a human instruction. Already-approved R2/AC2 requires reachable state-preserving retained-editor navigation, while R5/AC5 retains explicit draft/replacement actions and dirty-discard guards. The independent source review closes M1 using that requirement coverage: the separately retained probe changes only the post-Back action New→Return, preserving all other bytes/assertions, and both mapped cases plus12 retained-return/guard cases pass. The original literal failed cases remain failed. Neither this interpretation nor its independent acceptance creates new human approval or permits removing the replacement safety prompt. M2 closure remains tied to the unchanged original two-case repro, four additive source regressions and ten independent policy scenarios.

**Frozen scope and assets:** the approved existing fields, exact Work branch/Base equality semantics, optional four-type disclosure, saved-Name-only local search and retained guards remain the previously reviewed source. The current hash check adds no claim of new functional execution. Unsupported fields, schema/API/model/controller/service/preferences, shell/sidebar/header/navigation/theme/fonts/Chat/Settings and execution effects remain excluded. The two authored glyphs do not settle redistribution rights for the15 imported icons.

**Temporal status:** the corrected closeout report/summary still say final outside-core docs binding awaited a subsequent review. Those are preserved as-of statements; this separate record completes that step for the exact supplied27-file binding without editing or backdating them. The parent binding is content evidence, not an authenticity signature, clean-source provenance, or approval token.

## Evidence Gaps and limits

- No unresolved required check remains for the **exact bound docs/core/package** after the supplemental reconciliations. D1 prohibits a whole-current-worktree claim; new unrelated documents and any subsequent source drift are outside this verdict.
- Markdown anchors and remote links were not checked; the reproduced parent claim is local file existence only.
- No tests were rerun for this prose/binding closeout. Parent final gate results and previous reviewer executions were inspected/hash-verified as records, with original freshness/cache distinctions retained.
- Native Ubuntu graphical/window/IME/accessibility/display/all-weight font behavior, installed migration/restore, live manager/keyring/provider/model behavior and recipient acceptance remain unverified here.
- Clean-source/cold-cache reconstruction, SBOM/provenance, imported-icon rights, publication/deployment and exact human effect authorization remain separate gates. The unrelated Global Context plan's self-described approval was not independently verified and is not adopted.

## Decision

- **Verdict: pass — exact final docs/evidence binding only**, retaining the existing bounded source/layout PASS at the unchanged core/package above.
- **Required in-slice remediation: none.** Report D1 and restrict any closeout claim to this binding; a claim about the whole current checkout needs a new scope reconciliation and any changed executable input needs renewed validation/review binding.
- **Release Gate Decision: no-go for release/deployment/publication.** No human approval, accepted risk, broader-task acceptance or external-effect authority is inferred or granted.
