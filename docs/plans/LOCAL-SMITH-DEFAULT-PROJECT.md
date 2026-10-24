# LOCAL-SMITH-DEFAULT-PROJECT

## Status, intent and authority

**LOCAL_PLAN_APPROVED — fresh profiles only.** The operator requested Smith as a default built-in project, selected “pre-created ordinary can be deleted,” and resolved the initialization cohort with “new profiles.” This approves an ordinary editable/deletable starter only when the safe application-state read returns absent, plus scoped local implementation/testing/documentation. No existing-profile backfill, migration change, protected built-in or contribution executor is included.

Scoped local source edits and isolated validation are approved for this bounded task; approval expires at completion or material scope drift. No commit/branch/remote mutation, installation, application launch, live-state migration, repository access, provider/model/keyring/manager operation or runtime agent is included. Follow [Process](../PROCESS.md#approval-boundaries). No analysis below grants human approval or accepted risk.

Discovery uses the existing dirty `feature/autonomy` worktree, last observed HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`, not HEAD alone as a source baseline. Prior Projects/layout and concurrent Global Context work must be preserved. The latter actively owns service persistence files and is advancing disk6→7 in [its separate plan](LOCAL-SMITH-GLOBAL-CONTEXT.md). This task has not frozen or validated that changing combined candidate. Capture actual source identity and coordinate disjoint write ownership before implementation; do not allocate/redefine a durable schema unilaterally.

Risk: **elevated**, because one-time initialization and deletion persistence affect recovery/storage semantics. Architecture review and relevant tests are required.

## Confirmed product direction

- One ordinary, service-owned persisted project, displayed in the current Projects list/editor. No fake client record, reserved runtime identity, protected flag, pin/badge, special deletion rules or new controls.
- Initial Name `Smith`, repository URL `https://github.com/jdharrison/smith`, Base and Work both `main`, fixed agent `opencode`. Four type leaves remain null inside their required object. Repository Name remains optional/unset unless separately requested; the user did not specify an additional name.
- Source support for the URL and main/main metadata is `.smith/project.yaml`; this is not remote availability or contribution-permission verification. Do not import portable `id: smith` as the required opaque32-hex service identity, or import its workflow templates as executable behavior.
- Existing edit/rename/Save/Cancel/confirmed metadata-only Delete and stale/uncertain recovery apply normally. Deletion, renaming, clearing repositories or emptying the registry never triggers reseeding.
- Preserve service order, current no-selection/startup-destination behavior, independent editors and existing UI/components. No global shell/sidebar/header/navigation/theme/font/Chat/Settings changes.
- This is contribution setup metadata only: no checkout, Git execution, account provisioning, tests, branch/PR creation, context injection or agent/container launch follows from a public URL.

## Independent inputs and unresolved decisions

| Input | Finding / recommendation | Reconciliation |
|---|---|---|
| Producer `b60c91b3-2e35-4688-b6ca-109d9ba4483e` | Prefer fresh profiles only; leave existing state unchanged. Existing-profile backfill is a substantive product choice. | User subsequently selected this fresh-only scope explicitly. |
| Architect `1f7d58ca-f12c-45df-8120-f8d7dbc2893d` | Initially preferred fresh plus eligible existing state with a versioned once-only migration decision. | Disagreement retained; explicit user fresh-only choice resolves D1. Follow-up architecture review confirms the smaller no-schema-change design and requires no-clobber publication. |
| Designer `aab95525-627d-4541-bba3-2fd30b38c086` | Ordinary authoritative record, no UI special case/auto-selection, persistent edit/delete, truthful metadata-only copy. | Adopt within the confirmed scope. |
| Repository Name | Architect/designer suggested `smith`; producer suggested leaving it unset. | Keep optional Name unset: avoids introducing metadata absent from the user's proposed configuration. This does not change the required project Name. |

**D1 — resolved by the operator: new profiles only.** A profile is eligible only when the safe state read returns `None`. Existing current/legacy/empty/full state follows its existing load/migration/recovery behavior with no bootstrap insertion, receipt or capacity effect. Empty registry is not a fresh-profile indicator.

Existing-profile coverage and all backfill eligibility/duplicate/capacity/version-boundary proposals are deferred. The earlier architect preference for backfill is superseded by the explicit user cohort choice, not a vote among analyses. No new disk/API/schema version or bootstrap marker is needed.

## Implementation constraints from source

**Implemented at discovery:** `Application::open_with_cloud` distinguishes missing state from loaded state (`service/src/app.rs`). `Registry::default()` is empty and is reused by legacy migrations (`service/src/app_projects.rs`, `app_v2.rs`). Do not turn that shared default into an implicit seed.

Projects validation ties a nonzero registry revision and latest retained receipt together. The reconciled fresh bootstrap creates registry revision1 with one content-free internal receipt and existing-format metadata fingerprint, ordinary generated IDs and no expired IDs. It consumes normal fresh-state capacity without evicting evidence. Do not weaken validators, modify shared empty defaults or add a receipt-free special case.

Fresh-only uses an explicit absent-state bootstrap without changing the public Project shape/API or current durable schema. A helper in `app_projects.rs` constructs fresh state with application/registry revision1, validates full state/capacity and publishes with the existing `filesystem::publish(..., 0o600, false)` no-clobber primitive before readiness. Replacement-capable `persist()` is not sufficient for this initial publication. If another destination appears, fail and preserve it; do not adopt, replace, retry or delete uncertain published state. The startup `None` arm alone invokes the helper, returning the already-published state without a duplicate migration increment. Full-state removal creates a genuinely new absent profile; retained ordinary deletion does not.

Fresh-only architecture follow-up `1f7d58ca-f12c-45df-8120-f8d7dbc2893d` found no remaining product/schema blocker and requires no-clobber/publication/corruption/deletion tests. Implementation ownership for this task: delegated service owner writes only Projects module/tests and the exact absent-state startup arm, re-reading that shared hunk immediately before editing and preserving concurrent Global Context changes. No `app_v2.rs`, provider, runner, instruction or production Flutter edits. The parent owns canonical docs and separately assigned compiled fixtures; if the shared arm's contract changes, stop and reconcile rather than apply stale text.

## Requirements and blocking acceptance

| Requirement / acceptance | Task / expected evidence |
|---|---|
| R1/AC1: agreed eligible profiles have exactly one real ordinary Smith record with the proposed exact metadata; identity survives restart | T1/T2: service initialization/list/persistence tests and exact record assertions |
| R2/AC2: edits/rename/delete survive reopen, including deletion of the last project; no reset/recreation after refresh/reconnect/upgrade | T2/T3: ordinary CRUD, repeated restart and migration cases; existing confirmation/uncertainty guards |
| R3/AC3: existing records/order/IDs/receipts/fingerprints/expired IDs/history/settings and capacity are preserved under the selected cohort | T1/T2: supported frozen migration/empty/full/duplicate/exhausted-ledger/headroom cases; unrelated state comparison |
| R4/AC4: corrupt/unsupported/unsafe/unpublishable state is preserved, not treated as fresh; no partial or duplicate bootstrap | T2: ownership/publication/corruption tests using isolated roots |
| R5/AC5: existing UI and no-selection behavior remain; normal Smith card is searchable/editable/deletable; zero repository/provider/tool/context effects | T3: existing-shaped widget/controller and real Rust↔Dart cases, API/effect counters, scoped source inspection |
| R6/AC6: affected canonical docs and final stable source/artifact/review evidence accurately bind the combined candidate | T4/T5: source manifests, commands/logs/failures/skips, package verification and fresh independent assurance |

## Execution sequence and write scope

1. **T0:** D1 is resolved fresh-only; preserve independent disagreements and capture the actual dirty/untracked baseline. Keep implementation ownership to the explicit startup hunk/Projects module; record concurrent drift rather than claiming whole-tree stability.
2. **T1:** reconcile explicit bootstrap eligibility, ordinary ID/revision/receipt semantics and any required version boundary. No permission/limit broadening.
3. **T2:** implement fresh-only bootstrap in `service/src/app_projects.rs` plus only the `None` arm in `service/src/app.rs`; focused `app_projects_tests.rs` additions. Do not touch `app_v2.rs`, provider/runner/instruction rendering, unrelated client routing or migration policy.
4. **T3:** real compiled Projects fixtures plus narrowly affected client tests demonstrate ordinary list/edit/delete/restart behavior. Prefer no production Flutter changes.
5. **T4:** update affected canonical Scope/Architecture/Design/Process/Host documentation/details and observed assessment together. Distinguish this one-time starter from blank New Project forms and unchanged nullable field defaults; coordinate shared-document ownership.
6. **T5:** focused tests first, formatting/analyzer/locked warning-denied native gates, compiled service↔Dart, then required root `make test build`, rebuilt-package verification and actual-package helpers against stable final inputs. Preserve failures/skips/cache distinctions in a new task-specific bundle; prior layout evidence is historical, not current mixed-worktree validation.
7. Obtain fresh independent assurance/adversarial review for initialization, preservation, deletion-not-resurrection and no extra effects. Missing/failing evidence blocks acceptance. Stop before all unapproved external/live-host effects.

At plan reconciliation no implementation or tests/builds had been performed; the execution disposition below records subsequent outcomes. Native/installed migration/restore, contribution execution, clean-source release provenance/rights/publication and deployment remain separate gates.

## Execution disposition

**Implemented selected fresh-only behavior; full combined acceptance BLOCKED.** T0/T1 are reconciled. T2 adds only `projects::bootstrap`, the exact absent startup arm and seven tests. T3 adds a distinct real compiled fresh-profile case while preserving the seven original scenarios through explicit existing-empty fixtures; only isolated dart_cloud fixture compatibility changed. T4 documents this narrow initialization exception without changing UI, blank New forms or optional field semantics. No public schema/API/migration/default/provider/production Flutter changes belong to this task.

T5 ran against1,275 stable core inputs (`e51a718bbc9d990622ff64e3847e7bae095416d66f344fb48fb7e6be5e8d12d9`): seven bootstrap and eight rebuilt compiled Projects cases passed; formatting/full analyzer and production library Clippy pass. Broader Projects tests24/29 and focused Flutter168/169 pass, with required failures retained. An isolated exact pre-task service overlay reproduces the same five Rust failures and instruction-test Clippy lint; it does not excuse them. Root test/build fails at Clippy, build is not reached and current-package checks/helpers are NOT RUN. No old export is promoted as current.

[Parent evidence](../evidence/default-project/README.md) and [fresh independent review](../evidence/default-project/REVIEW.md) bind the actual source, baseline control, commands, failures and no-effect limits. Bounded feature-source review passes; overall assurance/combined acceptance and release are blocked, with no accepted-risk waiver. No further unrelated remediation is authorized by this task. Capture a new stable combined source and rerun required gates after the concurrent owner resolves its fixture/lint/navigation/contract issues. No live profile was initialized or altered, and no commit, remote action, application launch or installation occurred.
