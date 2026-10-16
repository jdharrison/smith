# LOCAL-SMITH-ROOT-PROMOTION

## Status, intent and authority

**Approved local implementation; validation pending.** The operator requests promotion of the contents of `next/` to the repository root, removal of obsolete root material except specifically needed files, and completion of `feature/autonomy`. Git history, rather than another copied legacy source/archive tree, should provide historical recovery.

Planning baseline: `168865b0422b787f08ab33e89d052cd4adbd11f7`, existing `feature/autonomy`. At initial inspection the checkout was clean and the checkpoint was pushed. Local and independent read-only GitHub inspection found `main` at `b75636265d7060f063c54a2f890dd5a136db2dc2`, with the feature branch 14 commits ahead and zero behind; no existing PR was observed. These are inspection-time facts, not guarantees about the next implementation or merge revision.

The operator explicitly approved the proposed cutover (“yep perfect”), including legacy CLI retirement, Git-backed historical retention and unchanged installed identities. Follow-up approval permits preserving leftover local directories and clearing identified disposable caches; rebuild validation is approved. Approval binds this plan, repository `smith`, existing `feature/autonomy`, base `168865b0422b787f08ab33e89d052cd4adbd11f7`, and the independently checked exact disposition manifest. It expires on completion, revocation or material unreviewed scope/base drift. No commit, push, PR creation, merge, branch deletion, tag, upload, release or installed-host change is authorized. A possible later install must bind exact rebuilt artifacts and host effects separately. Later Git effects require separately bound approval under [Process](../PROCESS.md#approval-boundaries).

**Concurrent work:** `LOCAL-SMITH-INSTALLER-PIPELINE.md` and new `scripts/prepare-installer-release.py` appeared during discovery. That work is outside this plan's authorship and must be preserved. The pipeline plan describes active release-candidate tooling, tests, workflows and documentation changes. Its final state must be reconciled before freezing migration paths. Neither its presence nor its recorded intent grants this plan publication authority.

Risk: elevated source/build/policy cutover, with critical review of workflow permissions and any inherited release behavior. This is not a runtime identity migration.

## Objective and defaults

- Make the promoted product the **only supported active implementation** in the checkout.
- Preserve current behavior, tests, legal notices, operational identities, protocol/storage compatibility and approval boundaries.
- Prefer a small, understandable working tree. Retain historical bytes through an immutable, reachable Git commit, not a new `legacy/` or `docs/archive/` copy.
- Finish the bootstrap branch through reviewed integration into `main`; future ordinary work starts from updated `main` using normal branch policy.

## Decisions proposed for implementation approval

1. **Retire legacy root source/CLI support intentionally**, including the old `smith` orchestration/chat entry points. Do not retain compatibility shims merely to keep obsolete commands buildable. Update the current compatibility promise in `docs/SCOPE.md` and related canonical policy. This does not uninstall or change any existing legacy installation.
2. **Keep installed identities unchanged.** Removing the `next/` repository directory does not rename `smith-next.service`, the `smith-next` launcher or XDG namespaces, keyring identity fields, executable/protocol identity, manifests, desktop IDs or deterministic ownership-recognition bytes. A cosmetic runtime rename would require a separate migration plan.
3. **Use history-backed retention for retired tracked materials.** Preserve exact historical bytes/hashes through `168865b` and any later approved pre-cutover checkpoint. Keep current contracts and relevant evidence in the working tree; replace active historical navigation with commit-pinned links. Do not rewrite historical outcomes or manifests.
4. **Prefer a normal merge commit, not squash/rebase**, to preserve bootstrap/checkpoint ancestry. Leave local and remote `feature/autonomy` intact after merge unless separately authorized to delete it. No release tag is required to preserve history.

These decisions are approved for this local source cutover. The normal merge method is the recommended later integration path, not approval to perform a merge.

## Proposed final layout

```text
smith/
├── client/                 promoted Flutter desktop, tests and assets
├── service/                promoted independent Rust service crate
├── installer/              promoted independent native CLI/GTK crate
├── dagger/                 promoted builder/package tooling
├── scripts/                only audit-proven active repository/pipeline helpers, if needed
├── docs/                   current policy, active contracts, relevant evidence and history index
├── roles/                  existing development/review method
├── .github/                current ownership, PR template and Ubuntu workflows
├── .smith/                 portable/truthful self-project metadata, if retained
├── Makefile
├── dagger.toml
├── dagger.lock
├── VALIDATION.md
├── README.md
├── AGENTS.md
├── LICENSE
├── SECURITY.md
├── rust-toolchain.toml
├── .gitignore
└── .gitattributes
```

No `next/` active source tree, legacy root Cargo package, copied legacy archive, new root Cargo workspace, `apps/` indirection or framework replacement. The two Rust crates remain independent workspaces and the installer's `../service` dependency remains valid.

## Keep / promote / retire inventory

The baseline has 1,721 tracked entries: 1,232 under `next/`, 98 legacy `client/`, 46 legacy `src/`, nine legacy `scripts/`, 257 `docs/`, 57 `reference/`, seven `roles/`, four `.github/`, one `.smith/`, and ten root files. Counts must be recomputed after concurrent work settles.

| Existing location | Proposed disposition | Conditions / reason |
|---|---|---|
| `next/client/`, `next/service/`, `next/installer/`, `next/dagger/` | Promote tracked contents to root siblings | Preserve source, component scripts, locks, assets/notices, live fixtures, goldens and intended evidence. Do not merge new client files into the legacy client tree. |
| `next/Makefile`, `dagger.toml`, `dagger.lock`, `VALIDATION.md` | Promote | Preserve Make target semantics; repair active paths. Old validation remains historical, not validation of the cutover. |
| Root and next READMEs | Replace with one current root entry point | Reconcile actual cloud/app3/disk4/Core2 behavior; do not concatenate obsolete narratives. |
| Legacy root `client/`, `src/`, `Cargo.toml`, `Cargo.lock` | Retire tracked implementation | Explicit support withdrawal; preserve retrieval through checkpoint history. |
| Nine baseline root scripts | Retire | `bump-version.sh`, `check-docker.sh`, `install-prebuilt.sh`, `install.sh`, `list-agents.sh`, `package-chat.sh`, `smoke-prebuilt.sh`, `smoke-user-service.sh`, `validate.sh`; no active promoted build dependency identified. |
| New/current pipeline helpers under `scripts/` | Retain if consumed by approved active tooling | Do not apply the baseline legacy-script deletion to concurrent additions. Rebase helper/test/workflow paths together. |
| Root tracked `output.log` | Retire from tracked tree | Historical diagnostics, not product input. Removal is not erasure from Git history. Never print raw contents. |
| `AGENTS.md`, `LICENSE`, `SECURITY.md`, `roles/`, `rust-toolchain.toml` | Keep and narrowly align guidance | No toolchain/dependency upgrade. Preserve legal bytes. Role prompts are development method, not implemented runtime orchestration. |
| `.gitignore`, `.gitattributes` | Keep/retarget | Add promoted exclusions before moving; keep old ignored outputs protected. Preserve necessary notice/evidence byte exemptions. |
| `.github/` | Keep active controls, replace obsolete paths/commands | Reconcile concurrent pipeline work. No legacy root-Cargo/macOS pipeline may remain as the current product gate. Do not restore or trigger the legacy automatic publisher. |
| `.smith/project.yaml` | Retain only with portable, truthful active commands | Inspect without exposing private locator values; do not invent a configuration consumer or claim enforcement. Remove obsolete settings rather than preserving dead root-Cargo validation. |
| Canonical docs and current implementation report | Keep as current entry points | Update source/command paths, legacy support withdrawal and history-retention policy; distinguish implemented from target. |
| Active supporting plans/contracts and relevant latest evidence | Keep exact necessary dependency closure | Includes current cloud API contract, migration/promotion review/evidence and concurrent installer-pipeline work. Classify links explicitly; do not decide by filename age or `NEXT` substring. |
| Retired guides, superseded plans/reports and historical evidence no longer needed locally | Remove from working tree after classification | Record immutable commit/path/hash and use pinned historical links. No copied archive directory. Latest review evidence must remain interpretable. |
| `reference/voice-chat-v1/`, `reference/python-installer-v1/` | Retire working-tree snapshots under history-backed policy | No active build dependency identified; verify all 46 voice and seven Python payload hashes recover from pinned history. Do not remove live migration fixtures inside promoted components. |
| Ignored/untracked build outputs, caches, raw logs, unknown local work | **Not deletion candidates** | Git does not retain these. Preserve locally without broad cleanup or relocation into active product inputs. Unknown new files require classification. |

An exact per-file manifest is a **pre-implementation gate**, not supplied by this group-level plan. It records original path, disposition, destination, Git mode, content hash, checkpoint containing removed bytes, and reason. Account for every tracked entry and collision, plus all concurrent/untracked source work. New files cannot be made recoverable by claiming the old checkpoint contains them.

## Technical invariants and active path repairs

Evidence: `next/Makefile:35–86`, `next/installer/Cargo.toml`, `next/service/Cargo.toml`, `next/dagger/src/smith_next/main.py`, `next/client/lib/native_control.dart`, `next/service/src/filesystem.rs`, `next/service/src/cloud_keyring.rs`, `next/installer/src/maintenance.rs`.

- Preserve explicit Dagger component inputs and sibling paths. Do not replace them with broad repository/home mounts.
- Preserve both Rust workspaces/lockfiles, pinned builders/runtime revisions, complete paired client/service package, separate CPU/Vulkan runtime subtrees and exact executable-relative lookup.
- Rebase ignore/attribute rules, active README/component links, Make help, workflows, package scripts, tests and self-project validation commands. Inspect `../docs`, `make -C next`, literal `next/` and source-root discovery assumptions.
- Keep `dagger/INSTALLER.md` as the packaged operator guide unless a separately reconciled need justifies moving it. Correct superseded guidance there; do not add another duplicate guide just for the cutover. Concurrent pipeline work already intends this correction.
- Runtime constants containing `next` are not stale source paths. Keep their exact bytes, including unit/shim descriptions/comments used for ownership recognition.
- Historical `next/...` paths and source hashes are original evidence. Preserve historical records byte-for-byte or leave them retrievable at the checkpoint; use a relocation ledger rather than global replacement.
- Preserve live fonts/icons/notices, Rust compile-time fixtures, Flutter golden PNGs, installer publisher/release-manifest fixtures and generated files intentionally tracked as inputs.

## Documentation and history contract

Keep a compact `docs/HISTORY.md`: pre-cutover full commit ID(s), source path map, retired capabilities/material categories, retrieval instructions, historical limitations and links to exact Git blobs. A machine-readable retirement/move manifest supports complete reconstruction.

Current canonical documents should state the promoted product directly, retaining broader orchestration as target/deferred where appropriate. Remove obsolete implementation guidance or point to history rather than requiring readers to resolve layers of supersession paragraphs.

Keep `VALIDATION.md` at root with current disposition and the new cutover evidence. Older records may stay verbatim or be reached through pinned history; the old 1,262-test PASS does not validate the moved tree. Ignored raw logs/exported binaries are not preserved by Git and must not be claimed as checkpoint-recoverable.

Historical source retention is satisfied only after verifying object contents and continued checkpoint ancestry. No force-push/reset, retagging, broad `git clean`, history rewrite or local-output purge is part of this plan.

## Independent analyses and reconciliation

Fresh discovery contexts: architect `8312beb2-47ce-4dbc-9f9d-463635bdf1f8`, producer `cf10b90a-8d20-4c79-89d7-f45909806b25`, designer `5904d49b-1a74-4747-a051-e3373a57709a`.

| Concern / disagreement | Reconciled proposal / remaining gate |
|---|---|
| Is promotion dependent on the old product? | No active dependency identified on root legacy `src/`, client or nine scripts. Verify exact final dependency graph after concurrent changes. |
| Retain every historical document/reference versus user-requested lean root | Producer/designer revised retain-all advice: immutable Git-backed retention is sufficient with explicit policy, exact inventory and tested retrieval. Keep live fixtures/contracts and relevant current evidence. |
| Root promotion versus installed-product rebrand | Freeze operational identities; migration is source-layout only. |
| Legacy CLI compatibility promise versus removal | Explicit approval of retirement and canonical-policy amendment required. Not a mechanical-only rename. |
| Retire unsafe publisher versus concurrently building a safer candidate pipeline | Do not blindly remove the new pipeline. Reconcile its reviewed final code, helper paths, trigger/permissions and disclosure boundaries; retire only obsolete legacy behavior. |
| Prior PASS versus relocated-tree confidence | Run fresh configured root gates on a stable clean source. Old evidence is not inherited acceptance. |
| Legal/host/release gaps versus source-layout scope | Carry forward existing icon-rights, notice/SBOM/provenance and installed/graphical/account limitations. Do not claim clearance or expand this relocation into full release remediation. Any new distributable upload/publication remains separately gated. |
| Merge strategy and weak server enforcement | Recommend history-preserving merge; independent inspection found no required reviews/status checks configured at inspection time despite protected-main label. Enforce documented checks manually; do not change repository settings without separate approval. |

Pre-implementation prerequisites were resolved: operator approval recorded above; concurrent pipeline work completed its local gate and was captured/preserved; independent exact-manifest preflight PASS preceded moves. Merge method and actual merge approval remain later action gates.

## Requirements / acceptance / task traceability

| REQ / AC | Task, priority and dependencies | Definition of done / evidence |
|---|---|---|
| R1 / AC1: only one active product; complete approved disposition | T1 P0: freeze final base, coordinate pipeline work, inventory every path before moves | Exact keep/move/retire map; no unresolved collisions, lost new work or implied deletion of ignored material. |
| R2 / AC2: legacy support withdrawal and history policy explicit | T2 P0, after T1 and approval: update affected canonical scope/architecture/process/design/host assessment | Current policy matches intentional retirement; installed data, namespaces and compatibility invariants unchanged. |
| R1,R3 / AC3: tracked promotion preserves live inputs | T3 P0, after T1: protect ignored outputs, retire approved old tracked paths and promote tracked `next/` contents | Hash/mode/path audit; standalone workspaces intact; no legacy-client overlay; no unintended runtime literal changes. |
| R4 / AC4: root developer/tooling paths and automation valid | T4 P0, coordinated with T3: Make, Dagger/package/pipeline, CI, ownership, metadata, current docs | `make help`; isolated helper/workflow tests; Ubuntu root aggregate/package gates; no obsolete publisher/root-Cargo current gate. |
| R5 / AC5: lean but recoverable history and legal/evidence truth | T5 P0, after T1: history index and reviewed docs/reference cull | All retired tracked hashes recoverable at named reachable commits; 53 archived payloads verify; current links resolve; notices/live fixtures preserved; historical outcomes unchanged. |
| R3,R4 / AC6: final promoted source works | T6 P0 self_check after T2–T5: clean-snapshot validation and artifact identity | All configured component/compiled cross-language/GTK/native-runtime/package gates pass; source stable; skips/failures retained; package/service copies match. |
| R1–R5 / AC7: independent assurance covers full cutover and branch | T7 P0 after T6: fresh architecture/adversarial assurance | Final path/artifact/evidence binding; no blocking unresolved issue; review full `main...feature/autonomy` change, not just move diff. |
| R6 / AC8: controlled bootstrap completion | T8 P0 after T7 and respective approvals: commit/push/PR/merge/verify | Exact revisions/reviews/checks/approvals rechecked; checkpoint ancestry preserved; branch retained unless named deletion approved. |

Owner of execution/review scheduling: implementing agent under operator approval; operator owns scope, remote/merge/deletion decisions. Assumptions expire on material base/path/scope drift or concurrent edits affecting a frozen manifest.

## Execution sequence and stage gates

### Plan → develop

1. Approve this bounded cutover, including legacy CLI retirement, unchanged runtime names and Git-backed archival policy. Local commit authority must be explicit if desired.
2. Finish or coordinate concurrent installer-pipeline work; recapture HEAD, status and source identities. Preserve all new work. Finalize the exact manifest before destructive tracked-path operations.
3. Implement on existing `feature/autonomy` as its final bounded source cutover; no rename/rebase of the bootstrap branch. Add promoted ignores first. Move tracked paths deliberately rather than moving whole directories containing outputs/caches.
4. Retire old client without mixing its cache/generated content into the replacement. Leave ignored local outputs untouched or preserve them outside the collision path using a separately approved, recorded local move; do not delete them to make the rename succeed.
5. Prefer one coherent tested migration commit over artificial intermediate commits with intentionally broken paths. Documentation/history/automation corrections belong in the same tested candidate. Commit only when separately approved.

Blocking: unapproved removals, unresolved concurrent edits/collisions, lost evidence, changed operational identity, or unsupported compatibility claims.

### Develop → self_check → assurance

Run from the **promoted repository root**, in an approved clean tracked-source snapshot without inherited legacy outputs:

```sh
make help
env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
python3 -B dagger/package.py --root installer/dist --verify
git --no-pager diff --check
```

Also run new/updated pipeline-helper and workflow invariant tests from the concurrent pipeline's final root locations. Validate workflow syntax/permissions/triggers and current doc links. A source/path audit must classify remaining `next` occurrences; zero occurrences is neither required nor safe.

Require actual compiled Rust↔Dart socket/cloud/timestamp fixtures, isolated GTK tests, actual installer ELFs and no-Python CLI runtime/package verification. Preserve exact counts/controlled ignores and all failed attempts. Bind source inventory, commands/exits, artifact identities and fresh assurance to the stable final tree; do not silently rerun failures until green. A second absolute checkout location is useful path-isolation evidence.

Builders may use the approved dependency/cache network needed by existing gates. No normal-host app launch, service/user-manager/keyring access, real providers, model downloads or installation is authorized by these checks. Missing capability is a reported blocker, not reason to broaden host access.

Blocking: failed/missing gates, unstable source, weakened assertions, hidden skips, unsafe workflow authority, broken active links, unverifiable history retrieval, or open concrete assurance finding.

### Review request → merge → branch wrap-up

1. Prepare a full-branch review packet against current `main`, with retirement notice, exact root manifest, test/artifact evidence, independent reviews and known non-release limitations.
2. With separately bound approval, push the named final branch and create a PR to `main`. Inspect workflow triggers first: validation, Actions artifact uploads and public distribution are distinct effects. No tag or release is implied.
3. Require successful applicable PR checks plus independent full-diff assurance. At discovery there were no observed GitHub checks for `168865b`; retained local validation is not a GitHub PASS.
4. Obtain explicit merge approval for exact PR/base/head and normal merge method. Recheck drift/checks/reviews immediately before action; no admin bypass or assumed protection enforcement.
5. Verify resulting `main` tree, checks and ancestor reachability of the historical checkpoints, including `168865b`. Integration readiness is not release/deployment readiness.
6. Optionally delete named local/remote `feature/autonomy` only after explicit deletion approval and ancestry verification. Otherwise report it integrated and retained. New work then branches from updated `main`.

No automatic release/publish/deploy, branch deletion, protection changes, tags, retroactive branch renaming or force-push is part of wrap-up by default.

## Rollback and minimum safety

Before merge, correct the candidate through reviewed local edits; do not reset other work. After integration, a separately approved revert/correction can restore source from the exact manifest/checkpoint. Never reintroduce the unsafe legacy publisher as an incidental rollback. No installed-state rollback is needed or authorized for a source-only cutover.

If a later deployment is requested, existing disk4 compatibility, paired-binary/state snapshot/restore and exact keyring-generation requirements remain. Binary-only downgrade is not a valid rollback.

No merge with missing required source validation or unresolved concrete safety finding. No release claim while existing required release evidence is absent. Nonblocking documentation/navigation polish can be deferred with an owner/follow-up; behavior, evidence retrieval and authority boundaries cannot.

## Outcome and handoff

The original planning phase reconciled read-only producer/architect/designer inputs without source changes. After explicit local approval, independent preflight verified the exact1,721-entry manifest and all53 archived payloads. Implementation promoted1,231 tracked paths, retired454 obsolete tracked files, replaced the root README and preserved concurrent pipeline changes/local outputs. Current policy, active contracts, root tooling/CI/metadata and history navigation are aligned; installed identities and application sources remain unchanged.

[Promotion evidence](../evidence/root-promotion/README.md) records a fresh1,262-test aggregate PASS,34 helper tests,95-file package verification and stable1,230 core inputs. Initial harness discovery failure remains recorded; a fresh snapshot with empty independent Git boundary resolved ignored-ancestor discovery without production changes. Verified exports are available at root component `dist/` paths. Final assurance disposition is in [REVIEW](../evidence/root-promotion/REVIEW.md).

Stop before any unapproved commit, push/PR, merge, branch deletion, tag/upload/release or installed-host effect. Branch closure requires the full review packet, exact revision-bound approvals and ancestry verification described above; it has not been performed by local implementation.
