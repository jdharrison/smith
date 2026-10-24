# Independent assurance — structured Projects repositories

## Scope and verdict

- Reviewed the approved structured-repository extension against the **actual preserved working-tree baseline**, including the prior uncommitted Projects foundation and separate documentation cleanup; not against HEAD alone.
- Independently inspected migration, strict decoding, nullable metadata, legacy-client erasure refusal, replay identities, immutable capture, stale editors, uncertain writes/deletion recovery and metadata-only effects. Executed focused adversarial tests and inspected the final configured root/build/package evidence.
- Final binding covers the current source, supporting documents, raw gates and local artifacts. Only this review and `reviewer/` were written during closeout; parent source, evidence and policy were not edited.

**Verdict: pass-with-risk for the bounded configuration-only implementation and configured local source gates.** No unresolved implementation blocker was reproduced. The original medium duplicate-key finding is closed. The medium historical-failure residual M2 remains open; the documentation-only follow-up closes L1/L2 below. This verdict does **not** accept the remaining residual on the operator's behalf.

**Release Gate Decision: no-go.** This is an independent source-review recommendation, not human signoff, full supported-host/installed acceptance, accepted risk, or permission to commit, publish, release, install or deploy.

## Exact candidate and evidence binding

| Identity | Independently verified value |
|---|---|
| Work | `LOCAL-SMITH-PROJECT-REPOSITORIES` |
| Discovery/final-gate HEAD and branch | `14d960975e76f906efb5e447ff3dece0e8e4385b` / `feature/autonomy`; shared uncommitted work remains |
| Actual baseline | 1,357 archive entries; every archived byte count/hash/mode checked against the preserved manifest |
| Baseline canonical identity | `41bea2ed038625478da27b4cc5a8b276eeec2ef5b51cb9d3b4334b2b416c215c` |
| Baseline archive SHA256 | `b8b5ca53ae28bd3146389e32326e57757508865c5ebc9e35d650f705a000f0bb` |
| Parent [results](results.json) SHA256 | `fd4696a4d784f1d379fb44d38bc6fafcd1c35fcdba17f415acad827ca438a420` |
| Final core | 1,243 file records; canonical-array SHA256 `ed6dc46ef3938f7753ffb9a7a0c76e6be1a2ea88ed0ff760835610fa7cd53481` |
| Supporting inputs | 30 records; post-gate documentation/control snapshot, not retrospective documentation stability |
| Selected extension changes | 32 before/after records, checked against baseline and current files |
| Local artifacts | 189 records: one service, 91 client, 97 package files; bytes, sizes and modes match |
| Selected raw evidence | 78 records, all bytes/sizes/modes match; one closeout record added and only the documentation exporter's wording/hash updated |
| Historical Projects evidence | All 28 baseline files under `docs/evidence/projects/` remain unchanged |

The core JSONL matches both raw gate before/after arrays and current files; supporting and artifact records also match current files. The packaged guide matches `dagger/INSTALLER.md`. All four standalone/bundled service copies have SHA256 `30202135a35e4b188aa2ffd7f332d552c969585f0cf8a050ce23eb806e262d65`; exported and packaged clients match in bytes/modes. Package manifest SHA256 is `56d37d75668f5b8646fc4eb13b58a38e1fff3029a70a8457b33edac6d097237f`.

[Current bundle verification](reviewer/bundle-verification-doc-closeout-final.json) records the documentation-closeout local-file link check, original-seal lineage and identity comparisons. The [earlier verification](reviewer/bundle-verification-final.json) remains historical evidence for the original parent snapshot, not a binding of the corrected documentation. `reviewer/seal.json` separately binds this review's SHA256, parent bundle files and every retained reviewer artifact without a self-referential checksum. The final response supplies the review/seal digests as an additional anchor. These remain owner-writable local records, not signatures, immutable external custody or approval.

## Documentation-only closeout and preserved lineage

The parent corrected only the architecture report's section-8 row, the bundle README's Git boundary, and the approved plan's Git boundary among the 30 supporting inputs. Parent `results.json` limitations and generated supporting/change/artifact metadata were rebound; artifact metadata changed only its recording timestamp. The ignored evidence exporter changed exactly the Git-scope limitation string, and one `documentation-closeout.json` raw record was added. Every other prior selected raw record, final gate/count/compiled record, core-manifest byte and artifact file record is unchanged. No test, build, helper suite, application or service was rerun for this documentation-only update; prior passing executions are reused only for their identical bound source/artifacts.

The complete prior parent/reviewer tree and changed-document/exporter originals are retained in [reviewer-owned lineage](reviewer/lineage/pre-doc-closeout/REVIEW.md), copied byte-for-byte from the parent's preserved archive. All 44 members listed by the prior seal and all 48 archived files were checked. Original anchors remain:

