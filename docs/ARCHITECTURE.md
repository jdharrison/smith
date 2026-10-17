# Smith Architecture

This is a short orientation. [Architecture details](ARCHITECTURE_DETAILS.md) remain authoritative for detailed requirements and implementation limits; this page does not override them. [Scope](SCOPE.md), [Process](PROCESS.md) and [Design](DESIGN.md) govern supported capabilities, approvals and operator experience.

## How Smith fits together

**Implemented source:** Flutter `client/` talks through a bundled helper and private Unix bridges to the persistent Rust `service/`. The independent native `installer/` manages the paired product; `dagger/` is builder tooling. Smith serves one trusted, non-root Ubuntu operator, not remote tenants.

The service owns accepted work, history and routing. Closing the desktop detaches; restarting interrupts unfinished work without replay. Opening or reconnecting never automatically installs, enables, starts, downloads, infers or resends. Lifecycle effects require separate confirmation. Model/external text grants no permissions; private processes are not a hostile-same-user sandbox.

**Target/deferred:** autonomous delivery, tools, roles and workers. Chat does not supply them. Source checks are not installed acceptance. See [system and trust boundaries](ARCHITECTURE_DETAILS.md#system-model).

## Current next cloud routing and credential boundary

**Implemented source:** explicit LOCAL/CLOUD sends freeze their route and context. A new provider/credential generation requires exact context-transfer consent; retained consent covers later explicit sends there, including intervening history. Credentials enter only through private installer input and an existing unlocked same-user GNOME Keyring—not Flutter, environment or plaintext fallback.

Only fixed official OpenAI/Anthropic endpoints are used; no automatic retry or provider/model fallback. API billing is separate from consumer subscriptions. Catalog entries guarantee neither entitlement nor availability, cost or retention. Save and potentially billable Validate are separate effects. [Full routing, credential and recovery contract](ARCHITECTURE_DETAILS.md#current-next-cloud-routing-and-credential-boundary).

## Current next multi-chat storage and scheduling

**Implemented source:** at most 32 chats, 16 pairs/64 KiB each; four outstanding requests, one per chat and one executing globally. Queue age is bounded to ten minutes; cooperative inference budget is 180 seconds. Local context/output limits are 4096/1024 tokens; cloud generation allows 8192 total tokens including reasoning. Cleaned startup-only Vulkan-to-CPU fallback is not request replay. App3/Core2/disk4 are current; older binaries cannot read disk4. [Storage, migration and scheduler details](ARCHITECTURE_DETAILS.md#current-next-multi-chat-storage-and-scheduling).

## Saved message timestamps

**Implemented source:** saved times represent service admission and final validated reply publication, not model-only latency. Missing historical dates stay unknown. [Timestamp semantics](ARCHITECTURE_DETAILS.md#saved-message-timestamps).

## Client-side sequential message queue

**Implemented source:** volatile follow-ups wait for predecessor success and synchronized saved history. Failed heads block; unknown outcomes remain observation-only. Limits are eight follow-ups/chat and 32/desktop, plus one current-predecessor allowance/chat. Close warns before discarding unsent text; forced termination can lose it. This is not a durable outbox or cancellation. [Queue contract](ARCHITECTURE_DETAILS.md#client-side-sequential-message-queue).
