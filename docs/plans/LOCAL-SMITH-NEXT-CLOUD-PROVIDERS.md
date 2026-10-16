# LOCAL-SMITH-NEXT-CLOUD-PROVIDERS

- Retain normal installation and the local **Model** step; append an optional **Providers** step supporting both OpenAI and Anthropic, independent credential saving and explicitly confirmed validation.
- Add a per-chat grouped **LOCAL / CLOUD** model selector. Continue the same conversation across models/providers; never permanently bind a conversation to a model. Only accepted requests are immutable.
- Keep service-owned state, one worker, bounded admission, private transport, explicit context-transfer consent, keyring-only credentials and no implicit network effects, retries or provider fallback.
- Deliver source and isolated evidence only. This plan is not installed-host, credential-store, paid API, migration, release or deployment authorization.

## Status, approval binding and authority

**Approved local source scope (`LOCAL_PLAN_APPROVED`), implementation contract.** The human explicitly instructed implementation with “go ahead” after the independent planning/reconciliation discussion. The coordinator delegated initial plan drafting and then disjoint implementation work. This records the human's source-only authorization, not an agent approving its own work. Source validation and independent review outcomes are recorded separately in `next/VALIDATION.md`.

| Binding | Value |
|---|---|
| Work record | `LOCAL-SMITH-NEXT-CLOUD-PROVIDERS` |
| Repository / branch | `smith` / existing `feature/autonomy`; no new branch or rename |
| Base | `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` |
| Starting worktree | Clean at the coordinator's initial inspection; concurrent message-queue/scaffold changes appeared during planning and were preserved and integrated, not attributed to this feature |
| Target | Resulting scoped, uncommitted local diff against that base, including this plan and required canonical-document updates |
| Approved effects | Source/documentation changes and isolated synthetic validation for the agreed feature; main-agent official-document research as separately tasked |
| Expiry | End of this implementation task/session, explicit revocation, or material scope/base/authority change, whichever occurs first; rebind before continuing outside that window |
| Risk | Critical: credentials, egress, consent, persistence/recovery and public protocol |
| Excluded effects | Commits, branch creation, push, PR/merge, release, deployment, normal-host keyring access/mutation, real keys, paid/live inference, installed-state migration, service/manager changes, model downloads and GPU probes |

