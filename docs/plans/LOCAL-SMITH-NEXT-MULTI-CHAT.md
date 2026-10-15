# LOCAL-SMITH-NEXT-MULTI-CHAT

## Approval, baseline and scope

**LOCAL_PLAN_APPROVED for local implementation:** the operator's explicit `implement now` follows the reconciled proposal in this conversation (2026-10-14). Repository `smith`, existing branch `feature/autonomy`, base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`; target is the scoped uncommitted source/test/documentation candidate. Approval expires at completion of this implementation/review cycle or material scope/base change. No commit, branch creation, remote mutation, installed migration, manager action, model download, real inference or deployment is authorized.

Risk: **elevated** (persistence, protocol, scheduling, recovery and operator flow). Concurrent UI-redesign work is pre-existing and must be preserved; source review is against the combined snapshot, not a falsely clean base. UI components now exist in `next/client/lib/ui/`; do not replace their theme/assets/layout or create a competing shell. Current service domain and `text_chat.dart` remain the original single-chat implementation at handoff. The implementation owners have disjoint write sets: service owner `next/service/**`; client owner `next/client/**`; coordinator docs and installer compatibility checks. Do not edit another owner's files without a serial handoff.

Independent planning: producer `0dee81a3-82bf-474e-bd90-adbb9c7c888d`, architect `0dcc4c83-c1a3-4bc5-bb62-4c7a033a8662`, designer `60fba527-ff1f-4aa9-b309-9ca1139b02b8`, planner `45f5f35a-b8ba-4f13-b402-278458aae4b7`. These are planning contexts, not implementation assurance or approval.

## User-requested title and Settings follow-up

The operator subsequently requested hiding chats in Settings and first-message titles capped globally at 40 characters. This bounded local source/test follow-up uses Unicode scalar values consistently in Rust/Dart: sanitize and collapse first-message whitespace, truncate to 40 without an added ellipsis, and retain `New chat` only as an empty-result fallback. Preserve full prompts and atomic first-submit admission. Enforce 40 scalars on new renames and ordinary displays, but preserve historical 256-byte stored/wire titles and replay receipts; no installed migration or data rewrite is authorized. Settings hides chat/history controls without unmounting the selected chat/draft. Model-generated titles and deployment remain excluded. Client focused evidence is in `next/client/CHAT_TITLE_VALIDATION.md`; service focused tests cover boundaries, prompt preservation, replay/restart and legacy-title compatibility. Independent combined review (`08b05c71-b0d9-4e9d-a2ea-17ad6544850e`) initially found a Rust/Dart U+FEFF trimming mismatch. Remediation removes FEFF only from newly derived titles and uses explicit Rust-compatible incoming title whitespace validation in Dart, preserving historical metadata/receipts. Focused re-review passed: 4 service title tests, 71 client tests, scoped analyzer, and 104,485 cross-language helper/parser checks with zero mismatches. Service self-check additionally passed 8 multi-chat tests, 25 v2 unit tests and warning-denied offline Clippy; client details are recorded in its validation note. These are scoped source checks, not a rerun of aggregate builds/package/installer/socket gates or installed desktop validation; no new recipient package or deployment is claimed.

## Reconciliation

| Issue | Approved scope following the implementation instruction |
|---|---|
| Durable empty creation versus latest UI handoff | New opens/reuses one process-local unsaved workspace; first accepted message atomically creates the conversation and job. No create-then-submit sequence. |
| Producer's earlier durable-draft expansion | Deferred: unsent drafts, scroll and local unadmitted uncertainty are process-local. Accepted prompts/jobs and history are service-durable. |
| Parallel requests versus native parallel inference | Four outstanding chat jobs, one executing. True parallel inference is deferred; `runner.lock`, one worker and `--parallel 1` remain. |
| Shared memory | Global model/configuration and current-instance inventory; isolated conversation context. No implicit cross-chat memory. |
| Archive during work | Target queued/running/uncertain work blocks archive/delete; rename may proceed with fresh metadata. Unrelated chats remain usable. |
| Delete and runner cleanup | Atomic logical deletion of current-store conversation content and associated results/fingerprints; no runner stop or secure memory/media/backups erasure. Minimal replay IDs and content-free delete receipt remain. |
| Competing UI designs | Reuse `SmithWorkspace`, `ChatSidebar`, `ArchivedChatsDialog`, existing responsive geometry, Settings and ChatPanel. Only integrate state, per-row availability/status and identity/revision binding. |

## Frozen application v2 wire contract

Core lifecycle schema remains unchanged. App request/reply envelope fields remain, with `schema_version: 2` for the desktop. V1 installer requests/replies retain their exact original DTO shapes, including `Operation`; never add conversation fields to v1 `state`. Version correlation applies to recognized errors and preflight failures too. No production desktop fallback to v1.

