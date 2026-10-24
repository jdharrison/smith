# LOCAL-SMITH-LOCAL-OPENCODE-RUNNER

## Status, authority and intent

**LOCAL_PLAN_APPROVED for source-only implementation and isolated validation:** after reviewing this plan, the operator explicitly answered “yes go ahead”. Live Docker/image preparation, real clone/inference, installation, publication and deployment remain excluded. Approval expires on completion or material scope drift. This is authorization to implement the reconciled contract below, not functional acceptance. The operator requested the simple flow “container + clone repos + run prompt + return reply”, without Smith Agent selection, then confirmed OpenCode as the default/fixed harness, use of Smith's installed local models if possible, and exclusion of cloud. This record reconciles source discovery and independent producer/architect/designer analyses. No implementation or live acceptance is claimed by this planning change.

Repository: Smith; existing `feature/autonomy` bootstrap branch; observed HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`, with extensive concurrent dirty/untracked work to preserve. Owner: developer/planner. Risk: critical trust-boundary/Git/provider/scheduling/recovery change. No commit, branch mutation, installation, daemon access, image build/pull, repository fetch, inference, model download, deletion, deployment or remote mutation is authorized by this record. Detailed source implementation must follow reconciliation of the blocking contracts below; actual host effects require exact separate approval under [Process](../PROCESS.md#approval-boundaries).

Agent-navigation hiding is independently owned by [LOCAL-SMITH-HIDE-AGENTS-NAVIGATION](LOCAL-SMITH-HIDE-AGENTS-NAVIGATION.md). Do not modify its shell/editor/navigation work or remove profile data. New Dev runs must not consult the Smith Agent registry, even while its metadata remains present.

## Rootful endpoint amendment

The operator subsequently approved using the existing rootful Docker daemon for the host service. [LOCAL-SMITH-DEV-ROOTFUL-DOCKER](LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) supersedes this plan's rootless-only prerequisite for **new local-runner execution only**: select fixed `unix:///run/docker.sock` without fallback, preserve the legacy receipt adapter's rootless behavior, and retain all worker restrictions and live-effect boundaries. Docker remains operator-provided; installer provisioning and new Dagger integration are excluded. The host authority is explicitly accepted, not deferred verification or broader risk acceptance. See that record for independent analysis, tests and source status.

## Presentation amendment

The [approved Dev jobs-list/modal slice](LOCAL-SMITH-DEV-JOBS-LIST.md) supersedes the inline form/current-result presentation for the local runner. Retained v2 jobs appear in a status list, New job uses an Edit/Review/Start modal, and row activation opens keyed read-only details. This is not a new scheduler, execution slot, stop/reconciliation capability or change to the frozen model/context/runtime contract. Backend and installed effects remain separately scoped.

## Full-clone amendment

The operator explicitly requested ordinary full Git clone/fetch for code references instead of custom shallow acquisition. [LOCAL-SMITH-DEV-FULL-CLONE](LOCAL-SMITH-DEV-FULL-CLONE.md) supersedes depth-one acquisition below: native full-history clone with normal branches/tags, one advertisement plus one pack POST, unchanged credential-free public-HTTPS relay, network-isolated worker, limits and ten-repository execution ceiling. There is no shallow fallback, new model Git/history tool, submodule or LFS support. This source change grants no live clone or rollout authority.

## Three-container queue amendment

