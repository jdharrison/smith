# Root promotion — independent assurance/adversarial review

**Final closure: PASS for the bounded full-branch/root-promotion local source/build/package scope. G1, F1 and L1 are CLOSED; no open local-source blocker was identified. Release Gate Decision remains NO-GO.** See “Final independent closure” below for exact final evidence and documentation bindings. The initial review and its then-correct pending/FAIL disposition are preserved as history, not the current verdict.

## Validation scope

- Fresh source-oriented review of the approved `LOCAL-SMITH-ROOT-PROMOTION` cutover: exact tracked dispositions, promoted inputs, current policy, build/pipeline paths, history retrieval, legal resources and portable metadata.
- Full-branch integration assessed against `main` **`b75636265d7060f063c54a2f890dd5a136db2dc2`**, through checkpoint/HEAD **`168865b0422b787f08ab33e89d052cd4adbd11f7`**, plus the uncommitted promotion and concurrent pipeline work. This is not a naïve comparison of the obsolete root client with the replacement client.
- Read `AGENTS.md`, Scope, applicable Architecture/Process/Design/Host Profile, `roles/assurance.md`, the approved plan, disposition ledger, retained reviews and relevant source/tests/workflows. Historical reviews are reused only for their bound scope; they do not validate relocated paths or confer approval.
- **Only this file is authored.** Reviewer actions were read-only Git/Python/source inspections. No gates, tests, builds, engine/manager/GUI/keyring/provider/model operations, source fixes, staging, commits, cleanup or remote effects were performed.

**Claim type:** implemented-source/evidence assessment, not deployed guarantees or human acceptance. Review cutoff: **2026-10-16T16:20:44Z**. The second validation harness attempt is parent-owned and pending binding; this document does not anticipate its result.

## Findings by severity

### Critical

None established in the inspected cutover or integration seams. This is not an exhaustive whole-product security certification.

### High — G1: final promoted-tree validation/evidence gate remains open

**Acceptance blocker, not a demonstrated production-source defect.** Plan AC6/AC7 and `docs/PROCESS.md:149–159` require the final root aggregate, package, helper/workflow checks and source/artifact binding before integrated acceptance.

At inspection, `validation.json` retained the first attempt's `status: FAIL`: aggregate Make exit **2**, build not reached, explicit package verification exit **1**, and actual-package helper verification blocked. That attempt's module discovery selected the wrong schema before application tests ran. It is not evidence that the service/client test assertions failed, nor a passing build. Root `VALIDATION.md:3–9` correctly leaves current acceptance pending.

The operator reports a diagnosed harness issue: an ignored nested source snapshot without its own Git boundary; a reversible scratch-only Git boundary was added for the second attempt, with original failures preserved as `attempt1*`. This review does not independently certify that diagnosis, the scratch mutation, completion of the second run, or any final artifact. No production fix is demanded from this first-attempt failure alone, and no blind rerun was initiated here.

**Required closure:** parent must bind the completed final commands/exits/logs, stable complete inputs, controlled ignores/skips, actual exports and package/service equality; retain attempt 1 and identify the harness difference. Independently verify those final results and rebind assurance to any changed inputs. Do not treat a running command, earlier 1,262 passes or this source assessment as final validation. A fresh final verdict must follow that evidence.

### Medium — F1: retained current cloud contract has an unresolved retired-document dependency closure

**Concrete documentation/path defect within plan AC5, not a cloud execution/consent bypass.** `docs/plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md` is deliberately retained as the **current** catalog/adapter contract, linked by `docs/SCOPE.md:50`, `docs/ARCHITECTURE.md:71`, and `docs/PROCESS.md:59`. It is not merely an explicitly historical review.

Its local links still resolve to retired files:

| Contract lines | Missing local target | Significance |
|---|---|---|
| 3, 31 | `LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md` and anchors | Parent approval/API freeze and concrete source seams |
| 9, 57 | `LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION.md` | Exact catalog expansion/research/architecture provenance |
| 55 | `LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS.md` | Explicit amendment to current attempt-eligibility semantics |

**Reproduction:** resolve each Markdown destination relative to `docs/plans/`; the target is absent. The same objects are available via `git --no-pager show 168865b0422b787f08ab33e89d052cd4adbd11f7:docs/plans/<target>`. This is therefore broken current navigation/closure, **not lost historical bytes**, and not grounds to restore all old plans.

The general historical-evidence warning in `VALIDATION.md:23` and the evidence index does not clearly classify these dependencies inside this actively referenced contract. In particular, the saved-key amendment link is presented as a current contract dependency. Plan sections “Documentation and history contract” and AC5 require current contracts and historical navigation to remain interpretable.

