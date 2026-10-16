# Smith Scope

## Status

**Normative target policy, with explicitly labelled implemented subsets.** Executable source and reproducible evidence establish current behavior; [Architecture Report](SMITH_LINUX_ARCHITECTURE_REPORT.md) records implementation limits. Authority remains: explicit human approval and active scope; executable evidence/current configured records; canonical Scope/Architecture/Process/Design policy; role method; explanatory and historical material. Target policy is not an implemented guarantee.

## Product boundary

Smith is a local-first control plane for **one trusted operator on one Ubuntu workstation**. The implemented product is persistent, bounded text conversation through a Flutter desktop, a Rust service and a separate native installer. It is a development candidate, not an accepted production release.

Bounded autonomous software delivery across configured repositories remains the **target**, not a capability supplied by today's chat. Roles, subagents, triangulation and delivery pipelines are deferred runtime capabilities; `roles/` defines development/review method. Model text, queued messages and background continuation grant no tools, permissions or external-effect authority.

## Supported target

- Prepared Ubuntu **24.04 LTS x86_64** recipients; Ubuntu LTS x86_64 is the supported workstation design boundary. No new other-OS packaging, validation or design without an explicit scope amendment.
- Flutter is the primary desktop interface. Native service query/control and installer CLI are secondary; non-GUI commands do not establish supported pre-login/headless deployment.
- Unprivileged same-user systemd management, private Unix bridges, safe owner-controlled XDG roots and deliberately scoped egress. No unauthenticated LAN/public or multi-tenant service.
- Compatible native libraries and graphical session for desktop/GTK; no recipient Python, Rust, Flutter SDK, Dagger, Docker, OpenCode or external Ollama dependency. Builders have separate requirements in [Host Profile](HOST_PROFILE.md).

**Implemented repository boundary after promotion:** `client/`, `service/`, `installer/`, `dagger/` and root `VALIDATION.md` are current. Service and installer remain independent Rust workspaces, not a root Cargo package. Legacy root source/CLI, Halo chat-only packaging, sidecars and copied voice/Python-installer references are retired from the working tree. Old `smith serve`, `chat`, setup, orchestration and service commands are no longer supported source entry points; no compatibility shims or automatic installed migration are promised. Retirement does not stop, uninstall or modify any existing installation. Runtime names including `smith-next.service`, `smith-next` and XDG/keyring/protocol identities **remain unchanged**.

Historical bytes and dated limitations are retained through Git commit `168865b0422b787f08ab33e89d052cd4adbd11f7`, with the path map and retrieval policy in [History](HISTORY.md) and exact dispositions in the cutover ledger. Do not turn historical source into a supported alternate product. See the [pinned prior Scope](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/SCOPE.md) for superseded stages, not current requirements.

## Current next login-boot scope

**Implemented source:** the persistent service owns accepted work and bounded durable histories independently of desktop lifetime. Close/Quit detaches; explicit Stop is separate. Core schema2 reports binary, unit, enablement, runtime and bridge independently. Enabled means manager `enabled` **and** the exact owned `default.target.wants/smith-next.service` link to the exact owned unit. The deterministic user unit retains `Type=simple`, `Restart=no`, `UMask=0077`, five-second stop budget and `[Install]/WantedBy=default.target`.

Registration/reload, Enable and Start require separate expiring confirmations. Enable never uses `--now`; Start never enables; neither invokes `loginctl` or changes lingering. Boot serializes read-only observations for at most15s, then synchronizes conversations under separate finite query bounds before mounting chat. It does not install, enable, start, download, infer or resend automatically. Only an exact installed/registered/enabled/safely-stopped state offers separately confirmed Start. This targets startup after login, not pre-login/post-logout continuity, reboot recovery or proven real-manager behavior.

## Current next native-runner scope

**Implemented source:** service-owned llama.cpp `b11429` with separate SHA-pinned CPU/Vulkan subtrees in the paired client, no default weights and no external Ollama/TCP inference endpoint. One executing inference, bounded threads, context4096, output1024 tokens and180s cooperative dispatch budget. Model-default reasoning remains; only complete valid final text is published. No tools, automatic download or completion replay.

