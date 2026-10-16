# LOCAL-SMITH-NEXT-MESSAGE-TIMESTAMPS

## Intent and authority

**Approved local-source scope:** operator request, “add a very simple timestmap next to the SMITH / YOU messages so i can see historical and metrics extrpolated from the date deltas.” Work on existing `feature/autonomy`; base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, with pre-existing queue/cloud/scaffold changes preserved. No Jira integration is used for this local record. No commit, publication, installer launch, installed-state migration, service lifecycle action, real keyring/provider access or inference is authorized. Persistence makes this elevated work.

**Implemented source:** compact absolute local date/time beside saved message speakers, with seconds and timezone offset. This is not a metrics dashboard or model-latency measurement. Retention and transcript order do not change.

## Independent analysis and reconciliation

- Architect investigation (`d1ea36bc-50bb-4697-9dab-b84a136ed2f8`): `Message` is strictly role/content and reused in provider contexts and consent hashes. Jobs have real service `accepted_ms`, but successful history has no completion times. Recommend logical timing sidecar inside the existing atomic durable record, not timestamp fields in provider messages or a second file. Add an independent read-only schema-3 query with revision/count correlation.
- Producer/designer assessment (`03080019-e134-4d93-80b9-5171208f30d8`): date, seconds and timezone; wrapping header at narrow/2x sizes; no fabricated old dates, status completion timestamps, reordering or resend. Suggested explicitly labeling unknown times and optionally qualifying local-attempt times.
- Planner reconciliation: keep the scope small by omitting timestamps when unknown, including local receipts/status/provisional answers. No local-attempt clock is added. Tooltip/accessibility describe the saved event. Unknown historical dates remain absent, not epoch zero or migration time. A visible “Time unknown” on every old/status bubble is unnecessary clutter for the requested simple change.
- Storage decision: preserve the strict, concurrently implemented disk3 contract; introduce **disk4** with a required bounded timing sidecar and explicit strict disk1/2/3 migration. Application schema remains 3, Core remains 2, existing v1/v2/v3 history/results and provider message serialization remain unchanged. Old binaries reject disk4. This source-only addition does not authorize installed migration. Shared service edits must be surgical and preserve concurrent cloud work; no ownership over unrelated cloud changes is claimed.

## Implementation contract

1. Keep `Message { role, content }` unchanged. Persist nullable `user_at_ms` / `assistant_at_ms` per retained completed pair in a conversation-keyed collection within `Durable`. Old pairs are unknown. Metadata is bounded, validated, trimmed/deleted atomically with its history, survives job eviction/restart, and is reserved in completion capacity preflight.
2. YOU is the exact successful request's service-admission wall time. SMITH is sampled once after validated final output and publication fences, for the same transaction that saves history/result. Neither is generation-start time nor exact fsync completion. Invalid/pre-epoch clock values remain unknown. Accept only positive epoch milliseconds through 253402300799999 (UTC year 9999); no monotonic-order requirement or wall-clock rewriting.
3. Add schema-3 `conversation_timestamps` with exact arguments `{conversation_id}` and exact result `{conversation_id, history_revision, turns:[{user_at_ms, assistant_at_ms}]}` (maximum 16 pairs). Return nullable values for unknown pairs. Existing message/history/query shapes and v1/v2 projections stay exact. This read has no model/keyring/lifecycle effects.
4. Client attaches times only after matching service instance, conversation ID, saved history revision and pair count. Missing/failed/mismatched optional metadata leaves history usable without invented dates. Read through the existing serialized API owner, with no resends, provider fallback, or queue behavior changes.
5. Render `YYYY-MM-DD HH:mm:ss ±HH:mm` in workstation-local time beside SMITH/YOU, small existing theme style and wrapping rather than clipping/shrinking. Tooltip/semantics specify admission or reply completion. No timestamps for progress/error status, never-sent waiting entries, or provisional answers without authoritative time.
6. Deltas are approximate wall-clock end-to-end intervals, including service queueing and overhead. Clock corrections may yield negative deltas. No throughput/performance promises or calculations are added.

## Acceptance and validation