**Smallest remediation:** give these dependencies explicit full-checkpoint links, or add a clearly scoped relocation note/map identifying all three historical dependency records while keeping the active contract readable. Preserve historical source claims, hashes and outcomes; no global replacement of historical evidence is needed. Check the target anchors as well as the blobs. This active-contract closure should be resolved before claiming plan-complete source/document acceptance; it does not require a runtime code change or new host permission.

### Low — L1: current as-built pipeline assessment retains a superseded inspection statement

`docs/SMITH_LINUX_ARCHITECTURE_REPORT.md:61` says the inspected workflow “still contained pre-promotion paths and a retired root-Cargo gate.” The **current** `.github/workflows/ci.yml:74–95` already uses root Make/package paths and has no root-Cargo gate. Its surrounding wording partly identifies a prior inspection, so this is not evidence of unsafe executable CI, weakened validation or a false PASS.

**Follow-up:** qualify the statement as pre-repair inspection or update it when parent binds final validation. Nonblocking documentary accuracy item, distinct from G1 and F1.

## Checks performed and integration assessment

### Exact disposition, inputs and retrieval

Independent read-only reconstruction used `git ls-tree -rz 168865b`, `git cat-file blob`, SHA-256, current destination reads and executable-bit checks, rather than rename heuristics.

- Ledger contains exactly the **1,721** baseline entries: **1,231 promote, 454 retire, 34 keep, one root README replacement and one next README replacement-source disposition**. Every original path, Git blob, SHA-256 and Git mode matches the baseline tree. All mapped destinations exist with the expected executable classification; no missing promoted file or executable-mode loss was found.
- **24 mapped baseline records differ in content**, including both README records targeting the same root README. Differences are root controls/metadata, the six policy/report documents, Make/validation, component documentation and the disclosed Dagger engine-version configuration. No promoted Rust/Dart/C++ application source, component script, test, lockfile, golden/font/icon/notice payload or Dagger Python implementation changed against its mapped baseline bytes.
- Independently parsed the **original** archived manifests from Git and hashed their payload objects: **46 voice + seven Python-installer payloads**, zero mismatches. This does not rely only on the ledger's `verified` flags.
- `git merge-base --is-ancestor 168865b HEAD` returned **0**; HEAD remains exactly the baseline. `git rev-list --left-right --count b756362...168865b` returned **0 / 14**. Reachability is established locally now, not guaranteed after an unapproved future history rewrite/deletion.
- All **22** simple literal Rust `include_str!`/`include_bytes!` paths found in promoted Rust inputs resolve. This complements complete file preservation; it is not compilation.
- Root `LICENSE`, retained font OFLs and embedded resource notices remain preserved. `client/assets/README.md:16–17` still explicitly blocks icon redistribution without rights. Preserving notices is not a complete license/SBOM clearance.
- Ignored leftovers are expressly outside Git-backed recovery. The ledger/evidence describes preservation under ignored scratch, not checkpoint recoverability. No deletion or blanket cleanup was requested or performed by this review.

### Full branch, prior assurance and unchanged runtime behavior

The 14-commit branch includes earlier orchestration/legacy work which is now intentionally retired. Treating its old root client as the promoted source baseline would manufacture false deletions and miss the actual product. The reviewed final integration instead consists of the mapped next product, retained governance, concurrent pipeline and approved retirement.

Read the retained whole-cloud-checkpoint review and saved-key final closure, plus Git-pinned multi-chat, GPU-startup, installer-redesign, cloud-service and cloud-integration reviews. Their histories retain earlier failures, remediation and distinct scoped verdicts. Neither a cancelled older native-migration review nor a later narrow PASS is promoted into whole-product release acceptance.

The pipeline review's **1,230-input build manifest** was independently compared through the relocation map. Exactly eight inputs differ: `Makefile`, `client/README.md`, `dagger/INSTALLER.md`, `dagger/README.md`, `dagger/pipeline.md`, `installer/README.md`, `installer/assets/README.md`, `service/README.md`. Make differs from the reviewed concurrent pipeline only in its help location; the remaining changes are documentation, with `dagger/INSTALLER.md` embedded into the package and therefore requiring fresh artifact binding. All executable/component-test inputs in that manifest remain byte-identical. This supports reuse of prior source findings, **not inheritance of a relocated-tree test PASS**.

Risk-focused current-source inspection included:

