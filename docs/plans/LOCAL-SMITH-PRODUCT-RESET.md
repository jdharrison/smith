# LOCAL-SMITH-PRODUCT-RESET

## Status and authority

**Approved product direction; repository layout and implementation below are proposals, not implemented guarantees.** The operator explicitly requested a local WIP commit of the existing work so it is preserved, followed by consideration of a repository restructure and a simple new desktop client with the existing client retained as reference.

Checkpoint scope: current source, tests, documentation and local packaging scripts on the existing feature/autonomy branch. No push, merge, tag, release, branch rename, runtime/service changes, downloads, normal-path installation or deletion. Generated builds, QA copies and raw validation output stay outside Git. This record authorizes no immediate directory moves or new runtime/persistence/cloud capabilities; consequential implementation needs independent producer/architect/designer reconciliation and bounded acceptance evidence first.

## Product direction from the operator

Smith is a desktop-first, open-source, customizable AI application. Local services are the default; explicitly chosen cloud implementations can replace appropriate services. Chat is the MVP experience. Custom/background autonomous work remains a later capability, not prerequisite infrastructure exposed in onboarding.

Desired journeys:

1. **First use:** separate installer prepares Smith/service and an exact local model through existing Ollama -> open directly into text chat. No voice or model weights in the default app bundle.
2. **Subsequent launch:** load saved configuration and conversation state -> chat directly, without repeating setup.
3. **Repair/configuration:** keep the conversation/draft visible with Retry/Open installer and a compact installed-model choice if needed; temporarily unavailable infrastructure never repeats onboarding or resets history.

**Updated approved lifecycle direction (2026-10-07):** the first-use desktop flow starts the Smith service through an explicit setup/start boundary. The desktop then connects and synchronizes authoritative service state. Closing or quitting the desktop must detach the UI without terminating the Smith service or prematurely ending already-authorized operations. Later launches attach and synchronize; they must not create duplicate work or treat local UI state as authoritative. This supersedes the earlier desktop-session-only/no-background-after-close MVP restriction.

Persistent goal-oriented chat is the foundation: goals, conversations, configuration, and operation records belong to the service, with durability and recovery designed explicitly. Roles, subagents, triangulation, other capabilities and pipelines are later additions, not onboarding prerequisites. Continuing an authorized operation after window close does not grant new autonomy or permissions.

Explicit **Stop Smith service** is separate from closing/quitting the desktop and must state the effect on in-flight work and saved data. Independently connected provider services remain untouched. The service management mechanism, first-start consent and resource disclosure, logout behavior, restart/recovery, login autostart, reboot/pre-login availability and lingering require separate bounded design/approval; no host change or crash/reboot continuity is authorized or promised by this direction. Headless service operation is distinct from the terminal client.

## Proposed minimum architecture

- One coherent desktop distribution with the compatible Smith backend included; normal operation has no backend-path or port-management form.
- Thin desktop UI, a small application core, stable typed provider/service contracts, and separate adapters. Configuration and installation are deterministic application behavior, not model-issued commands.
- Service-owned authoritative state independent of desktop lifetime, with attach/synchronize contracts and duplicate-start/work prevention. Versioned persistent setup state, goals, conversations and operation records; ownership, atomic updates, migrations, retention/export/deletion and recovery must be designed and tested before implementation.
- Credentials use an explicitly designed secure store/reference boundary, never plaintext chat records, Git or diagnostic logs.
- Explicit local/cloud choice; no silent cloud fallback or uploading private chat as a failure recovery action. Destination and data-sharing consequences are visible.
- Speech can be configured or disabled; basic text chat must not be blocked by optional voice provisioning. This is a proposed simplification to confirm in the reconciled client plan.
- First client slice: one conversation surface and minimal repair/settings; provisioning is a separate installer, not an application setup-state machine. No voice, pipeline UI, universal plugin framework or service dashboard.

## Approved text-only/installer reset

**Current approved direction, implemented in source:** [text reset](LOCAL-SMITH-NEXT-TEXT-RESET.md) supersedes the voice/wizard refinement below. Preserve the voice experiment as a checksummed excluded reference and trim active execution/dependencies to text. Keep native service, bounded durable context and operation identity, but ignore the legacy setup flag as a readiness gate. A separate recipient installer handles artifact publication and consented lifecycle/model provisioning; it is independent of build tooling, not a turnkey OS/Ollama/GPU installer. Default weights remain external; optional offline packs need future license/provenance review. Source reset does not update the installed candidate or authorize real downloads/inference/GUI. The earlier desktop-first-start consent is now presented by the installer, with authority boundaries preserved.

## Approved native Rust installer migration

**Current approved direction, implemented in source; not a release or new host deployment:** [the native installer plan](LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md) replaces the Python recipient runtime and artifact publisher with one independent Rust crate/workspace and lockfile at `next/installer`. `smith-installer` is an actual native Linux x86_64 ELF, not a launcher around Python; recipients need prepared Ubuntu 24.04 x86_64 native runtime libraries, not Python or build toolchains. Python Dagger/package tooling remains builder-only metadata/orchestration. `make installer/build` exports the complete paired recipient package to `installer/dist/`; `make installer/binary` exports `installer/build/smith-installer` for Make's explicit `--artifacts-only --client BUNDLE` and/or `--service BINARY` publication. Only artifact mode accepts `--yes`, bypassing no safety checks; full recipient mode keeps each artifact/unit registration/reload/Start/model pull/preference/inference consent separate.

