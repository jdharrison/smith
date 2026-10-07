# Local offline-chat candidate packaging

## Status and authority

**Implemented packaging support:** `scripts/package-chat.sh` assembles existing
build outputs, `scripts/install-prebuilt.sh` installs an extracted candidate, and
`scripts/smoke-prebuilt.sh` checks isolated mock assembly/installation. These are
not a publication workflow or proof that a candidate is ready to ship.

**Target product:** Ubuntu **24.04 LTS x86_64**, one trusted non-root operator,
Flutter Halo desktop with bundled Rust Smith bridge, Home/Chat text-first;
a standalone backend candidate is retained as a secondary option.
**Implemented in current source:** the backend profile is explicitly
`serve --chat-only --model <exact-installed-tag>`, with direct loopback Ollama,
no intent dispatch or provisioning, and one bounded in-memory default session.
Halo opens primary Flutter guided Boot/Setup → Home/Chat with ordinary read-only Settings under the approved [desktop wizard plan](plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md), extending [Flutter boot](plans/LOCAL-SMITH-FLUTTER-BOOT-P0.md). CLI remains secondary query/status/configuration with existing commands retained; future headless deployment/lifecycle is separate, not CLI, and deferred. Plain `serve`, `chat`, and systemd
registration remain legacy orchestration paths, not this slice. Source inspection
is not final runtime/test evidence; the candidate must satisfy
[the offline-chat plan](plans/LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0.md).
See [Scope](SCOPE.md), [Architecture](ARCHITECTURE.md), [Process](PROCESS.md), and
[Host Profile](HOST_PROFILE.md) for canonical policy in the repository.

Every archive is labeled **LOCAL CANDIDATE — NOT A PRODUCTION RELEASE**, including
clean-source candidates. Dirty source identity is recorded rather than hidden.
No commit, push, tag, publication, live install, manager mutation, dependency/model
provisioning, or accepted risk is authorized by assembling a local candidate.

## Preprovisioning is not offline chat

The application archives do **not** contain Ollama or model weights. Provisioning
may require internet, separately approved package installation, or verified
transfer from another machine. Record the Ollama version, exact model identity
and digest, download size, disk/RAM/VRAM needs, and actual inference performance.
There is no model download, dependency install, or cloud fallback in these
packaging scripts.

After provisioning, **the target** is cold-start and contextual text conversation
without external networking. Internet search, remote Git/Jira/GitHub, tools,
voice, uploads, persistent history, and streaming are outside this slice.
Conversation state is bounded in memory and lost on backend restart; do not
promise saved conversation history. Offline inference must be proven with the
actual chat-only profile, not legacy chat or an echo/mock provider.

Recipient prerequisites (not a build toolchain):

- Ubuntu 24.04 x86_64, Bash and Python 3.12 from the Ubuntu baseline.
- Backend's ELF runtime libraries, separately provisioned/running Ollama on loopback,
  and an explicitly selected installed model (already present or confirmed setup pull).
- Halo's graphical session, GTK3 and native plugin/system runtime libraries,
  and its bundled reviewed `smith_backend/smith`, explicitly started from Boot/Setup,
  or an untouched compatible externally managed chat-only bridge.
- `tar` and `sha256sum` for checking and unpacking archives.

Each manifest lists ELF `DT_NEEDED` entries inspected with `readelf`, **without
executing the binaries**. This is a direct dependency inventory, not a complete
Ubuntu package resolver: transitive libraries, dynamically loaded plugins,
graphics drivers, and model resources still need clean-host validation. The
whole Halo candidate includes its Flutter runtime, plugin libraries, assets,
complete `lib/`/`data/` and paired backend at `smith_backend/smith`;
copying just `smith_halo` will not work. Rust/Cargo, Flutter/Dart, CMake and
compiler tools are not needed by recipients. They are needed by the separate
build coordinator; assembly additionally needs Python, Git and `readelf`.
OpenCode, Docker and Python voice sidecars are not chat-only dependencies.

## Build-free assembly (coordinator)

Run from the repository root. First finish and validate the separately managed
builds. The assembler consumes **only**:

- `target/release/smith`
- the full `client/build/linux/x64/release/bundle/`

It never builds, fetches dependencies, runs help/version probes, launches either
application, invokes managers, or publishes. It rejects missing artifacts,
non-native main executables, unsafe payload file types/modes and changing source
or payload bytes during assembly. Output must be a **new absolute directory**
under an existing non-symlink parent; existing candidates are never replaced.
Use an ignored `target/` location so evidence/artifacts do not enter source identity.