- `service/src/app_v3_worker.rs:46–186`: original permission/identity checks, fresh exact-generation retrieval, durable may-have-sent publication, recheck before one HTTP attempt, bounded/sanitized error handling and final-secret echo rejection remain. Source promotion does not add a fallback credential/provider or ambient authority.
- `client/lib/native_control.dart:15–25`: packaged helper lookup is executable-relative; removing the repository `next/` directory does not change that lookup. Installer's `../service` dependency and independent workspace remain valid (`installer/Cargo.toml:21–38`).
- `dagger/src/smith_next/main.py`: explicit component inputs, non-root workers, compiled service/cloud fixture injection, real native CLI/GUI package fixtures, isolated Xvfb and no-Python recipient gate remain. No home/session bus/user-manager/GPU/model mount or installer/run dependency was added to test/build.
- Complete mapped source equality preserves `smith-next.service`, launcher/XDG/keyring/wire identities and deterministic maintenance recognition bytes more strongly than searching for zero `next` occurrences. Remaining executable `next/` matches are runtime namespaces, deliberate negative-layout tests or historical comments, not an identified stale production source path.

### Pipeline, approval and policy preservation

Compared current helpers/workflows against the exact concurrent pre-cutover originals and retained pipeline review, not just the old baseline publisher.

- Release caller is unchanged. CI removes only retired root-Cargo checks and rebases next paths. Both candidate checkouts still select the full event tag ref; permissions remain read-only, actions pinned, checkout credentials disabled, jobs bounded, tag candidate uploads success-gated with 14-day retention. Main/PR does not upload distributables; no GitHub Release/tag/deployment creation is introduced.
- `scripts/prepare-installer-release.py` changes only package-loader and component-version paths. Canonical annotated tag, event commit/HEAD, fetched-main ancestry, clean tracked source, both locks/versions, safe archive member/mode bounds, extraction/reverification and source recheck remain. Tests add root/next-decoy and next-only refusal cases, not removal of source identity assertions.
- Synthetic actual-package roundtrip remains explicitly opt-in via `SMITH_TEST_PACKAGE` (`scripts/tests/test_installer_release.py:436–446`). A green default helper suite can still skip it; parent must enable and record it for final cutover evidence.
- Six rewritten canonical/report documents preserve implementation-versus-target distinction, untrusted text, exact human effects/expiry, independent review, no self-approval, separate push/merge/tag/upload/release/deploy/destructive authority, source-versus-host evidence, no replay, live migration/paired restore/keyring boundaries, and outstanding legal/provenance/host gaps. Intentional legacy support withdrawal is approved; it is not an accidental safety-policy deletion. No concrete permission expansion or active safety-policy loss was established.
- `.smith/project.yaml` uses relative locators and root commands, removes private Jira binding, retains independent/human approval metadata and expressly disclaims an implemented consumer/enforcement. `make test` includes quality checks; empty descriptive format/lint lists do not remove executable component format/lint gates. No confirmed credential/private operator path was found in the bounded candidate text scan. Three syscall-log home-prefix pattern hits were shared tool-directory references, not operator account paths; raw contents were not emitted.

### Current links versus original historical evidence

A read-only Markdown-link scan found **60 full-commit Git links**, all with locally retrievable target objects. This checks local object availability, not GitHub access or vendor URLs. The root README, History and six canonical/report entry points have no missing relative file target in this scan.

Retained checkpoint/saved-key reviews, client validation records and the dated golden-cleanup record contain original relative links/next paths. Those are explicitly historical in their own dated context and/or root `VALIDATION.md:23`; rewriting their original commands/hashes is not requested. The golden record's old `../../../../../docs/...` paths are not evidence that the PNGs or current widgets are missing. F1 is distinguished because the API contract remains an actively consumed current contract, not just dated validation evidence. The scan is not an exhaustive Markdown parser, anchor validation or network link checker.

## Required Checks Evidence

Reviewer checks below are read-only inspections, **not execution of the parent-owned gates**.

