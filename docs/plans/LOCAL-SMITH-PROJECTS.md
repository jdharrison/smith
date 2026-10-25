# LOCAL-SMITH-PROJECTS

## Approved local intent and boundary

The operator requests a Projects destination to manage a project list and summary/settings details containing name, repositories and a project-owned default/only OpenCode agent. The follow-up clarifies that the chat/settings hybrid describes UX only: "a settings/summary page, then we'll do dev". This approves the bounded metadata-only local implementation described here, not project conversations or agent execution.

Baseline: clean `c1339f3`, existing `feature/autonomy` checkout. Preserve concurrent/user work and existing bootstrap history. No branch creation/rename/rebase, commit, push, PR, release, live installation/state migration, normal-session credentials/provider/model/manager probes or application launch is authorized. Local source changes, source fixtures, builds/tests and canonical documentation are included. Approval ends on completion/revocation or material scope drift. Runtime orchestration remains deferred.

Risk: elevated persistence/protocol/client interaction change. Architectural and independent assurance review are required; missing/failing required evidence blocks source acceptance. No runtime permission, filesystem root, network destination or provider capability is added.

## Independent inputs and reconciliation

- Producer context `ced5f7d3-886c-4d04-8935-828d6c43eee1`: metadata-only project CRUD, inert repository locators, fixed per-project agent, no chat effects; the UX-versus-conversation ambiguity is resolved by the operator's clarification.
- Architect context `16a27a6a-82be-4d3f-8d89-e6c0c03ebcb3`: service-owned registry, not presentation preferences; bounded revision-checked/idempotent operations; explicit disk migration and legacy compatibility; no additional worker/scheduler.
- Designer context `0ab5cf61-ee10-4187-ac6a-cfd574d6fbfe`: Chat/Projects/Settings, wide list/detail and compact accessible layout, explicit Save/Cancel and exact-target metadata-only delete confirmation, preserve mounted chat and drafts.
- Planner reconciliation: accept the metadata-only interpretation. The architect's exact-contract follow-up requires replay-before-admission checks, independent registry revisions,32 receipts/1,024 tombstones, delete fingerprint scrubbing, frozen disk4 migration and explicit locator grammar; these are adopted below. Retain no unresolved product blocker. No project chat, backlog, model selector, prompt template, repository validation/probing, clone, credentials, Docker/OpenCode provisioning, containers or execution controls in this slice.

## Product and storage contract

The persistent record is `{project_id, name, repositories, agent}`. IDs are opaque stable 32-character lowercase hexadecimal identities; names are nonblank, trimmed, at most40 Unicode scalars with no controls. Projects may have no repositories. Agent is stored per project as exactly `opencode`; there is no selectable alternative, readiness claim, process or credential relationship.

Bound the registry to32 projects,16 distinct repository locators/project,2048 UTF-8 bytes/locator and safe-integer revisions. Locators support exactly `https://host[:port]/path`, `ssh://git@host[:port]/path` and `git@host:path`. Hosts are ASCII DNS-style labels (1–63 alphanumeric/hyphen bytes, alphanumeric ends, total≤253); ports, when present, are decimal1–65535. Paths are nonempty slash-separated ASCII segments containing only letters/digits/`._~-`, with no empty, `.` or `..` segment and no trailing slash. Do not decode percent escapes or normalize identities; duplicate comparison is exact and case-sensitive. This intentionally excludes local paths, arbitrary usernames/passwords, percent escapes, query/fragment, whitespace, controls and shell metacharacters. Validate syntax locally only; Rust/Dart share equivalent vectors. Repository metadata never grants access or authorizes a future clone.

Extend the existing service-owned atomic application state to disk5. Explicitly migrate disk1–4 with an empty project registry while preserving histories/configuration/cloud references/provenance/timestamps/receipts. Unsupported/corrupt bytes remain preserved. Older binaries cannot read disk5; any later installed rollout/restore needs separately approved compatible state and paired artifacts. Keep app3/Core2 and exact existing chat replies unchanged. No metadata in chat prompts or provider context.

Project mutation transactions use the existing application owner/mutex/publication fence, but do not enter inference queues or appear as chat operations. Retain32 recent project receipts plus1,024 expired-ID tombstones, separate from chat retention; never evict tombstones to permit reuse, and reject new work atomically when required replay capacity is exhausted. Registry revision is independent of chat/application revision, although every successful mutation also uses the existing application commit fence. Exact retained replay is checked before fresh instance/revision admission and increments neither revision; changed arguments/action conflict. Scrubbed mutation reuse is expired, not a fresh mutation. Only fresh requests require the currently observed instance and registry revision. On uncertain transport/publication outcome, retain original request identity and query its receipt rather than replaying or allocating a replacement automatically. Deletion removes the project record and scrubs all its retained metadata-derived replay fingerprints, preserving content-free historical receipts/opaque IDs; those old receipts cannot imply continued project existence. It never removes repository files, chats, credentials or installations.

