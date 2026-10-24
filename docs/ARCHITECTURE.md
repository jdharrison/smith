# Smith Architecture

This is a short orientation. [Architecture details](ARCHITECTURE_DETAILS.md) remain authoritative for detailed requirements and implementation limits; this page does not override them. [Scope](SCOPE.md), [Process](PROCESS.md) and [Design](DESIGN.md) govern supported capabilities, approvals and operator experience.

## How Smith fits together

**Implemented source:** Flutter `client/` talks through a bundled helper and private Unix bridges to the persistent Rust `service/`. The independent native `installer/` manages the paired product; `dagger/` is builder tooling, including [early export-location checks](ARCHITECTURE_DETAILS.md#next-generation-artifact-installation). Smith serves one trusted, non-root Ubuntu operator, not remote tenants.

The service owns accepted work, history and routing. Closing the desktop detaches; restarting interrupts unfinished work without replay. Opening or reconnecting never automatically installs, enables, starts, downloads, infers or resends. Lifecycle effects require separate confirmation. Model/external text grants no permissions; private processes are not a hostile-same-user sandbox.

**Target/deferred:** autonomous delivery, tools, roles and workers. Chat does not supply them. Source checks are not installed acceptance. See [system and trust boundaries](ARCHITECTURE_DETAILS.md#system-model).

## Current next cloud routing and credential boundary

**Implemented source:** explicit LOCAL/CLOUD sends freeze their route and context. A new provider/credential generation requires exact context-transfer consent; retained consent covers later explicit sends there, including intervening history. Credentials enter only through private installer input and an existing unlocked same-user GNOME Keyring—not Flutter, environment or plaintext fallback.

Only fixed official OpenAI/Anthropic endpoints are used; no automatic retry or provider/model fallback. API billing is separate from consumer subscriptions. Catalog entries guarantee neither entitlement nor availability, cost or retention. Save and potentially billable Validate are separate effects. [Full routing, credential and recovery contract](ARCHITECTURE_DETAILS.md#current-next-cloud-routing-and-credential-boundary).

## Current next multi-chat storage and scheduling

**Implemented source:** at most 32 chats, 16 pairs/64 KiB each; four outstanding requests, one per chat and one executing globally. Queue age is bounded to ten minutes; cooperative inference budget is 180 seconds. Local context/output limits are 4096/1024 tokens; cloud generation allows 8192 total tokens including reasoning. Cleaned startup-only Vulkan-to-CPU fallback is not request replay. App3/Core2 remain unchanged; the implemented Agents extension uses disk10 (not installed acceptance), retaining strict disk9 and earlier migration contracts. Older binaries cannot read the new state; disk7 global instructions remain preserved. [Storage, migration and scheduler details](ARCHITECTURE_DETAILS.md#current-next-multi-chat-storage-and-scheduling).

## Projects metadata registry

**Implemented service/domain/editor source, validation and assurance pending:** disk9 adds a unique lowercase kebab key, optional description/exact project-context prompt and repository read-only metadata while retaining opaque identity, hidden Name and fixed harness compatibility values. Four App3 Projects `*_v3` operations replace unversioned/v2 list/writes, which fail `UPGRADE_REQUIRED`; receipt lookup and App3/Core2 stay unchanged. Strict legacy conversion preserves existing registry/replay/global/Agents data; uncertain writes remain query-only. The editor's hidden-Name acknowledgement recovery remains a parent-owned closeout item in the [plan](plans/LOCAL-SMITH-PROJECTS-UX.md#current-sourceevidence-disposition); source presence is not graphical acceptance. No context injection, repository access, read-only security enforcement or Dev execution follows. [Exact schema, migration and compatibility](ARCHITECTURE_DETAILS.md#projects-metadata-registry).

**Implemented fresh-profile initialization:** a safely absent application-state file receives one ordinary editable/deletable Smith metadata project with key `smith`, empty description/prompt and read-only repository metadata. Explicit bootstrap—not shared empty defaults—validates full disk9 state and publishes0600 without replacement before readiness, with ordinary generated IDs and initial application/registry revision1/internal receipt. Existing profiles are never backfilled; retained deletion stays deleted. No filesystem root or execution capability is added. [Exact initialization contract](ARCHITECTURE_DETAILS.md#projects-metadata-registry).

## Agents definitions registry

**Implemented source, acceptance pending:** 2–32 profiles, permanent built-in identities/current saved metadata and editable prompts, plus service-generated custom IDs. Disk10 preserves strict legacy state; App3/Core2 retain versioned list/create/update and unchanged receipt lookup. Independent safe-integer revisions, shared replay protection and original-receipt query-only recovery remain; no execution or automatic Dev start follows. Dev ledger changes are canonical-ID reopen compatibility only. [Schema, recovery and rollback](ARCHITECTURE_DETAILS.md#agents-definitions-registry); [approved plan](plans/LOCAL-SMITH-AGENTS-CUSTOM.md).

## Dev initialization review slice

**Implemented preview source:** additive App3 job operations and a separate private bounded ledger supervise one optional rootless-Docker/OpenCode receipt-only sandbox. Saved context is frozen, readiness gates one dispatch attempt, and correlated message read-back plus stopped-runtime verification—not model output—establishes `recorded`. No mounts, published ports, worker network, provider calls or repository work is authorized. Restart/ambiguous ownership blocks new execution; no replay or automatic reconciliation. Prerequisite failures expose only fixed typed diagnostics through the existing message, never raw errors or inferred absence; query behavior and historical results stay unchanged. [Contract and unverified acceptance limits](ARCHITECTURE_DETAILS.md#dev-initialization-review-slice).

**Implemented local-runner source, live acceptance pending:** the [local-only one-shot runner](plans/LOCAL-SMITH-LOCAL-OPENCODE-RUNNER.md) uses fixed OpenCode, a tool-aware private bridge, shared Smith-local inference ownership and constrained checkout/read guards, without Agent/cloud dependencies. Its [approved endpoint follow-up](plans/LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) uses only local rootful Docker at `/run/docker.sock`, preserving the separate receipt runner's rootless policy. This explicitly accepts root-equivalent authority for the non-root host service, not for the worker; no socket mount, daemon provisioning or ambient/failure fallback follows. The [client jobs-list/modal source](plans/LOCAL-SMITH-DEV-JOBS-LIST.md) keeps browsing separate from authoritative admission and bounded observation, with request-keyed details and no Stop/reconciliation or backend schema change. Current scheduler source uses App3 v4 local-job operations and ledger schema4: three container slots, FIFO admission and one shared native inference owner. Restart never replays unfinished work. Source/fixture evidence is not real Git/OpenCode/model or installed acceptance.

**Retained read/write foundation, source only:** separate session operations and a private ledger share the three container slots and retain queryable receipts. Normal service construction and Python creation remain unavailable/policy-blocked; prepared wiring is not an enabled capability. [Retained-session contracts and remaining gates](ARCHITECTURE_DETAILS.md#retained-readwrite-job-foundation--source-only).

## Saved message timestamps

**Implemented source:** saved times represent service admission and final validated reply publication, not model-only latency. Missing historical dates stay unknown. [Timestamp semantics](ARCHITECTURE_DETAILS.md#saved-message-timestamps).

## Client-side sequential message queue

**Implemented source:** volatile follow-ups wait for predecessor success and synchronized saved history. Failed heads block; unknown outcomes remain observation-only. Limits are eight follow-ups/chat and 32/desktop, plus one current-predecessor allowance/chat. Close warns before discarding unsent text; forced termination can lose it. This is not a durable outbox or cancellation. [Queue contract](ARCHITECTURE_DETAILS.md#client-side-sequential-message-queue).