| Required check / command or scenario | Expected | Actual result/status | Evidence |
|---|---|---|---|
| Approved base/ancestry/full branch | Exact baseline, reachable history, known main span | PASS; exact HEAD above, ancestry exit 0, main span 0/14 | Reviewer read-only Git commands; plan baseline |
| Exact ledger/hash/mode/path audit | Complete disposition with no missing live inputs | PASS; 1,721 entries, no blob/hash/Git-mode mismatch, all mapped destinations present | `disposition.json`; independent Git/Python comparison |
| Original archived payload retrieval | Exact original manifest contents recoverable | PASS; 46 + 7 payload hashes match, zero mismatches | Original two `reference/*/MANIFEST.json` Git blobs and payload objects |
| Runtime identifiers/live fixtures/legal resources | No unintended byte or executable loss | PASS for source preservation; application/test/resource equality and 22 literal Rust includes resolve | Mapped source comparison; component manifests/resources |
| Current policy/README/history/metadata | Accurate support and authority boundary, usable active closure | No demonstrated safety loss; F1 current-contract links unresolved; L1 documentary qualification | Current canonical docs, README, History, metadata, contract |
| Pipeline integration/helper source | Root paths without weakening tag/archive/approval boundaries | Source inspection PASS; no new execution claimed | CI/release YAML, helper/tests, exact prior originals and retained review |
| Root `make help` | Root developer entry works | Retained first-attempt exit 0; not rerun | Inspected `validation.json` command record |
| Root aggregate `make test build` | All configured components/compiled integrations/GTK/runtime/builds complete on stable source | **NOT CLEARED**; first attempt exit 2 before tests; second harness result awaits parent binding | Inspected first-attempt `validation.json`; operator's second-attempt status |
| `python3 -B dagger/package.py --root installer/dist --verify` | Fresh actual full package and pairing verified | **NOT CLEARED**; retained first attempt exit 1 after build was not reached | Inspected first-attempt record; final artifacts/evidence pending |
| Helper suite including actual package | Root regressions pass and actual-package test is not skipped | First helper command recorded exit 0, but actual-package closure not established | `validation.json`; opt-in test at lines 436–446; parent final evidence pending |
| Workflow/shell lint, whitespace | Final root syntax/invariants clean | Prior pipeline review has historical passes; **no fresh final result certified here** | `docs/evidence/installer-pipeline/REVIEW.md`; parent must bind current results |
| Final stable inputs/exports and fresh acceptance | Final source, artifacts, logs and assurance agree | **PENDING**; source comparison performed, no final second-attempt package certified | G1; parent-owned final manifest/artifact evidence |

### Review identity anchors

- Disposition manifest SHA-256: **`950c2f930441b3b71d85c4e02bce95d4b44e558a8d58b0eb9e3c680475eabc0f`**.
- Read-only review inventory: **1,278 existing candidate files**, SHA-256 **`33d7ed4ffcee327f83f26be58c75890bae82329f721ef4c1f3706626fb9d8865`**. Recipe: union of non-null ledger destinations and `git ls-files --others --exclude-standard -z`; exclude `docs/evidence/root-promotion/**` so this review and concurrently written validation evidence are not self-referential; keep existing regular files, path-sort records `{path, sha256, size, git_mode}` with executable classification `100755`/`100644`; hash `json.dumps(records, sort_keys=True, separators=(",", ":")).encode()`. This is a source-review binding, **not** the parent's build-input inventory or a future Git tree.

| Inspected current input | SHA-256 |
|---|---|
| `.github/workflows/ci.yml` | `30c7e766e7b50f0f32f3c173427e9c23e4edfa643f8144cb781fd3c9ab8c3b0a` |
| `.github/workflows/release.yml` | `3fdfae46d6d29f3790100db60a906d0f4c2a599e258a64e26511d673b2b7dfa0` |
| `scripts/prepare-installer-release.py` | `248d35d053a99618ef55d4262b3245cfdaddc7520c5008c7aaf3acfc57b227fa` |
| `scripts/tests/test_installer_release.py` | `7bd14d6891095f6439d90e8f98b41f6c90470c70d25450b51cc204133d0dab33` |
| `scripts/tests/test_pipeline_config.py` | `f4489b8cf5c51483a591c8ee1adebcb3fe22c802723384b358f65a1bcf727454` |
| Current cloud API contract | `7ff4d1603cd876568e1b22cc3d55b01dc2f998ba8f09559245039f1a1970c11f` |
| Retained pipeline review | `5c710a73a52b0f8e6d3f64a0995595574da69469c69f290bb0a4ab8c3ba68cc6` |
| Retained whole-checkpoint review | `d868537fadee6dcc36bf8079f7178fc2223f73f430cd2387a6a7c9ae30d89b4c` |
| Retained saved-key review with final closure | `c70c6462d29a7e5c11aa465ce15a5f74de475470069aa0842d0cffcab57ff075` |

Owner-writable digests establish local comparison, not signed provenance, an independent external authenticity anchor or continuous edit monitoring.

## Evidence Gaps and scope separation

