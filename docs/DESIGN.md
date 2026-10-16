# Smith Operator Design

## Status

**Normative target design with labelled implemented source behavior.** [Scope](SCOPE.md) defines supported capabilities, [Architecture](ARCHITECTURE.md) technical contracts and [Process](PROCESS.md#approval-boundaries) authority. Source/widget evidence is not installed graphical acceptance. The promoted root product is text conversation, not the retired Halo/orchestration/voice UI; old stages are in the [pinned prior Design](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/DESIGN.md). [History](HISTORY.md) records relocation/retrieval, not an alternate-product guide.

## Design principles

1. **Visible agency:** what is happening, where, under whose authority, and what is pending.
2. **Evidence before confidence:** passed/failed/skipped/unknown remain distinct; acknowledgement/process exit is not completion.
3. **Intentional approval:** exact effects before action, explicit decline, no inferred authority from conversation.
4. **Recoverable failure:** scope, preserved state, uncertainty and safe next action without secret/raw diagnostic disclosure.
5. **Legible local/cloud boundary:** provider, destination, credentials and processing/retention/charges are factual, not locality or zero-retention promises.
6. **Accessible restraint:** text plus icons/color, keyboard/IME-safe controls, wrapping critical copy and reduced motion; decoration never establishes readiness.

## Next-generation desktop/service lifecycle target

**Implemented foundation:** the service owns accepted work/history/configuration independently of windows. Close/Quit detaches; it does not Stop Smith or terminate already-authorized work. Stop is a distinct intentional lifecycle effect, not observer cancellation or rollback. Clients synchronize instead of repeating setup/replaying requests. Service availability, model/provider readiness and operation outcome are separate facts. Returning provider outages are repair, not new onboarding.

**Target/deferred:** persistent goals, roles/subagents/triangulation and software-delivery workflows. Background chat continuation is not blanket autonomy. Headless/pre-login/post-logout continuity and live logout/reboot behavior are not guaranteed; `smith-next.service`, launcher and storage identities remain unchanged by source promotion. Retired legacy CLI support does not uninstall/migrate old host software.

## Current next-generation login Boot experience

**Implemented source:** minimal centered Smith mark with short live **Starting Smith**, **Connecting to Smith**, **Syncing conversation** status; no sidebar/transcript/composer/chat controls before verified readiness/synchronization. Core read-only observations are serialized≤15s; app synchronization has separate finite budgets. Repair/conflict/unavailable stops decoration with concise guidance/observe again, never waits forever or fabricates a percentage.

Successful presentation uses500ms eased fade-in,≥1s fully opaque hold,500ms fade-out on dark teal. Work starts immediately; slow readiness extends hold without restarting. Hold begins on first opaque frame, not presumed elapsed intro. During cosmetic hold after sync say **Connected to Smith**. Chat/restoration mounts after outro. Reduced motion uses opaque static mark/same timing, no fades. Setting changes/rebuilds never restart it. Repair/confirmation interrupts immediately, opaque/actionable, no intro replay on retry. Presentation timing cannot extend/replace Core readiness authority.

Opening/Retry never installs, enables, starts, downloads, infers or resends automatically. Only exact fresh installed+registered+enabled+safely-stopped state offers **Start Smith**: prepare once→show exact effect→separate confirmation→apply original token once→fresh bounded observation. Disabled/static/missing/conflicting/malformed/unknown states show external repair, not Start or an installer handoff/dialog. Enable remains separately confirmed in installer, never implied by Start. Login Enable targets after-login `default.target`; no `--now`, `loginctl`, lingering or pre-login/post-logout promise.

## Current next-generation terminal-inspired presentation

**Implemented:** native Flutter flat dark teal/cyan, thin borders/small radii, bundled JetBrains Mono (IBM Plex Mono build-time alternative). Shared window header `> SMITH`, last-observed connectivity, Layout16 logical pixels beside status, then New window. No manual refresh/version in header; read-only Settings shows build-derived version with declared-version fallback, otherwise Version unavailable. Native titles remain `smith - local autonomy` / installer `smith - installer`.

One workspace observer first checks30s after Boot readiness and30s after each completed check, including outages/Settings. Healthy checks do not flicker disconnected. This is last-verified app state/transport, not constant monitoring, continuing enablement, model/GPU readiness or completion. Recovery preserves history/drafts/accepted identity, never resends/starts service. Instance changes invalidate model facts; connected alone does not mean Send-ready. Explicit workspace Retry refreshes synchronized app state/inventory.

Chat and read-only Settings share collapsed rail; wide chat uses sidebar, narrow views use accessible modal/anchored history. Selectable inert SMITH/YOU text, no Markdown image/file/network loading, streaming or generated metrics. Transcript viewport spans pane; one8px scrollbar flush right with centered bounded-width messages≥16px clearance. Composer/recovery retain bounded alignment; multiline drafts scroll. Enter sends once when eligible, Shift+Enter newline; IME/composition/repeat cannot submit accidentally, and unavailable Send preserves draft. Settings/drawers/resizing preserve mounted history/draft. Short-height recovery uses reachable scrollable details rather than clipping. Tests across400×400–4200×2400/enlarged text are fixtures, not installed QA.

After Boot outro/preference hydration, shared header/workspaces/chat groups use one620ms paint-only fade/4–8px entrance, not per-message cascade. Layout, editor, reading positions and pane identity stay fixed. Pointer/focus/action semantics wait until settled, with no autofocus/buffered input; Close/Quit outside gate. Reduced motion, restoration warnings and selected-route/actionable recovery settle immediately/permanently. Unselected optional-route unavailability does not suppress healthy entrance. Status/navigation/layout/resizing never restart animation or authorize effects.

## Split-workspace experiment

**Implemented:** Layout selects1×1,2×1,1×2,2×2 (columns×rows), equal tiles with neutral separators/no active outline. One fixed header above two-axis scrollable minimum-size canvas, only workspace bodies repeat. Slots persist while hidden; independent Chat/Settings/selection. Settings hides chat controls, not Layout. No arbitrary split/docking/dev/terminal panes or parallel inference implied.

Below width/text-scale threshold, chat title+chevron opens bounded live saved-chat chooser with revision-checked context menus; New chat(+) and Archived chats directly on header. Archive opens browser, not archive-current. Wide sidebar/rail history remains. Menu selections affect originating visible tile only; stale membership/capability cannot revive through an open dialog. Global menu does not change active pane unless current becomes hidden; focus then moves to visible Layout, not composer. Wrapping/enlarged header, keyboard outline distinct from hover, dismissal/route disposal remain accessible. Synthetic pointers do not prove native window-edge/deactivation closure.

Each tile retains its own unsent blank; saved-chat tiles share authoritative history/draft/queue/operation state but independent reading/editor selections. Clicking/focusing composer activates tile; inactive cannot edit/send and hidden cannot keep focus. Follow bottom by default; scrolling up preserves desired absolute Y through clamping/hiding/resizing. Returning≤1px from bottom resumes following; Send does not override reader lock. Archive/delete repairs all affected selections, including hidden, without deleting unrelated chats/cancelling accepted work. One controller/API/observation/inference owner; additional presentation adds no worker.

**Workspace restoration:** safe local preferences remember layout/active tile/exactly4 Chat-or-Settings/saved-chat-or-null slots including hidden. Restore once after Boot before interaction, never restore focus/drafts/reading positions/dialogs/provisional work/model/service setup. Saved blank stays blank; missing/archived/deleted saved chat becomes independent New chat with warning/repair save, preserving page; do not silently substitute another chat. Transient detail failure keeps identity. Invalid/unreadable preferences are preserved, saving disabled with session-only warning; save failure warns without destroying UI. Writer contention means usable session without saving. No corruption reset/overwrite, resend/resume or readiness promise; abrupt death can lose latest preference.

## Session-only New window

**Implemented:** header icon tooltip **Open a new window — layout isn’t saved.** Directly launches current resolved bundle with fixed `--session-only-window`, detached/no shell. Pending disables only that action; sanitized error needs deliberate retry. Spawn success is not visible-window/service-readiness proof. Additional processes start default1×1/Chat with ordinary Boot selection, not cloned layout, and never load/save/lock/adopt preferences even after original closes. They can open further session-only windows.

Saved service conversations are shared/durable; per-process drafts/local follow-ups/Boot/observations/exit guard remain independent, without cross-window unsent synchronization. Closing one neither closes another nor stops accepted work. Session-only describes presentation, not ephemeral chat history. No monitor detection/placement, geometry persistence, shortcut or Close all. Native lifetime/multi-monitor/IME remains separately unverified.

## Current multi-chat interaction

**Implemented:** New chat reuses an unsaved blank; typing/navigation creates no durable empty chat. First accepted Send atomically creates chat+job and deterministic title: controls removed, whitespace collapsed,40 Unicode scalars (fallback New chat), without altering prompt. Later messages do not rename. Migration uses Current chat. Renames/new display use40 scalars (not graphemes); old valid longer titles/receipts remain unchanged/readable. Rename exposes original over-limit text for explicit edit, never silently truncates stored value. Per-chat context is isolated; saved local default/config remain global, future-send route selectable per chat.

At most4 outstanding chat requests,1 per chat,1 Running globally/others Queued. Unknown admission is not proven Running/Completed; capacity rejection keeps draft. Eligible Send immediately adds volatile request-keyed user receipt, clears only captured draft and shows Sending→observed Queued/Thinking/static accessible status. Newer draft stays intact. Success replaces progress; failure/unknown stops it and preserves recovery, no resend. Saved history replaces local receipts without duplication; local receipt is not admission/durability proof. Scroll anchors follow reader intent, not forced jumps.

Active rows Rename/Archive/Delete; bounded Archived browser Reactivate/Rename/Delete. Revision/identity-bound dialogs revalidate on dispatch; stale fails, never applies to new content. Rename can proceed with fresh metadata during generation. Target queued/running/uncertain work blocks archive/delete, not unrelated navigation/send. Archive reversible and preserves process-local draft. Delete exact-target confirmation says current history/results removed/cannot undo, backups kept; it is logical deletion, not secure RAM/media/provider/backup erasure. Deletion discards target local state; selected/hidden panes repair to preserved blank or independent blank. Reactivate need not select. Tombstone/capacity refusal is no partial delete.

Accepted jobs/history outlive client; service restart interrupts unfinished work, no automatic replay. Boot synchronizes, not waits for generation. Poll errors retain original ID. Verified terminal reply stays separately visible if saved-history sync fails; missing user text is not invented, already-visible reply not duplicated. Later sync replaces provisional view. Expired request resolution is confirmed local forgetting with unknown outcome/duplicate-risk warning, not retry/cancel. Bounded retention is not unlimited archival memory or shared cross-chat memory. Forced desktop death can lose unsent state.

## Saved message timestamps

**Implemented:** saved SMITH/YOU labels show service-recorded workstation-local `YYYY-MM-DD HH:mm:ss ±HH:mm`, offset evaluated at event, wrapping at narrow/enlarged widths. Accessibility/tooltip says **Accepted by Smith** for YOU, **Reply completed by Smith** for SMITH. Reopen shows saved instant, never observation time.

Old unknown dates, local waiting/Sending/Queued/Thinking/error/provisional receipts stay undated. Optional timing failure must not hide history or resend. Clock corrections do not reorder text. Differences are approximate wall-clock end-to-end including queue/overhead, not model-only latency/throughput/task-success; deltas can reverse. No timer/dashboard/durable unsent queue is added.

## Client-side queued follow-ups

**Implemented:** busy chat offers **Queue**, not steering; Enter/numpad Enter once with existing IME/repeat/Shift safeguards. Captured immutable text waits in this desktop and uses freshly synchronized predecessor results. Up next/details distinguish **Waiting locally—not sent**, Sending, accepted work, blocked failure, unknown outcome; no empty shelf required. Queue updates preserve per-view reading anchors.

Each head advances only after exact predecessor success and synchronized saved history. Definitive failure/not-sent blocks successors with explicit **Retry queued message** (new attempt/same logical position) or **Skip and continue** (abandon, not success/cancel). Unknown/running cannot retry/skip. Pause/revision-checked Resume/Remove/confirmed Clear affect future local work only. Removed dependency pauses remaining chain; observation recovery/forgetting expired ID never clears barrier silently. Instance/config/foreign history requires explicit review; route change cannot silently redirect captured text. Eight local follow-ups/chat,32/desktop plus current-predecessor slot,16KiB each; never-dispatched waiting has no execution timer/service slot.

Normal Close/Quit holds every new dispatch before asking **Keep open** or **Discard unsent text and close**, deduplicating hidden/shared chats/blanks. Cancel preserves queue/pauses; failed native/Dart exchange keeps window open. Accepted service work may continue; forced kill/crash can lose unsent text. Edit/reorder/durable outbox/active cancellation deferred. Model reply success is not successful tests/build or deployment approval. Generic Resume is not consent to a new cloud destination; use explicit **Review queued cloud send** where required.

## Current next cloud-provider experience

**Implemented:** unchanged local installation/Model flow then optional Providers, independent OpenAI API/Anthropic API, either/both/neither. Rerun Manage providers on setup/result paths performs no artifact/lifecycle work. CLI `smith-installer --providers [--bundle ROOT]` is separate, cannot mix artifact/model/dry-run/yes. Optional skip/failure does not undo verified local effects. API billing/access is not ChatGPT/Claude subscription login.

Opening/typing/selection/cached refresh/Skip/Close writes no credential/calls no vendor. Save/Replace (local keyring) and Validate (one fixed synthetic no-history request/no turn, possibly billable) have separate original-token confirmations. Show saved/unvalidated/exact model validated/locked/unavailable/failed/unknown independently. Success covers one model+generation, not every model, future access, spending cap or zero retention. Exact15 compiled model labels/IDs come from retained [API contract](plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md), not illustrative names/discovery.

Keys only in masked GTK/hidden bounded terminal input, not chat/flags/environment/logs. Existing unlocked same-user GNOME Keyring required; no install/activation/unlock/Prompt or plaintext fallback. Replace creates fresh generation retaining previous items; no P0 removal. **Uninstall keeps keyring items and does not revoke API keys.** Uncertain Save can leave inactive items: preserve/observe original ID, never repeat secret apply/adopt orphan or imply rollback. External keyring/vendor management is separate operator work.

Each composer has grouped LOCAL/CLOUD picker: observed local store/compiled cloud models with factual unconfigured/unavailable/not-individually-verified facts. Choose model dialog filters cached label/ID/provider/group with trimmed case-insensitive substring, no service/keyring/vendor access. Current selection stays independently shown when filtered out; clearing/typing/opening/Cancel/Escape/current selection has no route/draft/queue/send effect. Enter in filter never selects/sends; dismissal returns focus. Actual enabled choice revalidates active originating chat/instance/revision/generation; stale dialog cannot retarget another chat.

Fresh chats start saved local default. Selection is volatile/future sends only, including while prior accepted work retains original label/context. Reattach route hint is not lock; unavailable historical model preserves history/selection and allows explicit alternative in same chat. No forced new chat/Branch/Compare/substitution/fallback. Settings stays read-only; installer owns provisioning/local default/keys.

Before first transfer per chat/provider/generation, freeze exact destination/model/context counts and disclose possible charges/processing/retention; offer **Send / Cancel / New chat**. Generation is not verified account. Consent retained for later explicit sends including intervening retained local/other-provider history; same-destination model change no new prompt, replacement generation does. New chat optional/no auto-submit. Text/route/context change invalidates confirmation, never silently rebinds. Explicit sends still require fresh preparation/deadlines. Local deletion cannot remove vendor records; remote unknown may have processed/billed. Close/observation Stop is not service/remote cancellation.

Boot/automatic cached observation never probes keyring/validates remotely. Explicit Retry may observe saved keyring facts, never infer or resend. Matching active saved generation+supported model permits Send/Validate **without** mandatory probe/current-instance observation/prior validation. Cached locked/missing/failed checks advise; execution retrieves exact admitted key/rechecks before HTTP and fails closed if inaccessible. No saved generation stays disabled. Only explicitly validated exact model is marked individually verified; ordinary chat success invents no check. Unresolved/uncertain requests retain reservations/observation identity. Missing optional credentials leave history/local routes usable. One attempt/no automatic retry/fallback; cloud8192 total tokens including reasoning/180s is not guaranteed completion or cost.

## Current native model and focused installer experience

**Implemented:** separate native package at root `installer/dist/`, one paired app/service. Landing **INSTALL / UPGRADE**, **UNINSTALL** only; hidden read-only preview, selected flow **Checking Smith**, one focused page/primary and at most one secondary current-action control. No normal Logs/Details control: bounded internal diagnostics remain, but exact effects, blockers, uncertainty, preservation and safe next action must be visible. Successful low-level noise stays hidden. Dark teal/cyan/JetBrains Mono/icons are presentation only; grouped consent deferred.

Fixed setup recipe separately confirms artifacts→register/reload→Enable→Start→Model/check→optional Providers. Enable affects login selection only, Start runtime only, no lingering. Every needed effect retains original60s request/token, no silent renewal. Initial dry-run/decline writes no destination/lock or manager/provider work; later decline preserves earlier effects. CLI consent≤4096 bytes/60s, EOF/interrupt/timeout no approval; no recipient blanket yes. Bounded progress output supports pipes/sockets/terminals, not regular-file/device redirection; filesystem/fsync/artifact stdio have no global deadline. Incomplete output is not success/rollback.

Fresh GUI default Qwen3-14B Q4_K_M9001752960 bytes≈8.38GiB weights, reruns preserve selection. Seven pinned Apache-2.0 choices: Compact Qwen2.5 1.5B/SmolLM2 1.7B/Qwen3 4B; Balanced Mistral 7B/Granite 8B; Large Mistral Nemo12B/Qwen3 14B. Unsupported saved choice stays inert/read-only, never enters chooser/consumes submission. No arbitrary name/path/import. Preselection/opening is not consent; exact model/revision/size/SHA-bound download, checked preference, service-backed fixed greeting and permanent removal each confirmed separately. CLI lists `--models`, uses exact `--model`, `--download-model` (`--pull-model` alias), optional `--verify-inference`, no implicit CLI default. Full recipient commands still review artifacts/lifecycle, not model-only maintenance.

Download shows actual byte counts/coalesced latest numeric progress; hashing/loading/inference indeterminate/elapsed, never invented total. Stage/confirmation/result boundaries discard stale telemetry; critical messages stay ordered/immediate. No retry/resume/deadline extension/cache deletion. A failed/cancelled transfer can leave staging. Model-default thinking remains; complete nonempty final only, reasoning-only/truncated/timeout fail. Check saves no turn, proves neither GPU nor performance. Local runtime is bundled CPU/Vulkan, no external Ollama or OS/driver provisioning.

Manage models reachable from setup/review/results, not landing, independent of failed/expired artifact preview but still needs compatible verified service and own approvals. It bypasses publication, never differing-binary/identity safeguards. Selected model removal blocked: explicitly select another installed one first; any loaded runner incl idle may block until normal600s unload, no hidden Stop/unload. Destructive service-mediated removal preserves partial/orphan data, not rollback/history/preferences deletion. Model-only success says Model check/removal complete; install completion enumerates verified effects; failed check says incomplete, not installation verified.

Cancel/Close invalidates consent and joins installer-owned worker; accepted service work may continue, no auto-resend. Sticky process cancellation requires close/reopen before another session. Neither installer persists in host command path; keep complete extracted package. No app-side Open installer/provisioning/configuration dialog; external repair then observation-only Retry. Historical voice data remains inert/readable, never re-executed/reset.

## Existing-installation installer actions

Upgrade independently checks older managed release/helper/launcher/unit identities without executing old helper. Ordinary Install/Make refuse differing service bytes, no force/yes maintenance bypass. Review old/candidate identities and private preservation, separately confirm Stop/interruption and applicable register/reload/Enable/Start. Only exact static→current unit migration; exact current enabled preserved without churn, exact disabled separately enabled. Foreign/modified/override/unsafe/unknown or unproven cgroup/socket quiescence blocks. Declined/failed later stage keeps accepted effects; artifacts upgraded/service stopped differs from verified Core/app running and model readiness. Upgrade downloads no model; choose management explicitly afterward.

Uninstall exact confirmation: Stop/quiesce→disable/verify→unregister/reload/verify→preserve active menu/command/current/service away from active names. **Keep ALL conversations, preferences, models, releases, backups, stages and preserved objects**; retained objects consume disk. Candidate payload not required. No purge/recursive deletion/rollback/retry, lingering or legacy `smith.service` mutation. Partial/unknown result explains preservation and safe inspection. Runtime confirmations authorize displayed effects only, not agent live deployment/destructive scope.

## Explicit installer desktop launch

Verified joined-worker successful Install/Upgrade offers **Launch Smith** beside Close; not default/automatic. No Launch for Uninstall/model-only/cancelled/incomplete/uncertain. Click asynchronously validates installed selector/manifest/executable and dispatches installed release, not package payload/wrapper. Pending/duplicate launch blocked; pending Close waits dispatch outcome only, then desktop independent. Inline sanitized failure permits deliberate retry without changing install result/exit status. Launch requested means process dispatch, not visible window/service/model readiness. Closing installer does not kill launched desktop; no new service/model effect.

## Current installer visual design

Flat shared palette/embedded fonts/icons/native Cargo version; wrapping models/scrolling body/adaptive footer preserve controls under narrow/enlarged text. Exact confirmations shown once with effect-specific buttons, never preparation-as-approval or Confirm results. `--licenses` prints full notices display-free, not library-free. Resource failure before worker is visible, no font install/disk/network fallback. Fixture captures are not live accessibility/redistribution clearance.

## Acquiring an installer candidate

**Implemented workflow configuration, not hosted/release acceptance:** PR/main validates without retained distributables. Only eligible annotated `release/v<version>` after gates prepares Actions artifact `smith-installer_v<version>`, full same-name tar, archive checksum/build record/log/Dagger version,14-day retention. Source/target/digest and pending publication/install approval must be visible; failed gates offer no successful candidate. Green job is not a released/installed product; upload itself is separately approved disclosure.

Acquire full artifact, verify archive SHA against **independently trusted** approval/provenance (beside-archive checksum alone is not authenticity), extract into fresh directory, check inner `SHA256SUMS`, then explicitly run native GUI/CLI. Keep whole package for repair; no lone binary/source archive/recipient Make/Python/Dagger. Prepared Ubuntu24.04 x86_64 native libs/GTK/session/user manager/safe roots remain; no weights/OS/GPU provisioning. Current source guide: [recipient instructions](../dagger/INSTALLER.md). Missing/tampered/incomplete package blocks, never automatic reinstall/retry/fallback. Root relocation does not inherit old pipeline passes.

## Operator workflow

**Target/deferred delivery UI:** bounded objective/work record→visible independent analyses/reconciliation→scoped implementation→self-check/assurance/adversarial evidence→await exact human signoff→only approved effect→retain result. No delivery state inferred from chat prose, FIFO completion or an acknowledgement.

## Task states

| Target stage | Meaning |
|---|---|
| `new` | Intent accepted, not investigated. |
| `discovery` | Facts/constraints/risks gathered. |
| `preproduction` | Independent analyses/plan reconciled. |
| `production` | Scoped implementation/deterministic validation. |
| `review/signoff` | Evidence/review/human decision or remediation. |
| `completion` | Approved outcome finished with retained evidence. |
| `failed` | Stopped with cause/preservation/recovery path. |

This is future delivery vocabulary, not a claim the text service executes tasks. One semantic meaning across future CLI/bridge/UI; generated text/process exit must not advance it.

## Approval experience

Signoff must show work ID/repo/branch/base/target/diff/files, exact push/PR/merge/tag/release/deploy/destructive action, all destinations/side effects, self-check and independent/adversarial verdicts, failed/skipped/unknown checks/risks, compatibility/rollback, human approver/scope/expiry and explicit approve/decline. Missing/stale revision binding blocks approval. New commit/plan/effect/expired window needs new signoff; no self-approval. Exact command/token setup consent is not general Git/publication authority.

## Critical operator flows

Blocking disagreement retains options/owner; failed/missing evidence cannot offer release; injection suspicion identifies untrusted input/stops privileged progress; restart shows interrupted/stale/unknown rather than claimed continuation; decline preserves work/stops effects; partial cross-repo action exposes compatibility/merge order. Unknown stays unknown. Safe recovery never deletes retained state or replays accepted work to clear a warning.

## User-service operator flow

Keep installed/loaded, enabled at login, running and bridge/provider facts separate. Current actions are installer-scoped artifact publication/register/reload/Enable/Start, explicit maintenance Stop/Uninstall and exact eligible desktop Start—not retired `smith settings service` controls. Enable does not start; Start does not enable; neither changes lingering. No unmanaged spawn on manager failure, no unrelated PID/port kill or stale-socket deletion. Show conflict/unknown as block and preserved partial effects, not absence/success. Any future Stop UI must disclose active-work effects; observation Stop is not that UI.

## Accessibility and local operation

Text labels accompany color/icons; controls keyboard/IME-operable, critical actions reachable with≥48-logical-pixel targets, wrapping/enlarged text and narrow scrollable layouts. Long work shows factual stage/elapsed/known progress/cancel-or-observation meaning. Concise actionable errors/approvals must not require raw model/native diagnostics. Primary Flutter and secondary native CLI must preserve evidence/authority meaning. Voice remains deferred, not hidden toggle. Native window manager/IME/accessibility/fractional-display acceptance and real recipient/provider/model/lifecycle behavior remain separately approved gates, tracked in root `VALIDATION.md`.

## Design change triggers

Update canonical design for task/approval/recovery/privacy/bridge/accessibility changes; preserve current service authority, no-replay, consent and unsent-text safeguards across visual simplification. A prettier/smaller UI must not hide blocking uncertainty or expand effect scope.