Record source identity immediately before the coordinated builds:

```bash
bash scripts/package-chat.sh --source-identity > target/chat-source.json
```

Source identity hashes Git-tracked and nonignored untracked file contents/types
and modes (including missing tracked files), excluding build outputs. It does not copy
source contents or credentials into archives. Relative file names and their
hashes are retained. Review them and all metadata for sensitive names before
sharing. Source identity is **not** a hermetic dependency snapshot or compiler
attestation. Executable/mode changes also require a refreshed source identity.

The coordinator supplies a JSON file with **exactly** these fields (replace the
illustrative strings with recorded values):

```json
{
  "rustc": "exact rustc version used for the backend build",
  "cargo": "exact Cargo version used for the backend build",
  "flutter": "exact Flutter version/revision used for the client build",
  "dart": "exact Dart version used for the client build",
  "source_sha256": "sha256 value from target/chat-source.json"
}
```

Keep it under `target/`, e.g. `target/chat-build-info.json`. Do not include local
absolute paths, credentials, raw environment dumps or private transcripts.
Do not label toolchains merely detected after a build as the toolchains used.
Supply retained raw build/analyzer/test evidence separately. The assembler
requires the declared source hash to match current source; this declaration
still does **not** prove that existing binaries were built from those inputs.
A concurrent source change is a blocker: finish/reconcile the source snapshot,
rebuild as necessary and regenerate metadata. Do not bypass the check.

Example (replace the absolute output path with the intended new local path):

```bash
bash scripts/package-chat.sh --output /absolute/repository/target/chat-candidate-01 --build-info target/chat-build-info.json
```

Output:

- `smith-backend-<version>-ubuntu-24.04-x86_64-candidate-<head>-<source>.tar.gz`
- `smith-halo-<version+build>-ubuntu-24.04-x86_64-candidate-<head>-<source>.tar.gz`
- matching `.provenance.json` sidecars and outer `SHA256SUMS`.

Each archive has one top-level directory containing `payload/`, `LICENSE`, this
guide, `install-prebuilt.sh`, `manifest.json` and an internal `SHA256SUMS`.
Halo assembly explicitly copies `target/release/smith` to
`payload/smith_backend/smith`; an existing bundle backend must have that exact digest
and no unexpected entries. Provenance records the packaged backend path, version,
digest and source identity, plus both application versions, HEAD/branch/dirty status, source
hash inventory, lockfile digests, declared compiler versions, payload ELF
requirements, timestamp and unverified gates. It is not an SBOM, signature or
independent assurance verdict. Packaging makes no reproducible-build claim.

## Explicit install-only flow (requires install approval)

These are instructions, not effects of packaging. Do not run them against normal
user paths until the exact install/root/account/artifact effects are approved.

1. Independently retain/review the digest of the outer checksum file and the
   chosen archives. In the candidate directory, run `sha256sum --check SHA256SUMS`.
   A checksum file beside an archive detects corruption, **not authenticity**.
2. Inspect the trusted archive inventory (`tar -tzf <archive>`). Unpack into a
   fresh private staging directory, not an existing tree, using GNU tar with
   `--no-same-owner --no-same-permissions`. Do not extract untrusted archives:
   the installer verifies an **already extracted** package; it is not a safe
   archive extraction utility. Review the installer from a trusted source before
   executing it; an archive's own checksum cannot authenticate its installer.
3. Explicitly create a private, fresh installation root owned by the invoking
   non-root user. Pass absolute normalized paths with no symlink components:

   ```bash
   bash /absolute/staging/package/install-prebuilt.sh --package /absolute/staging/package --root /absolute/user-owned/install-root
   ```

The installer verifies the complete internal inventory before staging bytes. It
rejects root, unsupported hosts, writable/foreign roots, unsafe ancestors,
symlink/hardlink/special package files, unsafe modes, checksum traversal,
unlisted files and existing destinations. Trusted root-owned sticky temporary
ancestors are allowed for isolated smoke; the actual package/install root must
still be user-owned and non-group/world-writable. Package and install roots must
not overlap. A single trusted operator with no concurrent mutation is assumed;
these owner-controlled checks are not isolation from a malicious same-UID
process or root. No sandbox is claimed for Python/file operations.

