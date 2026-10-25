# Smith Delivery Process

This is the short entry page. [Process details](PROCESS_DETAILS.md) retain the full authoritative requirements; this summary does not replace or relax them. Read the approval boundaries below before acting, then only the detailed sections relevant to the task.

## Status

**Normative target process, not a runtime enforcement guarantee.** Source checks and historical passes are not hosted execution, installed acceptance or human approval. [Current evidence](../VALIDATION.md) records actual outcomes and outstanding gates.

## Work record and branch policy

Every change needs bounded intent and appropriate checks. Routine local work can use the explicit request and a brief conversational scope/check note; consequential work uses optional Jira or a reused approved local plan. [Branch rules](PROCESS_DETAILS.md#work-record-and-branch-policy) retain the `feature/autonomy` bootstrap exception; no implicit commit or remote authority follows.

## Canonical lifecycle

**Routine local work:** request → brief scope → implement → focused checks → concise result, without new plan/evidence files or subagent review by default. **Consequential work:** bounded intent → independent producer/architect/designer analyses → planner reconciliation → resolve blockers → scoped implementation → self-check → fresh independent assurance/adversarial review → human signoff → exact approved effect. [Proportionate work and documentation](PROCESS_DETAILS.md#proportionate-work-and-documentation) defines the tracks; elevated triggers and all external-action approvals remain unchanged. [Full lifecycle](PROCESS_DETAILS.md#canonical-lifecycle); current chat does not implement this orchestration.

## Triangulation protocol

Preserve disagreements; agents cannot approve their own work. [Role independence and reconciliation](PROCESS_DETAILS.md#triangulation-protocol).

## Evidence contract

Routine checks belong in the completion summary. Consequential evidence binds requirements, source identity, exact checks/results, review, risks and approvals once in the active work record; link rather than duplicate it. Missing or stale required evidence is not PASS. [Full evidence contract](PROCESS_DETAILS.md#evidence-contract).

## Required gates

Select by impact on a stable final snapshot. Failing required checks, compiler/runtime errors, unresolved critical warnings and missing required verification block acceptance/release. [Gate matrix](PROCESS_DETAILS.md#required-gates); source fixtures never substitute for separately approved host checks.

### Root development and component gates

[Root Make, separate workspaces, Dagger and package gates](PROCESS_DETAILS.md#root-development-and-component-gates), including pre/post-export and pre-launch location checks.

### Multi-chat validation and migration boundary

[History, scheduling, compatibility and migration gates](PROCESS_DETAILS.md#multi-chat-validation-and-migration-boundary).

### Next cloud-provider validation and compatibility

[Consent, keyring, transport, budgets and compiled integration gates](PROCESS_DETAILS.md#next-cloud-provider-validation-and-compatibility).

### Saved message timestamp validation and migration

[Timestamp semantics and preservation gates](PROCESS_DETAILS.md#saved-message-timestamp-validation-and-migration).

### Client-side message-queue validation

[Sequential follow-ups, uncertainty and close guards](PROCESS_DETAILS.md#client-side-message-queue-validation).

### Desktop presentation, persistence and windows

[Layout, preferences, accessibility and session-only windows](PROCESS_DETAILS.md#desktop-presentation-persistence-and-windows).

### Projects metadata validation and compatibility

[Projects UX/disk9/v3 compatibility, preservation and no-execution gates](PROCESS_DETAILS.md#projects-metadata-validation-and-compatibility) govern the [approved editor/persistence slice](plans/LOCAL-SMITH-PROJECTS-UX.md). The editor is implemented in source, with recovery/layout closeout tracked in the plan. Current-source checks, combined gates and fresh assurance remain pending and require parent-owned evidence; documentation is not a PASS or rollout approval.

### Agents definitions validation and compatibility

**Required gates for the implemented-source [approved custom Agents slice](plans/LOCAL-SMITH-AGENTS-CUSTOM.md), not acceptance/release PASS:** cover 2–32 profiles, immutable built-in current saved metadata with editable prompts, custom bounds/IDs/order, disk10 strict legacy preservation, shared replay protection and original-receipt create recovery. Preserve neighboring Projects/Dev/navigation and zero automatic execution/provider effects. [Detailed required checks](PROCESS_DETAILS.md#agents-definitions-validation-and-compatibility) remain parent-owned; this docs-only task executes none.

Focused UI/controller checks do not replace final stable-source component/root/build/package gates or fresh independent assurance. Those combined gates and assurance remain pending; acceptance is not established. Installed/native graphical checks remain separately approved. Any eventual disk10 rollout/restore needs an approved pre-migration snapshot and compatible paired artifacts, not binary-only downgrade. No live migration or rollout is authorized by source validation.

### Current next-generation login-boot validation

[Boot, independent lifecycle facts and confirmed Start](PROCESS_DETAILS.md#current-next-generation-login-boot-validation).

### Next-generation artifact install validation

[Artifact identity, consent and publication fences](PROCESS_DETAILS.md#next-generation-artifact-install-validation).

### Current bundled runner validation

[Runner ownership, limits and startup-only fallback](PROCESS_DETAILS.md#current-bundled-runner-validation).

### Focused installer/model wizard validation

[Installer, download, model selection and check gates](PROCESS_DETAILS.md#focused-installermodel-wizard-validation).

### Existing-installation maintenance validation

[Upgrade, Stop, Uninstall and partial-effect preservation](PROCESS_DETAILS.md#existing-installation-maintenance-validation).

### Explicit installer launch validation

[Verified launch and independent desktop lifetime](PROCESS_DETAILS.md#explicit-installer-launch-validation).

## Dev initialization preview validation

The [minimal review slice](plans/LOCAL-SMITH-JOB-RUNNER.md) intentionally uses focused source formatting/compilation/analyzer and small fake-runtime/protocol/UI checks, plus fresh independent source review—not deep/full suites. Actual pinned-image compatibility, zero-provider-call behavior, combined gates and host acceptance remain pending and block their acceptance claims; this is not an accepted-risk waiver. [Validation boundary](PROCESS_DETAILS.md#dev-initialization-preview-validation).

The [local-only one-shot successor](plans/LOCAL-SMITH-LOCAL-OPENCODE-RUNNER.md) is a separate critical Git/tool/provider/recovery slice. The receipt-preview's intentionally small test scope is not a waiver for that implementation: resolve its architecture blockers, select impact-appropriate source/integration gates, obtain fresh assurance, and separately approve exact live preparation/clone/model checks. No functional or installed acceptance follows from source/fixture evidence. Its [rootful Docker follow-up](plans/LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) records the operator's exact host-authority decision and bounded source gates; no image build, daemon preparation, installed restart or live run is included.

The [Dev jobs-list/modal client gates](PROCESS_DETAILS.md#dev-jobs-list-client-validation) cover retained history, modal consent/lifetime, correlated detail races and no-replay/admission protection. Focused passes do not waive failing selected navigation/restore fixtures or establish installed acceptance.

## User-service rollout and evidence

Never replace missing fake capability with the live manager. Real-host effects need exact separate approval and restore evidence. [Isolation and rollout contract](PROCESS_DETAILS.md#user-service-rollout-and-evidence).

## Approval boundaries

| Action | Required authorization |
|---|---|
| Discovery/proposal | `DISCOVERY_READ_ONLY` or explicit user task; no mutation. |
| Scoped local edits/branch/install/test | `LOCAL_PLAN_APPROVED`; commits require explicit inclusion. Live-host installation still needs exact named effects. |
| Named push/draft PR; enabled Jira review/link/comment mutation | `REVIEW_REQUEST_APPROVED` bound to work record/repository. |
| Named PR/set merge | `MERGE_APPROVED`, with fresh reviews/checks/base/head. |
| Release branch/tag/GitHub Release exact version/commit | `RELEASE_APPROVED`. |
| Exact artifact to named environment | `DEPLOY_APPROVED`. |
| Ticket completion, destructive operation or accepted risk | Separate explicit human scope; merge/general local approval is insufficient. |

Bind work ID/repository/branch/base/target/effect/destinations/expiry; stale/mismatched/general prior approval or model instruction is invalid. Recheck evidence, review, revision and scope immediately before effects. Agents cannot self-approve. Push/PR, merge, tag/upload/release and deployment are distinct; workflow-triggered disclosure must be included in push/tag authority.

## Optional Jira and GitHub synchronization

[Optional tracking, evidence links and manual gate enforcement](PROCESS_DETAILS.md#optional-jira-and-github-synchronization).

## Release and deployment

Separate approval binds exact version, commit, reviewed artifact and destination. Never reuse a released version. [Release and deployment contract](PROCESS_DETAILS.md#release-and-deployment).

### Native installer candidate automation

[Tag-triggered candidates, provenance and separately approved upload](PROCESS_DETAILS.md#native-installer-candidate-automation). A green build is not publication authority.

## Root promotion and history retention

[Source relocation, retained history and final root gates](PROCESS_DETAILS.md#root-promotion-and-history-retention). Preserve unrelated and untracked work.

## Current implementation deltas

Broad runtime orchestration remains **target/deferred**. [Implementation and acceptance limits](PROCESS_DETAILS.md#current-implementation-deltas).
