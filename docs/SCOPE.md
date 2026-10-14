# Smith Scope

## Status

**Normative target policy.** Current implementation facts and gaps are recorded separately in [SMITH Linux Architecture Report](SMITH_LINUX_ARCHITECTURE_REPORT.md).

## Product boundary

Smith is a local-first control plane for a **single trusted operator** using an Ubuntu/Linux workstation to plan, implement, validate, review, and selectively integrate changes to configured Git repositories.

Its purpose is bounded autonomous software delivery with visible evidence and human authority over external effects. Agent autonomy is a method for preparing work; it is not independent authority.

## Supported target

The only supported target for new design, validation, packaging, and agent instructions is:

- a self-hosted Ubuntu LTS x86_64 workstation;
- one trusted interactive user, with Flutter as the primary desktop interface and CLI as secondary query/status/configuration; existing CLI commands remain compatible;
- Docker Engine, Git, OpenSSH, Rust/Cargo, Python, and a local Ollama-compatible model service for the broader orchestration/development workstation; prebuilt recipient prerequisites are separate below;
- loopback-first local APIs and deliberately scoped outbound access; and
- the hardware and operating baseline in [Host Profile](HOST_PROFILE.md).

Existing implementation for other host environments is historical/as-built scope only. It must not define future requirements, broaden the validation matrix, or delay the Ubuntu target unless a user explicitly approves a scope change.

## Next-generation product and lifecycle direction

**Approved target direction, not implemented:** [the product reset](plans/LOCAL-SMITH-PRODUCT-RESET.md) makes persistent goal-oriented chat the desktop-first foundation, with explicitly selected local/cloud service implementations. The first-use desktop flow starts the Smith service through a defined consent boundary; clients attach and synchronize authoritative service state. Closing/quitting the desktop must not terminate the service or already-authorized operations. Explicit service Stop is separate. Roles, subagents, triangulation and pipelines remain later capabilities with existing authority boundaries intact.

This supersedes the reset's earlier no-background-after-close MVP rule, not the implemented legacy chat-only behavior below. The [debug boot slice](plans/LOCAL-SMITH-NEXT-DEBUG-BOOT.md) implements source-level loader/setup/home, a dedicated user-unit installer/start adapter, a private Unix bridge and a durable debug setup flag. It is not installed-host evidence or goal/chat/task persistence. Broader durable state/recovery still requires bounded implementation plans. The implemented login-boot source behavior below now covers deterministic login-scoped enablement and bounded client attachment, but logout/reboot/pre-login operation, lingering, installation and live service changes are not thereby verified or approved. A terminal client is an interface, not service lifecycle management by definition.

**Implemented source addition:** [artifact installation](plans/LOCAL-SMITH-NEXT-ARTIFACT-INSTALL.md) provides `next/` Make build/install targets for the full client/menu/icon/user command and standalone service binary. Artifact install does not register/start units or authorize normal-host deployment by the agent; host integration and implicit upgrade remain excluded.

**Current approved and implemented source direction:** [text reset](plans/LOCAL-SMITH-NEXT-TEXT-RESET.md) makes the initial `next/` product purely text and conversation-first. Voice source is preserved in an excluded checksummed reference, not hidden as an active capability. Durable bounded context, service-owned operations and local Ollama remain. The application observes runtime availability independently of legacy wizard-completion flags; provisioning belongs to a separate `smith-installer` recipient package. No default model weights are bundled; an optional offline model pack requires future license/provenance/import review. This supersedes the earlier next voice-priority/wizard direction, not the separate legacy profile below.

**Current approved native installer direction, implemented in source:** [the native migration](plans/LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md) provides one independent Rust installer crate/workspace at `next/installer`, replacing the recipient Python zipapp/publisher with an actual native Linux x86_64 ELF and complete paired payload under `installer/dist/`. Prepared Ubuntu 24.04 LTS x86_64 recipients need compatible native runtime libraries, not Python or build toolchains; Python Dagger/package tooling is builder-only. Full recipient mode has separate artifact/lifecycle/model confirmations; explicit `--artifacts-only --client BUNDLE` and/or `--service BINARY` serves Make developer publication, and only artifact mode permits `--yes` without bypassing safety checks. Direct reuse of the existing service Controller uses the packaged service as source, not the installer executable; client/service and unit/wire contracts remain unchanged. Excluded checksummed Python sources are historical reference, not a supported fallback. Source implementation/configured isolated gates are not release or installed-host evidence.