Auto attempts full offload on fixed `Vulkan0`, requiring strict actual full-offload startup evidence. Missing Vulkan or classified startup failure permits at most one independent CPU launch only after confirmed cleanup, under the same model lease/original deadline. No fallback after runner acceptance/request failure. Fixed `Vulkan0` is not stable device selection or a suitability judgment. Driver provisioning, device discovery/preferences, automatic partial offload, execution-status UI and measured performance remain deferred. Integrity checks and private processes are not a hostile-same-UID or GPU-driver sandbox.

## Current installer model vertical slice

**Implemented source:** complete paired native CLI/optional GTK3 installer package at `installer/dist/`; builder Python is not a recipient fallback. GUI landing offers **INSTALL / UPGRADE** and **UNINSTALL**, with model/provider management on setup/result paths. Opening/preview is read-only. Artifact publication, registration/reload, login Enable, Start, exact-model download, revision-checked preference and optional no-chat inference each retain separate consent. Full recipient mode has no blanket `--yes`; only explicit developer `--artifacts-only` mode permits it without bypassing safety checks or adding lifecycle/provider effects.

Seven revision/size/SHA-pinned Apache-2.0 local choices retain fresh-GUI Qwen3-14B Q4_K_M preselection; reruns preserve saved selection. Preselection is not authorization. No free-form models/paths/import or bundled weights. Explicit HTTPS download is bounded and pinned; no proxy/retry/resume/cache purge. Verification saves no turn and is not GPU/performance proof. Service-mediated removal blocks selected/loaded models, requires separate destructive consent and preserves partial/orphan data on failure.

## Explicit existing-installation maintenance

**Implemented source:** Upgrade/Uninstall independently verify older managed artifacts without executing old helpers. Only byte-exact legacy static or current installable `smith-next.service` units are accepted; unsafe/foreign/override/unknown identities or unproven quiescence block. Upgrade preserves old artifacts, migrates exact static units, preserves exact current enabled state without churn and separately enables exact current disabled state. Stop/Enable/Start remain separate confirmations. Ordinary Install/Make refuse differing installed service bytes; no force or implicit upgrade.

Uninstall stops/quiesces, disables and verifies before unregistering and preserving active integration away from active names. **All histories, state, preferences, models, releases, backups, staging and preserved objects remain**, consuming disk. No purge, recursive deletion, rollback, old-helper execution, lingering mutation or legacy `smith.service` migration. Explicit post-success Launch validates and dispatches only the installed desktop; nothing launches automatically. Source authority is not permission to maintain the live workstation.

## Current next cloud-provider scope

**Implemented source:** optional OpenAI API and/or Anthropic API management follows unchanged local Model setup. Save/Replace and potentially billable synthetic Validate are separate effects; rerun provider management does no artifact/lifecycle work. API access/billing is separate from consumer app subscriptions. Fifteen compiled identities (seven OpenAI/eight Anthropic) and fixed adapter behavior are defined by the retained [cloud API contract](plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md); listing is not account entitlement or live availability. No discovery, automatic validation, retry or provider/model fallback.

Per-chat **LOCAL / CLOUD** selection affects future sends, not accepted immutable routes/context. Same-chat switching and unavailable historical models must preserve readable history and explicit alternate selection. A new provider/credential generation requires exact context-transfer consent; retained consent covers later explicit sends to that destination in the same chat, including intervening retained history. A generation is not verified account identity, and queue Resume is not destination consent.

Credentials enter only through installer private input and an existing unlocked same-user Ubuntu GNOME Keyring session: no Flutter collection, plaintext/environment fallback, activation/unlock/Prompt or subscription login. Each Save creates a fresh generation; old and uncertain inactive items remain. No in-app removal; Uninstall does not remove keyring items or revoke vendor credentials. Saved active generation plus supported exact route permits an explicit attempt without mandatory probe/prior validation; fresh exact-generation retrieval before HTTP still fails closed. Cloud caps total generation including reasoning at8192 tokens under180s; local1024 and startup fallback remain unchanged.

