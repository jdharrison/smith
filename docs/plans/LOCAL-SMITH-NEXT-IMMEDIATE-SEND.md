# LOCAL-SMITH-NEXT-IMMEDIATE-SEND

## Intent and approval

User request: immediately add a sent message to chat and show Smith loading/thinking until a response arrives. The user explicitly approved the proposed client/test/design-documentation scope with “yep do it”, then requested continuation. Approval is local work for this conversation/session on `feature/autonomy`, base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, targeting the uncommitted source snapshot in the evidence record. Existing dirty work is preserved. No commit, remote mutation, deployment, service change, persistence change or model inference is authorized.

Risk: elevated operator-flow change. Implemented source only, not installed behavior or release readiness.

## Scoped plan and acceptance traceability

| Requirement | Implementation | Evidence |
|---|---|---|
| AC1: immediate user bubble and empty composer | Process-local request-keyed receipts created before serialized dispatch; captured original text | delayed admission and serialized-wait tests |
| AC2: truthful Smith Sending / Queued / Thinking feedback | observed-status mapping, accessible static progress presentation | widget status transition test |
| AC3: success replaces pending presentation without duplicates | authoritative-history reconciliation and provisional answer retention | completion and detail-failure tests |
| AC4: failure/uncertainty stops progress, never resends or clears newer draft | explicit receipt outcomes with existing query-only recovery | rejection, unknown admission, terminal failure tests |
| AC5: chat isolation and one-time scroll | per-controller receipts, added-ID scroll detection | promotion/navigation and two scroll regressions |

Implementation: `next/client/lib/text_chat.dart`, `workspace_controller.dart`, `chat_panel.dart`; tests: `next/client/test/optimistic_send_test.dart` and the rejection expectation in `assurance_regression_test.dart`. Canonical operator behavior: `docs/DESIGN.md`.

Producer/design intent was supplied by the explicit user request and approved proposal. Independent architect/assurance source review examined the implemented boundaries and adversarial outcomes. No additional protocol, service, persistence, or admission concurrency behavior is in scope.

## Review and disagreement record

Independent fresh-context review found a blocking scroll defect: retained failed receipt A became the last receipt after successful B reconciled, triggering another automatic scroll. Remediation tracks added IDs rather than the last surviving receipt; an adversarial widget test covers this scenario. Fresh review closed this blocker and found no remaining concrete immediate-send implementation blocker. The reviewer independently verified all six recorded source hashes and whitespace checks; Flutter results were reviewed as developer evidence, not independently rerun.

One broader presentation test looks for a nonexistent healthy-state `Retry observation` tooltip before submission. The equivalent healthy-state control does not exist; recovery uses a conditional text button. Both implementation and review left this unrelated scenario unchanged rather than adding an unrelated UI control or weakening test coverage. The combined validation gate therefore remains failing; this is not accepted risk or release approval.

## Evidence and remaining gate

Exact commands, toolchain, source hashes, host-clock timestamp and outcomes: [IMMEDIATE_SEND_VALIDATION.md](../../next/client/IMMEDIATE_SEND_VALIDATION.md).

- Immediate-send regressions: 8 passed.
- Seven-file relevant suite: 78 passed, 1 failed (pre-send presentation tooltip scenario above).
- Flutter analyzer, Linux debug build, formatting and whitespace checks passed.
- Independent scoped implementation review: scroll blocker closed; no further scoped implementation blocker.
- Overall assurance/release gate: no-go until the broader test issue is separately maintained and validated. No deployment performed.

Rollback: reverse only this slice's client presentation/submission capture edits, tests and documentation, preserving pre-existing multi-chat work. No storage migration, service rollback or installed-host action is involved. Source hashes bind the reviewed worktree, not a published artifact.