Backend installs to `ROOT/bin/smith`, mode `0755`, with
`ROOT/.smith-install-root` containing `smith-install-root-v1`, mode `0600`,
compatible with existing registration's marker contract. **Do NOT use the legacy
service unit or `settings service install/start/autostart` for this chat profile.**
The legacy unit selects `serve` rather than `serve --chat-only`; marker
compatibility is not profile/service approval. No service migration is supplied.

Halo installs its complete paired bundle to `ROOT/halo/`, including
`ROOT/halo/smith_backend/smith` and full `lib/`/`data/`. The standalone backend at
`ROOT/bin/smith` is secondary, not a prerequisite to install the Halo candidate. The installer prints the
absolute `ROOT/halo/smith_halo` launcher path. There is **no automatic desktop
integration** or PATH change. Embedded desktop metadata alone does not register
a desktop entry; use the absolute executable explicitly after launch approval.

Neither install runs the application, help/version probes, sidecars, Docker,
model/dependency installation, managers, enablement, desktop updates or lingering.
It does not touch Smith configuration/data. Installation proves copied bytes,
not executable compatibility or application health.

## Launch, upgrade and rollback boundaries

**Primary desktop flow, implemented in source:** after separate runtime approval,
launch the installed Halo executable. The wizard follows Detect backend → Local
runtime → Select/install model → Review & Start. It automatically validates only
`<actual Halo executable directory>/smith_backend/smith` (installed
`ROOT/halo/smith_backend/smith`), not cwd/PATH or a recursive scan. Invalid discovery
never silently selects another binary; **Advanced · explicit backend path** permits
a reviewed absolute executable. Opening also runs read-only `smith setup inspect`
for structured eligible exact local model inventory; `ready` is not inference proof.
Choose an observed installed `name:tag`, review, then explicitly **Start packaged
backend** when absence is confirmed. Readiness checks/four-second pulses never
start the chat backend, infer or download. Local ownership/mode/type/ELF and schema
checks are not artifact authentication. Ordinary Settings remains read-only.

**Partial empty-host setup:** unavailable Ollama receives guided separate provision/
start instructions for `http://127.0.0.1:11434`, with explicit Retry inspection. No
Ollama OS/runtime install/start, sudo, legacy install helper, manager integration or
headless implementation is supplied. Model weights remain absent from the archives.
**Review model download** invokes separate `smith setup pull --model <exact-name:tag>`;
only exact-model/operation-bound stdin consent within 60 seconds allows POST.
Schema-v1 NDJSON provides sanitized stages/known per-layer bytes and terminal status,
with 15-minute overall/30-second idle pull deadlines, explicit cancel and no auto
retry. Success requires fresh verification of the exact eligible local tag after
provider success, not verified inference. External Ollama may use internet and
persistent disk, with unknown transfer/storage sizes until reported. Cancel stops
local request/observation and cleans its captured setup child, not necessarily daemon
work or partial cache; no automatic rollback/cache deletion or cloud fallback.
The chat-only bridge remains isolated without provisioning tools/routes.