- Results: `97b73a7e54a12ad14ae55aad168496f87863e4a274e1796b8d379f8bd0ed604a`.
- Review: `9c837ab84e0ce3e561fb45d311d861c5675c15ae368a71be80442d3cf2943927`.
- Seal: `874f71db07529111a454526bf7151583d70d13c3b7d869749aa880a83c961b09`.

The parent closeout record references those same anchors. The original review/seal is superseded for current-document binding, never rewritten in the lineage. The [first follow-up verification](reviewer/bundle-verification-doc-closeout-initial.json) remains failed because the reviewer initially required even the explicitly changed documentation exporter hash to stay identical. Its [checker source](reviewer/verify_doc_closeout_initial.py) is preserved. The corrected checker requires the exact exporter wording substitution, verifies its retained original, and still requires every other old raw record to match; no gate or product assertion was relaxed.

## Required Checks Evidence

Commands below were actually executed or verified from retained raw command records. Earlier independent executions are not represented as new closeout executions. Every passing row has exit 0; original failures remain failed.

| Required check / exact command or scenario | Expected | Actual and evidence |
|---|---|---|
| `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build` | All configured quality/tests/builds complete on stable core | Parent attempt-2 exit 0, no recorder timeout. Raw stdout independently recounts **295 service + 283 installer + 9 GTK + 834 Flutter + 41 package = 1,462 passing executions**. Five explicitly ignored Rust subprocess fixtures, no additional skips found. [Raw audit](reviewer/final-raw-gates-1/audit.json), parent `results.json`/raw-file digests. |
| Component quality inside that command | Formatting, locked warning-denied Clippy/tests, analysis and Linux/native builds | Rust test scripts enforce formatting/`--locked`/`-D warnings`; Dart **90 files, zero formatting changes**, analyzer clean. Actual Xvfb GTK and Ubuntu no-Python CLI help/version/artifact dry-run executed. Service quality is **Rust 1.98 Bookworm**, not Ubuntu service proof. |
| `python3 -B dagger/package.py --root installer/dist --verify` | Exact payload/checksums/modes, native entrypoints and paired helpers | Parent and independent supplemental runs pass: **95 manifest-listed files / 97 package regular files**. Independent ELF/header/mode and exported-client/helper equality checks pass. [Verifier log](reviewer/final-raw-gates-1/verify.log). |
| `SMITH_TEST_PACKAGE=<repo>/installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v` | Actual-package archive/helper checks, no substituted fixture-only package | **34 passed** in parent run and independent supplemental rerun. The latter binds helper/workflow sources omitted from the 1,243-entry core manifest, and confirms package/source stability. [Log](reviewer/final-raw-gates-1/helpers.log), [binding](reviewer/final-raw-gates-1/audit.json). Synthetic archive-source identity is not clean-source provenance. |
| `git --no-pager diff --check` | No whitespace errors in tracked diff | Parent exit 0; source manifests separately include untracked inputs. Not a claim that `git diff` alone covers this extension. |
| Standalone Dart `test --no-pub --reporter expanded test/rust_projects_integration_test.dart`, with recorded `SMITH_TEST_CLOUD_SERVICE` | Real compiled Rust↔Dart execution, no fixture skip | Parent attempt-2 **7/7 passed** with current parser/fixture hashes; raw log digest and current input equality verified. Earlier independent freshly rebuilt isolated fixture also **7/7 passed**: [record](reviewer/records/compiled-fresh.json), [log](reviewer/records/compiled-fresh.log). Corroboration, not seven additional aggregate tests. |
| Isolated `cargo test --offline --locked --lib projects_ -- --nocapture` | Native domain/migration/replay/capacity/publication checks | Independent **22 passed**, zero ignores in selection: [record](reviewer/records/rust.json), [log](reviewer/records/rust.log). |
| Isolated `cargo test --offline --locked --lib assurance_ -- --nocapture`; disk5 seed/reopen selection | Adversarial corruption/null/shared-ID/grammar checks; actual legacy seed migration | **Three new reviewer tests plus seven pre-existing selected tests passed**; separately **one actual disk5 seed/reopen test passed**. [Records](reviewer/records/native-extra.json), [native log](reviewer/records/native-extra-0.log), [seed log](reviewer/records/native-extra-1.log). Only an isolated source copy received appended reviewer tests. |
| Independent eight-field-only dirty/restart/recovery and null-save/dirty-delete widget scenarios | No silent origin adoption, replay, newer-input loss or inferred null defaults | **9 passed**: [record](reviewer/records/reviewer.json), [log](reviewer/records/reviewer.log). Two earlier reviewer setup failures remain distinct from this pass. |
| Original duplicate-member repro before remediation | Reject conflicting nested `base`, `feature`, `hotfix` members | **3 failed**, exit 1: malformed replies were accepted. Original [log](reviewer/records/duplicates-original.log), [source-bound record](reviewer/records/duplicates-original.json), [unchanged source](reviewer/repros/duplicate_reply_test.dart). Not relabelled PASS. |
| Unchanged repro after fix; durable parser plus new adversarial/neighbor checks | Reject duplicates before collapse; preserve valid JSON and uncertainty behavior | **3 passed** unchanged; **30 parser cases passed** (23 durable + seven independent); **190 neighboring tests passed**. Exact argv/SDK/environment/hashes are in [follow-up index](reviewer/parser-followup-1/index.json) and its three JSON/log pairs. |
| `python3 -B docs/evidence/project-repositories/reviewer/run_retained_repros.py` | Retained repro copies still discriminate on final bound client | Original closeout **10 passed** (unchanged three + seven adversarial), exit 0, all then-bound core/supporting/parent/repro input hashes stable; not rerun for the documentation-only update: [record](reviewer/records/retained-repros.json), [log](reviewer/records/retained-repros.log). |
| `python3 -B docs/evidence/project-repositories/reviewer/verify_bundle.py final` | Original parent identities, archive/current bindings, retained-copy equality and local links | Original [verification record](reviewer/bundle-verification-final.json) passed for its original parent snapshot; preserved without alteration. |
| `python3 -B docs/evidence/project-repositories/reviewer/verify_doc_closeout.py final` | Corrected parent identities, documentation-only delta, sealed lineage, unchanged gate/source/artifacts and local links | Current [verification record](reviewer/bundle-verification-doc-closeout-final.json). No current parent/source/artifact/raw mismatch; all old gate/log records remain unchanged; retained copies and local file destinations verify. Remote links were not fetched; Markdown anchors are not claimed exhaustively validated. |

