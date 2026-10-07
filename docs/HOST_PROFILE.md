# Smith Ubuntu Workstation Host Profile

## Status and evidence

This document is the normative **host contract** for Smith’s single-workstation target. It records a non-sensitive local inventory captured on **2026-08-28** and distinguishes it from requirements that are not yet verified. Do not put credentials, serials, private keys, full network topology, or sensitive paths here.

The source assessment is [SMITH Linux Architecture Report](SMITH_LINUX_ARCHITECTURE_REPORT.md), especially its host requirements and resource analysis. Its prior RX 7900 GRE / 16 GiB reference is not treated as an observed fact because the current PCI probe identifies only a Navi 31 GPU family.

## Observed workstation inventory

| Area | Observed fact | Evidence |
|---|---|---|
| Operating system | Ubuntu 24.04.4 LTS (Noble), Linux 7.0.0-30-generic, x86_64 | `/etc/os-release`; `uname -srmo` |
| CPU | AMD Ryzen 9 7950X3D; 16 cores / 32 threads | `lscpu` |
| Memory | 61 GiB reported RAM; 8 GiB swap | `free -h` |
| GPU | AMD Navi 31, identified by PCI as Radeon RX 7900 XT/XTX/7900M family | `lspci` |
| Storage | 953.9 GiB NVMe with encrypted LVM root; separate 931.5 GiB disk currently not mounted | `lsblk`; `df -h` |
| Root filesystem capacity | 935 GiB total, 771 GiB available at inventory time | `df -h` |
| Toolchain | Rust/Cargo 1.98.0; Python 3.12.3; CMake 3.28.3; OpenCode 1.18.25 | tool version commands |
| Docker | Client/server 29.7.2; AppArmor, built-in seccomp, and cgroup namespaces reported | `docker version`; `docker info` |
| Ollama / AMD utilities | `ollama`, `rocm-smi`, and `rocminfo` were not found on `PATH` | executable discovery |
| Smith user service | Absent on 2026-08-28. On 2026-09-11 an enabled legacy unit was observed in `activating/auto-restart`, with no bridge and 8,091 restart attempts at the last read-only query; lingering remained disabled. The source hardening is not installed. | `smith settings service status`; read-only `systemctl --user show`; `loginctl` |

## Required baseline and unresolved settings

| Area | Required baseline | Current status |
|---|---|---|
| Operating system | Ubuntu LTS, x86_64, supported kernel/userspace | Satisfied by observed Ubuntu 24.04.4 LTS; maintain through supported updates. |
| Memory | At least 64 GiB physical RAM, with deliberate swap/recovery headroom | Appears to be 64 GiB-class hardware; verify installed physical memory before relying on it. |
| GPU | AMD GPU with sufficient VRAM for the selected local model | Exact marketed GPU model and usable VRAM remain unverified. |
| Inference | Native host Ollama using verified AMD acceleration when local GPU inference is required | Not provisioned/verified. |
| Storage | Deliberate allocation for repositories, Docker, models, and encrypted backups | Encrypted root and second disk are present; Docker/model/backups placement is undecided. |
| Service model | `systemd --user` supervisor with a deliberate login/lingering policy | A legacy unit is installed/enabled but unavailable in a restart loop because host OpenCode is not discoverable in its service environment. Source-only containment is implemented locally but not deployed; lingering is disabled. |
| Containers | Docker Engine reachable by the Smith user without interactive privilege prompts | Docker API is reachable; rootful/rootless decision is not yet recorded. |
| Network posture | Loopback-first inbound services; intentional outbound Git, registry, model, and search traffic only | Firewall and non-loopback exposure policy must be recorded and verified. |

## Chat-only candidate: recipient versus builder

**Implemented packaging target:** Ubuntu **24.04 LTS x86_64**, full Flutter Halo bundle with `smith_backend/smith` beside complete `lib/`/`data/`, plus a secondary standalone backend candidate; see [Chat Release](CHAT_RELEASE.md). The historical inventory above is not a new provisioning or compatibility observation.