**Historical bounded visual installer stage:** [the installer UI plan](plans/LOCAL-SMITH-NEXT-INSTALLER-UI.md) added the optional-feature GTK3 sibling `smith-installer-ui`; its initial Register/Start choices and model placement are superseded by the current minimal installer/model flow and login-boot behavior below. The CLI remains default full-recipient and Make artifact mode, with no GTK/display/Python requirement. Configured isolated UI gates alone are not release or installed-host evidence.

**Full CLI recipient scope:** install Smith artifacts and separately consent to registration, login enablement, Start, preferences and optional exact-model pull/inference. Models remain external and no weights are bundled. This is prepared-Ubuntu installation, not automatic OS/GPU provisioning or general upgrades. Agent source validation never authorizes live deployment/download; prior host upgrade approval is consumed. Cloud, voice, tools/search, pre-login/post-logout operation and lingering changes remain excluded from the initial text product.

## Current next login-boot scope

**Implemented source under [login-boot plan](plans/LOCAL-SMITH-NEXT-LOGIN-BOOT.md):** Core control schema 2 reports independent exact enablement facts and accepts persistent enablement only when manager state is `enabled` and the owned `default.target.wants/smith-next.service` link exactly targets the owned unit. The deterministic unit now includes `[Install]` with `WantedBy=default.target` and retains `Type=simple`, `Restart=no`, `UMask=0077` and its bounded stop budget. Enable and Start are distinct, expiring, separately confirmed actions: the manager runs fixed `enable smith-next.service` without `--now`; Start never enables; neither path invokes `loginctl` or mutates lingering.

Fresh recipient setup is artifacts → register/reload → Enable → Start, with each lifecycle effect separately confirmed and partial outcomes preserved. Maintenance recognizes only byte-exact legacy static and current installable units: Upgrade migrates exact static to current, preserves an exact current enabled state without churn, and explicitly enables exact current disabled state; Uninstall disables and verifies before unregister/removal while retaining data and preserved objects.

The application now gates the existing minimal chat behind a minimal Boot loader. It performs serialized read-only observations for at most 15 seconds, shows a restrained `CustomPainter` status mark with short live status and a reduced-motion static equivalent, and mounts chat only after verified bridge readiness and conversation synchronization. It never automatically starts, enables, installs, provisions a model, infers or resends. Only an exact installed, registered, enabled, safely stopped state offers separately confirmed **Start Smith**, followed by fresh bounded observation.

This is **login-scoped target behavior after user login**, not a pre-login or post-logout guarantee. Real user-manager operation, login/logout, reboot and installed-host maintenance remain unverified and require fresh exact approval. Component and aggregate source/build self-checks are recorded in [Process](PROCESS.md) and `next/VALIDATION.md`; they are not release, deployment or live-host evidence. Fresh independent review remains required after any remediation.

## Current next native-runner scope

**Implemented source:** [native-runner plan](plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md), extended by the first [GPU-preferred implementation slice](plans/LOCAL-SMITH-NEXT-GPU-PREFERRED.md), supersedes the external Ollama requirements in earlier `next/` stages above; legacy profiles below are unchanged. The paired client bundles separately verified llama.cpp `b11429` CPU and Vulkan runtime subtrees, owned lazily by the persistent Rust service. No default weights are bundled. Production Auto attempts explicit full offload on `Vulkan0`; missing Vulkan runtime or a classified, confirmed-cleaned startup/offload-evidence failure permits exactly one independent CPU launch under the same model lease and original deadline. Completion dispatch failure never replays on CPU. CPU explicitly disables devices and GPU layers and does not require loading the Vulkan backend.

