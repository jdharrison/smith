# Final docs closeout — late concurrent core drift blocks current-checkout binding

- **Scope:** the final no-drift recheck required by the Projects layout docs/evidence-binding request. This is not a functional review of concurrent Global Context implementation.
- **Authoritative final decision:** **FAIL for current-checkout unchanged-source closeout.** The exact27-file parent docs/evidence binding itself is **PASS**, and the previously reviewed package still matches. The current executable source is no longer the reviewed layout candidate.
- **Preservation/effects:** no source, bound file, old report/summary/index, export or test was changed by this reviewer. No tests/builds/launches/remote or host operations were run. All earlier review-docs checks, the initial successful core observation, the failed finalizer and the prior reports are retained verbatim.

This record **supersedes any current-checkout PASS reading of `REVIEW-DOCS.md`**. That earlier report describes the successful27-file/core/package checks before concurrent source edits arrived. `DOCS-CLOSEOUT.json` already records the finalizer's failed no-drift checks; it was not rewritten to hide them. The original M1/M2 closures remain historical source-bound results, not newly failing tests and not approval of the concurrent work.

## High — H1: previously frozen executable inputs changed during final review

**Scenario:** the finalizer independently rehashed actual source bytes/modes and recomputed selected membership after the docs review. It found changed frozen client inputs and new instruction files. A subsequent read-only observation at **2026-10-17T22:55:12Z** recorded six changed reviewed inputs and five new selected core files. This current selector had **1,270 files**, not1,265; the recorded observation was internally stable during that read, not validated by any gate.

| Previously reviewed file | First changed current line at observation |
|---|---:|
| `client/lib/app_api.dart` |9 |
| `client/lib/cloud_routing.dart` |4 |
| `client/lib/workspace_cloud.dart` |91 |
| `client/lib/workspace_controller.dart` |1250 |
| `service/src/app.rs` |15 |
| `service/src/app_v2.rs` |167 |

New selected core paths, starting at line1:

- `client/lib/instructions.dart`
- `client/lib/instructions_controller.dart`
- `client/test/instructions_controller_test.dart`
- `client/test/instructions_protocol_test.dart`
- `service/src/app_instructions.rs`

Exact expected/current byte hashes, sizes, modes and baseline/current changed-hunk ranges are in [FINAL-DRIFT.json](FINAL-DRIFT.json). The full observed selector is retained in [late-current-core-observation.json](late-current-core-observation.json). This is a finite observation; ongoing concurrent work may change it further.

**Reproduction:** compare each current record to the1,265-entry `validation-rebound/core-manifest-before.json`, and independently enumerate `git --no-pager --no-optional-locks ls-files --cached --others --exclude-standard -z` using that manifest's source roots/exclusions. The review's read-only [finalizer](finalize_docs.py) returned exit1; [drift recorder](record_final_drift.py) captured the differences without executing or modifying product code. A drift recorder completing successfully is not a test/source PASS.

**Impact:** the requested unchanged-source condition is false for the current checkout. Prior passing Make/Flutter/native/helper/source-review evidence cannot validate these changed service/API/routing/controller inputs. The exact HEAD/branch remaining unchanged does not repair a dirty-worktree binding. The old package is still the reviewed layout artifact, but it cannot be represented as a build of this now-different source.

**Required remediation:** preserve concurrent work. Provide an isolated exact reviewed layout snapshot for its historical closeout, or separately validate/review/rebind the intended combined candidate under the appropriate scope. Do not revert others' files, rerun unapproved gates, edit old hashes/reports, or infer a waiver. No authority or acceptance for the separate Global Context task is inferred from its plan text.

## Required Checks Evidence

| Check / scenario | Expected | Actual / result | Evidence |
|---|---|---|---|
| Parent final docs binding | Every bound file's actual byte/size/mode and canonical list match supplied digest | **PASS:**27/27; before/after checks and late observation match | [before](bound-files-before.json), [after](bound-files-after.json), [supplemental](supplemental-verification.json), [late drift record](FINAL-DRIFT.json) |
| Parent narratives versus retained gate/review counts/cache/failures/approval scope | No fabricated gate execution or human-mapping approval | **PASS:** detailed reconciliation remains valid for the historical exact candidate; no specific human New/Return mapping direction is claimed | [docs review](REVIEW-DOCS.md), [retained-log checks](gate-and-review-observations.json) |
| Final core records and selector membership | Same reviewed1,265 files and digest | **FAIL**, finalizer exit1; six changed inputs/five new core files in subsequent observation | [failed final summary](DOCS-CLOSEOUT.json), [actual differences](FINAL-DRIFT.json) |
| Package identity | Same99-file/20-directory/26-asset tree and modes | **PASS**, no drift | [package](package-recomputed.json), [late record](FINAL-DRIFT.json) |
| Old evidence/report/index preservation | No reviewer changes to any480 pre-existing layout evidence files | **PASS** | [initial snapshot](existing-evidence-before.json), [late record](FINAL-DRIFT.json) |
| Local Markdown links | Reproduce claimed130 local file paths, not anchor or remote validation | **PASS** | [links](links-rechecked.json) |

The initial strict-verifier Process clause and multiline unittest false negatives were independently reconciled without changing any parent input; their original failures remain in `technical-verification.json`. D1's two unbound Global Context planning/evidence documents were already reported. H1 is a later, substantive executable-source drift finding, not another verifier-parser correction.

## Independent digest anchors

- **Verified final parent docs canonical SHA256:** `d7a32b500cbaf874f1f8ea840cd102f71406be7bd6a3f7b653a31c1a7d26c7a8`.
- **Verified raw parent binding SHA256:** `a5bf0e6e5eab220f6eb2b08023449969ce2f3d5682dac232a4c28d50e964c16d`.
- **Historical validated/reviewed core:**1,265 files, `f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164`.
- **Observed current selector, not a validated candidate:**1,270 files, `23c0e3c7d687631a72b1a69a3186d5f777f42bca3262a70b9ccc6f521214e8d9` at22:55:12Z.
- **Unchanged reviewed package tree:** `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725`.

The earlier finalizer's `d45e67...` hash covers its then-current projection of the original1,265 listed paths; it is **not** the full current selector digest. The later full selector above explicitly includes additions. No digest here is an approval token or clean-source provenance claim.

## Evidence Gaps and decision

No validation/review of the concurrent Global Context code has been attempted or inferred. Native Ubuntu graphical/IME/accessibility/font/display acceptance, installed migration/restore, live manager/provider/model/keyring behavior, clean-source/cold-cache provenance, SBOM/imported-icon rights and exact external human authorization remain separate limits.

- **Verdict: fail for current-checkout unchanged-source closeout; pass for the exact parent docs bytes/evidence binding to the historical reviewed candidate.**
- **Required remediation:** isolate the original candidate or obtain separately source-bound validation/assurance for the intended combined changes. No core edits or risk waiver were made.
- **Release Gate Decision: no-go.** Historical source PASS is not current mixed-worktree acceptance, human approval, risk acceptance or publication/deployment authority.