Apply [AGENTS](../../AGENTS.md), [Scope](../SCOPE.md), [Architecture](../ARCHITECTURE.md), [Process approval boundaries](../PROCESS.md#approval-boundaries), [Design](../DESIGN.md) and [Host Profile](../HOST_PROFILE.md). The explicit user direction narrowly supersedes the initial-product cloud exclusion and installer-only active-selection restriction for this task. The main agent must update the affected canonical documents in the same implementation change; this plan does not silently rewrite their historical implementation claims.

Initial plan drafting changed this file only. Implementation ownership is recorded below; fresh independent assurance/adversarial review follows implementation and remediation. Earlier planning reviews are not final assurance.

## Reconciliation and exclusions

The inputs are the independent architecture review and producer/designer requirements relayed in the task discussion, followed by the human's final clarifications. No unavailable upstream artifact or new review PASS is asserted.

| Earlier option / concern | Reconciled decision and authority |
|---|---|
| Cloud instead of the local Model step | Rejected by the human: normal local Model remains, then optional Providers. Cloud failure/skip does not undo local success. |
| Installer-owned global active selection | Superseded: installer maintains the local default and provider credentials/checks; chat selects its route per Send. |
| Permanently model/provider-bound chats or mandatory new chat | Rejected: same-chat swapping is required. New chat remains available, not required. Branch/Compare is deferred. |
| In-flight work retargeted by a selector/global configuration change | Forbidden: snapshot routing at service admission; changes affect future sends only. |
| Owner-only plaintext credential files | Rejected: Ubuntu Secret Service/keyring only, with no programmatic unlock/prompt or plaintext/environment fallback. |
| Implicit first cloud history transfer | Forbidden: service-bound explicit consent for a previously unapproved provider/credential generation in that conversation. |
| Same approved destination, different supported model | Normal explicit Send; no repeated transfer ceremony. Exact model is still bound per request. |
| Credentials or validation treated as blanket API authority | Forbidden: save is local-only; validation and chat are distinct expressly requested effects. |
| OpenCode as a shortcut/provider broker | Deferred: no OpenCode, Codex/Claude Code, browser/subscription login, arbitrary compatible endpoints, plugins or agent/tool capabilities. |
| Illustrative model names | `gpt6.0`, `gpt6.1.sol`, `claude5.5` are not verified IDs. Main owns the authoritative API/model freeze before adapter coding. |

Other exclusions: streaming, images/audio/files, search/tools, remote conversation IDs/chaining, autonomous/background API work, parallel inference, durable unsent drafts, automatic model discovery, arbitrary URLs/headers, automatic credential deletion, secure-erasure claims and unit/linger changes.

## Detected stack and concrete seams

**Implemented at the base:** Rust 2021 service with `serde`, `serde_json`, `libc`, `sha2`; declared Rust 1.83 minimum; one worker thread and durable FIFO. Flutter/Dart uses strict app-v2 DTOs over a verified native helper/private Unix socket. Installer directly reuses the service Controller and exact app-v1 DTOs. Core schema 2 and deterministic unit bytes are separate and remain unchanged.

- `next/service/src/app.rs:140-165,407-469,529-621`: local configuration, operation DTO, worker ownership and provider injection.
- `next/service/src/app_v2.rs:105-233,711-812,1049-1102`: storage, migration, v2 operations and dispatch. Current dispatch reads global configuration; this must not determine a routed job's model.
- `next/service/src/providers.rs:151-175`: retain the local `Provider` seam and its existing fixtures where practical. Add a separate injectable cloud/keyring dependency and an explicit test constructor rather than making old `Application::open` tests access D-Bus/network.
- `next/service/src/app_transport.rs:194-209`: version recognition/error correlation must include 3; retain finite framing and peer checks.
- `next/installer/src/setup.rs:486-637`: preserve v1 helper behavior; add an explicit version-aware/v3 exchange and typed result validation for new flows.
- `next/client/lib/app_api.dart:225-269`, `app_v2.dart:235-419`, `workspace_controller.dart:894-1004`: extend native protocol validation and immutable Send intent, not an HTTP implementation in Flutter.
- `next/installer/Cargo.toml:21-30`: the existing HTTPS/Rustls/Tokio family informed dependency selection. The final service uses a single-connection Hyper/Rustls/Tokio transport so the 16-KiB raw-header ingress limit is enforced before parsing; installer model downloads retain Reqwest. Lockfiles and Linux runtime gates must be reviewed. Keyring dependency selection must support cancellable async D-Bus without unlock convenience behavior.

## Requirements, acceptance and ownership

| Requirement / acceptance | Implementation task | Blocking evidence |
|---|---|---|
| REQ-001 / AC-001: normal Model then optional Providers; both vendors coexist; partial results truthful | TASK-003 installer | Fake GUI/CLI skip/save/check/failure/rerun tests; no effect on opening/selecting |
| REQ-002 / AC-002: same-chat LOCAL/CLOUD selection, future-only changes, unavailable history retained | TASK-002 backend; TASK-004 Flutter | Mixed local/cloud context tests; selection during queued/running work; no re-execution |
| REQ-003 / AC-003: keyring-only bounded write/read, generation identity and no secret persistence/output | TASK-002 backend; TASK-003 installer | Fake D-Bus lock/prompt/timeout/ambiguous-item/partial-write and secret-canary tests |
| REQ-004 / AC-004: exact consent and immutable request binding, no first export without consent | TASK-002 backend; TASK-004 Flutter | Changed text/history/model/generation/instance/request/token/expiry negative tests |
| REQ-005 / AC-005: exact official HTTPS, finite limits, no proxy/redirect/retry/fallback | TASK-001 research; TASK-002 backend | Official-contract freeze; injected HTTP/TLS negative tests and exact request counts |
| REQ-006 / AC-006: disk3 migration, bounded history/provenance, restart interruption, exact legacy local behavior | TASK-002 backend; TASK-005 integration | v1/v2 migration/corruption/receipt/u64/capacity tests; Rust-Dart round trips |
| REQ-007 / AC-007: truthful route/credential/check/readiness and unknown remote outcomes | TASK-002 backend; TASK-003 installer; TASK-004 Flutter | Observation has zero cloud calls; unknown-outcome adoption; stale readiness tests |
| REQ-008 / AC-008: source-only scope, canonical policy, stable combined evidence and independent review | TASK-005 main | Diff/source identity, complete gate logs, review/remediation records; no live effects |

### Parallel ownership

- **Backend implementer:** `next/service/**`, service lockfile, backend fixtures. Own all v3 wire behavior, keyring generation/lookup, cloud adapters, migrations and routing/consent. Do not edit installer/client sources.
- **Installer implementer:** `next/installer/**`, installer lockfile, installer fixtures. Own Providers GUI step/rerun management, CLI management, safe secret entry and v3 client adapter. Consume this contract; do not independently define routes or cloud model IDs.
- **Flutter implementer:** `next/client/**`. Own protocol v3 support, grouped selector, per-chat process-local selection, consent and operation recovery/display. Never collect/store API keys or call vendor APIs.
- **Main/integrator:** official API/model freeze, canonical documents, `next/VALIDATION.md`, Dagger/package integration changes if needed, stable combined validation and fresh independent review. Coordinate dependency conflicts and any amendment to this contract before agents diverge.

## Wire contract: common rules (frozen for parallel implementation)

All structures below list **every** key. Unknown and missing keys are errors, including missing nullable keys. Type notation in tables is descriptive, not an alternative JSON encoding.

- `Id`: 32 lowercase hexadecimal characters; service-generated credential generations and approval tokens use fresh entropy, never key hashes.
- `Hash`: 64 lowercase hexadecimal SHA-256 characters. Context hashes are content-derived and must be deleted with conversation content; they are not secret-key fingerprints.
- `Safe`: integer in `0..9007199254740991`. New provider revisions, times, counters and history revisions use this range; reject exhaustion.
- `ConfigRevision`: retain v2's lossless u64 representation: JSON number through `9007199254740991`, otherwise canonical decimal string through `18446744073709551615`. Clients echo without conversion. Disk and v1 retain numeric u64.
- `ProviderId`: exactly `openai` or `anthropic`; local route provider is exactly `local`.
- `Error`: existing exact `{ "code": string, "message": string }`; uppercase code grammar/256-byte cap and nonempty message/1024-byte cap remain. Messages are fixed/sanitized, never remote bodies, keys, headers or sensitive paths.
- Models/labels are nonempty bounded strings (model <=256 UTF-8 bytes, label <=256 bytes, no controls). Cloud model IDs must occur in the compiled reviewed catalog, not merely match a grammar.
- Existing 4-MiB frames, 16-MiB encoded state, 16-KiB user input, 64-KiB retained content/final answer, 32 conversations, 16 retained pairs/conversation, 32 retained jobs and 1024 replay tombstones remain. At most four chat jobs outstanding, one per conversation, one actual worker.

Request envelope:

```json
{"schema_version":3,"service":"smith-next","protocol":"app","call_id":"11111111111111111111111111111111","op":"route_catalog","args":{}}
```

Reply envelope retains the existing exact keys: `schema_version`, `service`, `protocol`, `pid`, `instance_id`, `call_id`, `ok`, `result`, `error`. Echo schema 3/call ID on every recognized success/error path. `ok:true` means result non-null/error null; `ok:false` means result null/error non-null. Schema 3 must never silently fall back to 1/2. Retain managed binary/unit/MainPID and socket inode/SO_PEERCRED checks.

### Shared structures

**Route** (three keys):

```json
{"provider":"local","model":"qwen3-14b:q4_k_m","credential_generation":null}
```

For cloud, `provider` is `ProviderId` and `credential_generation` is an `Id`. Only catalog display entries for unconfigured providers may contain a cloud route with generation null; such a route is never valid for prepare/submit/validate. Local model bytes/digest are resolved and snapshotted service-side from the exact curated record. No endpoint/key supplied by a client.

**Destination** (two keys): `{"provider":ProviderId,"credential_generation":Id}`. Local never requires an approved-destination record. A credential replacement is a new destination even if the operator believes it belongs to the same vendor account; Smith does not claim account introspection.

**Validation** (seven keys):

`{"request_id":Id,"model":string,"credential_generation":Id,"status":"succeeded"|"failed"|"interrupted","completed_ms":Safe,"error":Error|null,"delivery":"not_sent"|"may_have_sent"|"received"}`.

Successful validation has null error and delivery `received`; failed/interrupted has an Error. This is the latest explicitly requested check for the current generation, not a permanent availability/billing guarantee. A new generation resets validation to null.

**ProviderView** (six keys):

`{"provider":ProviderId,"revision":Safe,"credential_generation":Id|null,"credential_state":"not_configured"|"not_observed"|"available"|"locked"|"unavailable"|"missing"|"conflict","credential_error":Error|null,"validation":Validation|null}`.

Provider revision starts at 0 and changes only when its active generation/configuration changes, not on probes/check completion. `not_configured` and `available` have null credential_error; all other states have a fixed Error (`NOT_OBSERVED`, `KEYRING_LOCKED`, etc.). Runtime observation resets to not_observed on service restart for configured providers; a retained successful check is not a fresh keyring observation.

**ModelChoice** (six keys):

`{"group":"local"|"cloud","label":string,"route":Route,"available":boolean,"unavailable_reason":Error|null,"validated_model":boolean}`.

Local availability requires this instance's successful local inventory and exact installed model; validated_model is false (inventory is not inference verification). **Superseded cloud eligibility under the approved [saved-key amendment](LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS.md):** a supported model and matching non-null active credential generation are sufficient for an explicit attempt. Current-instance observation and prior validation success are not prerequisites. Cached observation errors and null/failed/interrupted validation remain advisory facts. `validated_model` is true only for the exact model named by a successful explicit validation of that generation. Other models display not individually verified. available is equivalent to unavailable_reason null and means eligible to attempt, not guaranteed key accessibility, authentication, quota or account access. Actual dispatch freshly retrieves/rechecks the exact admitted credential.

**OperationSummary** remains the six-key v2 shape:

`{"request_id":Id,"conversation_id":Id|null,"kind":string,"status":"queued"|"running"|"succeeded"|"failed"|"interrupted","phase":string,"config_revision":ConfigRevision}`.

**Operation** adds exactly `result` and `error`. Running/queued: both null. Success: typed non-null result, null error. Failure/interruption: null result, Error. Terminal phase is `complete`. New kinds/results are specified below. Old kinds/results retain their validation including inert speech receipts.

## Schema 3 operations

### Existing local operations delegated without new result keys

Schema 3 accepts `state`, `conversation_get`, `poll`, `models`, `configure`, `configure_checked`, `verify_model`, `remove_model`, `submit`, `submit_new`, `conversation_rename`, `conversation_archive`, `conversation_restore`, `conversation_delete` with exactly their current v2 arguments/results. They remain **local-only**, including submit/submit_new; Flutter's new Send uses submit_routed instead. Normalize these delegated operations into the existing v2 fingerprint domain, preserving their replay behavior. New operation names below use a distinct v3 fingerprint domain.

`state` retains exactly `config`, `inventory_observed`, `inventory`, `conversations`, `operations`, `retention`, `revision`. `config` remains the local installer default (`model`, `voice`, `revision`), never a cloud selection. `inventory` remains the local inventory shape. Schema-3 operations lists/poll additionally recognize the new kinds below, with the same summary/poll envelope shape. Conversation summaries/history retain current keys. New routing facts are obtained separately, avoiding a rewrite of every v2 DTO.

Recognized v3 job kinds: `provider_probe`, `provider_save`, `provider_validate`, `submit_routed`, `submit_new_routed`. Both routed chat kinds return the existing exact turn result: `{"assistant_text":string,"audio":null,"speech_error":null}`. Do not invent an audio/tool path.

### `route_catalog` — cached local query

Args: `{}`.

Result (four keys):

`{"config_revision":ConfigRevision,"inventory_observed":boolean,"providers":[ProviderView],"models":[ModelChoice]}`.

Exactly two provider entries, ordered openai then anthropic. Models: observed local models in existing catalog order, then supported OpenAI models, then Anthropic models. Maximum 256 local + 8 per cloud provider; no duplicate provider/model pairs. No keyring access, filesystem model hashing, DNS, API discovery or inference occurs in this query. It uses current cached local inventory, compiled cloud catalog and provider metadata. Calling existing `models` explicitly refreshes native inventory; it never queries cloud.

### `provider_probe` — explicit bounded local keyring observation

Args (four keys):

`{"request_id":Id,"instance_id":Id,"provider":ProviderId,"expected_provider_revision":Safe}`.

Return OperationSummary; kind `provider_probe`, conversation_id null, initial status running/phase `inventory`. This is an exclusive global worker operation, refused with BUSY if any job is pending. It performs only bounded same-user keyring lookup/read, never unlock/prompt/create, DNS or vendor HTTP. Expected provider revision and instance must match before admission and execution.

Success result: `{"provider":ProviderView}`. A completed observation of locked/missing/unavailable is a successful probe operation with an unavailable ProviderView, **not** credential readiness. Internal cancellation/protocol/storage failure is a failed operation. Update runtime cached observation only; do not restore readiness from a retained probe after restart.

### `provider_prepare` — prepare save or validation, no effect

Args (six keys):

`{"request_id":Id,"instance_id":Id,"provider":ProviderId,"action":"save"|"validate","model":string|null,"expected_provider_revision":Safe}`.

For save, model must be null. For validate, model must be an exact supported model for this provider and an active generation must exist. No keyring or HTTP access. Reject if any job is pending, revision/instance is stale, or request ID already belongs to work/tombstones. Preparation is volatile, not a durable job.

Result (nine keys):

`{"request_id":Id,"provider":ProviderId,"action":"save"|"validate","model":string|null,"expected_provider_revision":Safe,"credential_generation":Id|null,"approval_token":Id,"expires_at_ms":Safe,"endpoint":string|null}`.

credential_generation is the currently active generation (possibly null for save), not a proposed replacement. endpoint is null for save and the exact compiled vendor endpoint for validate. Token lifetime is **60 seconds from preparation**, enforced by a monotonic clock. This deadline limits both admission and the start of the keyring write or validation HTTP dispatch; it is not renewed by the UI, polling, or a later apply. The UI displays the exact save/replace effect or synthetic validation effect and potential billing before confirmation.

### `provider_apply` — consume prepared action once

Args (four keys):

`{"request_id":Id,"instance_id":Id,"approval_token":Id,"api_key":string|null}`.

api_key is required only for save, null for validate. Accept 1..4096 bytes of printable ASCII excluding whitespace/control characters; reject rather than trim/normalize. It travels only in the private helper stdin/socket exchange or installer library call, never argv, environment, durable records, event/log Debug output or the model path. Do not Debug-format the enclosing request/command.

Validate original token/action/request/instance/revision/deadline and global idle state, then consume token with durable admission. Return OperationSummary with conversation_id null. Save: kind provider_save, phase `credential`; validate: kind provider_validate, phase `verification`; both initially running. The local config revision at admission supplies the unchanged summary field, while task metadata also binds provider revision/generation.

Save allocates a fresh service generation at admission, keeps the key only in a bounded volatile worker slot, and stores a key-free task. Publish the keyring item before atomically activating its generation/incrementing provider revision/resetting validation. Preserve the old active configuration if activation fails. No old item is automatically deleted or reused. Mark uncertain keyring writes honestly; no automatic repeat. Restart interrupts unfinished save, never reloads a secret from durable task data and never replays the write.

Save success result: `{"provider":ProviderView}`; the activated generation is non-null, credential_state available, validation null.

Validate reads exactly the active immutable generation, sends one fixed synthetic prompt (`Say hello in one short sentence.`), uses no conversation history and does not append a turn. Success result (three keys): `{"provider":ProviderView,"route":Route,"assistant_text":string}`. ProviderView.validation records the completed check. Failed/interrupted checks update the current-generation validation fact without fabricating successful authentication; poll result is null/error non-null. No whole-provider success implies every catalog model was tested.

**Save replay special case:** never compute/persist a hash of api_key, even for deduplication. The request fingerprint uses only schema/op/request/instance/original token, excluding api_key entirely. Once a save ID is admitted, a duplicate with the same public binding returns the original receipt without reading, comparing, validating or storing the supplied key again. Different public binding gives ID_CONFLICT. The original ID cannot be reused to replace a key. Every actual replacement requires a fresh prepare/confirm/request/generation. Clients never automatically resend a secret-bearing apply.

Credential removal/purge is not required by this P0 and is deferred. Replacement and retained-item behavior must be disclosed. The operator can revoke keys through the vendor/keyring outside Smith; Smith must fail closed when its active item disappears or changes. Do not add an undocumented remove/unlock/cleanup operation.

### `conversation_routing` — separate provenance/approval query

Args: `{"conversation_id":Id}`.

Result (five keys):

`{"conversation_id":Id,"history_revision":Safe,"last_route":Route|null,"approved_destinations":[Destination],"turn_routes":[{"request_id":Id|null,"route":Route|null}]}`.

last_route is the last **accepted** routed Send, not a permanent lock or proof of completion. It is null for history without recorded routing. turn_routes has one entry per retained history pair, in the same order; legacy unknown provenance has both fields null. At most 16 approved destinations per conversation; refuse additional first-destination admission with CAPACITY instead of evicting permissions invisibly. No secret bytes or API account names are returned. Missing conversations fail; none is created by this query.

### `route_prepare` — bind one cloud Send and determine transfer consent

Args (seven keys):

`{"request_id":Id,"instance_id":Id,"conversation_id":Id|null,"text":string,"route":Route,"expected_config_revision":ConfigRevision,"expected_history_revision":Safe|null}`.

A null conversation_id requires null expected_history_revision (atomic first-message creation later). Existing conversation requires its exact history revision and must be unarchived with no pending job. Validate the full text, model/catalog, generation, config revision, instance, current route readiness, global capacity, and complete-pair context before preparing. No inference/keyring/network access, no persisted empty chat, no durable job, no destination approval yet.

Result (twelve keys):

`{"request_id":Id,"conversation_id":Id|null,"route":Route,"config_revision":ConfigRevision,"history_revision":Safe|null,"context_sha256":Hash,"context_messages":Safe,"context_bytes":Safe,"requires_consent":boolean,"approval_token":Id|null,"expires_at_ms":Safe|null,"dispatch_expires_at_ms":Safe|null}`.

context_messages counts the bounded alternating context including the proposed user message; context_bytes is total UTF-8 content bytes. Hash compact serialization of the service-owned bounded `Vec<Message>` (typed fields role then content), before transmission; callers treat the digest as opaque and never compute context themselves.

For local route: requires_consent false, token/deadlines null. Local Send may skip prepare and submit directly. For every cloud route: return a token, admission expires_at_ms = preparation + **60 seconds**, dispatch_expires_at_ms = preparation + **600 seconds**. Both are fixed at preparation and enforced monotonically in this instance, never sliding. The existing ten-minute queue-age budget still applies independently. Do not confuse the 60-second confirmation/admission window with the later dispatch deadline.

requires_consent is true if this conversation has not approved this provider/generation, including every first cloud send for a new chat. Model changes do not change Destination. New credential generations always require new consent. The dialog identifies provider/model, existing-context transfer, bounded context counts, account/generation boundary, possible billing and retained approval for later explicit Sends. Confirm only the exact frozen request. Decline/close performs no submit. A New chat option is available; it is never forced.

Destination approval authorizes later **explicit Sends** in that same conversation to the same provider/generation, including a later return after local/other-model use; it is not background authority. Disclose that these later sends can include the then-retained conversation, including intervening turns. Every cloud Send still gets a fresh request-bound preparation/deadline, even when no new dialog is needed. Do not re-prompt solely for a same-destination model change.

### `submit_routed` — immutable admission, optional atomic chat creation

Args are all seven route_prepare fields plus exactly:

`{"approval_token":Id|null,"confirm_transfer":boolean}`.

Local route requires null token and confirm_transfer false. Cloud requires the original token; when its requires_consent was true, confirm_transfer must be true following an explicit UI confirmation. When false, confirm_transfer must be false. Model output cannot manufacture this action; same-UID clients remain within the documented trusted-local boundary, not a cryptographic human-authentication guarantee.

Resolve retained request IDs before current readiness/token-expiry checks; retained same-fingerprint input returns the original summary and never repeats work or creates another conversation. Different request payload/binding is ID_CONFLICT; tombstones remain REQUEST_EXPIRED. The fingerprint includes all non-secret args, including original token and confirmation; no auto-reprepare or fresh-token substitution for an uncertain ID.

For fresh work, revalidate preparation bindings and recomputed context under the admission mutex. Snapshot exact Route, local model digest or cloud endpoint/API-policy identity, credential/provider revision, local config revision, input/context/digest, history revision, output/time budgets and permission deadlines. Persist admission before waking the worker. For null conversation_id allocate title/ID atomically with the job, using current title rules. If a new destination was explicitly confirmed, add its approval in this same admission transaction. This durable permission for later explicit Sends is separate from this request's expiring dispatch authorization: subsequent request failure/expiry does not revoke the already-confirmed conversation permission. Rejected/declined/expired-before-admission preparation adds no approval.

Return OperationSummary: kind `submit_new_routed` for creation, otherwise `submit_routed`; conversation_id is the allocated/existing ID; initial status/phase queued, then running/generation, then a terminal status/complete. Both kinds share the existing turn result shape. Store last_route at admission. Do not mutate global local config, other chats, or an already accepted request when the dropdown changes. Existing v1/v2 config mutations can retain their BUSY rule; routed selection itself has no service configuration mutation and remains changeable while work runs.

Before cloud HTTP dispatch: check ownership, target/history integrity, snapshot policy/catalog identity, generation/provider revision, keyring item lease, queue and permission expiry. Retrieve only the snapshotted generation; never substitute a newer key. Persist delivery `may_have_sent` before making the one request. Failure to persist means **zero HTTP calls**. After that publication, recheck ownership/expiry immediately before invoking HTTP: filesystem synchronization is not a real-time deadline guarantee. If expired, do not call HTTP; publish PERMISSION_EXPIRED and delivery not_sent if the terminal state can safely be committed. An unexpected publication failure fences the service rather than replaying. Expiry prevents this request's traffic, not the separate retained destination permission explicitly granted at admission. A valid HTTP dispatch may finish within its 180-second operation budget even if the authorization's dispatch deadline passes afterward. Apply the same pre-dispatch delivery marker/final deadline check to provider_validate, using its original 60-second preparation deadline.

On a fully validated reply set delivery received and atomically publish the turn/provenance/result using the existing request/conversation/history publication fence. Failures never append partial history. Timeout, connection loss or restart after possible dispatch must not be represented as proof of no remote processing/billing. No retries, fallback or recovery resends.

### `request_routing` — observe accepted routing and delivery

Args: `{"request_id":Id,"instance_id":Id}`.

Result (five keys):

`{"request_id":Id,"route":Route|null,"delivery":"not_applicable"|"not_sent"|"may_have_sent"|"received","context_sha256":Hash|null,"dispatch_expires_at_ms":Safe|null}`.

For routed chat, return its accepted route/context/deadline (local deadline null and delivery not_applicable). For provider_validate, return its route, null context hash and its preparation deadline. For other/legacy operations all optional fields are null and delivery not_applicable. REQUEST_EXPIRED for absent/tombstoned IDs. This query has no effect and does not consult keyring/network. It supplies route identity after reattach without changing existing operation-summary keys.

## Cross-cutting permission, replay and compatibility guards

- At most **32 live preparation tokens total**, across route/provider preparations. Expire opportunistically using a monotonic clock. A repeated prepare with the same request/public args returns the same still-live token/deadlines, never extends them; differing args are ID_CONFLICT. Keep at most **1024 expired unused preparation IDs in memory per instance**, without eviction; an ID in that set returns CONSENT_EXPIRED, and exhaustion refuses further preparations with CAPACITY. This prevents silently renewing an expired preparation after dropping its token. Consumed preparations are instead protected by the durable job/tombstone. An already-admitted ID passed to prepare returns ID_CONFLICT (or REQUEST_EXPIRED if tombstoned); use poll, not prepare, for recovery. Expired preparations require an explicit new attempt with a new request ID. No durable job/tombstone is created by an unused preparation. Restart destroys volatile preparation metadata and interrupts all admitted unfinished jobs; old instance bindings remain invalid. The tokenless local route_prepare query does not allocate preparation metadata.
- No global provider probe/save/validate or local inventory/config/removal operation is admitted while another job is pending. Preparation of an otherwise eligible routed chat may coexist with unrelated queued/running chats, but respects four-job/per-target capacity. A token does not reserve a queue slot; final admission can return BUSY without effects. Provider preparations require idle state too.
- Keyring work is bounded to **3 seconds total per operation**, including discovery/session/lookup/create/read; never wait under the application mutex or in the two-second socket handler. It executes on the sole worker. No keyring autoactivation, unlock method, Prompt invocation or interactive fallback. A library that hides such behavior is unsuitable.
- Preserve ordinary local behavior/DTOs and test seams. v1/v2 cannot name new operations and never infer cloud from a model string or default. Old submits resolve only the legacy local config, and reject UPGRADE_REQUIRED if the target has v3 routing/approval state that the old caller cannot represent. New submit/submit_new delegated under v3 are still local; new Flutter uses submit_routed consistently.
- Do not silently omit v3 work or mislabel it as a local operation in legacy responses. Exact legacy guard: v1/v2 state returns UPGRADE_REQUIRED if any retained v3 job exists or any conversation has non-null last_route/nonempty approved_destinations. v1/v2 poll of a v3 job returns UPGRADE_REQUIRED. v1/v2 conversation_get or target mutation/submit returns UPGRADE_REQUIRED if that target has non-null last_route/nonempty approved_destinations; v1 legacy-target submit applies the same check. Unaffected local scenarios retain exact prior shapes/results. Local models/configure/configure_checked/verify_model/remove_model remain local-only with their existing safety guards and can still be called directly; no new default routes them to cloud. Provider-aware installer flows use an explicit schema-3 configuration/state/poll adapter from the outset, including later local Model management, while retaining the old v1 library method/DTOs for compatibility. Do not retry an effect under another version.
- New schema-3 state/poll consumers accept old operations and the enumerated new kinds. Never use legacy `Operation` deserialization on a v3 summary/poll without explicit shape/revision handling. No automatic version fallback for a mutation or unknown outcome.
- If configuration/state/credential checks fail, keep readable history. No missing-model/key condition may delete, regenerate or silently reroute history. Unavailable historical model identities remain visible, and another explicitly selected route can continue the conversation.

### Error codes clients must recognize

Retain existing INVALID_REQUEST, BUSY, CAPACITY, STALE_REVISION, INSTANCE_MISMATCH, ID_CONFLICT, REQUEST_EXPIRED, STATE_UNAVAILABLE, MODEL_UNAVAILABLE, MODEL_RESULT_LIMIT, MODEL_TRUNCATED, CONTEXT_LIMIT, QUEUE_EXPIRED and INTERRUPTED semantics.

New fixed categories: UPGRADE_REQUIRED; PROVIDER_NOT_CONFIGURED; PROVIDER_NOT_VALIDATED; ROUTE_UNAVAILABLE; CREDENTIAL_CHANGED; KEYRING_UNAVAILABLE; KEYRING_LOCKED; KEYRING_MISSING; KEYRING_CONFLICT; KEYRING_PROMPT_REQUIRED; KEYRING_TIMEOUT; CREDENTIAL_OUTCOME_UNKNOWN; CONSENT_REQUIRED; CONSENT_EXPIRED (not admitted within preparation window); CONSENT_MISMATCH; PERMISSION_EXPIRED (accepted but not dispatched before deadline); CLOUD_AUTH; CLOUD_QUOTA; CLOUD_RATE_LIMIT; CLOUD_UNAVAILABLE; CLOUD_PROTOCOL; REMOTE_OUTCOME_UNKNOWN.

Synchronous errors before admission have no job unless replay resolution finds an existing one. Worker failures are durable terminal errors. Raw HTTP/D-Bus error strings must never become these messages. A known rejected HTTP response can have delivery received; an ambiguous transport result has may_have_sent. Clients show the operation outcome and delivery uncertainty separately. A failed probe/check is not a reason to use another provider.

## Disk3, credential boundary and recovery

**Integrated storage amendment:** the independently approved [message-timestamp extension](LOCAL-SMITH-NEXT-MESSAGE-TIMESTAMPS.md) builds on the cloud disk3 extension with current disk4 and strict disk1/2/3 migration. Application schema remains 3; provider messages and consent hashes remain role/content only. References below to adding disk3 describe the cloud extension point, not permission to downgrade the combined current store. Final combined validation includes disk4 and its downstream installer compatibility tests.

Extend the existing domain/scheduler, not a second state database or general job framework. Storage schema becomes 3; validate v1/v2 with their frozen structs first, then migrate atomically. Preserve old configurations (including unsupported names), histories, receipts, IDs/tombstones, revisions and inert speech metadata. Do not fabricate historical model provenance. A migrated running/queued job interrupts before readiness without dispatch.

Required new non-secret state:

- Exactly two provider configuration slots: provider ID, safe revision, current opaque generation, latest bounded Validation. Actual item secrets, key hashes, D-Bus sessions/paths and approval tokens are not serialized.
- Conversation extension: last accepted route, bounded approved destinations, retained-pair-aligned turn provenance. Existing summary/history wire keys remain unchanged.
- Accepted v3 job extension: immutable route/identity/config/provider/context/limit/permission snapshot and delivery state. Global secret-save task contains provider/revision/new generation and action metadata only; secret bytes live in a dedicated volatile, non-Debug/non-Serialize buffer.
- Preserve full-u64 config compatibility; new counters use safe integers. Replace the v2 assumption that every pending routed job's revision/model must equal current global selection with validation of its immutable snapshot. Ordinary legacy invariants remain where applicable.

Use one new Secret Service item per save generation under narrowly namespaced attributes identifying Smith-next, provider, generation and the non-secret save request ID. Match exactly one item, require it unlocked, and use encrypted Secret Service sessions supported by the chosen implementation/library; do not fall back to a plaintext credential file/environment or a less secure session silently. Only the service resolves/uses item secrets. GUI/CLI briefly holds the newly entered key for submission, then clears its field/buffer; Flutter has no secret entry path. Avoid unbounded copies; best-effort zeroization is not a guarantee against swap/core dumps or hostile same-UID processes.

Choose and document the supported Ubuntu Secret Service implementation during TASK-001 (prepared-session prerequisite, not something Smith installs/starts/unlocks). Require a same-user local Unix session bus and no arbitrary TCP D-Bus address. Metadata/ownership checks and immutable names mitigate mistakes, not a sandbox against malicious same-UID keyring edits. Bind a volatile secret lease to the exact generation during worker use and refuse observable item identity/attribute drift; never reread a replacement mid-request.

Credential publication is not a transaction spanning keyring and JSON. Write immutable item first, then activate reference atomically. If keyring creation may have happened but activation did not, keep the old active reference, preserve the original key-free job/generation, report CREDENTIAL_OUTCOME_UNKNOWN, and leave the possible orphan for explicit operator recovery. No background reconciliation that activates a key, auto-retry, auto-delete or rollback. Each explicit confirmed save creates at most one bounded item; old items may accumulate and that preservation is disclosed rather than presented as cleanup. A service restart must not activate an orphan from an interrupted save.

Preparation tokens and keyring-availability observations are process-local. Persisted wall-clock expiry fields are evidence/display only; monotonic deadlines govern live preparation/dispatch, and no admitted job is replayed after restart. Cloud jobs that may have dispatched retain that delivery state and an explicit unknown-outcome error when interrupted. Queue-only jobs retain not_sent. Owner teardown must cancel bounded cloud/keyring waits, fence publication and join cleanup; closing a client still only detaches.

Preflight encoded bytes for the new context snapshots, destination/provenance arrays, errors, verification output and completion transitions within existing state/frame limits. Reserve transition/revision headroom for pre-dispatch delivery publication as well as terminal publication; accepted work cannot discover capacity exhaustion after spending remotely. Minimum future state-transition reservations: queued local chat 2, running local chat 1; queued cloud chat 3, running cloud chat before delivery publication 2, after it 1; running provider_validate before delivery publication 2, after it 1; running provider_probe/provider_save 1. If implementation adds another durable phase, increase the reservation before admission. Initial destination approval is part of admission, not an unreserved later transition. Logical deletion removes routing/provenance/approvals/context hashes with history and matching jobs; retain only existing opaque replay protection/content-free delete receipts. No secure media/keyring/vendor/backups erasure claim.

Rollback requires a separately approved pre-migration state snapshot plus compatible paired binaries. Old binaries cannot read disk3; binary-only downgrade is invalid. Keyring items are outside the JSON backup: restore must not silently reconnect an old reference to a different generation. No live backup, migration or restore is authorized by this source plan.

## External API contract freeze and network policy

**TASK-001 completed as public-documentation research, not live API validation:** [the API contract](LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md) records official documentation URLs/date, exact model IDs, endpoint/header/body/response rules, bounded output and synthetic fixtures. Initial entries are `gpt-6-astra`, `claude-sonnet-5-5` and `claude-haiku-5-5`. UI agents use the service-returned catalog, not illustrative names. Account access and real API behavior remain separately unverified.

Proposed fixed endpoints to confirm in that freeze: OpenAI `https://api.openai.com/v1/responses`; Anthropic `https://api.anthropic.com/v1/messages`. Only those exact HTTPS hosts/443/paths are production capabilities. No configurable base URL, redirect, proxy/environment discovery, custom trust bypass, cookie jar, browser login, arbitrary organization/project headers or model-list API. Credential destination is the selected provider/generation; provider storage/retention/account billing remains an external policy, not a Smith guarantee.

Use a single-connection Hyper HTTP/1 transport with Rustls/Tokio and normal certificate/hostname validation. The read-time 16-KiB raw-header allowance includes status lines, whitespace and informational responses; it replaces the initial Reqwest implementation's insufficient post-parse check. There is no pool or high-level retry layer, proxy discovery, redirect handling or decompression; reject 3xx. Keep auth headers out of logs and attach them only to the fixed vendor endpoint. No tool, search, file, image/audio, streaming, background execution, remote conversation chaining, provider fallback or model substitution. The existing native Vulkan-startup-to-CPU exception remains unchanged and never becomes cloud fallback.

Budgets: **180 seconds end-to-end from worker dispatch**, including at most 3 seconds for credential access; **10-second maximum connection phase**, clipped to the original deadline; **16-KiB response-header budget**, **1-MiB response-body budget** and **64-KiB final text**. Disable automatic decompression or enforce the same post-decompression cap. Bound reads incrementally, including error bodies and slow drip. Cap cloud output at **8192 total generation tokens**, including reasoning, under the reviewed API contract; the earlier provisional 1024-token cloud cap is superseded because current models include reasoning in that budget. The native local runner's 1024-token cap is unchanged. No automatic budget increase or truncated-answer retry is permitted. Request context uses deterministic request-only trimming of oldest complete pairs, reviewed token accounting/conservative byte accounting and finite template overhead. No remote token-count request or context-error retry.

Use a worker-owned async runtime/cancellation-aware client; construct futures inside that runtime. Production has no test endpoint/environment override. Isolated library injection supplies fake keyring and transport implementations; protocol/TLS fixtures may use private test-only endpoints without authorizing production endpoints. Fixed application endpoints are not an OS network sandbox. `store:false` or an equivalent supported non-storage request option, where verified, is not a vendor zero-retention claim.

## Installer contract and scope

- Preserve artifact/lifecycle/local Model actions, default local selection, download/verification/removal consent and explicit successful-result Launch behavior. Providers is an additional optional step **after Model**, not a mandatory alternative or a reason to provision local dependencies differently.
- Show two independently configurable entries: OpenAI and Anthropic. Opening the step, typing, selection, catalogue reads, Skip and Close perform no credential write or HTTP. An explicit local probe is allowed without API consent and must be labelled local keyring observation.
- Save/Replace freezes provider/input, prepares save once, displays the exact keyring/replace/preservation effect, then applies the original token with the secret once. Validate prepares/checks one exact supported model separately and discloses one synthetic billed request. No save-and-test implication or bulk consent. Show Saved / Not validated / Validated model / Locked / Unavailable / Failed / Unknown accurately.
- GTK owns presentation only; extend the existing joined worker, typed commands/events and bounded critical channel. Secret-bearing command variants must use a non-Debug or explicitly redacting secret wrapper; no derived/logged representation may expose bytes or appear in Report/details. Never spawn a provider CLI or run network/D-Bus on the GTK thread. Cancellation stops observation/owned installer work, not already accepted Smith work; retain request IDs and never resend.
- Provide a rerun **Manage providers** action from appropriate setup/result paths, not a redesign of the two-action landing. It requires the same verified installed service as model management and does no artifact/lifecycle work.
- Add exactly one narrow CLI mode: `smith-installer --providers [--bundle ROOT]`, an interactive provider-management session against the verified installed service. Reject combinations with artifact mode, --yes, local model/download/verify flags and --dry-run. It performs no installation or lifecycle changes. After read-only service/catalog observation, offer explicit provider selection, local probe, Save/Replace, Validate and Done; all effects use the same wire approvals. CLI full-recipient output points to this optional mode after the existing Model work; do not add unsolicited prompts to existing piped recipient scripts or make cloud a requirement for their success.
- CLI secret entry requires an interactive terminal with echo disabled and restored on every exit; bounded 4096-byte/60-second input, cancellation/EOF fail closed. Never accept keys in flags, environment, generic echoing confirm callbacks or unbounded stdin. Library tests inject a synthetic secret-input source. The CLI stays GTK/display-free but cloud needs an accessible unlocked Secret Service session; no attempt to unlock or start one.
- Preserve distinct local installation success and optional provider outcomes. A failed chosen validation is reported as failed/incomplete provider setup, with already completed artifacts/model/key save preserved. Skip is not validation. A lost result is uncertain, never proof that the key/API effect did not occur.

## Flutter contract and scope

- Use schema 3 in production without silent v2 fallback; preserve pure v1/v2 validation helpers/fixtures for compatibility tests. Extend accepted new operation kinds and use the separate routing reads instead of relaxing strict DTO checks.
- Add the grouped dropdown to each chat composer. Options come from route_catalog. Selection is process-local per chat/new draft and can change while accepted work is queued/running; it is not an immediate service mutation. On reattach, last_route is a hint only when no unsent local selection exists; new chats initialize from the saved local default. Never automatically substitute another model if that hint is unavailable. Preserve text/history and allow explicit selection of a different available model.
- On explicit Retry, serialize state/detail/poll, local inventory refresh and explicit local provider probes for configured providers as needed. No vendor validation, save or chat resend. Boot/automatic 30-second observation remains state/detail/poll/catalog reads only; it does not probe keyring or infer. Cloud Send with a saved active generation must not require Retry/local probing, successful validation or prior inference after restart. Probes remain optional local observations; the explicit Send/Validate operation freshly retrieves its exact saved credential. Completed adverse observations/checks are advisory, but unresolved probe/operation reservations and unknown-admission identity protections remain binding.
- Capture route plus text/draft/config/history/instance identities at the Send gesture. Prepare cloud Send using that request ID. If transfer consent is required, show the exact frozen destination/model/context disclosure and explicit Send/Cancel/New chat choices. Do not show a dialog for an already approved destination model switch. Selection/text changes before confirmed admission invalidate a pending preparation rather than quietly replacing its fields; a fresh explicit Send prepares new work. Once submission may have been accepted, preserve its original identity and only observe it.
- Keep the existing one-outstanding-per-chat/four-global admission, immediate local receipt, per-chat draft/uncertainty/scroll handling and no-replay recovery. A later dropdown change never changes the route label of an existing pending/terminal receipt. Use request_routing to recover accepted route/delivery after reattach. Display historical routes from turn_routes when known; label old unknown provenance honestly.
- No permanent chat model lock, no forced new conversation for a route change, no auto-fork, no Branch/Compare prerequisite, and no credential entry/store in Flutter.

## Prioritized tasks and milestones

| Task | Priority / dependencies | Scope and definition of done | Checks |
|---|---|---|---|
| TASK-001 | P0, main | Freeze official API/model contracts and choose supported Secret Service implementation/library; update canonical target scope and host prerequisites. Resolve only technical compatibility gaps, not another product-Q&A round. Maps REQ-003/005/008, AC-003/005/008. | Official-source record, dependency/license/MSRV review, exact per-vendor fixtures; no live credentials/inference. |
| TASK-002 | P0, backend; adapter portion after TASK-001 | Implement disk3/v3, immutable routing, bounded tokens, secret boundary, provider probe/save/validate, two adapters, provenance/legacy guards and fake tests. Preserve local runner and unit. Maps REQ-002..007 and matching ACs. | Focused migration/admission/keyring/HTTP tests, then `make service/test` from next/. |
| TASK-003 | P1, installer; parallel against this wire contract | Optional Providers step and rerun/CLI mode, v3 protocol adapter, safe input and exact separate confirmations; leave existing local install/model path intact. Maps REQ-001/003/007 and matching ACs. | Fake backend/wizard/CLI tests, isolated GTK tests, `make installer/test`. |
| TASK-004 | P1, Flutter; parallel against this wire contract | Per-chat selector, v3 DTOs, prepared first-transfer Send, future-only selection and route-aware recovery; no API/key access. Maps REQ-002/004/007 and matching ACs. | Strict DTO/controller/widget/keyboard/uncertainty tests, `make client/test` including real Rust-Dart fixtures. |
| TASK-005 | P0 integration/review gate after components, main | Update all affected canonical docs and validation ledger; reconcile lockfiles/package changes, retain stable combined source/evidence and obtain fresh assurance/adversarial review. Maps REQ-001..008 and all ACs. | Component gates, `make test build`, package verification, repository whitespace checks and independent review. |

Milestones are local diff/evidence checkpoints, **not PRs or commits**: (1) shared contract/fake fixtures, (2) backend source + component tests, (3) parallel installer/client integration, (4) stable combined self-check, (5) independent review/remediation. No publication/deployment milestone is authorized.

## Validation Contract

These are required future checks, **not checks claimed run when this plan was authored**.

1. Exact v1/v2 local envelopes/receipts and v3 delegated/new DTOs; unknown keys, nullable omissions, full-u64 echoes, wrong instance/call IDs and malformed-frame correlation. Real compiled Rust-Dart fixture must exercise routed local admission, fake cloud/consent behavior, migration and reattach.
2. Same chat local A -> local B -> OpenAI -> Anthropic -> local; exact isolated context and mixed-route provenance. Approved-destination model switches have no redundant ceremony; new generation needs consent. Old unavailable models do not lose history. Selector changes during queued/running work affect only later submissions.
3. First-transfer reject/decline, changed text/context/history/model/provider/generation/config/instance/request, wrong confirmation, admission expiry, dispatch expiry and clock adjustment. Prepares create no chat/job/cloud call. Token/capacity bounds, same-prepare stable deadline, duplicate apply/submit and tombstone behavior.
4. Fake Secret Service locked/unavailable/missing/ambiguous/prompt-required/timeouts; no autoactivation/unlock/prompt/fallback. Secret canaries must be absent from disk3, fingerprints, all stdout/stderr/Debug/events/replies/artifacts and inherited native-runner environment. Immutable generations, missing/drifted items, uncertain save and restart orphan behavior are mandatory.
5. Zero HTTP on startup/state/catalog/probe/save/selection/Retry/migration/replay. Validation sends exactly one synthetic request and no chat. Fixed endpoints/headers, valid TLS policy, blocked redirects/proxies, disabled retries, bounded oversized/slow responses, malformed/tool-bearing/truncated/refusal cases and no cross-provider/local fallback.
6. Crash/loss before dispatch, after may_have_sent publication, after remote reply before local commit, and after commit before acknowledgement: at most one attempt; truthful unknown outcome; no auto-resend. Cancellation/owner teardown must fence/join without touching independently managed service lifetime.
7. Four-chat FIFO, per-target busy checks, provider-management BUSY, expired permission before HTTP, immutable request snapshots, late-publication fences, byte/revision completion reservation with extra dispatch transition, terminal eviction and delete residue. Unsupported/corrupt disk bytes are preserved.
8. Installer optional step/skip/partial failure and both providers, no plaintext echo, terminal restoration, CLI no-GTK/no-display prerequisites, no host manager/keyring/API use in tests. Flutter narrow/keyboard/IME/accessibility/dialog/poll/selection-staleness tests retain current behavior.

Run focused checks first, then from `next/`: `make service/test installer/test client/test`, followed by stable combined `make test build`; include `make package/test`/package verification if dependency/payload changes affect those gates. Service scripts require formatting, locked warning-denied Clippy/tests and locked build; client fixture gates must not be silently skipped. Main owns Dagger/package adjustments if the new Rust dependency closure changes native runtime inputs. These build workflows may need approved toolchain/package network access, but never vendor inference, real keyring, model weights or live service access.

Retain base/worktree/source snapshots, exact commands/environment/result/timestamps, synthetic fixtures, sanitized logs, candidate identities and independent review/remediation in the task evidence and `next/VALIDATION.md`. Do not claim a stable aggregate PASS across concurrent agent edits. Existing unrelated failures are reported and remain release blockers, not weakened or fixed outside scope. Actual unlocked/locked user-session integration, exact-model provider calls/billing and installed migration are separate explicitly approved later gates, not synthetic-test claims.

## Stage Gates

| Transition | Required input / completion | Blocking conditions | Output |
|---|---|---|---|
| plan -> develop | This bounded human source approval; fixed shared contract; TASK-001 before adapter coding | Scope/authority conflict, unresolved model/API/security compatibility | Scoped task ownership, researched contract, fixtures |
| develop -> self_check | Implemented diff on known base and component tests | Missing functionality or unauthorized effects | Source-bound component logs and developer checklist |
| self_check -> assurance | Stable combined diff, complete required gate evidence and rollback compatibility | Failing/missing checks, secret exposure, unexplained replay/consent/migration gap | Fresh independent architecture/adversarial review input package |
| assurance -> human source signoff | Findings closed/retested, residual risks explicit | Unresolved critical finding or evidence mismatch | Review verdict and remediation evidence; no agent approval |
| source signoff -> release/deploy/live QA | Separately bound human approval, exact artifact/environment/effects | No matching approval, no pre-migration restore evidence, missing live prerequisites | Only separately authorized effect/evidence; outside this task |

## Review Checklist and minimum release safety

- Every REQ/AC maps to a task and reproducible evidence; normal local installation remains intact.
- Same-chat switching and future-only accepted identity are tested, not replaced by a permanent route lock.
- All wire fields/job kinds/phase/result shapes match this shared contract across three implementations.
- No credentials/key-derived hashes reach durable state, logs, environment or model context; keyring failure is fail-closed.
- No hidden egress/retry/provider fallback, expired authorization dispatch, version fallback mutation or invented readiness.
- Disk3/legacy/rollback/capacity/delete semantics are validated and documented.
- No release with failing builds/tests, compiler/runtime errors, unresolved critical warnings or missing required evidence.
- Explicit deferrals: OpenCode/agent runtimes, Branch/Compare, credential purge and live QA are main-agent-owned follow-ups, due **before any future scope that needs them**, not implied complete. No deferral waives a blocking P0 invariant.

## Implementation Handoff / Agent Instructions

1. Work only in the assigned subtree; preserve other agents' changes. Do not commit, create branches or touch installed state.
2. Main freezes official API/model/keyring dependency choices before adapter implementation; UI agents consume the runtime catalog and can implement against synthetic fixtures immediately.
3. Backend establishes v3 envelopes/types/migration and fake keyring/cloud seams first; preserve existing local Provider tests and Core/unit contracts.
4. Installer and Flutter implement the exact operation names/keys/results above. Add explicit v3 adapters rather than relaxing v1/v2 validation or inventing fallback behavior.
5. Bind and persist each accepted route/context/permission; keep secret bytes volatile and all live effects on the bounded sole worker. Make every error/unknown outcome observable without retry.
6. Run isolated focused/self-check gates, then let main freeze the combined snapshot and run aggregate gates. Report exact failures and partial outcomes.
7. Obtain fresh independent review/remediation, update canonical docs/evidence, and stop before any real credential, paid API, installed migration, commit or deployment effect without separately bound human approval.