The v2 domain operations below return JSON objects. IDs are opaque 32-character lowercase hexadecimal strings. Newly generated metadata/history/state revisions and sequences are nonnegative integers capped at9007199254740991. **Compatibility correction from independent review:** legacy configuration revisions retain all u64 values; v2 uses JSON numbers through9007199254740991 and canonical unsigned decimal strings above that through18446744073709551615. This applies to config/operation replies and expected-config/checked-management revision arguments; v1 and disk stay numeric. Noncanonical representations are rejected and exhausted configuration revisions fail without mutation. This lossless representation fixes valid-v1 migration without renumbering old authority. Null-valued identity/result/error fields are explicit.

- `state {}` -> `{config, inventory_observed, inventory, conversations, operations, retention, revision}`. `config` retains `{model,voice,revision}`. `inventory` retains existing `{models,voices,providers}` (current-instance observation only). Conversations are bounded summaries; operations are retained operation summaries without content results/payloads.
- Conversation summary: `{conversation_id,title,archived,metadata_revision,history_revision,sequence,origin_request_id}`. `origin_request_id` is null for migrated state. Ordering is stable descending sequence; updates do not reorder rows.
- Operation summary: `{request_id,conversation_id,kind,status,phase,config_revision}`. Global operations have null conversation_id. Status is `queued|running|succeeded|failed|interrupted`.
- `conversation_get {conversation_id}` -> `{conversation,history}` with the summary and the bounded existing role/content message shape.
- `submit_new {request_id,text,output:"chat",expected_config_revision}` -> operation summary. Atomically allocate conversation, persist submitted text and accepted job; no durable record on rejected admission.
- `submit {request_id,conversation_id,text,output:"chat",expected_config_revision,expected_history_revision}` -> operation summary.
- `conversation_rename {request_id,conversation_id,expected_metadata_revision,title}` -> `{request_id,conversation}`.
- `conversation_archive` / `conversation_restore` with `{request_id,conversation_id,expected_metadata_revision}` -> `{request_id,conversation}`.
- `conversation_delete {request_id,conversation_id,expected_metadata_revision,expected_history_revision}` -> `{request_id,conversation_id,deleted:true}`. Persist a content-free success receipt so lost acknowledgement can be queried.
- `poll {request_id,instance_id}` -> operation summary plus `{result,error}`. Conversation binding is verified by the client; lookup of `submit_new` supplies its service-generated ID. New submit success keeps `{assistant_text,audio:null,speech_error:null}`; retained legacy submit may have a validated inert `speech_error`, and legacy succeeded speak may retain `result:null`/`AUDIO_EXPIRED`. These are readable compatibility data, never executable speech. Lifecycle success results mirror the immediate mutation result above.
- `models`, `configure`, `configure_checked`, `verify_model`, `remove_model` remain global. Preserve installer v1 shapes and exclusive admission. Administrative/provider work is not silently queued behind chat; configuration/model operations reject while any chat is queued/running. The v2 client may only invoke its existing text/observation capabilities plus the conversation operations.

Exact retained duplicate requests return the same identity/effect without execution; changed arguments/target/version conflict. Preserve v1 fingerprint semantics for migrated requests. Tombstoned IDs fail `REQUEST_EXPIRED`. For `submit_new`, the surviving conversation's origin request ID supports read-only discovery, not replay after expiry. Deleting a conversation never remaps the designated legacy conversation or recreates it. Strict parsers must reject malformed identities, statuses, lengths and cross-conversation replies before applying client state.

## State, bounds and scheduler

Storage v2 strictly migrates valid v1 to one `Current chat`, preserving existing history/config/results/IDs/expiry. Legacy voices stay inert. Old unfinished work is interrupted without replay; unavailable old submitted text is not fabricated. Fresh storage need not contain a durable empty chat. Corrupt or unsupported state is preserved and blocks readiness. Atomic synced publication remains; no automatic plaintext migration backup.

Bounds: 32 active+archived conversations; 16 turn pairs/64 KiB per conversation; 16 KiB input; 32 retained operations; 1024 expired request IDs; four outstanding chat jobs (one running/three queued); one outstanding submit per conversation; 4 MiB wire frames and 16 MiB disk state. Stored/wire-compatible titles are trimmed, nonempty, single-line, at most 256 UTF-8 bytes with control/bidi-control characters rejected. The title follow-up additionally limits newly written titles to 40 Unicode scalar values; historic titles/receipts remain readable. IDs are service allocated and never reused within the durable store lifetime.

Preflight encoded bytes including JSON expansion, reserve valid bounded completion capacity for all admitted jobs, and evict terminal records only with durable replay IDs. Logical `CAPACITY` refusal must not latch storage failure. Never evict outstanding jobs or whole conversations. Delete preflights tombstone space before changing anything.