Root test vertices supplied execution output; only container/directory setup was cached during the test phase. Later dependency/build steps reused cache. Neither the counts nor checksums establish an uncached/hermetic build. The official CLI digest matches `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af`; the connected CLI/server report v0.21.9.

## Requirements and adversarial assessment

- **R1/R2, AC1/AC2:** exact ordered nested objects/required keys; nullable repository name and six independent nullable leaves; no migration labels/defaults; unrelated saves preserve nulls. Native, Dart and widget checks cover exact-case name/URL uniqueness, multiple null names, independent populate/clear and equal Base/Work.
- **R3/R6, AC3/AC7:** pure conservative branch/pattern grammar, ASCII/255-byte total boundaries, strict shape/type/capacity rejection and compact/enlarged/keyboard widgets. No Git invocation or pattern execution follows validation. Source/widget accessibility is not native graphical acceptance.
- **R4, AC4:** strict frozen disk1–5 migration, malformed-byte preservation and disk6 output. Rich disk5 and actual compiled seed checks preserve ordered URLs/IDs/revision/old fingerprints (including null)/receipts/tombstones. Parent's original restart-inventory comparison failure is preserved; corrected tests explicitly separate volatile reset from durable preservation.
- **R5, AC5/AC6:** full-field immutable capture/dirty comparison/current-origin review; same-revision restart and captured-dialog fencing; newer-input reconciliation; exact-target deletion recovery. Unknown/expired receipts keep the original pending identity and block writes while permitting observation. Recovery remains query-only, never resend.
- **R5/R7, AC6/AC8:** legacy list/create/update/delete always refuse before replay/admission, including empty arrays/deletes/historical IDs. Current four `*_v2` operations share old Projects identities; historical receipt lookup stays unchanged. New nested argument changes conflict on ID reuse; deletion scrubs old metadata-derived fingerprints. Chat/protocol/preference neighboring regressions pass.
- **Inertness:** reviewed extension paths remain registry/editor metadata only; compiled fixtures assert zero synthetic inference/HTTP/keyring calls. This establishes tested paths, not a hostile-same-UID sandbox or universal absence-of-effects proof. No repository execution/provider credentials/container/PR authority was introduced.

## Findings and disposition

### Critical / high

None reproduced within this bounded extension review.

### Medium — M1 CLOSED: duplicate nested reply members collapsed before validation