Native CLI `--models` and the focused installer expose a seven-entry revision-pinned Apache-2.0 catalog while retaining Qwen3-14B as default. Cloud, voice, OS/driver provisioning, implicit upgrades, pre-login/post-logout operation and lingering changes remain excluded. Source/fake validation is not evidence that `Vulkan0` is a suitable dedicated device, that host Vulkan works, or that inference is accelerated. Device discovery/stable selection, preference UI/schema, automatic partial offload, execution-status UI and measured performance remain deferred. No installed-host action, GPU probe, model weight, or real inference is authorized or claimed.

## Current installer model vertical slice

**Implemented source under [model-wizard plan](plans/LOCAL-SMITH-NEXT-MODEL-WIZARD.md):** a focused native GTK installer owns Qwen3-14B-default choice/download, checked preference, service-backed no-chat inference and rerun add/remove. Default preselection is not automatic download/consent. Existing saved selection is preserved; removal is separately consented, service-mediated and blocks selected/loaded models. Model-default reasoning replaces forced-zero, with final-content-only handling. Earlier artifact-only GUI and external Ollama descriptions above are historical stages, not current requirements. No real weights, installed upgrade or live inference occurred under source scope; source/fake evidence is not release or deployment readiness.

## Explicit existing-installation maintenance

**Implemented source under [maintenance plan](plans/LOCAL-SMITH-NEXT-INSTALLER-MAINTENANCE.md), extended by [login-boot](plans/LOCAL-SMITH-NEXT-LOGIN-BOOT.md):** optional GTK installer offers Install / Upgrade / Manage models / Uninstall. Maintenance accepts only byte-exact legacy static or current installable `smith-next.service` units. Upgrade preserves artifacts, migrates exact legacy static to current, preserves exact current enabled state without churn, explicitly enables exact current disabled state, separately confirms Stop/Enable/Start and verifies the new pair. Explicit Uninstall stops/quiesces, disables and verifies enablement removal before unregistering and preserving active integration away from active names, while retaining ALL state, models, releases, backups and preserved objects. No purge, implicit upgrade, old-helper execution, lingering mutation or legacy `smith.service` migration. Ordinary Install/Make differing-binary guards remain. Implementing these paths is not permission for an agent to upgrade/uninstall the workstation; live effects and model downloads still require separately bound approval.

## Staged local text-chat delivery

**Separate legacy target:** the following `src/`/`client/` chat-only candidate, packaging scripts and desktop-owned backend lifecycle remain as-built legacy scope. The native `next/` installer does not migrate these entry points or remove their separate recipient requirements, historical evidence or release blockers.

**Implemented subset:** [LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0](plans/LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0.md) introduces `smith serve --chat-only --model <exact-installed-tag>`. The approved [Flutter boot plan](plans/LOCAL-SMITH-FLUTTER-BOOT-P0.md) adds primary desktop Boot/Setup, factual readiness pulses, and explicit Start before Halo Home/Chat; ordinary Settings remains read-only. Opening/readiness checks never start a chat backend. The approved [desktop wizard plan](plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md) adds automatic validation of `<actual Halo executable directory>/smith_backend/smith`, an explicit advanced absolute path, read-only structured `setup inspect` inventory, observed installed-tag selection and review before explicit Start. Only an explicitly confirmed separate `setup pull` may download the exact model through already-running loopback Ollama, with operation/model-bound stdin consent, bounded NDJSON progress/cancel and finite deadlines; no automatic retries. Inventory readiness is not inference verification; this is partial empty-host setup, with guided separate Ollama installation/start and no sudo, legacy install helpers or service changes. Download may use daemon internet/persistent disk; size and cancellation/cache outcome may be unknown. The chat-only bridge remains isolated with no provisioning tools/routes; a compatible ready external backend attaches untouched. Only a captured desktop-owned child is stopped on normal close, with session correlation, parent-pipe EOF and bounded TERM/KILL cleanup—not a crash-proof guarantee. It uses direct loopback Ollama with an exactly named installed local model; no orchestration dispatch, tools, provisioning, or cloud fallback is part of the serving profile. The separate setup command is not a chat capability. One shared default session retains bounded memory only, cleared on backend restart. Voice, Work, integrations, uploads, persistent history, and streaming are deferred.

