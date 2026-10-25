# Receipt-only native developer worker (review slice)

## Current local-runner variant (source only)

`dev-worker/Dockerfile.local` prepares `smith-dev-worker-local:1.19.3` with the matching native `dist/smith-service`, pinned stock OpenCode, Git and the immutable `guard.mjs`. This is separate from the historical receipt-only image documented below. The current local runner deliberately addresses the operator's existing rootful Docker at `unix:///run/docker.sock`; only the receipt adapter below remains private-rootless. Host service access is root-equivalent authority explicitly accepted by the operator, never a socket mount or authority grant to the worker. The service itself stays non-root.

Docker/CLI, actual service-account access and image preparation are operator prerequisites; runtime never installs, pulls/builds, starts a daemon, changes permissions/groups, uses sudo or falls back to another endpoint. Installer provisioning and new Dagger integration are excluded. Build context remains `service/`, now with `dev-worker/Dockerfile.local` and `guard.mjs` allowlisted by `.dockerignore`. Image preparation and live clone/inference require separate exact approval and have not been performed by the rootful source task. See [rootful authority and evidence](../../docs/plans/LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) and [runner contract](../../docs/plans/LOCAL-SMITH-LOCAL-OPENCODE-RUNNER.md).

Both host adapters use a shared logging policy: `--log-driver=local --log-opt=max-size=1m --log-opt=max-file=1 --log-opt=compress=false`. Explicitly disabling compression avoids Docker rejecting the one-file configuration at start. The [logging fix](../../docs/plans/LOCAL-SMITH-DEV-LOGGING-FIX.md) is source-only; existing failed containers/uncertain records are retained, with no automatic retry or reconciliation. A later deployment must preserve the manifest-verified client/helper/service pairing.

Archive extraction in both recipes uses `tar --no-same-owner`: the trusted image builder, not an upstream archive UID, owns OpenCode. The local worker requires root-owned immutable executables; preserving upstream UID1001 made the earlier image fail its environment guard before Git. Mode0555 alone did not satisfy that check. Recipe fixtures protect the extraction flag; actual rebuilt-image metadata must still verify ownership and modes.

The [full-clone source](../../docs/plans/LOCAL-SMITH-DEV-FULL-CLONE.md) uses ordinary native Git clone with full reachable history, normal remote branches and tags through the existing credential-free HTTP relay. There is no depth/single-branch/no-tags restriction, direct worker networking, submodule or LFS support. One advertisement and one POST replace the shallow exchange; the ten-repository ceiling and existing32-request, byte, workspace and deadline limits remain unchanged. Oversized repositories fail without shallow fallback. The isolated native Git/broker fixtures are not real OpenCode/model acceptance. [Current bounded QA results and required paired-upgrade boundary](../../docs/plans/LOCAL-SMITH-DEV-SMOKE-QA.md) supersede historical preparation status only for their named effects.

## Legacy receipt-only worker

**Implemented source, not live-validated:** `dev_runtime.rs` uses only the explicit
operator-private `runtime_root/docker.sock` endpoint and `/usr/bin/docker`. The
constructor performs no probes. The host freezes the inspected local image ID,
creates one restricted named container, exchanges correlated JSON frames over
non-TTY attached stdin/stdout, then verifies exact-owned shutdown. Stopped
containers are retained: there is no automatic removal/prune or retry. An
immutable random per-attempt owner label prevents a competing create failure from
stopping another attempt's container. Ambiguous create/start acknowledgement
keeps cleanup uncertain even after a stopped snapshot, since a pending daemon
request cannot be ruled out.

The caller's `Dispatching` callback is a **durable persistence barrier**. Failure
must abort dispatch; successful callback return authorizes the single stdin
prompt write. `may_have_sent` remains conservative after any attempted write.
A receipt proves exact user-message read-back, not inference or job completion.
`cleanup_unknown` requires operator reconciliation; an unavailable daemon is not
proof of absence or shutdown. `is_stopped` only reads, returns true for verified
absence/stopped, false for running, and errors on unavailable/ambiguous ownership.
Do not reuse request IDs. A stopped retained container deliberately blocks rerun.

