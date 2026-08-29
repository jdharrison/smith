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
| Smith user service | `smith.service` not installed/active; systemd user lingering is disabled | `systemctl --user`; `loginctl` |

## Required baseline and unresolved settings

| Area | Required baseline | Current status |
|---|---|---|
| Operating system | Ubuntu LTS, x86_64, supported kernel/userspace | Satisfied by observed Ubuntu 24.04.4 LTS; maintain through supported updates. |
| Memory | At least 64 GiB physical RAM, with deliberate swap/recovery headroom | Appears to be 64 GiB-class hardware; verify installed physical memory before relying on it. |
| GPU | AMD GPU with sufficient VRAM for the selected local model | Exact marketed GPU model and usable VRAM remain unverified. |
| Inference | Native host Ollama using verified AMD acceleration when local GPU inference is required | Not provisioned/verified. |
| Storage | Deliberate allocation for repositories, Docker, models, and encrypted backups | Encrypted root and second disk are present; Docker/model/backups placement is undecided. |
| Service model | `systemd --user` supervisor with a deliberate login/lingering policy | Smith service absent and lingering disabled; background operation is not yet configured. |
| Containers | Docker Engine reachable by the Smith user without interactive privilege prompts | Docker API is reachable; rootful/rootless decision is not yet recorded. |
| Network posture | Loopback-first inbound services; intentional outbound Git, registry, model, and search traffic only | Firewall and non-loopback exposure policy must be recorded and verified. |

## Required host capabilities

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

- Smith runs as a user service, not a privileged system service.
- Decide explicitly whether user lingering is enabled; do not imply boot-time operation from login-only behavior.
- Use an environment-file or secret-reference approach deliberately; interactive-shell environment variables do not automatically reach systemd user services.
- Never register a transient development binary as the permanent service executable.
- Test a controlled stop/restart and stale-job recovery before relying on background operation.

## Validation checklist

Before treating a workstation as a Smith home, verify:

1. Docker, Git, OpenSSH, Rust, Python, Ollama, and systemd user services work for the intended account.
2. Local model inference uses the intended GPU path or is explicitly accepted as CPU fallback.
3. All intended local Smith APIs bind to loopback; no accidental worker/model/sandbox port is exposed.
4. Disk-space guardrails and backup/restore paths are tested.
5. A supervisor restart leaves no falsely-running task or unbounded worker.
6. The approved project can be built and tested from a clean clone using the documented toolchain.
