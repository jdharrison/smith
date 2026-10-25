# LOCAL-SMITH-PROJECTS-UX

- Extend Dev → Projects configuration and persistence inside the existing shell; match the existing Agents-style list/editor without changing Agents.
- Keep project context separate from global instructions, agent definitions, providers, models and roles. Save metadata only; no Dev runtime or project-context bootstrap is included.
- Preserve the dirty working tree, existing recovery guarantees and historical evidence. Source implementation, validation, independent assurance and installed acceptance remain distinct.

## Approval, baseline and ownership

**LOCAL_PLAN_APPROVED:** the operator approved the reconciled editor/persistence plan with “yep lets continue, dont touch agents tab anything as it is also working. proceed with plan.” This record binds that approval to repository `smith`, existing branch `feature/autonomy`, base `14d960975e76f906efb5e447ff3dece0e8e4385b` **plus the pre-existing modified/untracked working-tree inputs**, and the bounded local Projects implementation, documentation and isolated validation. HEAD alone is not the input identity. Preserve concurrent Agents, global-context, navigation and other work; the coordinating session owns the actual baseline/final source manifests and results. Approval expires on completion, revocation or material scope drift.

**2026-10-17 scoped remainder approval:** the operator's “continue” authorizes the parent to finish this reconciled local Projects remainder, not a new capability slice: hidden-Name recovery in `_reconcileSubmission` (adopt authoritative `saved.name` while retaining newer input), footer Save/Cancel with project overflow remaining in the header, top-left editor alignment like Agents, disk9/v3 fixture reconciliation, preservation/recovery tests, scoped documentation and parent-owned final validation/review/evidence. It remains bound to repository `smith`, branch `feature/autonomy`, base `14d960975e76f906efb5e447ff3dece0e8e4385b` plus the dirty/untracked baseline recorded in `target/projects-ux/baseline-source.jsonl` (not HEAD alone). Preserve all unrelated WIP; the parent must bind the actual final source separately. This is scoped local authority only, with the same completion/revocation/material-drift expiry; actual Dev bootstrap remains excluded.