**Target interface policy:** Flutter is primary; CLI is secondary query/status/configuration, not removed or restricted by this preference. Future headless operation is a separate deployment/lifecycle capability, not synonymous with CLI, and is deferred. The wizard narrowly supersedes the boot provisioning exclusion for explicit model pull only; no Ollama host installation/start, service integration or headless implementation is added.

**Candidate target:** Ubuntu 24.04 LTS x86_64, with a full Flutter desktop bundle including `smith_backend/smith` and complete `lib/`/`data/`, plus a secondary standalone backend candidate. The general workstation toolchain below is not a recipient prerequisite for this slice; runtime versus builder requirements are separated in [Host Profile](HOST_PROFILE.md) and [Chat Release](CHAT_RELEASE.md). Plain `serve`, `chat`, and systemd registration remain legacy orchestration paths, with no automatic migration.

**Not a production release:** actual provider pull/cancel integration and version compatibility, real network-denied inference, clean-host desktop installation, final candidate-bound validation/review, clean-source/license/SBOM evidence, and remediation of the legacy release workflow remain publication blockers. Local candidate preparation does not authorize publication or host/service mutation.

## In scope

- Local planning, implementation, validation, and review of configured repositories.
- Self-hosted Smith development, beginning with the `feature/autonomy` bootstrap branch.
- Provider- and model-swappable roles behind stable capability, evidence, and approval contracts.
- Docker-isolated workers with explicit filesystem, network, credential, and tool permissions.
- Optional Jira/GitHub work-tracking integration when the operator enables it; local intent, evidence, review, and approval remain valid without it.
- Durable, inspectable evidence for consequential work.
- Clear operator visibility of task state, failures, recovery requirements, risks, and pending approvals.

## Explicitly out of scope

- Multi-tenant operation, remote public service hosting, or unauthenticated LAN exposure.
- Unattended privilege escalation, arbitrary host filesystem access, or worker Docker-daemon control.
- Automatic merge, release, deployment, Jira completion, accepted-risk decisions, or permission expansion.
- Treating model text, repository text, web content, ticket text, MCP metadata, or tool output as trusted instructions.
- Claiming private container networking creates a complete host air gap.
- Distributed execution, remote workers, or durable job scheduling until explicitly designed and implemented.

## Work and risk classes

| Class | Examples | Minimum handling |
|---|---|---|
| Routine | Documentation, isolated bug fix, non-sensitive test | Bounded work record, scoped plan, self-check, independent review as appropriate. |
| Elevated | Public API, storage, task state, client/operator flow, dependency update | Architect and assurance review, explicit validation/rollback evidence. |
| Critical | Credentials, SSH, mounts, filesystem roots, Docker/network exposure, tool capabilities, approval logic, Git push/merge/release/deploy behavior | Independent architecture and adversarial review, reproducible evidence, explicit human approval before external effect. |

## Scope-control rules

1. Every work item needs a bounded, reviewable intent record: use Jira when its optional integration is enabled, otherwise use an approved local task/plan record.
2. New ordinary branches may use `feature/<KEY>-<slug>` / `fix/<KEY>-<slug>` when ticket-linked, or `feature/<slug>` / `fix/<slug>` otherwise; no `develop` branch exists today.
3. `feature/autonomy` is a one-time bootstrap exception because it predates this policy. It must not be renamed or force-pushed merely to conform.
4. Cross-repository work requires one coordination plan, one PR per repository where PRs are used, explicit compatibility evidence, and a documented merge order.
5. An agent must stop and request approval when requested work exceeds the recorded intent, active scope, or risk classification.

## Definition of progress

Code generation, a passing narrative, a model vote, or a created pull request is not completion. Completion requires the lifecycle, evidence, independent review, and human approval conditions in [Process](PROCESS.md).