The operator explicitly requested a concurrent FIFO Dev queue with at most three containers and confirmed “continue”. This authorizes source implementation and isolated checks, not installed rollout. The [current queue contract](../ARCHITECTURE_DETAILS.md#local-only-one-shot-successor--source-implementation-live-acceptance-pending) supersedes this plan's original single-job/global-reservation choice: three occupied container slots, separately serialized native inference, v4 Dev operations and private ledger4. Independent architecture/producer/designer analyses were reconciled; unchanged worker capabilities, frozen inputs, no-replay and separate live-effect approval boundaries remain.

## Product contract

**Target:** saved project + exact task -> owned isolated container -> clone configured public HTTPS repositories -> OpenCode inspects the checkouts using Smith's frozen selected local model -> save/display a correlated final reply -> verify owned-container stop.

- Service-owned FIFO queue with at most three occupied container slots; native inference remains single-owner/serialized. No board, role orchestration or automatic retry.
- OpenCode is the harness, not a model/provider. Use one fixed admitted Smith-local route; never upstream default/free/cloud discovery or fallback.
- Reuse the currently configured local model without a new picker. Freeze configuration revision, catalog name and installed artifact digest; missing/incompatible selection fails explicitly, without download or substitution. Existing Chat cloud selection is irrelevant to Dev.
- First slice: public HTTPS, read-only repository inspection. No SSH/private credentials, code writes, package installation, builds/tests, arbitrary shell, subagents, web tools, commits, pushes or deployment. A saved `read_only:false` does not add permissions.
- Preserve existing global/project context as disclosed immutable inputs, plus exact task, but omit Agent text. This preserves the current context behavior rather than silently deleting saved instructions. Confirmation names every included source; repository text never grants authority.
- Cloning requires network access to named repository destinations. “Local-only” describes inference, not an offline clone operation. Model context must never leave the workstation.
- Keep host roots, checkout paths, SSH/keyring/config, Docker socket, runner socket, model weights and native API key out of the worker. No host-published port or new external model endpoint.

## Implemented baseline and feasibility evidence

**Implemented:** `service/src/dev_runtime.rs` creates a `--network=none` receipt sandbox; `dev_worker.rs` disables providers/tools and uses `noReply:true`. It neither clones nor infers. The required rootless socket was absent in the earlier diagnostic observation; no new live readiness claim follows.

`service/src/providers.rs` exposes text `chat(model, messages) -> String` and documents that native child ownership follows the long-lived app worker thread. `service/src/runner.rs` uses a private Unix socket, verified model descriptor, native runner lease, bounded HTTP and a content-only parser that deliberately rejects tools. Its context/output limits are4096/1024 with a32-token margin; these do not certify OpenCode prompt or function-call compatibility. `catalog.rs` lists seven installed-model identities, not compatibility certifications.

### Pinned public-source inspection — 2026-10-18

Read-only public source was inspected at OpenCode commit `60c9be092129745688bb3c3a752affc6cbd66ede` (the existing1.19.3 pin):

- [`provider/provider.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/provider/provider.ts): `@ai-sdk/openai-compatible` is bundled; a custom provider/model can specify a local base URL without a provider key. Do not select built-in `openai`, whose loader uses Responses.
- [`session/llm.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/llm.ts): normal execution uses `streamText`/`fullStream`. The compatible adapter expects Chat Completions SSE, not Smith's current non-streaming text return. A bridge may buffer validated native completion then emit the required SSE, including typed tool calls.
- [`session/prompt.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/prompt.ts): `noReply` skips inference; explicit non-default session title avoids title generation; real inspection may require several inference/tool rounds for one task.
- [`session/summary.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/summary.ts): summary performs counters/diffs, not another model inference. `snapshot:false` avoids its diff path.
- [`session/processor.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/processor.ts) / [`session/retry.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/retry.ts): processor retry behavior exists even when SDK retries are zero. Host duplicate-inference suppression and terminal latching are required; do not promise zero internal retry attempts from configuration alone.
- [`session/llm/request.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/llm/request.ts): a short fixed internal inspection prompt can replace the large provider-default prompt. This is internal harness configuration, not restoration of Smith's Agent registry.
- [`tool/read.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/tool/read.ts) / [`session/instruction.ts`](https://github.com/anomalyco/opencode/blob/60c9be092129745688bb3c3a752affc6cbd66ede/packages/opencode/src/session/instruction.ts): stock read behavior includes large outputs, attachments/LSP and nearby-instruction loading. A configuration-only “read allowed” rule is not proof of the proposed bounded clone-root guard.

These are source findings, not proof the packaged executable matches the source or that any catalog model handles its tools. No OpenCode/model/daemon execution was performed.

## Reconciliation and blocking contracts

Independent producer, architect and designer analyses agree on one retained form, local-only execution, immutable snapshots, no Agent dependency, durable final replies and conservative uncertainty. The planner preserves these issues rather than voting them away:

| Issue | Reconciled disposition / owner |
|---|---|
| Architect initially proposed a single text-only inference | Rejected as the default: cloning alone does not expose files to a model. Implement a bounded real repository-inspection loop. “One-shot” means one submitted task, not exactly one model request. Owner: developer/architect. |
| Existing provider cannot represent tool calls/results | Add a separate typed Dev completion contract; do not loosen or repurpose Chat's content-only validator. Count the actual tools and messages together. Owner: developer/architect; blocking before adapter completion. |
| Dev currently uses a separate thread | Reserve shared capacity and dispatch the attempt through the long-lived app worker/native owner. Do not create a second runner/provider instance or inference slot. Owner: developer; blocking scheduling tests. |
| Built-in read/steps do not enforce the required limits | Resolve a trusted read guard/tool contract before enabling inspection: canonical clone-root containment, no symlink escape, text-only bounded reads, no execution/LSP/automatic instruction promotion. `agent.steps` is advisory, not a hard iteration cap. Owner: developer/architect; unresolved implementation blocker. |
| Clone egress versus network-free inference | Resolve exact fetch topology and destination enforcement before enabling cloning. Prefer a narrow host-mediated broker or separately bounded fetch phase; do not grant unrestricted networking to the inference worker. No host Git/home/credentials fallback. Owner: developer/architect; unresolved implementation blocker. |
| Upstream processor may retry | Admit only monotonic correlated new continuations, reject replayed/duplicate completion requests and latch terminal failures. Disable title agent/automatic compaction and use explicit fixed model/session title. Owner: developer; blocking fixtures. |
| Model/context suitability | Keep4096/1024 bounds initially; reject actual template+tool overflow rather than silently truncating required task/context. Tool-aware template/parser support and exact model digest require compatibility evidence. Unsupported means unavailable, never cloud/default fallback. Owner: developer/assurance; blocking functional acceptance. |
| Context text after removing Agent selection | Preserve disclosed global/project text and exact task, remove only Agent text/dependencies. No silent semantics change to saved fields. Owner: developer/designer. |

Do not represent the new runner as implemented, enable unsafe partial execution, or mark these contracts resolved on narrative confidence alone.

## Implementation milestones and traceability

| Priority / requirement / acceptance | Task and likely modules | Done when / required evidence |
|---|---|---|
| P0 / LOCAL-1: fixed local model, no Agent | Freeze admission DTO/model/context and version Dev operations; `app_dev_jobs.rs`, `app_v3.rs`, client domain/controller | Strict new/legacy protocol fixtures; zero Agent/cloud/keyring calls; model/config/revision drift refuses without dispatch |
| P0 / LOCAL-2: shared bounded inference | Add tool-aware deadline/digest-bound native completion and app-worker reservation; `providers.rs`, `runner.rs`, app worker/admission | Chat behavior unchanged; no concurrent inference across API versions; deadline never resets per tool round; no replay/fallback after dispatch |
| P0 / LOCAL-3: narrow bridge | Container-loopback compatible facade -> correlated bounded stdio -> shared native provider; `dev_worker.rs`, `dev_runtime.rs`, small private broker module | Patched-SDK-compatible SSE/call/result fixtures; strict tool schema; duplicate/foreign/oversize/stall/disconnect rejection; no configurable host endpoint or secrets |
| P0 / LOCAL-4: actual read-only checkout inspection | Resolve fetch and read-guard blockers; pinned worker image includes trusted Git/tools | Frozen ordered URLs/effective refs; public HTTPS only; resolved commits; no helpers/hooks/submodules/LFS/plugins/private auth; containment/size/disk/egress/execution-denial fixtures |
| P0 / LOCAL-5: durable truthful reply | Version private Dev storage/results, preserve historical receipt records; `app_dev_jobs.rs`, `dev_jobs.dart`, controller/view | Correlated final assistant text persisted before teardown; separate reply availability/stop certainty; completed only after verified stop; cleanup-unknown retains reply |
| P0 / LOCAL-6: minimal UX | Project/task/local-model row; confirmation and preparing/cloning/running/result; client Dev files/main wiring only | Existing live-region/selectable reply, keyboard/IME/hidden-pane/narrow tests; cancellation/drift no dispatch; query-only original-request recovery |
| P0 / LOCAL-7: reviewed evidence | Canonical docs with implementation, source checks, fresh assurance, then separately approved live acceptance | Stable scoped source/check binding; no skipped/failing required evidence relabelled PASS; actual image/model/repo/lifecycle proof remains separate |

Persist the effective checkout refs in the approved snapshot; do not infer branch permissions from stored Base/Work patterns. Ref/default-branch resolution and all numeric caps must be frozen in the implementation contract before dispatch is enabled. Use existing256KiB framing, bounded32-record/4MiB private ledger and resource limits unless a separately reviewed change demonstrates necessary adjustments. Reserve serialized prompt/result/commit evidence headroom before admission; capacity failure never evicts evidence automatically.

## UI/result and recovery contract

The read-only model row comes from authoritative service local configuration, never active Chat route or its cloud-aware status label. Start confirmation discloses exact project/task/context, ordered repository destinations/effective refs, local model/digest and retained artifacts. No model choice, provider fallback or credentials are inferred from repository files.

List/get/Refresh/Resolve remain retained-state reads with zero Docker/Git/model probes. Lost acknowledgement retains its original identity. Restart marks unfinished work uncertain and blocks new dispatch; no replay or automatic reconciliation. Preserve old `recorded` as receipt-only and all historical Agent snapshots. No existing app-state migration or installed upgrade is implied by this plan. A versioned private-ledger change needs strict legacy decoding/corruption preservation and later approved compatible restore evidence.

A final reply is selectable inert text, not a command or software-delivery approval. Record model and repository commit identities, final-message correlation and timestamps. A captured reply survives cleanup failure visibly; do not call the run completed until owned shutdown is verified. Stopped containers/records remain retained; deletion/pruning is not authorized.

## Stage gates and validation

1. **Plan -> develop:** resolve fetch/read-guard/protocol/budget contracts and bind a bounded source plan; preserve unrelated Agent-navigation work. Missing security/authority decisions block execution code, not source investigation or isolated fixture work.
2. **Develop -> self_check:** implement only the reconciled slice; use synthetic Git/model/SSE/container fixtures and temporary roots, never ambient services as test fallback. Run affected Rust/Flutter/compiled integration/compatibility checks required by [Process](../PROCESS.md), preserving exact failures and skips.
3. **Self_check -> assurance:** fresh independent review of stable source/evidence; test privacy, path/URL/tool authority, capacity, cancellation/publication, duplicate/restart and cleanup uncertainty. No self-approval or accepted-risk waiver.
4. **Assurance -> live validation/release:** separate human approval names candidate/image hashes, daemon/account, public repository hosts/refs, installed model digest, resource/time/egress budgets, artifact retention and stop/restore effects. Rootless setup, image build/downloads, model acquisition, installation, clone and inference are distinct effects—not authorized here. Release/deployment retains separate gates.

**First functional acceptance:** in an approved sandbox, clone Smith and answer “What language is this project in?” using actual inspected checkout files and citations; persist the reply across desktop reconnect, leave the host checkout untouched and verify owned shutdown. Also demonstrate no cloud/default fallback, no additional credentials/host mounts, denied writes/execution, and ambiguous interruption without replay.

## Reconciled implementation contract — source approval

A follow-up independent architecture review conditionally recommended source implementation of the following resolution, with all listed negative fixtures required before acceptance. These decisions supersede the table's open topology choices, not its verification gates:

- Keep `--network=none`. Container-local Git HTTP facade relays only smart-HTTP v0 advertisement/pack operations over correlated stdio. Host constructs frozen HTTPS:443 destination paths/headers, pins validated public DNS addresses with normal TLS/SNI, and disallows redirects, credentials, ambient proxies, arbitrary headers and protocol fallback. Pin advertised Base ref (or HEAD if Base absent), require its SHA1 OID among the ordinary clone's advertised wants, reject SHA-256/object-format and shallow/deepen/filter/have extensions, and validate bounded pkt-line bodies/capability allowlists. Fetch closes irreversibly before private task/global/project text is sent. Actual detached checkout OID must match pinned acquisition evidence.
- Native descriptor-relative/no-follow reader enforces clone-root containment, rejects `.git`, symlinks/special/binary/attachment reads, bounds files256KiB, path depth32, directories/entries/total inodes20,000, and text output2KiB/32 lines. At most16 reads/32KiB aggregate; reserve single-use call capacity before I/O and never reset after failure. Clone workspace remains256MiB tmpfs with bounded inodes and128MiB expanded inspected tree; pack64MiB job-wide, advertisement256KiB, pack request16KiB. Projects may retain16 repository metadata entries; the [full-clone amendment](LOCAL-SMITH-DEV-FULL-CLONE.md) preserves the explicit10-executable-repository ceiling and unchanged32-HTTP-request budget, with one advertisement and one POST per repository.
- Use one immutable dependency-free image plugin and no discovered/default plugins or project config. Trusted before-hook requests the native reader only for an exact host-authorized call ID/arguments; native stages bounded safe text in an exclusively created private per-job directory. Builtin Read is redirected ONLY to that staging file at offset1/fixed limit, so it never traverses repository instruction ancestors. Permit staging directory access narrowly. Success after-hook restores original args and replaces output/metadata with the native result; failure at any step terminates the attempt (after-hook is not finally). Native-generated staging filenames never interpolate model call IDs; host records/acknowledges results before OpenCode sees them. No attachments or LSP.
- Fixed system-transform and native validation suppress automatic repository instruction promotion. Host requires the exact admitted task and append-only transcript, validates issued calls before SSE, authorizes reads once and matches subsequent tool results to recorded output. Guard readiness binds job/policy/image-plugin identity and is readiness evidence, not hostile-worker authentication. Missing guard, mutation, unsupported transcript or retry closes the attempt without inference/replay.
- A separate tool-aware Dev completion API shares the long-lived app worker and existing native lease. Preserve Chat's tool-free contract. No cloud/discovery/model fallback; freeze installed selected model name/digest and config revision. Original inference deadline180s across <=8 rounds (<=8192 generated tokens), preparation90s, work cutoff start+292s, overall300s with final8s reserved cleanup. Every request must fit actual4096 context/1024 generation/32 margin including tool schema/history; quotas are not fit guarantees.
- New versioned Dev operations and a separate private versioned local-run ledger preserve the old receipt ledger byte-for-byte. Combined capacity32 and4MiB new-ledger bounds; unreadable/ambiguous legacy state fails new Dev admission closed. Replay IDs are shared for uniqueness, never reset or silently reused. No Agent dependency or app-state schema change. Keep the global reservation through reply publication and verified cleanup; publication/cleanup uncertainty fences unsafe dispatch. Gate new provider-effect jobs atomically with app-state admission to close races.

Mandatory isolated fixtures include malformed pkt-lines/DNS/address classes, sealed-fetch egress refusal, checkout/OID mismatch, descriptor/symlink/binary/directory bounds, staging permissions/ranges/exclusive identities, before/execute/after failures, single-use/concurrent read authorization, result-ack ordering, exact transcript preservation, duplicate inference, context/deadline/aggregate limits, corrupt legacy capacity, scheduler/cleanup/publication races and original-request recovery. Native Git/OpenCode/model interoperability remains separately approved executable acceptance, never inferred from source fixtures.

## Readiness findings closeout — 2026-10-18

**LOCAL_PLAN_APPROVED, bounded source follow-up:** the operator requested “address both” after the readiness review found inconsistent fully qualified branch handling and final replies accepted without read evidence. This authorizes only the two local fixes, isolated regression checks, matching documentation and fresh independent source review; no Docker/daemon, repository download, model inference, installation, commit or remote effect is included. Existing dirty/untracked work remains preserved.

**Implemented source:** bare and `refs/heads/` branch inputs share branch-name/checkout-reference normalization for host advertisement selection, worker clone arguments and checkout evidence; an absent Base remains `HEAD`, with no fallback when the exact ref is missing. The broker rejects a final model response until a successful nonempty authorized read result has been recorded and consumed in the validated append-only transcript. A reserved/failed read or an unconsumed result is insufficient. This is evidence of a read, not proof that the answer is correct, sufficiently inspected or properly cited; the first functional acceptance still verifies inspected-file evidence and citations.

**Focused validation PASS on the dirty working tree:** commands below ran from `service/`, offline with existing dependency/build caches. No live runtime effect ran. All returned exit0; the four scoped source hashes matched before and after the final combined check group.

| Exact check | Actual result |
|---|---|
| `cargo test --offline --locked --lib dev_broker::tests -- --test-threads=1` | 7 passed,0 failed/ignored; includes three new negative tests and the existing successful read/transcript/final-reply path |
| `cargo test --offline --locked --lib dev_ -- --test-threads=1` | 88 passed,0 failed/ignored,209 filtered out |
| `cargo test --offline --locked --lib app::local_dev::tests -- --test-threads=1` | 22 passed,0 failed/ignored,275 filtered out; overlaps the preceding invocation by one test |
| `cargo clippy --offline --locked --lib --bin smith-service -- -D warnings` | PASS; no warnings/errors |
| `rustfmt --edition 2021 --check --config skip_children=true src/dev_broker.rs src/dev_fetch.rs src/dev_fetch_tests.rs src/dev_worker_local.rs` | PASS |

Branch fixtures exercise bare/qualified main and nested refs, distinct OIDs, absent Base, synthetic advertisement/pack/checkout evidence, and missing-exact-ref sealing without fallback. Broker fixtures reject no-read completion, reserved/empty rejected results and recorded-but-unconsumed evidence; the existing happy path still succeeds after exact result consumption.

Fresh independent read-only architecture/adversarial assurance inspected both fixes and immediate integration paths and found no scoped correctness/architecture blocker. It did not duplicate executable checks or grant release/live-effect approval. Its reviewed four-file identities match the parent check group:

```text
62601058249b0063cc02cdc5c0d41059700670a4d2eb6b13ddb6dd50058a6199  service/src/dev_broker.rs
1b634c4e7f2e62f447f4c5dbd92ef47360e4ce8bfd6131659fdd3bb46990bf02  service/src/dev_fetch.rs
06281e8b2e2998fbbb28824409220cf043f32ae455e7d79d18df32529e46b59c  service/src/dev_fetch_tests.rs
71ea9337e97d107e72d3beddba94fa9e800c61797cc135f418035ed01fe3a4d3  service/src/dev_worker_local.rs
```

These untracked-file hashes bind only the bounded follow-up, not the entire candidate. The minimum evidence gate also permits directory enumeration/empty-file output from the existing reader; it does not certify relevant file-content inspection or answer correctness. Full component/root/package, compiled integration, real Git/OpenCode/model compatibility, live readiness, installed acceptance and release approval remain unestablished. No UI code changed or Flutter checks were rerun for this follow-up.

## Planning evidence and remaining scope

This turn performed repository/source inspection, independent role analyses and public pinned-source fetches only. No production code was changed for this new feature, and no tests, Docker/daemon/image actions, clone, model/provider inference, install or deployment were run. Earlier receipt/diagnostic test results remain bound to their historical source, not this proposed runner. Neither the architecture recommendation nor this plan is runtime, installed or release acceptance.

Planning-only self-check: scoped `git --no-pager diff --check` passed, and a Python read-only check of all11 touched documentation files found no trailing whitespace or missing relative file-link targets. No executable tests/builds were run for this planning change. Fresh independent read-only assurance found no blocking contradiction in proposal labeling, local-only/provider restrictions, preserved blockers, approval boundaries or legacy/uncertainty behavior. Its PASS is limited to planning-document consistency; it did not independently validate upstream/package/model behavior or source immutability. The next gate remains fetch/read-guard/protocol/budget contract resolution and a bounded implementation approval, not execution enablement.