No Git branch/commit/remote mutation, Jira/GitHub effect, application/GUI launch, installation, live migration, host manager/provider/model/keyring probe, repository operation, release, deployment or accepted risk is included. Remote destinations: none. Follow [approval boundaries](../PROCESS.md#approval-boundaries); source approval is not installed rollout authority.

Risk: **elevated**, because strict storage/protocol compatibility and retained editor recovery change. Independent architecture/design inputs and fresh assurance are required by [Process](../PROCESS_DETAILS.md#triangulation-protocol). This record captures the approved reconciliation supplied to the documentation owner; it does not fabricate missing review identities or establish those reviews as passed. The coordinating session retains their evidence and closes any blocking disagreement before acceptance.

Write ownership for the remainder: coordinating parent owns Projects UI/recovery/layout and final test/review/evidence; native/client test owners reconcile disk9/v3 fixtures and add preservation/recovery cases in their assigned source/test sets. This documentation owner edits only Projects sections of `SCOPE`, `ARCHITECTURE`, `DESIGN`, `HOST_PROFILE` and `PROCESS` entry/detail pairs, the Projects assessment in `docs/SMITH_LINUX_ARCHITECTURE_REPORT.md` and this plan. Agents/navigation sections/code/tests, shell/theme, root `VALIDATION.md`, `docs/VALIDATION_DETAILS.md`, all evidence directories and other plans are excluded from this documentation write set. No tests/builds/application launches or host effects are part of this docs-only assignment.

## Approved contract and reconciliation

| Decision | Approved resolution |
|---|---|
| Scope versus requested Dev bootstrap | Editor/persistence only. No runnable control, project conversation, context injection or bootstrap into Dev. The existing Dev placeholder and shell remain unchanged. |
| Human ID versus service identity/Name | UI ID is `key`: 1–40 lowercase ASCII kebab characters, matching `[a-z0-9]+(?:-[a-z0-9]+)*`, atomically unique across the registry. Keep opaque32-hex `project_id` for targets/receipts and retained hidden `name` plus fixed `agent:"opencode"` compatibility metadata. No extra Name or Agent/OpenCode UI. Key edits never rename the hidden Name. Ordinary creation supplies Name=key; Duplicate retains the saved source Name. |
| Description/prompt requiredness and counts | Description is optional/empty, single-line ≤60 Unicode scalars, rejecting controls and Unicode line/paragraph separators. Prompt is optional/empty, exact ≤1000 Unicode scalars, LF/tab allowed but other controls rejected; do not trim, normalize, truncate or interpolate it. Counters use scalars consistently in Rust/Dart. |
| Repository requiredness/read-only | 0–16 ordered repositories/project; Name stays optional/null, URL required; non-null Names and URLs independently unique by exact case-sensitive equality. Required boolean `read_only` defaults true for new/migrated repositories. Hide writable controls while true, retain and validate every branch/type value. This is inert classification, not enforced access control. |
| Branch settings and toggles | Base is an optional exact safe branch; Work is an optional exact safe branch or terminal-prefix `/*` pattern. Equal Base/Work is valid. Four independent type toggles map null to unselected and an exact pattern to selected. Explicit first selection supplies `feature/*`, `fix/*`, `release/*` or `hotfix/*`; deselection clears that leaf to null. Opening/loading/toggling read-only never replaces custom patterns or nulls with presets. |
| Action placement | The original header-action layout is superseded by the approved remainder: Save Changes/Cancel in the editor footer; project Duplicate/Delete overflow remains in the header. Keep the editor top-left aligned like Agents without editing Agents or the shell. |
| Hidden Name acknowledgement recovery | On confirmed-create reconciliation against an identifiable saved target, adopt its authoritative hidden `name` while preserving newer visible input and existing explicit origin review. Do not regenerate Name from a later key edit or silently rebind origin. |
| Duplicate and draft authority | After guarding dirty replacement, copy the saved source into an independent unsaved draft, clear its human key, retain description/prompt/hidden Name and all ordered repositories including inactive settings. Allocate a fresh service identity only on Save using create; copy no request/receipt/history/pending state. |
| Legacy population | Strictly migrate valid pre-disk9 projects to key `project-<opaque project_id>`, empty description/prompt and repository read-only true. Preserve existing Names/nulls/order/IDs/registry revision/receipts/fingerprints/tombstones and unrelated state. This explicit approved conversion supersedes the earlier proposal to leave new fields unconfigured; it is not a Projects mutation or a project backfill. |

The exact [schema/migration/recovery contract](../ARCHITECTURE_DETAILS.md#projects-metadata-registry) and [editor contract](../DESIGN_DETAILS.md#projects-summary-and-settings) are canonical. App3/Core2 stay unchanged; disk9 and `projects_list_v3`, `project_create_v3`, `project_update_v3`, `project_delete_v3` replace older Projects shapes. Receipt lookup/shape is unchanged. Unversioned/v2 Projects list/writes always fail `UPGRADE_REQUIRED` on the service before legacy replay/admission, including empty arrays/deletes; no client fallback. Shared bounded replay protection, atomic publication, origin fencing and query-only uncertainty remain.

Existing fresh-only starter eligibility remains: safely absent state gets one ordinary project with key `smith`, hidden Name `Smith`, empty description/prompt, the public Smith URL, read-only true, Base/Work main/main and null repository Name/type leaves. Existing empty/populated/legacy state is never seeded; deletion stays deleted. A migrated existing Smith project receives the ordinary deterministic legacy key, not an implicit rename to `smith`.

## Requirements, acceptance and task traceability

All ACs below are **blocking** for this slice. They describe required evidence, not observed results.

| Requirement / acceptance | Task and owner | Required evidence / definition of done |
|---|---|---|
| REQ-001 / AC-001: scoped Agents-style Projects list/editor; no Agents/shell change | TASK-003 UI; TASK-004 integration | Given wide, 400×400/2× text and four-pane layouts, when searching saved key/description or navigating/resizing/hiding, then preserve selection/drafts/scroll/focus rules and ≥48px actions; inspect scoped diff and real-font Projects goldens without unrelated rebasing. |
| REQ-002 / AC-002: separate unique human key and bounded inert context | TASK-001 service; TASK-002 client; TASK-003 UI | Given malformed/duplicate keys and Unicode/control inputs, when saving, then enforce 1/40/41 key, 60/61 description and 1000/1001 prompt boundaries with inline errors and atomic uniqueness; compiled restart round-trip preserves exact text, opaque identity and hidden Name. |
| REQ-003 / AC-003: repository schema/read-only and branch/type preservation | TASK-001–003 | Given 0/16/17 repositories, null/custom type leaves and exact/pattern Work, when editing, selecting/deselecting types or toggling read-only through Save/restart, then preserve inactive values, retain custom patterns, apply presets only on explicit selection and clear deselected leaves to null. Reject bad/duplicate URL/Name/Base/Work/type syntax with reachable errors. |
| REQ-004 / AC-004: explicit draft CRUD and safe recovery | TASK-001–003 | Given dirty/stale/missing input or lost acknowledgements, when New/Duplicate/Add/Remove/Cancel/Delete/Refresh/Resolve runs, then keep draft actions non-durable, guard replacement, create independent copies, confirm exact metadata-only deletion, preserve newer typing, fence same-revision restarts and query the original receipt/current list without resend. |
| REQ-005 / AC-005: lossless strict disk9 migration/compatibility | TASK-001–002 | Given rich disk1–8/current/corrupt fixtures, when opening/migrating or an older writer calls, then apply only approved conversions, preserve old replay/registry identities and unrelated state, reject corrupt/unsupported bytes without clobber/reseed, refuse old v1/v2 operations and retain historical receipt lookup. Cover disk7 instructions/snapshots and disk8 global/Agents preservation separately. |
| REQ-006 / AC-006: inertness and unchanged neighbors | TASK-001–004 | Given metadata CRUD and navigation, when operations settle/restart, then observe zero repository/Git/path/provider/model/keyring/container effects or context transfer; preserve chat projections, global config, Agents definitions, queue/resource limits, preferences v4/strict v1–v4 and session-only no-store behavior. |
| REQ-007 / AC-007: source-bound documentation, gates and independent review | TASK-000; TASK-004–005 | Given the stable actual final tracked/untracked candidate, when accepting the slice, then retain requirements/source/check/review bindings, failures/skips and rollback limits; missing/failing required evidence blocks acceptance. No historical feature pass substitutes for current validation. |

## Prioritized tasks and milestones

| Priority / task | Scope, dependencies and completion check |
|---|---|
| P0 / TASK-000 — coordinator | Freeze the approved contract and actual dirty/untracked baseline; retain independent producer/architect/designer inputs and resolve conflicts. Done when scope, ownership and source identity are bound; no implicit branch or PR action. |
| P0 / TASK-001 — service | After TASK-000, update `service/src/app_projects.rs`, `app_v2.rs`, `app_v3.rs` and directly affected native tests/fixtures. Done when strict disk9, v3 operations, migration, uniqueness, publication/replay/capacity and zero-effect cases meet AC-002–006. |
| P0 / TASK-002 — client domain | After the agreed TASK-001 interface, update `client/lib/projects.dart`, `projects_controller.dart`, Projects API/dispatch and protocol/controller/compiled fixtures. Done when immutable all-field capture, strict decoding, v3-only calls and original-ID recovery meet AC-002–006. |
| P0 / TASK-003 — parent UI | Consume TASK-002 in `client/lib/ui/projects_view.dart` and Projects-only tests/goldens. Implement ID+Description row, full-width Prompt/counters, retained searchable list, expandable repositories, project Duplicate/Delete and repository Remove overflow actions, footer Save/Cancel and existing guards. Done when AC-001–004/006 hold without touching Agents or the outer shell. |
| P0 / TASK-004 — docs/integration | After source inspection, update the five canonical entry/detail pairs and relevant observed Projects report; coordinating parent owns all validation ledgers/results. Run configured focused/component/compiled/root/build/package checks on stable inputs. Done when AC-007 has exact current evidence, not narrative PASS. |
| P0 / TASK-005 — fresh assurance | After developer self-check, give a fresh reviewer scope, source/base/diff, contract and retained evidence, not builder private reasoning. Adversarially check migration corruption, old-writer erasure, custom-pattern loss, replay/origin drift, dirty overflow actions and hidden recovery. Done when blockers are closed with evidence; assurance is not human approval. |

Milestones: M0 approved/bound contract → M1 service/client compatibility → M2 integrated editor → M3 stable self-check → M4 independent assurance. Implementation owners may work in disjoint write sets after interface agreement. No PR creation/publication is part of these milestones. P1 runtime/bootstrap and P2 broader redesign are excluded, not silently completed.

## Stage gates and minimum release safety

1. **Plan → develop:** approved scope/baseline, independent input reconciliation and traceability are required. Material contract drift or an unresolved architecture/safety concern blocks implementation of the affected change.
2. **Develop → self_check:** first run focused Projects native/protocol/controller/widget checks and actual compiled Rust↔Dart round-trips with isolated synthetic state/providers. Verify 64-container/duplicate-member reply rejection, state/frame bounds, registry capacity/tombstones and zero external effects. Preserve intermediate failures; do not weaken checks or fix unrelated baseline defects.
3. **Self_check → assurance:** run affected formatting/analyzer and locked warning-denied native checks in both independent Rust workspaces, then configured `make service/test`, `make client/test`, `make installer/test`, root `make test build`, `python3 -B dagger/package.py --root installer/dist --verify` and actual-package helper checks where supplied by the configured workflow. Run `git --no-pager diff --check`. Bind results/artifacts to stable final tracked and untracked inputs; cached/skipped/cancelled/not-run checks are not fresh passes. Required failures/missing evidence block acceptance.
4. **Assurance → human signoff:** retain fresh review/adversarial findings and closure against the exact candidate; authors cannot approve their own work or accept residual risk. The coordinating parent updates root `VALIDATION.md`/task evidence; this documentation pass records no test PASS.
5. **Release/installed:** remain NO-GO without distinct approvals/evidence. Native Ubuntu graphical/IME/accessibility checks, installation/live migration/restore, compatible paired artifacts and a pre-disk9 migration snapshot with applicable keyring relationships require separate exact authority. Older binaries cannot read disk9; binary-only downgrade is not rollback.

Review checklist: approved scope and all REQ/AC/TASK mappings covered; no Agents/shell or external capability drift; lossless strict legacy conversion/old-writer refusal; exact context/pattern/read-only preservation; dirty/stale/unknown recovery; stable evidence and fresh independent review; compatible rollback readiness. No release with broken core behavior, failing required checks, missing verification or unresolved critical warnings.

**Deferred owner/follow-up:** operator/product must approve and obtain architecture/design review for any project selection/bootstrap/Run, immutable execution snapshots, capability enforcement or context-export consent **before that future slice starts**. No deadline or execution authority is inferred. Native/installed acceptance owner is the coordinating operator, due before any approved rollout. These exclusions do not waive this slice's blocking ACs.

## Current source/evidence disposition

**Implemented source, not graphical acceptance:** service/domain disk9/v3 work and the new editor are present. Inspection of `client/lib/ui/projects_view.dart` on 2026-10-17 found ID/Description, exact Prompt/counters, saved-key/description search, expandable/read-only repositories and saved-source Duplicate; the prior “editor integration pending” assessment is superseded. Source remains concurrent, not a stable final validation identity.

**UI source closeout inspected — 2026-10-18:** `_reconcileSubmission` now adopts authoritative `saved.name` while retaining newer visible input; Save/Cancel are in the footer, project overflow stays in the header, and the editor is top-left aligned. These changes were already present at the start of the lean UI pass. That pass adds only the missing stacked-field gutter; see [UI QA closeout](#ui-qa-closeout) for current focused evidence and remaining acceptance boundaries.

**Broader validation/assurance pending:** native/client disk9/v3 fixture reconciliation and preservation/recovery coverage are outside the lean UI closeout; no combined pass is inferred from the focused UI results below. Parent owns focused/compiled/component/root/build/package results, final tracked/untracked identity, fresh independent assurance and blocker closure. This documentation owner ran no tests/builds/application or host effects and edits no validation ledger/evidence directory. Historical Projects/repository/layout/default-project evidence remains intact and does not validate this slice. Graphical/installed acceptance, live migration and rollout remain separately gated and unauthorized here.

## UI QA closeout

The operator narrowed this continuation to “keep it lean, just round off prjects/agents ui/ux”. This is local UI polish/isolated validation within the existing approval, on the same `feature/autonomy`/base plus dirty inputs; not backend expansion or installed acceptance. Agents source and goldens are unchanged. No application launch, installation, migration or external action occurred.

**Implemented:** Projects stacked pairs now retain a16px gutter. Updated UI assertions cover the existing top-left anchor, header overflow, footer Save/Cancel and single visible Refresh; recovery assertions remain intact. Refreshed Projects goldens match that intended layout.

**Focused evidence — 2026-10-18:** 125 tests passed across `projects_view`, `projects_layout`, `projects_navigation`, `projects_layout_golden`, `agents_view`, `agents_navigation` and `agents_golden`; four-file formatting and targeted Dart analysis passed. Logs, original failures, original Projects goldens and the190-file client source/test/asset manifest are retained under `target/ui-ux-closeout/`. Fresh read-only assurance `a8197dad-a0a2-4fe4-8e40-9779d63e3bae` found no scoped blocker; this is suitable for operator UI QA, not release approval. Combined/native/backend/installed gates above remain open.

### Manual UI QA

- Projects: New/edit, Save/Cancel, Duplicate/Delete and repository Add/Remove; confirm dirty-switch guards.
- Toggle Read Only and branch types; confirm saved settings are retained.
- Agents: edit both profiles, Save/Cancel, and search without losing the selected draft.
- Resize to a narrow/short pane, enlarge text and navigate Back/Return; confirm fields/actions remain reachable.
- Use Tab/Shift+Tab and multiline Enter/IME; confirm focus stays visible and Enter never saves.
- Switch destinations/pane layouts and return; confirm drafts and reading positions survive.

## Implementation handoff

1. Re-read owned shared hunks and confirm the approved contract/source baseline; preserve concurrent work.
2. Complete only the assigned disjoint Projects changes. Do not edit Agents or the outer shell, reinterpret saved custom patterns, inject context or add a runnable Dev control.
3. Reconcile every visible/hidden field through validation, dirty detection, immutable capture, stale-origin review and authoritative post-write refresh.
4. Run focused self-check, then the stable combined gates; preserve failures and report missing capability rather than using the live host.
5. Obtain fresh independent assurance; have the coordinating parent record actual results and stop before every unapproved external/installed effect.
