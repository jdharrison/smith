# LOCAL-SMITH-NEXT-DEBUG-BOOT

## Intent and authority

User requests a debug-text loader → setup OR home shell to validate real client/service communication and factual install/start/setup state. Latest target keeps the service alive independently of desktop close. Source implementation and isolated validation are in scope; no real-host install, unit registration/start/enable, provider work, login changes, commit or remote effect is performed by the agent.

Base checkpoint `04b72f5`; existing bootstrap branch retained. Risk elevated/critical: local lifecycle, filesystem roots, Unix transport, consent and setup persistence. Existing legacy Rust/client/service unit/data remain untouched.

## Independent inputs and reconciliation

Independent producer/designer/architect assessments require separate installation/runtime/bridge/setup observations, read-only boot, explicit action-bound consent, fresh identity checks, durable completion, cancellation/no duplicate actions, and desktop detach-only behavior. Architecture selects a dedicated unprivileged systemd user service with Restart=no and no enable/autostart/linger; no detached fallback.

The minimal native management/transport adapter lives in the Rust service executable, not an extra CLI or Flutter FFI plugin. Flutter invokes its bounded schema-v1 JSON control interface; the adapter checks systemd MainPID and Unix SO_PEERCRED UID/PID before reading/mutating service state. This reconciles the need for native peer credentials without introducing a new Dart plugin. Production manager is fixed `/usr/bin/systemctl`; injected fake commands are available only in tests, not ambient PATH.

## Component contract

Bundled helper: executable-relative `smith_backend/smith-service`; explicit compile-time development override for native run, never PATH/cwd discovery. Defaults use validated absolute XDG data/config/runtime roots: `smith-next/bin/smith-service`, `systemd/user/smith-next.service`, `smith-next/state.json`, `smith-next/service.sock`. Runtime root must be private; namespace directories 0700 and socket/state 0600. No symlink/unsafe/conflicting replacement or unverified socket adoption.

Control arguments:
- `inspect`: read-only, one JSON observation.
- `prepare --action <install-binary|install-unit|start|complete-setup>`: read-only proposed effects and approval token.
- `apply --action <action> --approval <token>`: revalidate action/path/artifact/snapshot/time binding; mutate only the approved action.
- `serve`: foreground Unix-socket service for the user manager; no providers/tasks/tools.

Control JSON (schema_version=1, service="smith-next"): `ok`, `binary_state` and `unit_state` (missing/installed/conflict/unknown), `runtime_state` (stopped/running/unavailable/conflict/unknown), `bridge_state` (ready/unavailable/conflict/unknown), nullable `setup_complete`, `message`, nullable `error`, and `effects` containing binary_path/unit_path/state_path/socket_path and human-readable action effects. Prepare additionally returns `approval_token`; incompatible/malformed results fail closed. Socket framing is bounded newline JSON with identity/version/PID, get_state and complete_setup only. Successful completion follows an atomic synced state write, never local UI preference.

## Traceability and acceptance

| Requirement / acceptance | Task | Evidence |
|---|---|---|
| R1/A1: loader performs reads only and home requires verified completed setup | T1 Flutter controller/shell | Widget tests for all states, no implicit mutation |
| R2/A2: dedicated managed service and real typed bridge | T2 Rust service/management adapter | Temporary-root socket/persistence tests and fake-manager tests, peer identity rejection |
| R3/A3: exact confirmed effects, stale/declined/double-click guards | T3 prepare/apply + confirmation UI | Negative action tests, widget cancellation/correlation tests |
| R4/A4: state survives client detach/reopen, corruption is not reset | T4 service-owned setup state | Restart test, read-only attach, corrupt state test |
| R5/A5: compatible helper bundled, native run stays usable | T5 Make/Dagger bundle wiring | make test, make build and inventory verification |

## Implementation and review follow-up

The source subset is implemented and `make test` / `make build` passed through Dagger with component tasks switched to UID/GID 1000. Native agents reported 18 Rust / 75 Flutter tests; the latter includes a real compiled Rust↔Dart socket detach/restart fixture. Fresh assurance independently reran the aggregate test and verified the current source/artifact manifests. See [source-bound evidence](LOCAL-SMITH-NEXT-DEBUG-BOOT-EVIDENCE.json) and `next/VALIDATION.md`.

Three medium findings were fixed and re-reviewed closed: exact-owned stopped pending-reload units have a separately consented reload-only repair; control helpers cooperatively terminate captured manager groups and use race-safe Linux direct-child parent-death protection; production control/serve rejects UID zero. Flutter cleanup uses TERM with 11-second grace, then KILL and one-second confirmation; unconfirmed cleanup blocks reuse and unknown outcomes require Refresh. Abrupt-death protection is direct-child only and does not promise group cleanup or rollback of already accepted jobs. The helper operation budget is 45 seconds, separate from cleanup; each manager command has a ten-second total budget and 16 KiB stdout cap. Socket payloads are bounded to 4095 bytes plus newline, with two-second I/O budgets and a serialized server.

Readiness and setup facts are separate; safe persisted completion routes unavailable/stopped returning installations to Home repair. Base XDG roots must already exist and be safe. Stop/uninstall/upgrade/crash-stale socket repair remain deferred. PASS applies only to source and isolated validation; no normal-host manager install/start, GUI launch, provider effect or release approval occurred.

## Stage gates and exclusions

Plan → source implementation → self-check → fresh assurance/adversarial review. All native validation remains under /test. Manager/bridge commands, frame sizes, request deadlines, child cleanup, duplicate starts and locks are bounded. Install binary, register unit, start and complete setup remain distinct confirmed actions; partial failures retain factual intermediate observations. Temporary outage preserves durable completion and is a repair state.

No real systemctl mutation is invoked in validation. No GUI launch, autostart, lingering, root/sudo, model/speech setup, inference, notifications, pipeline execution, goals/conversation storage, arbitrary endpoints or legacy migration. Actual desktop close/reopen against the real user manager, installed-host integration and release provenance remain separately approved gates; fakes cannot establish them.

Rollback of local source is separate from operator-approved installed-state recovery. Do not remove user data, replace unrelated units/listeners, or implicitly uninstall/start/stop anything. Stop/uninstall UI is deferred for this initial debug flow; manual recovery instructions must identify the dedicated unit only and require the applicable operator approval.