## App3 contract

- `projects_list({})` -> `{revision, projects:[{project_id,name,repositories,agent}]}`.
- `project_create({request_id,instance_id,expected_revision,name,repositories,agent:"opencode"})` -> mutation receipt.
- `project_update({request_id,instance_id,expected_revision,project_id,name,repositories,agent:"opencode"})` -> mutation receipt.
- `project_delete({request_id,instance_id,expected_revision,project_id})` -> mutation receipt.
- `project_receipt({request_id})` -> mutation receipt, or explicit unknown/expired error without replay.
- Mutation receipt: `{request_id,project_id,revision,deleted}`. Receipts contain no names/repository strings. After acknowledgement the client refreshes the authoritative list, never treats a stale receipt as a project snapshot.

Use strict request/reply fields, stable IDs and finite bounds. All new project operations are app3-only. Keep project replay identity explicitly scoped to the project-operation family, separate from existing chat/provider receipts. Mutations reserve state capacity and fail atomically on stale revision, invalid identity/agent/input, capacity exhaustion or publication failure.

## UX contract

Add Projects between Chat and Settings in the existing rail. Wide view: selectable project list and summary/settings editor. Compact/enlarged view: reachable project chooser/list and one scrollable editor. Only name, repeatable repository rows and fixed OpenCode information are editable/displayed; explain that no agent runs yet. Create/edit uses explicit Save/Cancel; selection/opening creates no durable empty project. Empty registry and empty repository list are supported. Confirm deletion against the original project identity/service instance/revision, stating metadata only; deleting the selection returns to empty selection, not another project. A valid deletion receipt plus authoritative target absence completes that selection cleanup across restart, including dirty editors; absence alone during uncertainty cannot establish success.

Share one registry controller/API owner across the desktop's panes. Preserve per-view editor/selection state and existing chat mounting/drafts/reading/queues; no duplicate inference or API owner. Project values are service data, not workspace preferences. If Projects destination is persisted, use an explicit presentation-preference version migration accepting old Chat/Settings records; selected project/edit text need not persist. Session-only desktops retain their existing no-preference-store contract while reading the same service registry. Editors and delete confirmations capture both instance and revision. Dirty editors retain those origins across refresh/restart, including unchanged registry revisions, until explicit review of the current instance/revision; old confirmations cannot silently rebind. Unknown/stale mutation outcomes block further writes until reconciled; failures preserve form edits. Refresh remains available during uncertainty. Resolve also observes the authoritative list on unknown/expired receipts while retaining the original pending identity and write block; present membership cannot establish the original outcome. No automatic mutation retry.

## Traceability and tasks

| Requirement / acceptance | Task / touchpoints | Required evidence |
|---|---|---|
| P1 Stable named projects and bounded inert repository lists survive restart; stale/duplicate requests cannot overwrite/recreate | T1 service registry/domain in `service/src/app_projects.rs`, existing app dispatch/state and tests | CRUD/empty/rename/restart, syntax/bounds, idempotency/conflict/delete replay, revision/instance/capacity/publication negative tests |
| P2 Each project independently owns exactly OpenCode; absent tools do not block metadata | T1 service + T2 strict Dart models/validators | unsupported/malformed agent rejection, defaults and no tool/provider calls |
| P3 Preserve disk1–4 and exact existing app1/2/3 contracts, no live effects | T1 migrations + fixture updates only where needed | legacy/corrupt/unsupported preservation, frame/state reservations and compiled Rust↔Dart project round trip |
| P4 Projects UX supports explicit CRUD and factual failure/recovery | T2 controller + T3 `client/lib/ui/projects_view.dart`, `smith_workspace.dart`, `main.dart` | fake-API state/uncertainty/stale-action tests, widget wide/compact/enlarged/keyboard tests |
| P5 Existing Chat/Settings/layout/windows continue unchanged; preferences contain presentation only | T3 navigation + presentation-preference migration | draft/reading/navigation regression, preferences old/new/unsupported and session-only tests |
| P6 Canonical policy/evidence describe implemented subset honestly | T4 Scope/Architecture/Process/Design/report and root `VALIDATION.md` | recorded exact commands/outcomes/failures/skips and fresh independent assurance |

Implement T1 and T2 in disjoint write sets after contract review; T3 consumes the agreed Dart interface; T4 follows actual outcomes. Do not expand the schema or UX with deferred delivery features.