Durable queued records are authoritative. One provider-owning worker selects FIFO work using a bounded wake signal/condition variable; persist running before dispatch. Queue wait beyond ten minutes is rejected before inference. Keep existing 180-second provider budget, CPU/Vulkan fallback, cleanup/lock and idle ownership. No per-chat runner. Completion checks owned state, running request, target and history revision and atomically publishes terminal result/history. Restart interrupts ALL queued/running work; no automatic resume/replay. Close only detaches.

Archive/delete recheck target quiescence under admission lock. Delete removes all target title/history/prompts/results/content-derived fingerprints and lifecycle receipts, retaining only opaque replay protection and a content-free deletion receipt. Missing target IDs never alias another chat. Delete does not promise erasure from allocator/KV memory, journals, swap or backups.

## Client and UI contract

One global connection/model/inventory owner, per-chat draft/history/pending/provisional/error/scroll state. One process-local ephemeral workspace; New preserves its unsent draft when reusing it. First submit promotion retains presentation identity/focus/draft revision. Accepted prompts/jobs can be rediscovered after close, but unsent drafts and unadmitted identities have no cross-desktop-restart guarantee.

Single bounded fair short-exchange arbiter around the existing native helper; no overlapping helper calls and no polling sleep holding the arbiter. Central coalesced observation of up to four pending chats. Finite queue-aware observation budget (ten-minute queue plus existing 210-second execution observation allowance); timeout retains query-only uncertainty, never replay. Background work does not globally disable switching or submitting another chat.

Boot waits for verified Core plus authoritative index/selected detail/operation associations, not terminal generation. Current-instance inventory observation can be adopted on reattach during active work; never persist readiness across service restarts. Missing inventory disables Send and uses observation-only idle refresh/Retry, not inference.

Reuse UI callbacks and components. Rows show truthful queued/running/uncertain states and target-specific actions. Active menus: Rename/Archive/Delete. Archived modal: Reactivate/Rename/Delete, with confirmation for Delete only (existing immediate progress/error dialog is not extra consent). Bind dialogs to ID, title and revisions at opening; stale confirmation rejects, never silently rebases. Selected archive/delete falls back to the process-local blank; restore need not switch. Preserve per-chat scroll and draft across dialogs/Settings/resizing/switching. Preserve Enter/Shift+Enter/IME/repeat/selectable-text and provisional/expired recovery semantics.

## Requirements and task traceability

| REQ/AC | Task | Required evidence |
|---|---|---|
| 001 Isolated durable conversations and atomic first admission | Service schema/migration/context domain | v1/inert-voice migration, corrupt bytes preserved, fake provider A/B context capture, rejected first submit creates nothing |
| 002 Correct-target lifecycle and logical deletion | Service mutations + UI callbacks | revision/idempotency/lost-ack tests, target-only busy gates, deletion residue and tombstone-capacity tests |
| 003 Truthful bounded concurrent admission | Durable FIFO worker | one provider invocation, FIFO/cap/expiry, restart interruption, no dropped wake or late publication |
| 004 Compatible protocol and observation-only recovery | Versioned envelopes + client controller | exact real v1 installer, real Rust-Dart fixture, all versioned errors, no helper overlap/replay |
| 005 Minimal reused UI and preserved interaction | Client state integration | independent drafts/scroll, promotion, archive modal/confirmations, Boot during work, keyboard/IME, focus/narrow/2x text |
| 006 Safe evidence and ownership | Coordinator documentation/review | preserved prior changes, exact final source identity, self-check commands/results and fresh assurance/adversarial findings |

## Validation and gates

Focused Rust/client tests first; then from `next/`:

```sh
DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make service/test installer/test client/test
DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
```

Root `git --no-pager diff --check`. Commands are bounded and failures/timeouts reported. Dagger may acquire development dependencies/pinned runtime artifacts and use caches; no model weights, host GPU probes, live manager or installed state. Aggregate test includes package tests; build includes installer package verification. Historical UI evidence remains historical; combined candidate gets fresh results.

Update affected canonical Architecture/Design/Process/Scope/Host resource notes, implementation assessment and next documentation/evidence. Self-check precedes fresh independent assurance/adversarial contexts; no self-approval. Blockers require remediation and re-review. Approval is for source only, not release.

Live rollout is separate: pair client/service, capture exact approved pre-migration state backup, rehearse restore; old binaries cannot read v2 and binary-only rollback is invalid. Source rollback must preserve other workstreams. Deferred: true parallel inference (slot/failure isolation and measured RAM/VRAM gates), shared memory, persistent unsent drafts, model-generated titles, unlimited history, cancel/resume and any deployment.