Mutation guards require idle setup and chat, non-ready configurable backend and no
owned child; download additionally requires backend absence and consent-time recheck.
Backend/chat conflicts cancel local pull; Start requires an inventory-observed
selection and backend eligibility. Read-only detection/inspection may run with an
active backend. Unknown setup cleanup blocks another child, but no global daemon
lock is supplied. A fully validated sequence-zero terminal `error` is accepted as
preflight failure without consent/approval; terminal success/cancelled first are
rejected. Failure display preserves validated code/message with control/bidi
stripping and slash-prefixed path-like redaction, plus no-auto-retry/unknown-cache
guidance. Detection and advanced-path resolution use the inspection timeout
(10 seconds by default). See
[Architecture](ARCHITECTURE.md#desktop-setup-command-boundary).

A strictly compatible ready external backend attaches untouched, displaying its
exact model; a configured model is never substituted. Attachment sends/resets no
conversation and never stops the external process. Unknown/mismatched identity,
profile, provider/model or availability blocks sending and Start.

Desktop Start launches `serve --chat-only --port 4783 --model <exact-installed-tag>
--desktop-session <32-lowercase-hex-id>` without a shell. Desktop mode refuses
occupied-port adoption. Health `desktop_session` plus captured-child liveness
correlates readiness, not hostile same-UID authentication; startup is bounded to
15 seconds. Stop/normal close closes stdin and sends TERM, waits up to five seconds,
then KILL and up to two seconds for exit confirmation. Cleanup unknown blocks
restart and cancels normal exit. Parent-pipe EOF initiates graceful backend shutdown,
not a crash-proof immediate-termination or provider-cancellation guarantee. No HTTP
shutdown or port/PID-based kill is used. Pulses/recovery preserve mounted Home,
draft and transcript without resend/reset. Actual desktop/process integration is
still a required gate. The supplied icon's original asset is absent from the
workspace; integration remains pending.

**Retained secondary CLI alternative:** after the same approvals, launch an
externally managed installed backend explicitly:

```text
/absolute/user-owned/install-root/bin/smith serve --chat-only --model <exact-installed-tag>
```

Replace `<exact-installed-tag>` with the exact already-installed local name from
Ollama's `/api/tags`; there is no implicit default, automatic pull, or cloud
fallback. The backend talks only to `http://127.0.0.1:11434`, bypassing legacy
orchestration and warm startup. The bridge defaults to `127.0.0.1:4783` and
reports model availability separately from listener health. `--chat-only`
conflicts with `--echo`; never use plain `serve`, legacy `chat`, or the legacy
systemd unit for this slice. Explicitly launch the printed Halo executable.
Verify bridge identity, `chat-only` profile, provider, exact model, and availability
before sending; stale/legacy builds are a blocking error. These are source-backed
CLI instructions, not a claim that a packaged candidate has passed final tests.

Existing `ROOT/bin/smith` and `ROOT/halo` are refused instead of overwritten.
Upgrade into a **separate fresh root**, validate the candidate under the required
approvals, and explicitly switch the launcher path. Keep the earlier root for
rollback; explicitly switch back after stopping the affected application under
runtime approval. No data deletion, automated uninstall, stop, or service change
is performed. An installation failure preserves unrelated data; private staging
is cleaned. Partial Halo installation is removed on copy failure. Abrupt process
termination can leave partial install files requiring reviewed manual recovery;
no crash-atomic install/upgrade guarantee is claimed.

## Validation and no-ship gates

Local checks from the repository root:

```bash
bash -n scripts/package-chat.sh scripts/install-prebuilt.sh scripts/smoke-prebuilt.sh
shellcheck scripts/package-chat.sh scripts/install-prebuilt.sh scripts/smoke-prebuilt.sh
bash scripts/smoke-prebuilt.sh
```

Smoke uses temporary HOME/XDG/install/package roots, fake manager/network/compiler
commands, mock binaries and inert ELF copies for assembly. Packaged executables
are never run. Root/foreign-ownership/unsupported-host preflight is simulated,
not an actual account/host change. Fake commands are tripwires, **not syscall
containment**. Successful mock smoke is not offline-model, desktop, deployment
or live-systemd evidence.

**Reported self-check only:** wizard agents reported **127 Rust tests and the full
136 Flutter tests PASS**, plus client analyzer/Linux build PASS, not final
source-bound integrated evidence. Fresh static assurance was reported
**source-limited PASS with no blockers**; the subsequent low-severity client fixes
for preflight errors, diagnostics and bounded detection/advanced resolution also
passed focused independent static re-review. No final integrated checks are claimed. Earlier boot
agents reported 98 Flutter tests and 110 Rust tests PASS, then 108 Flutter tests
after race fixes. Earlier full-suite OpenCode noisy-probe checks failed twice before the
current backend agent reported a passing suite; no resolved root cause is claimed.
No final source-bound integrated evidence or approved release is established by
these reports or this documentation update.

**Blocking before publication:** actual provider pull/cancel integration and
provider/backend/client version compatibility, actual desktop/process integration,
candidate-bound Rust/Flutter build/test evidence,
clean-source reconstruction/build binding, license/SBOM review (including bundled
Flutter/plugins), fresh independent assurance/adversarial review, and explicitly
approved real contextual inference with denied external networking plus clean-host
installation without build tools. Verify all native dependencies, cold-cache
fonts/assets, unsupported profile failures and model requirements. Retain the
candidate digest outside the owner-controlled checksum bundle. Unknown/skipped
checks remain blocked. No final integrated tests are claimed by this guide update.
The legacy `.github/workflows/release.yml` still publishes generic Ubuntu/macOS
backend archives on `release/*` tags, without the paired Ubuntu 24.04 slice or
required evidence/approval gates. Remediating that workflow is a publication
blocker outside this documentation-only scope; do not trigger it for a candidate.
Apply the separate human approval boundaries in
[Process](PROCESS.md); local candidate assembly grants no publication rights.
