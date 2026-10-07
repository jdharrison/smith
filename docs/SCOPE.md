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
- Docker Engine, Git, OpenSSH, Rust/Cargo, Python, and a local Ollama-compatible model service;
- loopback-first local APIs and deliberately scoped outbound access; and
- the hardware and operating baseline in [Host Profile](HOST_PROFILE.md).

Existing implementation for other host environments is historical/as-built scope only. It must not define future requirements, broaden the validation matrix, or delay the Ubuntu target unless a user explicitly approves a scope change.

## Staged local text-chat delivery

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
