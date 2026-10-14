# Smith text client

## Current bundled native runner

**Implemented source:** [native-runner plan](../../docs/plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md) supersedes external Ollama setup/provider descriptions from earlier stages below for production `next/` only. Smith bundles separate CPU and Vulkan llama.cpp runtimes in the paired client release; no external Ollama is required. Weights remain separate. List choices with `smith-installer --models`; use `--model EXACT --download-model` (`--pull-model` alias) with separate exact-name download, preference and optional inference consent. The focused visual installer supplies model provisioning/service-backed checks/removal; Qwen3-14B is fresh-only preselection, preserving existing saved selection. Neither installer is persistently installed: retain the full package.

Runtime/model verification, private Unix IPC, one runner lease, bounded context and service-owned cancellation/cleanup are described in canonical Architecture. Production loads the held verified model FD; app-owner teardown cancels/joins native inference, while desktop close only detaches. No implicit upgrades, retries, cloud/tools, bundled weights or real-inference/real-GPU/clean-host/release guarantee. The service currently attempts fixed full offload on `Vulkan0`; classified startup failure may fall back once to independent CPU after confirmed cleanup. No driver discovery/provisioning or acceleration claim. Unused installer Ollama code remains legacy library/test coverage, not a production fallback. See [validation history](../VALIDATION.md) for observed checks.

**Implemented source — T1:** [LOCAL-SMITH-NEXT-TEXT-RESET](../../docs/plans/LOCAL-SMITH-NEXT-TEXT-RESET.md) replaces the active wizard/voice interface with one conversation-first dark desktop screen. [LOCAL-SMITH-NEXT-LOGIN-BOOT](../../docs/plans/LOCAL-SMITH-NEXT-LOGIN-BOOT.md) now gates that existing screen behind a minimal observation-only Boot surface while the login-started service becomes ready and authoritative conversation state synchronizes. There are no Core Home, setup stages, install/register/finish, microphone, playback or voice controls. The preserved voice source is excluded reference in `reference/voice-chat-v1/`, not a runtime dependency.

## Connection and model readiness

Launch performs finite serialized read-only Core observations for up to 15 monotonic seconds. Chat attachment requires `ok=true`, installed binary/unit, running service and verified ready bridge; conversation synchronization completes before the chat widgets mount. Strict control schema 2 also requires independent `enablement_state` (`missing`, `disabled`, `enabled`, `conflict`, or `unknown`). The `setup_complete` field remains strictly decoded for compatibility but **neither grants nor denies chat readiness**. Conflict, malformed, failed or unavailable identity facts block app attachment. Native managed executable/unit/MainPID/peer/inode verification remains in the helper.

A verified connection reads authoritative app state/history and queries any outstanding request before new inventory work. One bounded asynchronous installed-model inventory observation runs per attachment or explicit Retry; no inference probe or automatic retry occurs. Inventory is not inference verification. Sending requires a selected model present in that observation and a ready bundled-runtime observation. If unselected or missing, Send stays disabled with concise guidance to configure a model through the separate installer, then Retry. The app never selects a default or saves model configuration. No default/invented model, download, provider install/start or model provisioning is available in the app.

Boot is a full-screen rounded vector loop with one traveling segment and one short live status; reduced motion shows a static mark. Missing, disabled, conflict and unknown lifecycle facts get concise external repair guidance. Only an exact installed, registered, enabled, safely stopped service with an unavailable bridge can expose **Start Smith** after bounded observation. That button uses one `prepare('start')` token, an exact visible confirmation and one `apply('start', token)`, followed by fresh observation. The client recognizes `enable` as a schema action but never invokes it. No lifecycle action is automatic.

After chat mounts, repair stays compact: factual status text and observation-only **Retry**. Retry never submits/resends drafts or service work. Saved history and draft remain visible during outage, explicitly labeled stale/unavailable. Typing remains possible while disconnected; Send is disabled until readiness is observed.

## Minimal chat and external setup

**Implemented source:** [minimal-chat plan](../../docs/plans/LOCAL-SMITH-NEXT-MINIMAL-CHAT.md) removes the installer instructions dialog/button and model chooser. The neutral dark chat opens immediately with independently scrollable, selectable history and a fixed rounded multiline composer. No setup, install, voice, speech, model-save or terminal-launch action is exposed. Use the separate native installer package for provisioning and model preferences; return to chat and Retry to synchronize external changes. Connection verification and send gates remain mandatory. Exceptional expired-request confirmation remains a local recovery action, not provisioning.

## Source layout

```text
lib/main.dart           Boot-gated shell and existing conversation screen
lib/boot.dart           Typed Boot coordinator, bounded observation and vector surface
lib/text_chat.dart      Correlated text controller and bounded polling
lib/chat_panel.dart     Fixed editable composer and scrollable selectable history
lib/control_api.dart    Strict schema-2 Core contract and known lifecycle actions
lib/native_control.dart Canonical helper discovery and dedicated bounded inspect cleanup
lib/app_api.dart        Five-operation app API and 4 MiB stdin executor
linux/                  Flutter Linux runner and embedded icon
scripts/                Native tests and paired client build
test/                   Fake API/process, protocol, text, layout and Rust fixture tests
```

