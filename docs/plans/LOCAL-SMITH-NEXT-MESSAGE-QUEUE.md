# LOCAL-SMITH-NEXT-MESSAGE-QUEUE

## Intent, approval and boundaries

**Approved local implementation plan; implemented client source with scoped evidence below.** The operator requested client-side sequential follow-up messages, explicitly rejected steering, refined failure handling to a blocking head with explicit Retry or Skip, and approved execution: “lets execute, anything you run into just work around if any conflicts and we'll wrap up integration”. Approval covers this bounded source, documentation and isolated-test change on the existing `feature/autonomy` branch, base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`. No commits, remote changes, live inference, credentials, downloads, deployment or service lifecycle changes are authorized. Preserve any concurrent cloud-provider changes; do not bypass safety/interface conflicts.

Risk: elevated client concurrency, recovery and operator-flow change. Cloud execution, provider configuration, wire/storage migrations, durable outbox, steering, active-job cancellation, semantic task-result gates and general workflows are excluded.

## Reconciled design and independent inputs

Read-only independent producer, architect and designer analyses in the preceding review converged on a conversation-owned bounded volatile FIFO, exact completion/history gate, truthful unsent presentation and explicit recovery. Architect follow-up accepted client-only observed-instance fencing (not atomic restart fencing), fresh target detail before dispatch and conservative uncertain-admission handling. Designer follow-up accepted Pause/Remove/Clear/Resume, deferring Edit/reorder, and requires normal-close protection. User subsequently authorized explicit Retry of a definitively failed/not-admitted head and Skip-and-continue; neither is permission to replay unknown work or pretend to cancel running work.