- AC1: saved new pairs retain exact times on duplicate observation/restart and through retained-job eviction; old disk1/2/3 history stays unknown. Corrupt/unsupported bytes preserved. New schema rejected by old readers; rollback requires pre-migration state plus compatible binaries (JSON does not back up keyring items).
- AC2: both ordinary and routed local/fake-cloud completion paths publish timings atomically; failure/interruption adds no completed pair. Trimming/deletion maintain alignment. Provider payloads/context hashes remain role/content-only.
- AC3: strict query validation and client instance/ID/revision/count fences prevent false association. Optional timestamp failure does not disable chat/queue recovery.
- AC4: compact date/seconds/offset formatting, unknown omission, old/new mixed history, narrow/2x layouts and semantics. No receipt/status timestamp fabrication, duplicates or scroll regressions.
- Gates: targeted Rust/client tests first, service format/Clippy/locked tests/build, Flutter dependency resolution/analyzer/tests/Linux build, installer compatibility when practical, `git diff --check`, fresh independent assurance. Record actual commands/results and any skipped integration/graphical gates below, bound to source identities. Prior queue/cloud results are not timestamp evidence.

## Evidence

**Source self-checks passed on 2026-10-15; no deployment claim.** The final [evidence index](LOCAL-SMITH-NEXT-MESSAGE-TIMESTAMPS-EVIDENCE.json) binds component input hashes, exact commands, artifacts and outcomes. `next/VALIDATION.md` links this bounded slice separately from ongoing cloud-provider acceptance.

- Service: ten timestamp unit tests and three routed/fake-cloud timestamp tests passed; full `cargo fmt --all --check`, warning-denied all-target Clippy, locked/offline all-target tests and release build passed. Three controlled fixture entry points remain ignored in the ordinary Rust harness; the cloud fixture was explicitly invoked from Dart. Private test providers/roots only.
- Client: `flutter pub get --offline` passed without dependency changes; formatter (71 files), analyzer, **456 tests passed / zero failed / zero skipped**, and Linux debug build passed. The full run includes all six real Rust socket/cloud/timestamp journeys. Focused timestamp/controller/widget plus optimistic-send/queue tests had 69 passes. Real-font desktop and 400×400/2x fixture captures were inspected, not installed graphical QA.
- Downstream: installer formatting and both `multi_chat_compatibility` tests passed against disk4 with exact legacy DTO/lifecycle/restart assertions preserved.
- Initial independent assurance (`93873c5f-ff3a-4864-9632-724bf307adce`) identified two stale installer disk3 assertions and missing canonical documentation/evidence. The expectations were updated to disk4, both tests fully rerun, and canonical Architecture/Design/Process/Scope/assessment documentation updated. No runtime/security blocker was identified in that review. Final remediation re-review returned **PASS for timestamp source-scope closure**, independently rerunning both installer compatibility tests, ten service timestamp units, three routed timestamp tests and the compiled Rust↔Dart timestamp test without skips. The reviewer verified all recorded source inventories/artifact hashes and the 456-pass client log; both findings are closed, with no remaining scoped blocker. This is not release/deployment approval.

### Intermediate failures retained

1. Initial widget test leaked its test-only SemanticsHandle until framework verification; immediate `try/finally` disposal fixed it. All 16 widget/format tests passed thereafter.
2. Initial installer compatibility run failed its two disk3 assertions (actual4); only those version expectations were changed for this task, not behavioral assertions.
3. Initial real-socket command used a relative `SMITH_TEST_SERVICE` path, rejected by four existing fixtures. Absolute fixture paths yielded all six integration tests passing; timestamp/fake-cloud tests had already passed.
4. An initial full client run had 455 passes/one timestamp integration failure. Its detailed error was not retained in the truncated response; focused and full unchanged-source reruns passed, so no root-cause claim is made. The new test's wait predicate was then tightened to require metadata arrival as well as completed history, preserving the existing deadline and all assertions. Final full run passed 456 with source inputs stable through the subsequent build.

Full recipient/Dagger packaging, native installed graphical checks, live service/keyring/provider/model work and installed migration/restore were not run or authorized. Binary-only downgrade is invalid after disk4 writes; preserve the pre-migration state and compatible binaries under separate rollout approval.
