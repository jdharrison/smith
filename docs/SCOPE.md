# Smith scope

Smith is a local-first application for **one trusted operator on one Ubuntu workstation**. It is a development candidate, not a production-release guarantee.

This is the short overview. [Detailed scope and requirements](SCOPE_DETAILS.md) remain authoritative; this summary does not replace or relax them.

## What exists

**Implemented in source:** a Flutter desktop, persistent Rust service, and native CLI/GTK installer.

- Saved, isolated text conversations with rename, archive, and confirmed logical deletion.
- Curated local models through bundled CPU/Vulkan runtimes; optional OpenAI and Anthropic APIs.
- Per-chat LOCAL/CLOUD selection, with explicit context-sharing consent for each new provider/credential generation. No automatic cloud fallback.
- Up to four desktop panes and separate session-only windows. Drafts and waiting follow-ups are not durable.
- Projects configuration: service-owned human ID/key, optional description and exact context prompt, ordered repositories with read-only classification and retained branch/type settings. The [Projects-only editor is implemented in source](DESIGN_DETAILS.md#projects-summary-and-settings); parent-owned recovery/layout closeout, validation and fresh assurance remain pending, not graphical acceptance. Agents and the shell are outside this slice. Read-only is metadata, not security enforcement; no repository access, context injection or Dev execution follows. Only fresh profiles receive the ordinary editable/deletable Smith starter. See [detailed contract](SCOPE_DETAILS.md#current-projects-metadata-foundation).

- **Agents implemented source, acceptance pending:** 2–32 saved profiles under Dev → Agents, with permanent `developer`/`assurance` IDs and immutable current saved names/descriptions, editable prompts, and custom creation/editing. Profiles remain metadata only, not execution or independent review. [Bounds and exclusions](SCOPE_DETAILS.md#current-agents-definitions); [approved plan](plans/LOCAL-SMITH-AGENTS-CUSTOM.md).

**Historical preview source:** Dev's earlier saved-project/saved-agent/prompt form provided service-owned receipt-only container initialization. It records a prompt in pinned OpenCode; it does not run inference, code, clone repositories or provide a board. Optional rootless Docker and a separately prepared worker image are required only for this slice. [Bounds, evidence and exclusions](SCOPE_DETAILS.md#dev-initialization-review-slice).

The service owns accepted work independently of desktop lifetime. Closing the desktop detaches; restart interrupts unfinished work without replay.

## Supported environment

Prepared **Ubuntu 24.04 LTS x86_64**, a normal non-root account, native desktop libraries, safe user-owned directories, and a usable systemd user manager. No external Ollama or recipient developer toolchain is required.

Smith is private/loopback-first—not a public, remote, or multi-tenant service. Additional platforms require an explicit scope amendment. See [host requirements](HOST_PROFILE.md) and the [installation guide](../dagger/INSTALLER.md).

## Limits and safety

The service permits four outstanding chat requests, one per conversation and one executing globally. Histories are bounded:32 saved chats,16 turn pairs/64KiB per chat. Local inference uses separately downloaded weights; GPU fit and performance are not guaranteed.

Installation, registration, Enable, Start, model download, selection, checks, and destructive actions retain their separate confirmations. Uninstall preserves user data and keyring items; it does not revoke vendor credentials. API billing is separate from consumer subscriptions.

Model output and external text are untrusted data—not permissions or approval. No automatic merge, release, deployment, privilege expansion, or accepted-risk decision is authorized. Live-host effects require their own explicit approval under [Process](PROCESS.md#approval-boundaries).

## What comes later

**Dev local-runner source implemented; functional acceptance pending:** a [one-shot project/task runner](plans/LOCAL-SMITH-LOCAL-OPENCODE-RUNNER.md) uses fixed OpenCode, public HTTPS checkouts, bounded read-only inspection and Smith's selected installed local model; no Agent/cloud dependency. The [approved rootful follow-up](plans/LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) uses operator-prepared local Docker with explicit root-equivalent host-service authority, not installer provisioning or new Dagger integration. The [current Dev list/modal source](plans/LOCAL-SMITH-DEV-JOBS-LIST.md) shows retained local-runner jobs/statuses, New job and clicked-job details; it adds no Stop/reconciliation; the current service scheduler separately admits three container slots with FIFO queuing while serializing native inference. Exact image/model/tool interoperability and live/installed acceptance remain unverified; source changes grant no live clone/inference authority.

**Retained read/write foundation, source only:** separate session/receipt operations are implemented, but normal service construction and Python creation remain unavailable/policy-blocked. This is not an enabled Dev capability. [Contracts and pending gates](ARCHITECTURE_DETAILS.md#retained-readwrite-job-foundation--source-only).

**Target/deferred:** autonomous software delivery, general tools/coding workers, role orchestration, and general scheduling. Current Projects metadata and Agents definitions do not implement these capabilities.

Voice, model discovery, arbitrary endpoints, credential purge, distributed workers, and pre-login/headless operation are outside current scope.

Read [Architecture](ARCHITECTURE.md) for design, [Validation](../VALIDATION.md) for evidence, and [History](HISTORY.md) for retired source. Passing source checks, installation acceptance, release, and deployment remain separate gates.