1. **Promotion/plan acceptance:** G1 final validation/binding and F1 active-contract closure remain open. Source preservation and inspected runtime/authority seams have no concrete regression finding; they can stand independently of a pending build. They do not by themselves satisfy AC6/AC7. This is not a final all-checks-PASS review.
2. **Historical evidence:** previous scoped PASS/FAIL decisions remain bound to original source and environment. Local raw outputs are not guaranteed reconstructable from Git. Historical runner timing/lease and CLI timeout failures are not diagnosed, erased or accepted as harmless by a later successful execution.
3. **Broader release/host obligations, not new source-relocation bugs:** clean-source provenance/effective dependency locking; supplied-icon redistribution, complete licenses/SBOM; hosted Actions/retention/settings; real recipient graphics/IME/accessibility/windows; real manager/cgroup/login/reboot/maintenance; actual CPU/GPU/model resources; normal-session keyring/vendor entitlement/charges; disk4 pre-snapshot and tested compatible paired state/keyring restore. These remain release/deployment blockers where required, not justification to broaden this read-only review or restore retired products.
4. **External action authority:** no commit/push/PR/merge/deletion/tag/upload/release/install/deployment is approved here. Recheck exact final revisions, checks, independent review and human scope immediately before any separately authorized effect. Source integration readiness would still not equal release readiness.

## Verdict and required remediation

- **Source-preservation/runtime-boundary assessment: PASS for the inspected scope.** No missing promoted fixture/legal payload, changed runtime identity, unintended executable drift, archival byte loss, active safety-policy loss or new CI authority expansion was established.
- **Plan-complete source/document assessment: NOT YET PASS**, pending F1's small current-contract closure repair. L1 is a nonblocking documentary qualification.
- **Overall assurance verdict: FAIL / pending required evidence**, under `roles/assurance.md`: required final validation is not yet independently bound. This is not a prediction that the second build fails. Final integrated acceptance must await G1 closure and affected-scope re-review.
- **Release Gate Decision: NO-GO.** No risk accepted and no external action authorized.
- **Required remediation:** resolve F1 without rewriting historical results; parent completes and binds the final root gates/artifacts while preserving failed attempts; independent assurance verifies that final evidence and any intervening changes before issuing a final acceptance verdict. Update L1 with the final source assessment when practical. No production-source change is requested on the findings established here.

## Final independent closure — 2026-10-16

### Closure scope and current findings

**G1, F1 and L1 are CLOSED. No critical, high, medium or low finding remains open in the bounded local-source scope.** This closure supersedes the initial current-task pending-evidence verdict above, not the failed first execution, older product failures or release/host limitations.

Fresh read-only checks verified the final parent-owned results, original logs, snapshot/live inventories, actual exports, preserved failure records and later documentation deltas. Reviewer also executed only static `actionlint`, ShellCheck, shell syntax and Git whitespace checks. No aggregate, test suite, build, package verifier, artifact execution, Dagger/engine action, normal-host operation, staging, commit or remote action was initiated by the reviewer. Only this review is edited. Evidence/source cutoff is **2026-10-16T16:29:32Z**, with subsequent static/raw-log reconciliation before this append.

- **G1 CLOSED:** the corrected isolated snapshot completed configured root `make test build`, explicit package verification and the final actual-package helper suite. Exact command exits/log hashes, raw component counts, full snapshot inputs, live core inputs, package identities and root export copies were independently checked. This is actual final evidence, not inheritance of a pre-promotion PASS.
- **F1 CLOSED:** current API contract lines 3, 9, 33, 57 and 59 now point to full `168865b0422b787f08ab33e89d052cd4adbd11f7` Git blobs for the three historical records (two expansion citations). Both provider-plan anchors exist in the pinned blob. The added navigation note explicitly separates historical parent/follow-up records from current canonical policy. Snapshot-to-live diff changes only those destinations and the note; catalog/request/eligibility semantics and dated research remain unchanged.
- **L1 CLOSED:** current report line 61 accurately describes root component/package CI paths, no retired root-Cargo gate and root-layout negative tests. Its lines 7 and 93 now attribute the actual fresh root results without claiming hosted CI, release approval or validation from prose alone.

### Required Checks Evidence — final actual results

Let **D** = `docs/evidence/root-promotion/`, **F** = `target/root-promotion/validation-v2/`, **S** = `target/root-promotion/validation-source-v2/`. These are retained parent executions unless explicitly marked reviewer-executed.

