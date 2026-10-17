# Smith scope

Smith is a local-first application for **one trusted operator on one Ubuntu workstation**. It is a development candidate, not a production-release guarantee.

This is the short overview. [Detailed scope and requirements](SCOPE_DETAILS.md) remain authoritative; this summary does not replace or relax them.

## What exists

**Implemented in source:** a Flutter desktop, persistent Rust service, and native CLI/GTK installer.

- Saved, isolated text conversations with rename, archive, and confirmed logical deletion.
- Curated local models through bundled CPU/Vulkan runtimes; optional OpenAI and Anthropic APIs.
- Per-chat LOCAL/CLOUD selection, with explicit context-sharing consent for each new provider/credential generation. No automatic cloud fallback.
- Up to four desktop panes and separate session-only windows. Drafts and waiting follow-ups are not durable.

The service owns accepted work independently of desktop lifetime. Closing the desktop detaches; restart interrupts unfinished work without replay.

## Supported environment

Prepared **Ubuntu 24.04 LTS x86_64**, a normal non-root account, native desktop libraries, safe user-owned directories, and a usable systemd user manager. No external Ollama or recipient developer toolchain is required.

Smith is private/loopback-first—not a public, remote, or multi-tenant service. Additional platforms require an explicit scope amendment. See [host requirements](HOST_PROFILE.md) and the [installation guide](../dagger/INSTALLER.md).

## Limits and safety

The service permits four outstanding chat requests, one per conversation and one executing globally. Histories are bounded:32 saved chats,16 turn pairs/64KiB per chat. Local inference uses separately downloaded weights; GPU fit and performance are not guaranteed.

Installation, registration, Enable, Start, model download, selection, checks, and destructive actions retain their separate confirmations. Uninstall preserves user data and keyring items; it does not revoke vendor credentials. API billing is separate from consumer subscriptions.

Model output and external text are untrusted data—not permissions or approval. No automatic merge, release, deployment, privilege expansion, or accepted-risk decision is authorized. Live-host effects require their own explicit approval under [Process](PROCESS.md#approval-boundaries).

## What comes later

**Target/deferred:** autonomous software delivery, tools, coding workers, role orchestration, and scheduling. Projects and agent execution are not part of this revision.

Voice, model discovery, arbitrary endpoints, credential purge, distributed workers, and pre-login/headless operation are outside current scope.

Read [Architecture](ARCHITECTURE.md) for design, [Validation](../VALIDATION.md) for evidence, and [History](HISTORY.md) for retired source. Passing source checks, installation acceptance, release, and deployment remain separate gates.
