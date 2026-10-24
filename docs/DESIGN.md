# Smith Operator Design

This is the short entry page. [Design details](DESIGN_DETAILS.md) retain the full authoritative interaction and recovery requirements; this summary does not replace or relax them. Follow only task-relevant detailed sections, alongside [Scope](SCOPE.md), [Architecture](ARCHITECTURE.md) and [approval boundaries](PROCESS.md#approval-boundaries).

## Status

**Normative target design with labelled implemented source behavior.** Source/widget evidence is not installed graphical acceptance. [Status and historical boundaries](DESIGN_DETAILS.md#status).

## Design principles

Make agency, evidence, exact approvals and safe recovery visible. Preserve local/cloud privacy distinctions, accessible controls and truthful uncertainty. [Full principles](DESIGN_DETAILS.md#design-principles).

## Next-generation desktop/service lifecycle target

**Implemented foundation:** the service owns accepted work; closing the desktop detaches. Reconnect never replays or provisions. **Target/deferred:** autonomous delivery. [Lifecycle contract](DESIGN_DETAILS.md#next-generation-desktopservice-lifecycle-target).

## Current next-generation login Boot experience

[Readiness, bounded observation, repair and separately confirmed Start](DESIGN_DETAILS.md#current-next-generation-login-boot-experience). Opening the app grants no setup authority.

## Current next-generation terminal-inspired presentation

[Shared shell, status, keyboard input and paint-only entrance](DESIGN_DETAILS.md#current-next-generation-terminal-inspired-presentation). Presentation never establishes readiness or permissions.

## Split-workspace experiment

[Four retained panes, shared controller, reading positions and safe preferences](DESIGN_DETAILS.md#split-workspace-experiment).

## Session-only New window

[Independent unsent state and no preference-store access](DESIGN_DETAILS.md#session-only-new-window). Session-only does not mean ephemeral saved history.

## Current multi-chat interaction

**Implemented:** isolated bounded histories; four outstanding requests, one per chat, one executing globally. Unknown outcomes remain observation-only. [Chat lifecycle and recovery](DESIGN_DETAILS.md#current-multi-chat-interaction).

## Projects summary and settings

**Implemented Projects-only editor source; validation, assurance and graphical acceptance pending:** all-project list without search or routine Refresh, no redundant outer Projects title row, local Projects heading to the left of a plain Create button, ID/Description, expandable repository ID/URL/Read Only, draft Duplicate and guarded footer Save/Cancel with project overflow. Normal Prompt and branch/type controls stay hidden without dropping their saved values. Top-left alignment, footer actions and hidden-Name recovery are present in source; the [UI QA closeout](plans/LOCAL-SMITH-PROJECTS-UX.md#ui-qa-closeout) records focused checks, not installed acceptance. No extra project Name or Agent/OpenCode UI; Agents, navigation and theme stay untouched. Read-only is inert metadata, not security enforcement; no project-context injection, Dev runtime/bootstrap or repository access follows. [Editor, preservation and uncertain-write recovery](DESIGN_DETAILS.md#projects-summary-and-settings).

## Agents definitions and Dev navigation

**Implemented temporary navigation override:** Agents entry points are hidden in the wide and compact sidebars, controlled by one private shell constant. Agents code, saved profiles, existing/restored routes and retained editors remain intact. Dev/Run logic is unchanged; its agent selection is temporarily hidden as described below. [Bounded change](plans/LOCAL-SMITH-HIDE-AGENTS-NAVIGATION.md).

**Implemented source, not graphical acceptance:** Chat / Dev → Projects, Agents / Settings. Dev selects the minimal [initialization preview](DESIGN_DETAILS.md#dev-initialization-review-slice); wide panes have a top-anchored labelled sidebar with the48px disclosure integrated into the Dev row, not a separate navigation item. The root selects Dev while the trailing chevron changes child links only. The selected surface spans that entire row with a left accent; Chats/Dev/Settings share an icon/text grid, indented children share their own grid, and Settings is separated below the group. Compact panes retain the same hierarchy in a104px icon-only sidebar, with separate Dev and disclosure buttons and inline indented children. Disclosure is per-pane/session-only, initially collapsed; collapsing links leaves an active child page intact. Opening Dev reads saved selections/job facts only; execution requires its explicit scoped Start action. [Navigation contract](DESIGN_DETAILS.md#dev-navigation-and-placeholder). Projects retains its route identity; its fixed OpenCode field is labelled **Execution harness**, metadata only—not installed/readiness evidence. Agents uses fixed compiled Material code/shield icons, with no custom icons or new asset contract.

**Agents implemented source, not graphical acceptance:** a small AGENTS list heading with adjacent NEW AGENT, no Search, normal Refresh or outer Agents header. A wide list/editor or compact Back/Return supports 2–32 profiles: permanent built-in current saved names/descriptions are read-only, prompts editable; customs edit all three fields. New opens a blank draft with Save/Cancel. Custom name/optional description are≤40 Unicode scalars; exact prompt≤4096 UTF-8 bytes. Valid older built-in descriptions remain readable without truncation. Generic fixed custom icons add no asset contract. No delete/duplicate/model/harness/Run or automatic Dev start follows. [Detailed interaction contract](DESIGN_DETAILS.md#agents-profile-editor).

Loading, unavailable, dirty, saving, stale and unknown outcomes remain distinct. Dirty guards, explicit origin review, keyboard/IME,48px targets and retained focus/scroll remain. Newer typing survives acknowledgements; create binding requires the original request's receipt ID and authoritative list. Unknown or acknowledged-but-missing/mismatched creates block writes/New against duplication; Resolve queries only, never resends. Contextual Retry loading is limited to unavailable/stale observation recovery. Per-view drafts/selection/scroll remain volatile; hidden focus stays excluded.

Preferences write v4 and strictly read v1/v2/v3/v4: v3 adds Agents, v4 adds the Dev root. Older versions retain their exact destination allowlists; old valid records are not rewritten solely on load. No disclosure state, agent selection, prompt or draft is stored. Session-only windows still never access preferences. [Storage and recovery contract](ARCHITECTURE_DETAILS.md#agents-definitions-registry) and [approved custom Agents slice](plans/LOCAL-SMITH-AGENTS-CUSTOM.md). Combined gates and fresh assurance remain pending; focused UI/controller results are not acceptance.

## Dev initialization review slice

**Historical receipt-preview UI override:** the saved-agent selection was hidden without changing its old admission requirement. This is not the current local-runner workflow below.

**Historical receipt-preview source, graphical/runtime acceptance pending:** saved project, saved agent and exact task prompt feed one confirmed receipt-only sandbox start. Its form/current-result presentation preceded the local-runner list—not a board. Ready, dispatching, recorded, unavailable and unknown remain distinct; ambiguous admission retains its original identity and Resolve queries only. No model response, coding or automatic setup is implied. Fixed prerequisite diagnostics explain new failures without exposing private runtime text, implying safe retry or rewriting historical results. [Exact limitations and recovery](DESIGN_DETAILS.md#dev-initialization-review-slice).

**Local-runner source, graphical/runtime acceptance pending:** Dev now shows [retained Pipeline jobs with statuses](plans/LOCAL-SMITH-DEV-JOBS-LIST.md). **New job** opens a project/prompt/local-model modal with Edit → Review → explicit Start; activating a row opens request-specific details and a selectable saved reply. Closing either modal is not Stop; drafts remain volatile and cleanup/admission uncertainty stays visible. A three-container FIFO queue is implemented in source: excess jobs show Queued and reviewed submission remains available while other jobs execute. No Stop/reconciliation, model picker, Agent dependency, cloud switch or automatic setup follows. Repository-download/local-inference consent and reply/cleanup distinctions remain unchanged. The [rootful follow-up](plans/LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) keeps Docker operator-provided and uses fixed rootful-specific prerequisite messages without altering historical rootless results, uncertainty or no-replay rules; no installer or repair UI is added.

**Retained read/write source foundation, not functional acceptance:** the existing Dev surface also exposes a minimal Read/Write job form and attempt-history details with Follow-up, Finish and Delete. Runtime-dependent actions remain unavailable until the retained runtime is prepared; no local-model readiness or fallback is substituted. Known failed attempts can continue in a retained environment; unknown execution stays observation-only. Original-operation receipt recovery never resends, and clears only an unchanged submitted draft after proof. Finish disposes the environment while retaining history; neither Finish nor Delete merges or authorizes publication. The full phase-based workflow UI is deferred. [Source/lifecycle contract](ARCHITECTURE_DETAILS.md#retained-readwrite-job-foundation--source-only).

## Saved message timestamps

[Service admission/completion times, unknown historical dates and accessibility](DESIGN_DETAILS.md#saved-message-timestamps). These are not model-only latency measurements.

## Client-side queued follow-ups

**Implemented:** volatile follow-ups wait for predecessor success and synchronized history. Close warns about unsent text; accepted service work may continue. [Queue and recovery safeguards](DESIGN_DETAILS.md#client-side-queued-follow-ups).

## Current next cloud-provider experience

**Implemented:** explicit LOCAL/CLOUD selection and destination-bound context consent; no automatic retry/fallback. Keys stay outside chat; Save and potentially billable Validate are separate. [Provider, consent and recovery contract](DESIGN_DETAILS.md#current-next-cloud-provider-experience).

## Current native model and focused installer experience

[Separate setup/model confirmations, progress, cancellation and preserved partial effects](DESIGN_DETAILS.md#current-native-model-and-focused-installer-experience). No automatic download or inferred success.

## Existing-installation installer actions

[Upgrade, Uninstall, identity checks and retained data](DESIGN_DETAILS.md#existing-installation-installer-actions). Uninstall preserves user data and does not revoke API keys. Local Make exports now surface unsafe package-path permissions before building/launching, without automatically changing source-directory permissions.

## Explicit installer desktop launch

[Verified Install/Upgrade launch offer and dispatch semantics](DESIGN_DETAILS.md#explicit-installer-desktop-launch). Launch requested is not readiness proof.

## Current installer visual design

[Accessible native presentation, confirmations and resource failures](DESIGN_DETAILS.md#current-installer-visual-design).

## Acquiring an installer candidate

[Candidate provenance, full-package verification and explicit installation](DESIGN_DETAILS.md#acquiring-an-installer-candidate). A green job is not release or deployment approval.

## Operator workflow

**Target/deferred delivery UI:** [bounded work through independent review and exact human signoff](DESIGN_DETAILS.md#operator-workflow), not a capability granted by chat prose.

## Task states

[Future delivery vocabulary and state meanings](DESIGN_DETAILS.md#task-states). A reply or process exit does not establish task completion.

## Approval experience

Expose exact scope, revisions, effects, destinations, evidence, risks and approve/decline. Stale binding blocks action; no self-approval. [Full signoff contract](DESIGN_DETAILS.md#approval-experience).

## Critical operator flows

[Disagreement, failure, injection suspicion, interruption and partial outcomes](DESIGN_DETAILS.md#critical-operator-flows). Recovery must not erase state or replay accepted work.

## User-service operator flow

[Distinct installed, enabled, running and provider facts](DESIGN_DETAILS.md#user-service-operator-flow). Enable never starts; Start never enables.

## Accessibility and local operation

[Keyboard/IME, text labels, reachable controls, narrow/enlarged layouts and separate native acceptance](DESIGN_DETAILS.md#accessibility-and-local-operation).

## Design change triggers

Update the matching detailed contract when task, approval, recovery, privacy or accessibility behavior changes. [Safeguards for simplification](DESIGN_DETAILS.md#design-change-triggers).
