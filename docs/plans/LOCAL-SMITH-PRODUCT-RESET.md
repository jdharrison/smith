# LOCAL-SMITH-PRODUCT-RESET

## Status and authority

**Approved product direction; repository layout and implementation below are proposals, not implemented guarantees.** The operator explicitly requested a local WIP commit of the existing work so it is preserved, followed by consideration of a repository restructure and a simple new desktop client with the existing client retained as reference.

Checkpoint scope: current source, tests, documentation and local packaging scripts on the existing feature/autonomy branch. No push, merge, tag, release, branch rename, runtime/service changes, downloads, normal-path installation or deletion. Generated builds, QA copies and raw validation output stay outside Git. This record authorizes no immediate directory moves or new runtime/persistence/cloud capabilities; consequential implementation needs independent producer/architect/designer reconciliation and bounded acceptance evidence first.

## Product direction from the operator

Smith is a desktop-first, open-source, customizable AI application. Local services are the default; explicitly chosen cloud implementations can replace appropriate services. Chat is the MVP experience. Custom/background autonomous work remains a later capability, not prerequisite infrastructure exposed in onboarding.

Desired journeys:

1. **First use:** install -> setup backend and configuration -> select model and TTS/STT services (local or explicitly configured cloud) -> chat.
2. **Subsequent launch:** load saved configuration and conversation state -> chat directly, without repeating setup.
3. **Repair/configuration:** reopen setup intentionally when configuration is incomplete or incompatible; a temporarily unavailable provider shows recoverable chat status, not a forced first-run wizard.

No always-running feature in the MVP: no tray/background-after-close operation, service registration, login autostart, lingering, or implicit headless daemon. App-owned runtime is scoped to the desktop session; explicitly connected external services are not stopped on app close. Headless operation is separate future work, not synonymous with the diagnostic CLI.

## Proposed minimum architecture

- One coherent desktop distribution with the compatible Smith backend included; normal operation has no backend-path or port-management form.
- Thin desktop UI, a small application core, stable typed provider/service contracts, and separate adapters. Configuration and installation are deterministic application behavior, not model-issued commands.
- Versioned persistent setup state and durable local conversation data; ownership, atomic updates, migrations, retention/export/deletion and recovery must be designed and tested before implementation.
- Credentials use an explicitly designed secure store/reference boundary, never plaintext chat records, Git or diagnostic logs.
- Explicit local/cloud choice; no silent cloud fallback or uploading private chat as a failure recovery action. Destination and data-sharing consequences are visible.
- Speech can be configured or disabled; basic text chat must not be blocked by optional voice provisioning. This is a proposed simplification to confirm in the reconciled client plan.
- First client slice: one setup flow, one conversation surface and minimal settings. No pipeline UI, universal plugin framework or service dashboard.

## Repository separation proposal — not a migration plan

Illustrative boundaries, naming to be reconciled before moves:

```text
apps/desktop/          New minimal desktop client
crates/core/           Application state/config/conversation domain
crates/runtime/        Backend and desktop-session lifecycle
crates/contracts/      Versioned UI/runtime and service contracts
services/              Narrow local/cloud service adapters
reference/client-v1/   Preserved existing client as reference
packaging/             Desktop distribution/install tooling
```

Existing Rust backend code is a selective reuse source, not a requirement to move/rewrite everything at once. Preserve lifecycle/ownership, loopback/profile isolation, structured setup consent and negative tests where they fit the new contract. Existing broad orchestration/install paths must not become the new default installer. Keep reference material clearly separate from supported entry points without deleting or overwriting it.

## Known as-built limitations at checkpoint

The current client is not this product reset: setup preferences and transcript are memory-only; chat uses a shared bounded backend session; cloud/speech configuration is not supported by the new chat-only entry; empty-host Ollama installation/start is not supplied. Earlier widget/lifecycle and Rust checks plus independent static reviews support only their recorded source/scenario scope. They do not prove clean-host installation, real provider pull/cancel, desktop process integration, offline inference, or release readiness.

Existing release workflow, source reconstruction, license/SBOM and required release approvals remain blockers. A WIP commit preserves work; it is not a production release or accepted-risk decision.

## Next bounded planning step

1. Inventory current boundaries and identify reusable code/tests versus reference-only code.
2. Obtain independent producer/architect/designer assessments for these three journeys.
3. Reconcile repository layout, installer ownership, persistent data and secure credentials, local/cloud authority, optional speech, and desktop-session lifecycle.
4. Produce an incremental migration plan preserving the old client, executable paths/builds and tests; do not perform a bulk rename without that plan.
5. Implement the smallest new setup-to-chat slice, then saved-state direct-to-chat launch. Validate with fake providers first and independently review before approved real-host effects.