| Required check / command or scenario | Expected | Independently verified actual/status | Evidence |
|---|---|---|---|
| Corrected snapshot discovery | Expected module, no product workaround | PASS; fresh empty Git boundary, no staged entries, all **13** expected functions present in preflight log | D `validation.json`; F `validation-functions-preflight.log`; snapshot Git inspection |
| Root `make help` | Root entry works | PASS, exit 0 | Hash-matched F `validation-make-help.log` |
| `make DAGGER=<verified-v0.21.9-cli> test build` under recorded privacy environment | All configured tests, quality/runtime branches and paired builds complete | PASS, exit 0; **270.81s**, no timeout; command window within **16:18:29–16:25:37 UTC** | D `validation.json`, `component-results.json`; hash-matched F `validation-aggregate.log` |
| Raw aggregate counts | No failed suite or omitted component | **263 service + 281 installer + 9 GTK + 668 Flutter + 41 package = 1,262**; zero failed. Independently summed **8 service / 12 installer / 1 GUI** Rust summaries; five controlled Rust fixture ignores; zero Flutter/package skips | Raw trace nodes **22 / 62 / 73**, Flutter `+668: All tests passed!`, package `Ran 41 tests`/success; D component record |
| Quality, compiled integration and runtime wiring | Current format/analyzer/Clippy, actual fixtures, isolated GTK and no-Python CLI | PASS as successful configured aggregate branches; **81 files / zero changes**, analyzer no issues. Both `SMITH_TEST_SERVICE` and `SMITH_TEST_CLOUD_SERVICE` present; package has actual CLI/UI fixture variables. Xvfb GUI and `/fixture/smith-installer` help/version/artifact dry-run plus absent-Python checks present | Raw aggregate; unchanged component quality scripts and Dagger graph; D component results |
| Executed CLI/server | Matching reviewed versions | All four aggregate connections report **client/server v0.21.9** | Raw aggregate connection records; recorded CLI identity |
| Explicit snapshot package verification | Newly exported full recipient accepted | PASS, exit 0; **95 manifest-listed files / 97 total** | Hash-matched F `validation-package-verify.log`; D command/artifact records |
| Final helpers with actual rebuilt package | No silent actual-package skip | PASS, exit 0; **34 tests, zero skips**, explicit actual-package archive roundtrip; archive digest below | Hash-matched F `validation-helpers-actual-package.log`; D component/artifact records |
| Root exported package verification | Root copy, not only scratch package, accepted | PASS, exit 0; root destinations `service/dist`, `client/dist`, `installer/dist` inspected independently | F `validation-root-package-verify.log`; actual root exports |
| Source stability/binding | Same complete selected inputs and unchanged core | All **1,288** snapshot records match final snapshot bytes/size/mode; full digest matches. All **1,230** core records match live source and first attempt exactly; no missing selected input or new unreviewed source candidate | D source manifest; independent complete reconstruction; delta classification below |
| Export content/modes/pairing | Actual source-bound exports match snapshot/manifest | Exact root/snapshot/record tree equality: **91 client + 97 recipient + 1 service = 189 files**; directory modes included; four service copies equal; all 91 client/payload files equal | D `artifacts.json`; actual S/root export trees |
| Package manifest/checksums/ELFs/guide | Exact members/hashes/modes, native pair, current guide | Exact 95-file manifest set and all hashes/modes; all **96** checksum entries verified; actual CLI/UI/service/client headers are x86_64 ELF; `README.txt` equals current `dagger/INSTALLER.md` | Actual `installer/dist/`; D artifact anchors |
| Workflow/shell syntax — reviewer-executed | Current final source lint clean | `target/installer-pipeline/actionlint .github/workflows/ci.yml .github/workflows/release.yml`, `shellcheck scripts/install-dagger-ci.sh`, `sh -n scripts/install-dagger-ci.sh`: each **exit 0**, no diagnostics | Reviewer static command results; unchanged current workflow/helper identities above |
| Whitespace/index/ancestry — reviewer-executed | No whitespace, staging or historical reachability regression | `git --no-pager diff --check`: **exit 0**; cached diff empty; baseline ancestor query **exit 0**; HEAD unchanged | Reviewer read-only Git results |
| Active links and historical targets | Current navigation resolves without rewriting old evidence | **21 current entry/contract documents** checked; **65** pinned Git targets and **2** pinned anchors resolve; no missing local target/heading in this bounded scan | Current files, local Git blobs; F1 repair |
| First failure preservation | Original FAIL stays immutable/readable | All four `attempt-1-*` JSON hashes match final-record anchors and exact copies in S; all original command log hashes match; original aggregate remains exit **2**, package exit **1** | D `attempt-1-{validation,source-manifest,component-results,artifacts}.json`; original ignored logs |

The early `helpers-before` execution had **33 passed / one actual-package skip** before the package existed; it is not represented as zero-skips. The later 34-test run explicitly executed that case. The five Rust ignores remain controlled child/harness entrypoints; they are not Flutter/package skips or five unexecuted acceptance tests. Package 41 is counted once inside the aggregate, not again from explicit verification or the helper archive test.

The raw log contains ANSI controls. An initial anchored node parser returned zero summaries until ANSI escapes were removed in memory; the corrected parser then reproduced the exact 263/281/9 totals. No log/result file was changed. The complete gate is cache-using configured execution, **not** a clean-cache/hermetic rebuild; identical earlier ELF hashes are not claimed to be new reproducible-build provenance.