Original repros accepted conflicting duplicate members because `AppReply.parse` called `jsonDecode` before checking member identity. The root-cause fix in `client/lib/app_api.dart` preflights decoded names independently per object, scans strings atomically, distinguishes array scopes and caps containers at 64 before whole-document decoding. Frame/correlation and uncertain-mutation handling remain. Escaped aliases, Unicode identity, punctuation/backslash parity, mixed depth boundaries, malformed syntax, generated valid JSON and multibyte frame bounds passed. The exact original three-case test now passes unchanged, including the retained closeout rerun. No weakened product test or reviewer repro was used to close M1. Depth bounding is not a heap quota.

### Medium — M2 OPEN: historical installer artifact-guard failures remain unexplained

The service owner's retrospective record reports `artifact::tests::dependencies_reverified_after_exchange_before_success` and `launch::tests::native_launch_reports_spawn_then_reaps_even_without_receiver` failing during an earlier installer run (103 passes, two failures, two ignores). Original standalone logs/final-source binding were not retained for that run. Later complete native/root runs passed without an unrelated installer repair. This review does not infer a concurrency cause, erase the failures, or claim a demonstrated guard defect. Final required configured checks are verified passing; the residual is historical reproducibility/stability uncertainty, not a currently failing gate.

**Follow-up:** retain original source-bound output and fixture identities on recurrence; investigate the guard/fingerprint drift rather than add retries, weaken assertions or silently serialize validation. Accepted-risk authority remains with the human, not this review.

### Low — L1 CLOSED: stale closeout wording in the architecture-report evidence table

The parent corrected `docs/SMITH_LINUX_ARCHITECTURE_REPORT.md` section 8 to report the completed configured 1,462-execution aggregate plus 34 helpers, seven compiled corroborating cases and pass-with-risk source review, explicitly excluding installed/release/external approval. The independently inspected diff changes only that stale row. Current supporting/change/result hashes bind the correction; this reviewer did not edit the parent report. No additional execution is claimed.

### Low — L2 CLOSED: blanket no-Git wording omits synthetic fixture operations

The parent corrected the bundle README, approved-plan outcome and `results.json` limitations to distinguish **no Git mutation of the real Smith checkout, no project repository management and no remote mutation** from approved helper commits/tags/dirty states in private disposable synthetic fixtures. The ignored exporter now emits exactly that corrected limitation. The diff, retained exporter original, regenerated records and unchanged helper logs verify this documentation-only closure. Source-checkout mutation absence remains scoped to reviewed agent actions/disclosed workflow, not an audit of all concurrent host activity.

### Environment wording and target-policy limit

`docs/PROCESS.md` describes native quality on Ubuntu24.04, while the actual service-quality container in `dagger/src/smith_next/main.py::_service` is Bookworm. Installer/GTK/runtime Ubuntu evidence is not Ubuntu service-quality evidence. The final bundle and root validation disclose this correctly. The source verdict is for the configured/as-built gates, **not a declaration of complete target-policy/platform acceptance**; no supplemental supported-host service run or policy exception is inferred or approved here.

## Evidence gaps and separate gates

No required **configured local source** check is missing or currently failing in the bound evidence. Earlier owner self-check prose is not used to substitute for the observed final root gates. Original reviewer harness failures, parent focused-test failure, root attempt-1 infrastructure failure and subsequent passes remain separate records. The same existing Dagger builder restart is a disclosed build-tool effect; its immediate failed `buildctl` probe and later successful official CLI probe are both preserved. Neither explains the original stopped-builder cause or grants runtime container authority.

Still outside this verdict: native graphical/IME/accessibility/multi-monitor acceptance; live installed disk6 migration/restore and compatible paired-state/keyring rollback; normal-session manager/keyring/provider/model/GPU behavior; complete supported-host service validation; clean-host/clean-source provenance; effective dependency locking/hermeticity; license/icon rights, SBOM and publication clearance. Remote link reachability and exhaustive anchor validation were not tested. These are **not PASS**, and need their own evidence and applicable explicit human approval.

Selected raw originals remain under ignored `target/project-repositories/` and are digest-bound by the parent. Important reviewer failure/closure records and representative tests are copied under `reviewer/`; [reproduction notes](reviewer/repros/README.md) distinguish runnable retained parser repros from historical relocated UI/native source evidence. Local hashes are drift detection, not a retention guarantee.

## Final decision and required follow-up

**Source verdict: pass-with-risk, narrowly scoped as above. Release Gate Decision: no-go.** No release/installed/external approval or accepted-risk decision is granted. Preserve M2 and the original failure/review lineage; L1/L2 are closed by the verified documentation-only correction and fresh supporting/review binding. Retain actionable evidence if M2 recurs, and obtain the missing separately scoped operational/release evidence plus exact human authorization before consequential external effects. Any change to bound source, documents, raw records or artifact bytes invalidates this seal for the changed candidate.