Reconciliation: use 8 waiting/blocked entries per conversation and 32 desktop-wide (rather than the architect's larger proposed 16/64). Maintain 16 KiB UTF-8 input bounds. A pending original immediate send is the predecessor of follow-ups. Local waiting time does not consume operation/queue observation budgets. The existing four accepted service jobs, one per conversation and one execution remain unchanged.

## Requirements, tasks and evidence

| Requirement / acceptance criterion | Implementation task | Required evidence |
|---|---|---|
| R1/AC1: A, B, C submit once in order; B/C stay local until their predecessor succeeds and saved history synchronizes | T1: conversation-owned queue and serialized workspace dispatcher | exact fake-API call order, delayed completion/detail failure, repeated callback tests |
| R2/AC2: failed/not-admitted head blocks successors; explicit Retry preserves position and uses a fresh attempt identity; explicit Skip advances without claiming success | T1: immutable logical entries plus separate attempts, head recovery actions | failure, retry success/failure, skip, stale action and repeated-click tests |
| R3/AC3: unknown/running attempts cannot retry/skip; observation only; ambiguous admission including STATE_UNAVAILABLE retains original identity | T1: conservative rejection classification, uncertainty latch, no replay | lost acknowledgement, post-commit error, timeout/expiry/restart tests |
| R4/AC4: new-chat promotion/shared panes preserve exactly one queue; foreign history/config/instance changes pause, never silently retarget/rebind | T1: stable controller ownership, fresh dispatch detail and bound revisions | promotion/shared/hidden pane, revision/config/instance changes, capacity/fairness tests |
| R5/AC5: queue action captures immutable text and preserves newer drafts; finite counts/bytes; no wall-clock expiry of local waiting | T1/T2: distinct enqueue eligibility and UI action | limit, Unicode, draft, Enter/repeat/IME and long-wait tests |
| R6/AC6: truthful local waiting versus accepted states; Pause/Remove/Clear/Resume; Retry/Skip only when safe; removing a dependency pauses remaining work | T2: bounded accessible queue shelf and status display | widget/state/keyboard/compact-layout tests; no duplicate transcript sends |
| R7/AC7: normal Close/Quit holds dispatch while deciding; cancel preserves queues, confirm discards volatile unsent text without stopping accepted service work | T3: one shell-owned exit guard, native hook only if required | close request/reentry/cancel/failure, hidden/shared counts, disposal fence tests and Linux source build |
| R8/AC8: no cloud/protocol/storage/authority expansion, no dependency additions, no auto-resume after recovery | T1-T4: preserve app-v2 contract and document integration boundary | source review, existing regression suites, independent assurance |

### Dispatch and recovery contract

Enqueue immutable text, logical entry identity, conversation-controller identity, predecessor and current configuration/instance binding. Do not precompute future history revisions for all queued turns. Before a saved-chat head dispatches, fetch actual target detail through the existing serializer, verify the expected predecessor-success revision and membership/readiness, then capture one immutable submission attempt. Revalidate after awaits. Successful model text is not proof of tests/builds/deployment success. A service-instance race after observation remains possible under the unchanged protocol; preserve the attempted identity and pause successors rather than replaying.

Failed or definitely rejected heads retain their position. Retry is an explicit fresh attempt only after verified terminal failure/nonadmission and current readiness; double clicks cannot dispatch twice. Skip is explicit abandonment only after confirmed terminal/nonadmission or before any attempt. Unknown, expired/unresolved or still-running work cannot be skipped into a successor. Forgetting an expired observation does not clear this barrier. No active cancellation endpoint is added. Pause/Remove/Clear affect only local future work, never service jobs. Local deletion/archive is blocked while retained queue text or uncertain dependencies remain; external target removal preserves text and blocks dispatch.

The queue uses normalized service outcomes, not provider HTTP details. Provider/model configuration changes pause queued intents; explicit continuation requires renewed binding and an operator-visible explanation. The forthcoming cloud task owns authentication, egress, model selection and provider execution semantics; its unknown-remote-outcome contract must be integrated before cloud readiness can be claimed.

### Frozen client handoff contracts

`message_queue.dart` defines immutable row snapshots and a nullable `ChatController.messageQueue` compatibility seam. Row Retry/Skip/Remove include an expected `actionRevision`; Resume/Clear include the queue revision. Revalidate these tokens, target and eligibility before acting and after awaits; reserve a retry synchronously so stale/repeated callbacks cannot operate on a newer attempt. Entries may expose the active predecessor with truthful sending/accepted labels; `waitingCount` counts never-dispatched waiting entries only. Retained failed/unknown local messages consume bounded queue capacity; keep one current predecessor allowance per conversation so failure cannot overflow a full tail. Completed/skipped transcript receipts use existing bounded presentation/history mechanisms, not an unbounded queue archive.

T1 exposes `WorkspaceController.acquireQueueExitHold() -> QueueExitHold`, `releaseQueueExitHold(QueueExitHold)` and `queueExitPending`. Acquire synchronously blocks enqueue, ordinary immediate sends, retries and not-yet-dispatched queue work across all conversations. The identity-bound snapshot counts retained unsent/failed text and nonempty drafts, deduplicated across hidden/shared panes and blanks. Release requires the exact hold and preserves prior pauses/failure latches. Confirm retains the hold through disposal. Recheck after awaits before submission; already-dispatched exchanges remain potentially accepted and cannot be cancelled by the hold. T3 uses this API rather than pausing/resuming individual queues.

Fresh read-only architecture review of this implementation plan identified these two missing contracts (stale action tokens and close hold); both are incorporated here before delegation. No further architectural blocker was identified subject to those invariants.

### Ownership / parallel implementation

- T1: `next/client/lib/message_queue.dart`, `text_chat.dart`, `workspace_controller.dart`, dedicated controller tests. Preserve ordinary immediate-send behavior and existing cloud-independent wire shape.
- T2: `next/client/lib/chat_panel.dart`, optional queue presentation module, dedicated widget tests. Use the shared queue interface, not service calls from widgets.
- T3: `next/client/lib/main.dart`, an exit-guard module, `next/client/linux/runner/my_application.cc` only as needed, dedicated exit tests. Shell-level shared ownership and close status; no service Stop.
- T4 (parent): canonical documentation, focused/combined validation, integration and independent assurance. Other workers do not edit canonical documents or shared test fixtures.

## Validation and release boundaries

Run source formatting, focused new/existing queue-adjacent Flutter tests, analyzer, full Flutter suite and Linux build on the combined snapshot. Prefer cached dependencies (`--no-pub`) and isolated fake APIs; no real provider/model/service. Record exact outcomes, including pre-existing or concurrent failures, in a feature evidence record. Current CI at repository root is not evidence of this next-client feature. Build/self-check is not independent assurance, native graphical acceptance or deployment approval. Fresh assurance/adversarial review must cover ordering, identity, skip/retry, close, replay and provider-boundary handling.

Rollback: remove only this feature's additive client queue/exit changes and related docs/tests, preserving concurrent work. There is no durable schema change or host rollback.

## Evidence / integration status

Implemented source and independent-review remediation are recorded in [MESSAGE_QUEUE_VALIDATION.md](../../next/client/MESSAGE_QUEUE_VALIDATION.md). Final source-bound checks: formatting 65 files/zero changes, analyzer no issues, **412 tests passed/4 Rust-socket fixtures skipped**, Linux debug build PASS, whitespace PASS. Inventoried client sources remained unchanged across the final sequence; exact commands/results/hashes are in [MESSAGE_QUEUE_EVIDENCE.json](../../next/client/MESSAGE_QUEUE_EVIDENCE.json).

Independent core/UI reviewers reproduced and then closed foreign-history Resume, reattached-predecessor baseline, stale dialog, empty-draft guidance and compact-close defects. Additional normalized REMOTE_OUTCOME_UNKNOWN handling has two local/reattached negative-action regressions and scoped independent static closure; it adds no provider request or wire behavior. Original direct-send scrolling and draft-only removal behavior were preserved. Only the intentional busy Queue action changed existing recovery-tooltip expectations.

Cloud work was not present in the initial clean checkout, but its separate schema-3/client changes appeared during implementation. They were preserved; the final client snapshot includes them, without claiming full cloud integration or provider/backend assurance. An earlier moving-snapshot compile failure is retained separately in the feature evidence. The cloud owner must finish route/generation binding, explicit context-transfer consent, permission expiry and remote-delivery integration gates. A subsequent source comparison found `main.dart` and `workspace_controller.dart` changed after the validated snapshot; preserve those edits and rerun the combined gates once integration stabilizes. Native window-manager/IME, paired service fixtures and real long-running/cloud behavior require separately scoped evidence; no installed application, service, keyring or model was exercised, and no deployment/commit occurred.