### Harness cause and preserved failure

Verified diagnostic `RESULT.md` against its bound SHA-256 **`1ec7f4f142116b718241295d87aa60cc97edb5c8fbe6388860bf46619c596248`**; inspected `boundary.json`, `reversible.json` and their actual discovery logs. No-boundary and boundary-removed runs expose only the example functions; adding/restoring the same scratch Git boundary restores Smith functions, with recorded exit 0/no timeout and no product-byte edits. The diagnostic's recorded function extraction lists nine Smith names, so that extracted list alone is not evidence of a complete 13-function set. The final attempt's separate preflight and actual log independently verify all **13** expected names. This does not weaken the expected set to nine.

This supports the bounded observed cause: ignored nested snapshot/Git-boundary topology, not defective promoted Make/application code. It does not establish Dagger's internal filtering algorithm. Diagnostic `RESULT.md` retains its then-current pre-build NO-GO; later validation does not rewrite that historical statement. The first validation JSON is still `FAIL` with its original failure chain.

All first-attempt JSON SHA-256 values independently match:

| Preserved first-attempt record | SHA-256 |
|---|---|
| `attempt-1-validation.json` | `920d646186ab31c45bf3451e70dbda2c8124244795d7eb4ee7ade617f03f8aad` |
| `attempt-1-source-manifest.json` | `ace00b30647885c9d34a487b7262a04187d443e7f830e003aa8a443017a3e665` |
| `attempt-1-component-results.json` | `5ee0eada448196f12259b83055a817e618de55a28ab47e2ce9538025e1ca8c41` |
| `attempt-1-artifacts.json` | `0ac0b3bd78fc77bb91b88fffadf96074db9c391af57fb238dc6f599d678a60e7` |

### Exact final source and documentation binding

The complete tested snapshot's canonical records digest is **`e39161932c5914a01af649804a29f7a9d5020455cced71d0fc9763bb0b22ee16`**. Independently reconstructing all 1,288 `{path, sha256, size, mode}` records from S reproduces it. Core means the four component prefixes plus `Makefile`, `dagger.toml`, `dagger.lock`: **1,230 records**, independently reproduced live/snapshot/first-attempt digest **`0617ce1780c0f88d5f325c4ee9e104aaf61e38905a3d1a6544945fc586a82670`**. Both digests use path-sorted records and compact sorted-key JSON; actual modes retain the source manifest's octal-string representation.

Comparing every selected snapshot record with live source found exactly **nine** changed files: the four finalized root-evidence JSON records plus the following five documentation files. Diff inspection confirms documentation-only outcome/navigation closure, with no executable/test/build or embedded recipient-guide drift. The only new selected candidate since snapshot is this review. No selected file is missing.

| Later documentation-only file | Reviewed purpose | Final SHA-256 |
|---|---|---|
| `VALIDATION.md` | Actual fresh results, retained failure and final-evidence navigation | `70da7bab1978c42e8b35d69716e3681a1b0958e42e92f40c1f7511fd680e075b` |
| `docs/plans/LOCAL-SMITH-ROOT-PROMOTION.md` | Preflight prerequisites resolved; implemented outcome and unchanged stop boundaries | `5ed3dd05dc6d514700de274f4f610dd6c353769439c635f73df91e3ab69c82c4` |
| `docs/evidence/root-promotion/README.md` | Final evidence/failure/core/export index, no inherited release acceptance | `e80327a0ffccf0319eebf14c476914edb3b9388050349c4f91573b8bcbecb586` |
| `docs/plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md` | F1 historical links and navigation note only | `b2b22535954120aa19e4bec471ed0e0aa1abc9f0cc80b8c79969ff2f5f3cef06` |
| `docs/SMITH_LINUX_ARCHITECTURE_REPORT.md` | L1 corrected current CI assessment and attributed new evidence | `784885cfdbe6c7c485a607bdaa0c88f70eec442443520141e6ef30fb36a2e658` |

The final review inventory using the earlier **Git-mode/exclude-root-evidence recipe** is **1,278 files**, digest **`e21323f2a43a74dad46a835e90fb5060cea8c0acffac59863ed393b0d19b140a`**. This differs legitimately from the original review inventory due to the reviewed documentation closure. It is not substituted for the tested snapshot digest or represented as a committed Git tree. Pre-closure review-file SHA-256 was **`0d3635eecfc56c5684fb94a3b384f0c41cfbdc6cfbe248c1f736a56fc0d8cc71`**; its initial findings/history are retained above, with a final-status pointer added.

### Final evidence, logs and artifacts