## Validation and stage gates

1. Plan: reconcile independent analyses and exact protocol/bounds; unresolved architecture/authority concern blocks implementation.
2. Develop: focused Rust/Dart/widget tests first; preserve any failures and remediate causes without weakening gates.
3. Self-check: affected component format/analyzer/warning-denied Clippy/locked tests; actual compiled Rust↔Dart integration; complete root `make test build` and package verification if available under approved builder prerequisites. Record inability/skips rather than substitute host installation or claim acceptance.
4. Assurance: fresh context receives user scope, final diff, protocol/migration contract and evidence. Required failing/missing checks remain blocking; no reviewer can accept residual risk.
5. Stop: no commit, external mutation or live rollout. Report actual source status and outstanding host/GUI/installed/compatibility gates.

Required negative checks include credential-bearing/malformed locators, unsupported agent, stale instance/revision, duplicate/conflicting/expired mutations, deletion replay, publication uncertainty, corrupt bytes, old-state preservation, hidden-pane/unsaved-chat preservation, and zero repository/network/tool/inference effects during project CRUD.

## Implementation handoff

Implement only this inert Projects foundation. Maintain current native runtime names and user-manager/keyring/provider contracts. Export no private local paths or credentials into fixtures/evidence. Keep temporary test roots isolated; use configured component build/test workflows rather than launching or installing Smith. Update this record with final outcomes and any unresolved validation limitations.

## Recorded implementation and validation outcome

Implemented T1–T4: bounded service-owned registry/App3 metadata protocol/disk5 migrations; strict Dart models and shared query-only-recovery controller; per-view responsive Projects summary/settings with preserved chat mounting; presentation-preference v2 compatibility and canonical documentation. Project CRUD creates no chat job, repository/tool access or provider/keyring/container action. The fixed OpenCode assignment remains metadata only.

Fresh assurance initially found two blockers: dirty editors/open confirmations could adopt a refreshed service instance at unchanged registry revision, and unknown/expired receipts disabled all list observation. Their fixes retain captured instance+revision until explicit review and keep read-only observation available while preserving the original pending identity/write block. A follow-up probe found dirty deleted selection surviving confirmed deletion resolved after restart. Reconciliation now clears that exact selection only after acknowledgement plus authoritative absence, before applying cross-instance dirty-origin preservation. Original independent repros remained unchanged; final written closure and source/evidence binding are in [Projects REVIEW](../evidence/projects/REVIEW.md).

The first full gate failed at a cloud test's stale expected disk4 assertion (actual5). Its record/log/manifests remain preserved; cancelled parallel tasks and unexecuted build/package/helper stages are not passes. Correcting the expectation to the intended schema5 passed the focused cloud case and the final34-case cloud suite. A prior mistyped CLI checksum verification failed before execution; corrected official v0.21.9 verification passed, with no beta fallback.

Final UTC2026-10-17 root `make test build`, explicit actual-package verifier and helpers passed:289 service +283 installer +9 GTK +773 Flutter +41 actual-package =**1,395 aggregate tests**, five controlled ignores/no other skips, plus34 helpers. Format/analyzer/locked warning-denied Clippy/Linux build/no-Python Ubuntu runtime passed. Before/after1,242 core inputs match SHA256 `5226a499e634e06d284f14b841c061d71c470cb903eedba95f228c40d9779d23`; test steps executed fresh, dependencies/builds used cache. Service gate remains configured Bookworm, not supplemental Ubuntu proof. The four-case standalone compiled Rust↔Dart Projects fixture also passed, with zero synthetic inference/HTTP/keyring calls; this repeats aggregate coverage.

Concurrent unrelated runner source/docs were preserved and separately committed outside this task as `14d960975e76f906efb5e447ff3dece0e8e4385b` before the final gate. The initial Projects baseline remains `c1339f3670c70582db05a158b46821353662b87b`; final source binding includes the shared runner bytes, without claiming Projects ownership/review of that repair. This task performed no Git mutation or external effect. A later concurrent simplification of root `README.md` and the packaged `dagger/INSTALLER.md` was preserved. The source exporter refused the post-gate packaged-guide drift; the original passing attempt2 remains retained, and final configured gates/build reran against the new guide with unchanged application/test sources.

[Projects evidence](../evidence/projects/README.md) and root `VALIDATION.md` bind actual results, failures, artifact hashes and independent review. Native graphical/IME/accessibility, real host manager/provider/keyring/model/GPU, installed disk5 migration/restore, clean-host/release provenance and publication remain unperformed and separately approved. Stop before commit/push/PR/installation or Dev/container implementation; local source passes do not grant those authorities.
