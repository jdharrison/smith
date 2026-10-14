# Smith next service

**Implemented in source; not installed or approved for host rollout.** This Ubuntu/Linux-only package supplies a foreground service and two separate native JSON interfaces in the **same** `smith-service` executable: preserved Core lifecycle/debug control and a local text/voice application protocol. The service owns bounded durable app preferences, completed conversation history and operation metadata; a single worker calls fixed-loopback Ollama/STT/TTS providers. There are no tools, orchestration tasks, GUI/audio-device actions, cloud fallback, model downloads, provider startup, legacy migrations, detached service fallback, or client-owned daemon children.

Policy and bounded intent: [AGENTS](../../AGENTS.md), [scope](../../docs/SCOPE.md), [debug boot plan](../../docs/plans/LOCAL-SMITH-NEXT-DEBUG-BOOT.md), [frozen voice/chat plan](../../docs/plans/LOCAL-SMITH-NEXT-VOICE-CHAT.md), and [approval boundaries](../../docs/PROCESS.md#approval-boundaries). This service-only README describes implemented source; canonical documentation and cross-component integration remain parent-owned.

## Executable interface

```text
smith-service --version
smith-service inspect
smith-service prepare --action install-binary|install-unit|start|complete-setup
smith-service apply --action <action> --approval <approval_token>
smith-service serve
smith-service app  # one bounded JSON line on stdin; one JSON reply on stdout
```

No arguments prints a finite hello-world debug message. `--version` prints exactly `smith-next/1 smith-service/0.1.0` and exits. Control commands emit one JSON object on stdout; failures exit nonzero. Production Core control, `app`, and `serve` refuse effective UID 0 before root discovery, filesystem access or manager effects; there is no environment bypass. App requests, including prompts/audio, go through stdin, never argv. Finite hello/help/version remain inert. `serve` runs in the foreground until SIGTERM/SIGINT, independent of desktop lifetime. Its generated unit passes stable explicit `--data-root`, `--config-root`, and `--runtime-root` arguments; these flags are accepted only for `serve`, not arbitrary control targets.

**Do not run Core control or `app` against the normal host during source validation.** Even `inspect` makes a read-only manager query, and `app` verifies current managed-service facts before exchange. Accepted app work can invoke the configured production providers. The tests inject a Rust `Manager` implementation; production always executes `/usr/bin/systemctl --user --no-pager --no-ask-password` directly with a cleared environment and the validated runtime root. PATH, shell aliases, D-Bus address, and ambient “fake manager” variables cannot replace it.

## Fixed roots and separate effects

Data/config defaults are `$HOME/.local/share` and `$HOME/.config`, overridden only by absolute `XDG_DATA_HOME` / `XDG_CONFIG_HOME`. Absolute `XDG_RUNTIME_DIR` is mandatory. All three root directories must already exist, belong to the effective user, and not be group/world writable; runtime must be exactly 0700. Existing ancestors must be safe directories without symlinks (root-owned sticky temporary ancestors are allowed for sandbox fixtures). Paths must be normalized ASCII using letters, digits, `/`, `_`, `-`, and `.`; spaces, specifiers, traversal and ambiguous quoting are rejected rather than escaped.

| Action | Exact independent effects |
|---|---|
| `install-binary` | Create private data `smith-next/bin` directories; publish an exact copy of the current helper at `smith-next/bin/smith-service`, 0755; sync file and containing directory. No overwrite. |
| `install-unit` | Create safe `systemd/user` parents; publish the exact unit at `systemd/user/smith-next.service`, 0600 if missing; run only `daemon-reload`. An already verified file with manager `not-found` can receive a separately consented reload retry after partial failure. An exact installed, expected-fragment loaded unit with `NeedDaemonReload=yes`, `inactive/dead`, PID 0, no socket, no drop-ins and no transient flag also permits a separately consented **reload-only** repair: no directory mutation or unit overwrite. Failed reload preserves that repair path; Start remains blocked until reload is clear. |
| `start` | Start only `smith-next.service`; the daemon creates separate runtime `smith-next/service.sock` and `smith-next/app.sock`, each 0600, inside a 0700 namespace. Startup validates app state and atomically marks unfinished app work interrupted when needed; it never replays work or calls providers. No enable, login policy, lingering or detached fallback. |
| `complete-setup` | Send only `complete_setup` to the verified managed daemon; atomically sync `smith-next/state.json`, 0600, containing only `setup_complete`. |

The unit has `Type=simple`, `Restart=no`, `UMask=0077`, and a five-second stop budget. There is no `[Install]` / `WantedBy`, enable command, autostart, dependency startup or provider action. Logout/reboot survival is **not** promised. A successful `start` action need not imply bridge readiness; the UI must refresh `inspect`.

The source helper must itself be an owner-owned regular 0755 file with one link under safe ancestors. Raw Cargo development outputs can have group-write bits/hard links and therefore fail closed; use a safely copied bundled helper, not permissions weakening or PATH discovery. Installed binary identity is exact byte equality with that helper, not a package signature or upgrade mechanism. Different bytes, unit content drift, unsafe files, aliases and foreign listeners are conflicts, never replacement candidates.

## Preserved Core JSON contract

Every Core control observation has this exact base shape (paths below illustrate resolved XDG roots):

```json
{
  "schema_version": 1,
  "service": "smith-next",
  "ok": true,
  "binary_state": "missing",
  "unit_state": "missing",
  "runtime_state": "stopped",
  "bridge_state": "unavailable",
  "setup_complete": false,
  "message": "Read-only debug service observation",
  "error": null,
  "effects": {
    "binary_path": "/private/data/smith-next/bin/smith-service",
    "unit_path": "/private/config/systemd/user/smith-next.service",
    "state_path": "/private/data/smith-next/state.json",
    "socket_path": "/private/run/smith-next/service.sock",
    "description": "Read only; no installation, manager mutation, start or setup completion."
  }
}
```

- `binary_state`, `unit_state`: `missing | installed | conflict | unknown`.
- `runtime_state`: `stopped | running | unavailable | conflict | unknown`.
- `bridge_state`: `ready | unavailable | conflict | unknown`.
- `setup_complete`: boolean or null, never a local UI preference. A valid absent state file means false. A valid persisted true remains true during a manager outage; corrupted/unsafe state means null and blocks mutation.
- `message`: string; `error`: string or null. `ok` describes the control result, not aggregate readiness. Intermediate files remain observable after manager failure.
- `effects`: an **object**, exactly four resolved path strings plus human-readable `description`. If root discovery/argument preflight fails, paths are empty (unavailable), never actionable effects.
- Successful `prepare` additionally supplies string `approval_token`. Failed preparation supplies no usable token. Inspect/apply do not return approval authority.

Prepare reads only. Its 60-second token binds schema, service, exact action, root/source paths, helper SHA-256, deterministic unit SHA-256, manager facts, observations, state digest and filesystem identity/permissions. Apply takes a nonblocking directory lock, checks time and recomputes the binding before effects. Changed action, paths, source, facts, completed action, expired/future or malformed token fails closed. Install/start/completion remain four distinct confirmations. Tokens correlate trusted local operator consent; they are **not** secrets or authentication against an adversarial same-UID process. Core consent creates no token database. The separate app-state file described below is not part of this consent mechanism and does not reinterpret Core setup completion.

## Preserved Core transport, setup flag and bounds

Before controller bridge access, the exact installed binary and unit must match, the manager must report the expected `FragmentPath`, no drop-ins, no pending daemon reload, a non-transient unit, `active/running` and nonzero `MainPID`, and `/proc/<MainPID>/exe` path/bytes must match the installed helper. Socket type/mode/owner and before/after inode identity are checked; Linux `SO_PEERCRED` must match the effective UID and that exact MainPID **before any request is written**. Incompatible identity/schema/PID or state disagreement fails closed.

Socket requests are one newline-terminated JSON object:

```json
{"schema_version":1,"service":"smith-next","command":"get_state"}
```

Only `identity`, `get_state`, and `complete_setup` exist on the **Core `service.sock` transport**. Responses contain exactly `schema_version`, `service`, `pid`, `ok`, nullable `setup_complete`, nullable `error`. Unknown fields/commands, malformed JSON, wrong identity and oversized/truncated frames are rejected. Maximum payload is 4095 bytes plus newline; connection/read/write waits have two-second budgets, with an absolute frame-read deadline. The Core listener processes one connection at a time without unbounded queues/threads; app traffic uses a separate listener thread in the same managed PID. Its private directory lock excludes duplicate daemons; the controller's data-root lock excludes concurrent applies.

Manager stdout is capped at 16 KiB; properties are strictly parsed, stderr is not echoed, stdin is closed and child execution/cleanup has a total ten-second budget (nine seconds execution plus one second cleanup). Only the captured command's isolated process group is killed. Each manager call has that budget; a composite apply can make up to three manager calls and additional bounded socket exchanges. There is no detached service fallback on any error.

The short control helper catches SIGTERM/SIGINT cooperatively: manager polling observes cancellation, kills its captured process group, reaps the direct child within the cleanup budget and refuses subsequent commands/effects at cancellation checkpoints. Before each manager child execs, Linux `PR_SET_PDEATHSIG=SIGKILL` is armed and `getppid()` is compared with the parent PID captured before fork. A parent already lost during fork-to-arm prevents exec. If the helper is abruptly killed, the kernel kills **only that direct child** (more precisely, on loss of its creating parent thread). Parent-death signals are not inherited by forked descendants; no process-group/descendant cleanup guarantee is claimed after the helper is gone. Exec that changes privilege can clear PDEATHSIG; production uses the fixed non-privilege-changing systemctl, not arbitrary executables.

**Flutter coordination recommendation:** on timeout/dispose send SIGTERM to the captured helper first, continue draining bounded output and allow a bounded cleanup grace (at least the one-second manager cleanup budget, with margin); escalate to SIGKILL only after that grace and wait for the captured helper's exit. A kill/timeout is not rollback and can race an already accepted user-manager job. Show the action outcome as unknown and use a fresh read-only inspection before another action; do not stop the independently managed daemon or claim descendant cleanup from helper exit. Filesystem sync calls are not interruptible cancellation guarantees. No Flutter code is changed here.

Core `state.json` is strictly `{ "setup_complete": <bool> }`; it is a debug setup flag, not provider readiness, app configuration or conversation history. Unknown fields, invalid JSON, symlinks, hard links, unsafe modes, nonregular files or excess size are corruption/conflict, not first-use reset. Completion uses a 0600 create-new same-directory staging file, file sync, atomic rename and directory sync before success. A sync failure can leave a factual completed file but is still reported as failure; no rollback deletes data. Ordinary desktop/socket disconnect does not stop the daemon or clear setup.

Normal daemon shutdown removes only its captured Core and app socket inodes with matching type/owner/mode. Existing or crash-stale paths are never adopted/unlinked. An unclean crash can consequently require separately approved manual repair of **only** the dedicated `smith-next` socket paths; stop/uninstall/recovery UI is deferred. Do not remove legacy paths or unrelated listeners. There is no automatic upgrade or live binary replacement.

## Implemented application protocol

`smith-service app` accepts one newline-terminated JSON payload of at most **4 MiB excluding newline**, with a five-second stdin budget and bounded stdout/socket exchanges. Reads are buffered. The helper uses current Controller facts to verify the installed helper/unit, executable identity and managed MainPID, then checks private `app.sock` metadata, before/after inode identity and peer UID/PID before sending. This is a separate protocol and listener thread, not a larger or reinterpreted Core frame.

```json
{"schema_version":1,"service":"smith-next","protocol":"app","call_id":"0123456789abcdef0123456789abcdef","op":"state","args":{}}
```

Replies contain `schema_version`, `service`, `protocol:"app"`, managed `pid`, `instance_id`, echoed `call_id`, `ok`, `result` and `error`. Success has an operation-specific result and null error; failure has null result and `{code,message}` error. Call, instance and request IDs are **32 lowercase hexadecimal characters**, not hyphenated UUIDs. Unsupported schemas/operations, unknown fields, malformed/truncated/oversized frames and identity conflicts are rejected.

| Operation / args | Implemented result or behavior |
|---|---|
| `state {}` | Authoritative `{config,history,active,providers,retention}`. Config contains nullable model/voice and revision; history is ordered complete user/assistant pairs. Provider observations are cached, not an inline provisioning check. |
| `models {request_id}` | Worker admission; terminal `{models:[{name,digest}],voices,providers}`. Checks installed tags and speech health; missing speech does not fail LLM inventory. Provider readiness/errors are separate. |
| `configure {model,voice}` | Persisted config with a new revision. Model must be an exact observed eligible installed tag; voice must be observed or null. Blocked during work; no inference, start or download. |
| `submit {request_id,text,output:"chat"|"speech"}` | Admission; terminal `{assistant_text,audio,speech_error}`. Successful generation remains successful with retained text if synthesis fails or exceeds its input limit; no regeneration or silent truncation. |
| `poll {request_id,instance_id}` | `{request_id,kind,status,phase,config_revision,result,error}`. Instance mismatch fails; synchronize instead of silently resending. |
| `transcribe {request_id,audio:{mime:"audio/wav",data_base64}}` | Admission; terminal `{text,language,duration_ms}`. Transcription never implicitly submits a turn. |
| `speak {request_id,text}` | Admission; terminal `{audio}` for deliberate replay, without generation. |

Admissions return request ID, status, phase and config revision. Status is `running`, `succeeded`, `failed` or `interrupted`; phase is `inventory`, `transcription`, `generation`, `synthesis` or `complete`. One provider worker admits **one active operation, no queue**. Provider calls never run inline in a socket handler, so state/poll remain responsive during work. Identical request ID plus input fingerprint deduplicates; changed input returns `ID_CONFLICT`. Audio input persists only as a fingerprint, not raw audio.

### App durability, retention and worker ownership

The separate data `smith-next/app-state.json` is a 0600, bounded **16 MiB** state file. Accepted metadata is atomically synced before acknowledgement; completion and completed history publish coherently by file sync, atomic rename and directory sync. Preferences and up to **16 completed turn pairs / 64 KiB retained content** survive restart. This is bounded context and operation evidence, not an archival transcript or durable scheduler.

At most **32 jobs total**, including active work, are retained. Only terminal jobs can be evicted; each eviction retains the ID in up to **1,024 durable tombstones**. Evicted IDs return `REQUEST_EXPIRED` and cannot be reused. When admitting new work would require eviction with the tombstone store full, admission returns **`CAPACITY`**. There is no silent erasure of old IDs, automatic reset, or maintenance/reset API in this slice.

A restart creates a new instance ID and converts the single unfinished running job to `interrupted`; **no automatic replay or resume** occurs. More than one running record, unsupported schema, invalid nested metadata, non-hex digests, oversized errors/results or unsafe/corrupt files are rejected and preserved, not reconciled into apparent success or first-use reset. Complete app replies are checked against the frame bound.

Audio is transient and never persisted/logged. After restart, a completed submit still exposes its text with null audio and `speech_error.code = AUDIO_EXPIRED`; a completed speak retains succeeded metadata but null result and `error.code = AUDIO_EXPIRED`. Deliberate synthesis replay needs a new request ID. Desktop/helper disconnect does not stop the independently managed service or its authorized worker.

The final application owner revokes publication under the commit mutex, disconnects admission, retains the worker join handle and waits **up to 250 ms for worker exit after fencing**, joining if finished. A still-blocked worker is detached only after fencing; it cannot publish stale state into reopened roots or enter a later synthesis phase. **External inference is not guaranteed cancelled** by teardown or timeout. Filesystem synchronization is not an interruptible cancellation guarantee.

### Providers and payload bounds

Production uses fixed numeric-loopback HTTP only: Ollama `127.0.0.1:11434`, STT `127.0.0.1:4890`, TTS `127.0.0.1:4891`. There is no production fake-endpoint environment override, ambient proxy, redirect, cloud/model substitution, tool execution, model pull, provider/container start or automatic retry. Ollama requests use exact eligible local tags and text-only `/api/chat` with `stream:false`, bounded context/tokens, and tool-call rejection.

Connect budget is two seconds. Overall budgets are inventory 10 seconds, STT 90, LLM 120 including tag revalidation, and TTS 60 including voice revalidation. Provider JSON bodies are capped at 1 MiB without trusting Content-Length; audio bodies have their own limits. Text input is 16 KiB, model output 64 KiB, TTS input 4 KiB. Input WAV is mono 16 kHz PCM16, at most 60 seconds / 2 MiB; output is mono 24 kHz PCM16, at most 60 seconds / 2,880,044 bytes. Output audio has `{mime:"audio/wav",sample_rate:24000,data_base64}`. The desktop, not this service, owns microphone capture, draft review and playback.

## Validation and review limits

From this directory, `sh scripts/test.sh` runs formatting, strict Clippy and all Rust test targets, including a real compiled daemon under temporary private roots and an injected fake manager. Compiled-daemon integration tests require a non-root user and deliberately fail in root containers; pure/unit/library seams may run as root. The ignored helper fixture is invoked explicitly by parent-death/cancellation subprocess tests, never a skipped validation scenario. `sh scripts/build.sh` builds the locked release executable only. The package owns an independent Cargo workspace/lockfile. Direct dependencies are `serde`, `serde_json`, `libc`, `sha2`, **`base64` 0.22** and **`reqwest` 0.12** with default features disabled and `blocking`/`json` enabled. Tests inject providers at library-only seams and use private ephemeral HTTP fixtures; they do not contact configured production endpoints. Offline gates are `cargo fmt --all --check`, `cargo clippy --offline --locked --all-targets -- -D warnings`, `cargo test --offline --locked --all-targets`, and `cargo build --offline --release --locked`.

Coverage includes separate lifecycle effects, read-only preparation, consent expiry/action/source/path/fact binding and replay, reload/start failure without fallback, real peer-PID rejection, private permissions, malformed/oversized/idle frames, client detach, graceful restart persistence, corrupt state preservation, duplicate daemon/socket refusal, manager identity/drop-in/stale-load/transient drift, concurrent apply refusal, loaded pending-reload repair/failure retry without overwrite, root preflight, and fake subprocess output/deadline/descendant-pipe cleanup, abrupt helper death/direct-child exit, cooperative SIGTERM/group exit, and pre-exec parent-race refusal. App coverage additionally includes all four text/speech paths with fakes, explicit transcription/send separation, responsive single-worker admission, dedup/conflict/expiry/capacity, durable preferences/history, restart interruption without replay, transient audio and retained text on TTS failure, provider/WAV/text/frame bounds, nested corrupt-state preservation, and the gated worker drop/reopen publication-fence regression. No test queries or mutates the real user manager, invokes actual production models, or installs into the real home. Source-bound self-check and hashes are recorded in [VOICE-CHAT-VALIDATION.json](VOICE-CHAT-VALIDATION.json).

**Unresolved gates:** fresh independent assurance/adversarial review; parent-owned canonical docs and cross-component integration; separately approved systemd-user/desktop close-reopen and actual provider/model/microphone/playback smoke; clean-host packaging/provenance and crash recovery/manual repair UX. Filesystem checks are conservative validation for one trusted operator, not a race-proof sandbox against hostile same-UID mutation between checks. Tests using a fake manager cannot establish real unit loading, login/logout behavior or host integration. No host rollout, commit, release, or accepted-risk decision is implied by passing native tests.