All recorded final command logs independently match their SHA-256 values. All referenced final/prior JSON hashes match, not merely the narrative PASS fields.

| Final record/artifact | SHA-256 |
|---|---|
| D `validation.json` | `009e7588742a2825a9573f441714970e570f3970ce9ee846376a3860d2089859` |
| D `source-manifest.json` | `12e79af9e5a26fbf62cbb55a91b94b2900127f2686f340c35cf0f41d5dc997de` |
| D `component-results.json` | `269068a07de6e2e42a94f1aaaa509c1589af10396cb7cccee298d2e2bd90ad0e` |
| D `artifacts.json` | `29f02fdefa8156271f1cfaa228e6909d079fd5ab4a4be098d239242c3c263cfb` |
| F `validation-aggregate.log` | `1ef5542cf6732e8c608603618d506f48cb1a755e723660d4bbc88a19a15240d1` |
| F `validation-helpers-actual-package.log` | `d34b0125feeb3ec3067d99ac489ed66da03a1e187e746ec79375f1f2ec71c6cf` |
| F snapshot/root package-verification logs (each) | `4f6e997052bf8ed0fe4e97c0e068bea00d34f1891dc7ee807e21581cdbb6da2e` |
| Four equal service copies | `5cfe584b8e597771319b3d4a3da9572bd30df9059d89f4f68e6e38fd5b2c4dcb` |
| Recipient `manifest.json` | `52804bfae4a71ff783cff3a0c299e7e9b491b2d79c9d05ea22c0eafe3e7d5102` |
| Recipient `SHA256SUMS` | `1117369bf4408b9b92b614ec458f9a04e6ffe5f9ea25ab8d1a04c0bc13e974b1` |
| Actual-package helper roundtrip archive (recorded log digest) | `6dc1d066e17e5ede16c84496bd8ddcbcd398d6c44ce97dd09dc4662ef5627daf` |

All export record tree digests also reproduce from their actual byte/size/mode/kind/path entries. Standalone service, client-bundled helper, packaged standalone service and packaged client helper all agree. Header inspection is not artifact execution; helper archive roundtrip uses synthetic Git source identity and is not proof the package was built from a clean annotated release tag.

### Full-branch inventory/privacy closure and remaining boundaries

The original manifest-aware full-branch assessment remains applicable: checkpoint HEAD/base, all 1,721 dispositions, historical object retrieval and 53 archived payload checks are unchanged; executable/core inputs and source/pipeline seams remain exactly bound above. Later documentation changes do not broaden permissions, override policy, change runtime identifiers or lose live fixtures/notices. Current source inspection plus the new configured evidence closes local AC6/AC7 assurance without asserting every historical intermediate commit was a releasable product.

Reconciled the live selected candidate set against the snapshot and rescanned current candidate text, including later docs/evidence, for private account-home paths, private-key headers and common GitHub/cloud token forms. **No confirmed match** after excluding the already inspected shared tool-directory references. Source/inventory drift and new-file classification are closed for this bounded candidate. Pattern scans remain limited evidence, not a guarantee against all secret encodings or disclosure in arbitrary historical Git content. Build outputs remain ignored reviewed exports, not accidental new source candidates. Original historical review/evidence local links retain original context via History/index; they are not relabelled current contract defects.

**Evidence gaps remaining for this bounded local-source acceptance: none identified.** Outside it remain the previously recorded clean-source/hermetic/provenance, hosted Actions, licenses/SBOM/icon redistribution, real host/graphics/manager/provider/model/GPU/state/keyring-restore gates. No new evidence or accepted-risk authority closes those obligations. Raw logs/scratch and exports are local ignored objects, not promised Git-backed reproducibility.

### Final verdict and required follow-through

- **Verdict: PASS — bounded full-branch/root-promotion local source, configured build/test/package and evidence-binding assurance. G1/F1/L1 CLOSED; no open local-source blocker identified.** The inspected implementation meets the approved source-cutover contract and retains the relevant target-policy boundaries; broader target orchestration/host/release guarantees are not claimed.
- **Release Gate Decision: NO-GO.** Source acceptance is not release, publication, installed acceptance or human approval. No residual risk is accepted and no external effect is authorized.
- **Required local-source remediation: none.** Preserve the historical failure/evidence and this binding. Any later executable/test/build/embedded-guide drift requires relevant revalidation/review; documentation/evidence drift needs explicit classification and rebinding. Before any separately requested commit/push/PR/merge/deletion/tag/upload/release/deploy/install effect, recheck exact revisions, evidence, ancestry, independent review and named human approval. The candidate remains uncommitted; bootstrap/remote completion is not performed by this review.
