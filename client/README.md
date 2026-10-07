# Halo text-chat client

Halo is Smith’s separate Flutter desktop application for Ubuntu 24.04 LTS
x86_64. **Implemented in source, not an approved release:** Flutter is Smith's
primary desktop interface, opening **Boot / Setup** before **Home / Chat**, with
ordinary secondary read-only **Settings**, using `127.0.0.1:4783`. CLI is secondary
query/status/configuration with existing commands retained. Future headless
operation is a separate deferred deployment/lifecycle capability, not CLI.
The Halo candidate bundles the backend alongside the complete Flutter payload.
The client can explicitly start/stop only its captured chat backend child and run
separate read-only setup inspection or an explicitly approved model pull. It does
not install/start Ollama, use sudo/legacy install helpers or integrate services.

## Required backend contract

After separately approved installation/runtime effects, launch the installed
Halo executable. The guided wizard follows **Detect backend → Local runtime →
Select/install model → Review & Start**. It automatically validates only
`smith_backend/smith` under the **actual Halo executable directory** (installed
`ROOT/halo/smith_backend/smith`), not cwd, PATH or a recursive scan. Invalid discovery
never silently chooses another executable. **Advanced · explicit backend path**
allows a reviewed normalized absolute native Linux x86_64 executable. Ownership,
mode/type/ELF and schema checks are safety/behavioral checks, not artifact authentication.

Opening can run `smith setup inspect`, a read-only schema-v1 JSON inventory of
eligible exact local tags from Ollama at `127.0.0.1:11434`. Inventory `ready` is not
verified inference. Select an observed installed `name:tag`, review, then explicitly
**Start packaged backend** when absence is confirmed. Missing Ollama gets guided
instructions to provision/run it separately and **Retry inspection**: empty-host
setup is partial, not turnkey host installation/start.

**Review model download** runs separate `smith setup pull --model <exact-name:tag>`.
The confirmation names the exact tag and fixed endpoint; stdin approval must match
that tag and operation within 60 seconds. NDJSON reports sanitized stages and known
per-layer byte counts. Pull observation has a 15-minute overall and 30-second idle
deadline; no automatic retries. Success requires provider success plus fresh exact
tag verification. Ollama may use internet and external persistent disk; transfer
and storage sizes are unknown until reported. Cancel stops local request/observation
and cleans its captured setup child, not necessarily daemon work or partial cache.
There is no cache deletion/rollback or cloud fallback.