- **Recipient:** Ubuntu runtime libraries for the backend; Bash, Python 3.12, `tar`, and `sha256sum` for install/verification; separately provisioned/running Ollama at `127.0.0.1:11434` and an exact installed local tag, either already present or explicitly downloaded through setup before `serve --chat-only --model <exact-installed-tag>`. Halo additionally needs a graphical session, GTK3 and native plugin/system libraries. Flutter is primary: guided Boot/Setup automatically validates `<actual Halo executable directory>/smith_backend/smith`, with explicit advanced absolute-path configuration, read-only structured inventory, observed-model selection and review before explicit Start, or untouched attachment to a compatible ready external bridge. No cwd/PATH scan or shell fallback exists. Opening/checking never starts the chat backend, infers or downloads; opening can execute read-only `setup inspect`. Install the entire Halo bundle, not just its executable.
- **Builder:** Rust/Cargo and Flutter/Dart toolchains plus Linux compiler/CMake/native development dependencies; candidate assembly additionally needs Python, Git, and `readelf`. Recipients do not need build tools, Docker, OpenCode, or Python voice sidecars for chat-only operation.

Neither archive bundles Ollama/model weights or installs dependencies, starts applications/services, changes PATH/desktop registration, or migrates the legacy systemd unit. Empty-host setup remains partial: missing Ollama gets guided separate installation/start instructions, with no sudo or legacy install helpers. The approved [wizard plan](plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md) narrowly permits operator-confirmed model pull through already-running Ollama, not OS/runtime provisioning. Exact-model/operation-bound stdin approval, bounded NDJSON progress/cancel and finite deadlines do not constrain external daemon internet/disk effects; sizes may be unknown until reported and cancellation may leave continuing daemon work/partial cache. No automatic retries or cache deletion. Read-only inventory is not inference proof, and setup does not add provisioning tools/routes to the chat-only bridge. See [Architecture](ARCHITECTURE.md#desktop-setup-command-boundary) for mutation guards; inspection may run with an active backend, and no global daemon lock is claimed. Provisioning and launch/install effects need separate approval. Desktop Start does not provision/start Ollama, register services or mutate the legacy unit. CLI remains secondary query/status/configuration with existing commands retained; future headless operation is a separate deferred deployment/lifecycle capability, not CLI. Owned-child session correlation, stdin EOF and bounded TERM/KILL normal-close cleanup are described in [Architecture](ARCHITECTURE.md); no crash-proof guarantee is made. Actual provider pull/cancel integration and version compatibility, model resource/acceleration requirements and transitive runtime libraries remain to be validated; real network-denied inference and clean-host desktop installation without build tools are publication blockers, not completed checks.

## Required host capabilities

The following is the broader orchestration/development host contract, not the prebuilt chat-only recipient prerequisite list.

- Rust stable with Cargo, `rustfmt`, and Clippy.
- Docker CLI and a healthy Docker daemon.
- Git, OpenSSH client, `curl`, POSIX `sh`, `python3`, Python virtual environments, and `systemctl --user`.
- Native Ollama or a compatible local endpoint at the documented host address.
- `xdg-open` only when host file preview is intentionally enabled.
- Stable paths for the Smith repository and `client/sidecar` assets.
- Encrypted backup coverage for Smith config/data, approved repositories, audit/evidence exports, and credential references.

## Record before operation

The operator must capture these facts in an approved local deployment record; do not put secrets, private keys, tokens, serial numbers, or full network topology in this repository.

```text
Ubuntu release and kernel:
CPU model, cores, and threads:
RAM and configured swap:
GPU driver, VRAM visibility, and Ollama acceleration result:
Filesystem mount points and available capacity for repositories, Docker, models, and backups:
Docker version, rootless/rootful mode, and user access model:
Systemd user-service state and lingering decision:
Firewall policy and every deliberately exposed non-loopback port:
Installed versions: Rust/Cargo, Docker, Ollama, OpenCode, Python, Flutter (if client is enabled):
Backup target, encryption status, restore test date, and retention policy:
```

## Resource policy

- Preserve interactive responsiveness: do not start unbounded concurrent model calls, Docker builds, or pipeline workers.
- Until an application scheduler exists, permit at most one active consequential pipeline per project/branch and use the operator to serialize GPU-heavy work.
- Treat model VRAM, RAM, Docker overlay I/O, and build caches as shared finite resources.
- Stop or defer work rather than causing host-wide memory pressure, storage exhaustion, or interactive desktop failure.
- Pipeline state inside a worker container is not durable recovery state. Export required evidence before worker removal.

## Linux service policy

**Target host contract:**

- The separate legacy orchestration supervisor runs as a user service, not a privileged system service; this is not a requirement to register the Flutter-owned chat-only backend. Install as the intended non-root user to a stable, canonical root owned by that user; the root and executable must not be group/world writable, and service registration must verify the installer’s private root marker. Never register a repository build, development, temporary, `target/debug`, or `target/release` executable as the permanent service.
- Treat four facts independently: unit installed/loaded, running now (`ActiveState`/`SubState`), enabled at user-manager login, and user lingering. Bridge identity/health is a fifth application-level observation.
- Enabling `smith.service` selects login/default-target startup only. It does not start the service now and does not prove pre-login operation.
- `Linger=no` means the user manager normally depends on a login session. `Linger=yes` may permit pre-login/post-logout operation, but still does not prove Smith is healthy. Unknown must remain unknown.
- Smith and its installer never run `loginctl enable-linger` or `loginctl disable-linger`. If pre-login operation is required, record and approve that manual host-policy change separately, including rollback with `loginctl disable-linger <user>`.
- Interactive-shell environment variables do not automatically reach systemd user services. Required executables must be resolved through an explicit stable absolute configuration, the actual service `PATH`, or documented stable per-user locations. Ubuntu x86_64 OpenCode discovery must reject non-native files and require bounded semantic-version plus OpenCode-specific help behavior; this is behavioral validation, not a substitute for approved package/digest provenance at release. Any future environment-file or secret-reference mechanism requires explicit ownership, permission, key-allowlist, and credential handling design.
- Permanently invalid host-runtime configuration must fail before warm resource work and suppress automatic retry. Other failures must have a bounded restart burst with no host-wide start-limit action.
- Test controlled install/start/restart/stop/enable/disable/uninstall separation and atomic stale-job recovery with prior diagnostics preserved before relying on background operation. A manager failure must never create an unmanaged detached supervisor.

**Observed reference host:** on 2026-08-28, `smith.service` was absent/inactive and lingering was disabled. Read-only inspection on 2026-09-11 found a separately installed legacy unit enabled but unavailable in an unbounded restart loop after host OpenCode discovery failed in the user-manager environment; the retry count reached 8,091 during QA. No manager mutation was performed by this source hardening, and the fixed unit/binary have not been deployed. The repository sandbox smoke cannot establish login, pre-login, reboot, or real bridge behavior.

**Required rollout order:** pass `scripts/smoke-user-service.sh --sandbox`; review the generated unit and binary path; obtain exact approval for normal-path install and live manager commands; test login-scoped behavior; then separately decide whether a pre-login lingering test is needed. Keep the primary Flutter desktop client, voice sidecars, Docker, and Ollama provisioning outside the supervisor installer and unit lifecycle.

## Validation checklist

Before treating a workstation as a Smith home, verify:

1. Docker, Git, OpenSSH, Rust, Python, Ollama, and systemd user services work for the intended account.
2. Local model inference uses the intended GPU path or is explicitly accepted as CPU fallback.
3. All intended local Smith APIs bind to loopback; no accidental worker/model/sandbox port is exposed.
4. Disk-space guardrails and backup/restore paths are tested.
5. A supervisor restart leaves no falsely-running task or unbounded worker.
6. The approved project can be built and tested from a clean clone using the documented toolchain.