**Implemented diagnostic-only follow-up:** typed, payload-free failures publish only
fixed prerequisite messages through the existing job summary. Missing CLI/socket
metadata is distinct from access/safety failure; unsuccessful daemon/image checks
do not prove absence. Raw errors/output/private paths are never forwarded. The
same dispatch/cleanup flags and generic fallback remain; no extra probe, setup,
replay or historical-result backfill follows. See the
[scoped diagnostics record](../../docs/plans/LOCAL-SMITH-DEV-DIAGNOSTICS.md).

## Separate operator image preparation (not executed in this slice)

The recipe expects a separately compiled **Linux x86_64** service binary at
`service/dist/smith-service`, containing the private worker entry point. Its build
context is `service/`, Dockerfile `dev-worker/Dockerfile`; `service/.dockerignore`
allowlists only that recipe and `dist/smith-service`, excluding build caches and
retained diagnostics. The target image name is
`smith-dev-worker:1.19.3`. Building the recipe is a separately approved operator
action: it downloads Ubuntu packages and the official checksum-verified archive.
The runtime never builds images, pulls images, installs packages, or downloads
OpenCode. The Ubuntu base is version-tagged, not digest/reproducibility pinned.
The release artifact's archive layout and executable compatibility remain
unvalidated; recipe inclusion is not evidence that an image was built.

OpenCode pin:
- release `v1.19.3`
- target source `60c9be092129745688bb3c3a752affc6cbd66ede`
- official archive `https://github.com/anomalyco/opencode/releases/download/v1.19.3/opencode-linux-x64-baseline.tar.gz`
- SHA256 `531cff2441129c30f0edd5878af494108ad07f5feb46d7ee29a2a5b830a5765d`

## Source evidence and remaining acceptance gaps

Pinned official source at
`https://github.com/anomalyco/opencode/tree/60c9be092129745688bb3c3a752affc6cbd66ede`:

- `packages/opencode/src/session/prompt.ts`: `noReply: true` returns after user
  message creation and before the inference loop. Explicit model metadata avoids
  default model discovery. `ollama/smith-receipt-only` is an inert metadata name,
  **not** a configured/enabled model or gateway; `enabled_providers` is empty.
- `packages/opencode/src/session/schema.ts`: MessageID accepts the `msg` prefix,
  so the deterministic `msg_<32hex request id>` is schema-valid.
- `packages/opencode/src/config/config.ts` invokes background dependency install
  while loading config directories, even in pure mode. `packages/core/src/npm.ts`
  skips install for a non-writable directory. The image pre-creates a read-only
  global config directory and `.gitignore`; project config discovery is disabled.
  Merely setting an empty plugin list would not avoid the installer.
- `packages/opencode/src/effect/runtime-flags.ts` and `src/plugin/index.ts` support
  pure mode and explicitly disabling built-in plugins. The owned server receives
  an empty environment plus fixed clean paths/flags/config: no host keys, model
  credentials, proxies, tools, MCP configuration, or project config.
- `src/server/routes/instance/httpapi/handlers/global.ts` reports version/health.
  The wrapper requires `1.19.3`, creates `/session` (never `/session/init`), POSTs
  one `noReply` message and GETs its deterministic ID, validating role, session,
  message and the single exact text part. A lost POST ack permits GET only.

**Not established:** pinned executable zero-provider acceptance, absence of all
upstream incidental initialization work, actual Docker/image compatibility,
rootless daemon/host acceptance, cancellation and crash fixtures, or independent
assurance. No live Docker, daemon, provider/model, installation or image build is
part of local source validation. The parent owns integration, canonical docs,
ledger binding, fresh assurance, and any later live acceptance authorization.

Startup is bounded to 60 seconds; the host reserves cleanup inside its 90-second
total deadline, and bounds the prompt exchange to 10 seconds. These bounds cannot
preempt arbitrary caller persistence callbacks or uninterruptible OS operations.
Drop attempts cleanup; it does not guarantee an orphan-free result when daemon
state cannot be verified. EOF/cancel terminates attached exchange and triggers
exact-owned stop; the worker also bounds its own input/HTTP lifetime.