The Core prepare/apply management API is retained as a native contract. The application invokes only explicit `start`, and only through the bounded Boot eligibility and confirmation flow; install, register, enable and setup completion remain external. Active audio source/tests remain excluded after extraction of generic text safety tests.

## Text and durability boundary

Only `state`, `models`, `configure`, `submit` and `poll` are admitted by `NativeAppApi`. Configure rejects non-null voice; submit requires `output:"chat"`. App requests use schema-v1 envelopes and 32-lowercase-hex correlation IDs over stdin with helper argv exactly `['app']`; prompts never enter argv. Payloads are bounded to 4 MiB excluding newline. Core frames/caps remain unchanged.

The text controller serializes operations, preserves request IDs for unknown/malformed acknowledgements and finite poll timeouts, and queries them without replay. Polling has a 210-second budget; expiry of that budget is not cancellation. Instance changes synchronize and query retained work against the new instance; they do not erase pending identity or authorize resend. Reopening reads history/active work, including compatible legacy work, without invoking retired operations or recreating audio. Edits made during submission are preserved by draft revision tracking. History is replaced only by authoritative state reads; polling never appends synthetic turns. Verified terminal answers are retained separately by request ID until a later state read succeeds, even when reattached work has no locally captured user text. If the follow-up state read fails, the answer remains selectable as **Verified answer · awaiting history sync**, without inventing a missing user message. When the same answer text is already visible in saved history, the provisional status references that text rather than rendering it twice; text equality does not establish request identity. Successful synchronization clears the provisional view and shows only the authoritative bounded history.

`REQUEST_EXPIRED` retains pending identity until a confirmed **Forget request** action explains unknown outcome and duplicate risk. Forgetting changes only local metadata, preserves draft/history and neither resends nor cancels service work. Other unknown requests cannot be forgotten through this control. Service storage remains bounded to 16 turn pairs/64 KiB context with bounded jobs/tombstones; it is not unlimited archival history. Closing the UI disposes short-lived helpers and polling, not the independently managed service or already-authorized work.

## Native helper safety

The helper is `<actual desktop executable directory>/smith_backend/smith-service`, or an explicit absolute compile-time `SMITH_SERVICE_EXECUTABLE` developer override. Relative overrides fail closed; no PATH/cwd search or shell fallback exists. Before execution, the adapter rejects non-Linux, noncanonical/symlink/nonregular paths, group/world-writable executable paths or ancestors, nonexecutable files and non-x86_64 ELF headers. These are local safety checks, not hostile-same-UID authentication.

Mutation helpers retain the 45-second budget, combined Core stdout/stderr cap of 16 KiB and separate app output cap. Boot inspection uses a dedicated two-second helper path with one-second TERM grace and one-second KILL confirmation; the coordinator reserves that cleanup bound inside its 15-second monotonic total. Every observation remains serialized and awaits captured cleanup before another begins. Unconfirmed cleanup retains ownership and blocks reuse. Late launch results are cleaned. No daemon is adopted or stopped by desktop disposal. Existing canonical-path, malformed-contract, operation-bound and native cleanup regression tests are retained.

## Validation and builds

From `next/client/`:

```sh
flutter pub get --enforce-lockfile
# Explicit already-compiled candidate; tests use only private temporary fixtures.
SMITH_TEST_SERVICE=/absolute/path/to/smith-service sh scripts/test.sh
sh scripts/build.sh ../service/target/release/smith-service /absolute/path/to/verified/runtime
```

`test.sh` checks format, analyzes and runs tests. The Rust socket fixture skips explicitly only if `SMITH_TEST_SERVICE` is absent; a supplied invalid path fails. It copies the candidate into a private owner-controlled fixture, verifies 0755/one-link metadata, and starts captured foreground `serve` children with explicit private data/config/runtime roots. It uses bounded direct Unix-socket reads, checks initial flag=false chat eligibility and existing explicit flag-completion/detach/restart compatibility, and confirms child exit before deleting temporary roots. It does not call a real manager or provider.

`make client/build` from `next/` is parent-owned paired Dagger build wiring. `scripts/build.sh` builds the Linux release and safely copies the regular compatible helper into `smith_backend/`. Outputs are under `build/` and explicit exports under `dist/`. The GTK icon is embedded from `linux/runner/resources/smith.png`; builder tooling includes `glib-compile-resources`, not a recipient dependency. Bundle desktop/icon assets are not registered merely by building.

All unit/widget tests use fake app/Core APIs and fake helper processes. No live GUI, manager, provider inventory, inference, audio, model download or installed-service mutation is part of client validation. Parent canonical policy/build documentation and fresh independent assurance remain separate work. Live install/chat/close/reopen and candidate deployment require separate approval; passing source tests/builds does not establish clean-host or real-inference readiness.