Editing/selection is disabled during setup work, active chat, backend readiness or
owned-child lifecycle. Download additionally requires observed backend absence and
rechecks permission before writing approval; backend-state/active-chat conflicts
cancel the local pull. Start requires an observed installed selection and backend
Start eligibility. Read-only detection/inspection may still run with an active
backend; this is not a global provider lock. Cleanup unknown blocks another setup
child. See [setup boundary](../docs/ARCHITECTURE.md#desktop-setup-command-boundary).
A fully validated sequence-zero terminal `error` is accepted as a preflight failure
without consent/approval; terminal success/cancelled first remain rejected. The UI
preserves validated failure code/message with control/bidi stripping and slash-prefixed
path-like redaction, plus no-auto-retry/unknown-cache guidance. Automatic detection
and advanced-path resolution use the inspection timeout (10 seconds by default).
Real version compatibility remains pending.

Alternatively, a separately managed `smith serve --chat-only --model
<exact-installed-tag>` can be attached to when strictly ready. The observed exact
model is displayed; an explicitly configured model is never substituted. Attachment
does not send/reset a conversation, take ownership, or stop the external backend.

The client requires `/v1/health` to identify `smith-bridge`, report
`profile: "chat-only"`, and not report echo mode. Missing/legacy profiles and
unrelated listeners cannot receive messages from this client. Readiness also requires provider `ollama`, a nonempty exact model and affirmative
`provider_available` and `model_available`; unknown/mismatch blocks Send.
A connected bridge is not proof that inference works.

Navigation, Check readiness and four-second readiness pulses perform health reads
only. Opening additionally discovers/validates the paired executable and runs
read-only setup inventory; none starts a chat backend or downloads. Only explicit Send (or Ctrl+Enter) posts
to `/v1/chat-only`; there is no legacy endpoint fallback. Lifecycle configuration
is limited to Boot/Setup; ordinary Settings remains observational. No task/service/
speech reads, sidecar startup or file-opening capability is added to chat. Model
pull is isolated in the separate setup subprocess; the chat bridge has no
provisioning tools/routes.

Explicit Start launches without a shell using `serve --chat-only --port 4783
--model <exact-installed-tag> --desktop-session <32-lowercase-hex-id>`, with a
15-second startup budget. Desktop mode refuses occupied-port adoption. Owned
readiness requires matching health `desktop_session` and captured-child liveness,
not a healthy port alone; this is correlation, not same-UID authentication.
Stop/normal close closes stdin and sends TERM, waits up to five seconds, then
KILL and up to two seconds for confirmed exit. Unknown cleanup blocks another
start and cancels normal exit. Parent-pipe EOF initiates backend shutdown as a
backup, not crash-proof instantaneous termination or provider-work cancellation.
No HTTP shutdown or port/PID-based killing is used; external backends are untouched.
Readiness loss/recovery preserves the mounted Home, draft and transcript, with
no automatic resend or hidden conversation reset.
The chat-only backend is the capability boundary; hiding controls in the client
alone does not restrict legacy Smith orchestration.

## Conversation behavior

- Home has an expanded text composer; sending opens Chat immediately.
- Home/Chat share the unsent draft and active conversation across navigation.
- Enter inserts a newline; Ctrl+Enter or Send submits a nonempty message.
- Disconnection/model unavailability leaves editing available but disables Send.
- Waiting means a single request is pending, not token streaming or tool work.
  Responses appear as received text, without a simulated typewriter effect.
- Failures are distinct from assistant replies. The attempted text remains in
  the transcript and can be copied into the composer with **Edit failed message**.
  Replacing a different unsent draft requires confirmation. Nothing is retried
  automatically. A timed-out request may have been processed by the backend;
  deliberately resending may duplicate a turn.
- **New chat** confirms before clearing the visible transcript. It preserves
  the draft and sends no reset request immediately. The next explicit send
  requests a reset of the shared default backend conversation. A failed reset
  request stays pending until a successful explicit send; retry may reset again.
- The client stores drafts/transcripts in memory only. Closing/reopening loses
  them; there is no saved history, session list, or restore operation. The first
  explicit send from a newly opened client requests a fresh backend conversation.
- The backend has one shared default conversation with bounded in-memory context.
  Another client can affect that context, and backend restart clears it.
  Reconnection does not restore history or prove context survived. The visible
  transcript is not a guarantee that every earlier turn remains in model context.

Settings shows connection/profile/provider/model observations and these memory
limitations. It contains no service controls. Status is expressed in text, not
color alone; controls are keyboard operable and the Halo respects reduced motion.

## Deferred capabilities

Work, pipelines, agents, integrations, voice, automatic speech, uploads,
attachments, durable history, and streaming are not part of this slice.
Existing Work/voice widgets and bridge methods remain dormant for later scoped
integration; the release entry point does not construct their controllers or
mount their surfaces. The app uses Flutter’s local default font, not runtime
Google Fonts fetching. Existing dependency declarations are retained unchanged.

This is source-inspected local candidate work, not a verified offline release.
Actual provider pull/cancel integration, version compatibility, real local model
inference without external networking, clean-host installation, final source-bound
checks/review and publication/license/SBOM gates remain pending. See the
[approved wizard plan](../docs/plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md),
[boot plan](../docs/plans/LOCAL-SMITH-FLUTTER-BOOT-P0.md),
[offline-chat plan](../docs/plans/LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0.md) and
[canonical design](../docs/DESIGN.md).

**Agent-reported evidence, not rerun here:** 127 Rust tests and the full 136 Flutter
tests passed; the client also reported analyzer/Linux build PASS. Fresh static
assurance was reported source-limited PASS with no blockers; the low-severity client
fixes above still need focused review. These reports are not final integrated checks
or release approval.

## Offline developer validation

With Flutter/Linux toolchain and dependencies already cached, run from `client/`:

```sh
flutter pub get --offline --enforce-lockfile
flutter analyze --no-pub
flutter test --no-pub
flutter build linux --release --no-pub
```

Do not bypass missing tools/cache with downloads under the local validation
scope. These checks do not launch the app, models, or services. Desktop keyboard,
accessibility, actual contextual inference, and recipient installation checks
remain separate manual gates.

The Linux bundle is emitted under `build/linux/x64/release/bundle/` with the
`smith_halo` executable and `dev.smith.Halo.desktop` metadata. It includes the
client and Flutter/plugin runtime. `scripts/package-chat.sh` adds the release
backend at `smith_backend/smith` beside full `lib/` and `data/`; install the whole
paired Halo candidate. The standalone backend candidate remains secondary. The supplied icon's original asset is not
available in the workspace; integration is pending and metadata does not claim one. The client is independently packaged, not part of
the supervisor installer or systemd user-unit lifecycle.