Application schema3, Core2 and disk4 are current. Older data migrates strictly without invented model provenance/dates; older binaries cannot read disk4. Live migration/rollback needs separately approved pre-migration state plus compatible paired binaries and exact keyring-generation relationships, not binary-only downgrade. Fixed endpoints are not an OS egress sandbox; provider processing, charges and retention remain external facts, not zero-retention/cost guarantees.

## Current next multi-chat scope

**Implemented source:** isolated service conversations support atomic first-message creation, deterministic40-Unicode-scalar new titles/renames, archive/reactivate and confirmed logical delete. Existing longer valid titles/receipts remain readable. Up to32 active+archived chats retain16 pairs/64KiB each. Four chat requests may be outstanding, one per conversation and one executing globally; durable FIFO admission is not parallel inference or a general scheduler. Restart interrupts unfinished work without replay; close detaches. Disk4 saves nullable service admission/final-reply times, never invented historical/provisional dates or model-only latency.

## Client-side sequential follow-ups

**Implemented source:** volatile follow-ups wait for their exact predecessor's verified success and synchronized saved history: eight per conversation/32 desktop-wide, plus one current-predecessor allowance per conversation,16KiB input each. Failed heads block until explicit safe Retry/Skip; unknown/running outcomes remain observation-only. Revision/instance/context changes require review. Normal Close/Quit holds dispatch and warns before discarding unsent text; forced death can lose it. This is not steering, semantic task-success gating, durable outbox, running cancellation or authority for queued deployment.

Up to four panes share one workspace/API/observation owner and shared saved-chat controllers. Safe local preferences persist presentation only, never drafts/readiness/work. Additional `--session-only-window` desktops have independent drafts/queues and no preference-store access while sharing durable service chats/resource limits. No window creation, restoration, polling or animation authorizes lifecycle/inference/replay. See [Architecture](ARCHITECTURE.md) and [Design](DESIGN.md).

## In scope

**Target/deferred orchestration:** bounded planning, implementation, validation and independent review; provider-swappable roles behind stable capabilities; scoped Docker workers; optional Jira/GitHub; inspectable evidence, recovery and human signoff. These remain requirements for future approved slices, not claims about current chat execution.

## Explicitly out of scope

Multi-tenancy/public exposure; unattended privilege escalation, arbitrary host roots or worker Docker control; automatic merge/release/deploy/ticket completion/accepted risk; trusted model/repository/web/MCP/tool instructions; claims that private networking creates an air gap; voice, search/tools, gateways/arbitrary endpoints, streaming/media, Branch/Compare, model discovery, credential purge, distributed workers, durable general scheduling and pre-login/headless operation without separately approved designs. Offline model packs require separate license/provenance/import review.

## Work and risk classes

| Class | Minimum handling |
|---|---|
| Routine: docs, isolated non-sensitive fix/test | Bounded record, scoped plan, self-check and independent review as appropriate. |
| Elevated: interfaces, storage, client flow, dependencies | Architect/assurance review and explicit validation/rollback evidence. |
| Critical: credentials, roots/mounts, network, capabilities, approvals, Git/publication/deployment | Independent architecture/adversarial review, reproducible evidence and exact human approval before external effects. |

## Scope-control rules

Every item needs a Jira record when enabled, otherwise an approved local plan. Follow [Process](PROCESS.md) for branches, evidence and approval; `feature/autonomy` remains the one-time bootstrap exception, never retroactively renamed/force-pushed. Cross-repository work needs a coordination plan, compatibility evidence, per-repository PRs where used and merge order. Stop when intent, authority or risk scope is exceeded.

## Definition of progress

Generated text, a model vote, an acknowledgement, passing isolated tests or a PR is not completion. Source acceptance, installed acceptance, release and deployment are distinct gates. Required missing/failing evidence blocks the relevant outcome; final promoted-tree results and fresh assurance belong in root `VALIDATION.md`.
