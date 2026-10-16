# Client-side message queue — implementation and evidence

## Scope and authority

**Implemented client source; local validation, not deployment or full cloud integration approval.** Human authorization, requirements, exclusions and ownership are recorded in [LOCAL-SMITH-NEXT-MESSAGE-QUEUE](../../docs/plans/LOCAL-SMITH-NEXT-MESSAGE-QUEUE.md). Base: `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, existing `feature/autonomy`, uncommitted worktree. Concurrent cloud-provider source and its evidence remain owned by that separate task; none was reverted to obtain a queue pass.

The feature implements conversation-owned volatile FIFO follow-ups, exact successful-predecessor/history gating, explicit failed-head Retry/Skip, revision-bound controls, bounded input/counts and normal-close protection. Never-dispatched waiting is not timed as service execution. Unknown/running work remains blocked without replay or pretend cancellation. One local current-predecessor slot is separate from the eight/chat and 32/desktop follow-up limits. Cloud `REMOTE_OUTCOME_UNKNOWN` is explicitly retained as an unknown barrier even when the service record is terminal; that normalized-code guard does not prove the provider classifies delivery correctly.

No commit, branch creation, publication, deployment, installed-state migration, service-manager operation, keyring access, real vendor API call, model download/inference or live GUI launch was performed by this task.

## Final source checks

[Final evidence](MESSAGE_QUEUE_EVIDENCE.json) records UTC times, exact commands, exit codes and before/after SHA-256 inventories of client Dart/tests/native source and package files. **All inventoried inputs were unchanged during every final check and across the final gate sequence.** This is a point-in-time source snapshot, not protection against subsequent concurrent edits. Ignored scratch logs are named in the record.

All commands ran from `next/client`:

| Check | Command | Actual final result |
|---|---|---|
| Formatting | `dart format --output=none --set-exit-if-changed lib test` | PASS, 65 files, zero changes |
| Analyzer | `flutter analyze --no-pub` | PASS, no issues |
| Full client suite | `flutter test --no-pub --reporter expanded` | PASS, **412 passed, 4 skipped**, zero failed |
| Linux native/client build | `flutter build linux --debug --no-pub` | PASS, debug bundle built; not installed/launched |
| Whitespace | `git --no-pager diff --check` | PASS |

**Subsequent concurrent edits:** the end-of-task source comparison found `lib/main.dart` and `lib/workspace_controller.dart` changed after this successful gate sequence. Those cloud-task edits were preserved, not reverted. The recorded 412-test/build pass applies to the exact manifest snapshot; it is not a fresh PASS for the subsequently edited shared worktree. Repeat combined validation after cloud integration stabilizes.

Four Rust-socket fixture tests require a separately supplied compiled service fixture and remain skipped in this direct Flutter run. No paired-service/Dagger/recipient-package or cloud API acceptance pass is inferred. The pre-feature baseline was 337 passed/4 skipped. Final counts include concurrently added cloud-client tests; they are not all new queue tests.

An earlier [stable focused snapshot](MESSAGE_QUEUE_TEST_EVIDENCE.json) passed **126** queue/adjacent tests. After adding the normalized remote-outcome guard and strengthening the empty-draft warning assertion, a focused remote/core-assurance/UI-assurance run passed **11** tests. Those tests are also included in the final full run above.

## Regression and independent-review record

Initial combined integration exposed six failures: ordinary Send scroll suppression, obsolete Send-tooltip expectations when the composer now queues, and an unintended draft-only missing-chat retention expansion. Remediation distinguishes automatic queue receipts via `LocalTextSend.fromQueue`, hides empty busy shelves, updates only the intentional Queue-tooltip expectation while retaining geometry/recovery assertions, and restores pre-feature removal behavior for draft-only chats while preserving queued/uncertain work.

Fresh independent core assurance added permanent regressions for (1) Resume immediately re-latching against an already-reviewed foreign success and (2) enqueue during a reattached predecessor's terminal observation using post-completion history as its base. Fixes separate the reviewed foreign-operation baseline from the failed attempt ID and capture predecessor history from its correlated atomic state snapshot. Exact successor-history checks remain. The reviewer statically closed both findings without weakening the retained regression assertions.

Fresh independent UI/exit assurance added regressions for compact 400×400/2× close-dialog overflow, stale details-dialog callbacks after pane deactivation, and false unavailable/capacity feedback for an empty draft. Scrollable close consent, invocation-time eligibility checks and empty-draft-aware guidance remediate them. The reviewer statically closed all three findings. The warning assertion now checks the replacement wording as well as the implementation condition, so a wording change alone cannot satisfy it.

A final scoped core review found no concrete defect in the new normalized `REMOTE_OUTCOME_UNKNOWN` guard: local and reattached barriers remain non-retryable/non-skippable/non-removable, Clear can discard safe successors without clearing the unknown barrier, and Resume cannot advance it. Two deterministic regressions exercise all those actions and preserve exact request counts. Review did not certify the cloud delivery producer.

Review contexts were independent of the implementing contexts. Earlier reviewer-executed tests reproduced the findings; closure passes were source inspections while parent validation ran separately. Review is not human approval of external effects or a full cloud/release verdict.

## Concurrent cloud changes and integration boundary

The cloud-provider task appeared after this queue implementation began and subsequently changed shared client transport, routing, controller and presentation code. Its source was preserved. An intermediate combined run saw three old protocol/native-arbiter fixture failures; a later targeted run passed all 23 API/workspace cases after the concurrent task updated its code/fixtures.

[Concurrent-attempt evidence](MESSAGE_QUEUE_CONCURRENT_EVIDENCE.json) retains commands, summaries and source inventories for an unsuccessful moving-snapshot run: a new cloud-selector test was edited during analyzer/test execution and failed with a void-expression compile error; cloud routing changed during the build. That run reported 411 passed/1 failed/4 skipped and is **not** a valid aggregate candidate pass. The final stable run above supersedes its validation disposition. Its scratch-log names were reused by the final runner; the historical failure summaries/source inventories, not those reused log names, are the retained historical record.

Before claiming fully integrated cloud queue behavior, the cloud owner must validate frozen per-entry route/credential-generation binding, explicit context-transfer consent, preparation/dispatch expiry, no automatic fallback/retry, exact pending-route recovery, normalized remote-delivery uncertainty and the revision-bound cloud review UI. A generic queue Resume is not blanket permission for a new destination. The successful client fake suite/build does not replace real Rust-Dart schema-3 fixtures, backend/keyring/HTTP synthetic gates, or fresh combined cloud assurance.

## Remaining limits

- Queues and drafts are process-local; forced termination/crash loses them. No durable outbox or replay on reopen.
- No active-job cancellation. An unresolved head intentionally remains blocked.
- GTK close/Quit source compiles and Dart channel/real-shell mock tests pass, but actual window-manager/Alt+F4/Ctrl+Q/native IME/accessibility behavior was not exercised on an installed desktop.
- Observed-instance checks are not atomic restart fencing or exclusive ordering across separate clients.
- Real long-running generation, cloud billing/delivery and installed service survival are not inferred from fakes.
- Final paired-service, recipient/package, installed-host, cloud integration and release/deployment gates remain separate.