The installer directly reuses `smith_service::Controller` with packaged `payload/service/smith-service` as source, never the installer executable. Client/service source and unit/wire contracts are unchanged. Runtime roots/backend/provider remain lazy after artifact acceptance; dry-run/initial decline does not contact a manager/provider or write destination files/locks. The native publisher preserves held-FD/no-follow traversal, ownership/mode/single-link checks, bounded trees, cooperative lock, post-consent/under-lock revalidation, byte-identical canonical release manifests, no-clobber/synced publication, final dependency verification and atomic selector exchange with displaced objects retained. Late conflicts/partial effects remain visible, with no global transaction, same-UID filesystem CAS or automatic rollback.

Models remain external through already-running fixed-loopback Ollama, without a default tag or implicit pull. Native consent/cancellation and individual manager/socket/provider budgets do not promise a total filesystem deadline or external-daemon stop; bounded setup/progress output accepts pipes/sockets/terminals, not regular-file/device writes. Exact old Python sources/tests and evidence remain in excluded checksummed `reference/python-installer-v1/`, with no maintained fallback; historical hashes and the separate legacy target remain intact. Dagger defines actual Ubuntu 24.04 UID/GID-1000 native tests, a no-Python runtime gate and builder package tests consuming the actual ELF, not final integrated validation or independent assurance claims. Prior installed-host approval is consumed. No live install/manager/provider/model/GUI, commit or remote effect is authorized by source migration; fresh candidate-bound evidence/review and separate host/release approvals remain required.

## Approved bounded installer UI

**Implemented source direction, validation/review pending:** [the UI plan](LOCAL-SMITH-NEXT-INSTALLER-UI.md) adds optional-feature native GTK3 sibling `smith-installer-ui` to the full package, leaving default full-recipient CLI and developer artifact Make routes unchanged. A single near-black/green review → Install → Complete/Stopped/Failed page installs fixed paired app/service artifacts. Register and Start default off; Start requires Register. Install binds one consent to the read-only artifact snapshot; applicable Core effects retain fresh separate inline confirmations and original 60-second tokens. One owned worker uses bounded typed request-ID channels; cancel/close waits for its exit/join, preserving effects without rollback.

No GUI provider/model/download/inference or automatic app launch is added; model selection remains in the existing app. The GUI needs GTK3/session libraries, is not persistently installed, and does not change the app's instruction-only CLI fallback. Operator QA uses `./installer/dist/smith-installer-ui` from `next/` after building the full package if needed. Configured Ubuntu/Xvfb tests are not asserted passed. The native migration's final integrated assurance was cancelled, not a PASS; scoped UI evidence/review and later host/release approvals remain separate. Prior voice deployment and consumed host approval are unchanged.

## Historical setup and speech direction follow-up

**User-requested target refinement:** speech capture is an early core interaction, interchangeable with typing in one editable composer, with explicit Send by default. Setup should become Core → LLM → Services (STT/TTS/search), with a substantially simpler dark green/black presentation. Local Ollama setup is the first provider candidate; cloud comes later. Optional login startup and a unified Compose deployment with explicit shares are requested directions, not implemented capabilities or host approval. The [wizard plan](LOCAL-SMITH-NEXT-SETUP-WIZARD.md) reconciles a presentation-only first slice and records provider, consent, mounts/network, topology and service-gating decisions still required. The earlier suggestion to defer speech behind basic chat is no longer the user's priority; optional feature readiness versus mandatory services remains unresolved.

## Current scaffold follow-up

The operator initially chose a flatter `next/client`, `next/service`, and `next/dagger` layout, deferring a separate terminal client and top-level contracts/packaging folders. [LOCAL-SMITH-NEXT-SCAFFOLD](LOCAL-SMITH-NEXT-SCAFFOLD.md) bounds the barebones Flutter/Rust initialization and developer-only pipeline. [LOCAL-SMITH-NEXT-DEBUG-BOOT](LOCAL-SMITH-NEXT-DEBUG-BOOT.md) subsequently bounds source-level Loader/Setup/Home, an isolated managed user service/private bridge and a durable debug setup flag. The approved native migration adds `next/installer` as an independent component, rather than retaining runtime installation under Dagger. The earlier illustrative layout below is superseded; these steps do not authorize the full product reset, real-host rollout or legacy cutover.

## Repository separation proposal — historical illustration, not a migration plan

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
3. Reconcile repository layout, installer/service ownership, durable data and secure credentials, local/cloud authority, optional speech, and persistent service lifecycle with desktop attach/synchronize.
4. Produce an incremental migration plan preserving the old client, executable paths/builds and tests; do not perform a bulk rename without that plan.
5. Implement the smallest new setup-to-chat slice, then saved-state direct-to-chat launch. Validate with fake providers first and independently review before approved real-host effects.
