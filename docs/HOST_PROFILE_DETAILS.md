# Smith Ubuntu Workstation Host Profile

## Status and evidence

**Normative host contract plus dated, bounded observations.** Supported product: one trusted unprivileged operator, prepared Ubuntu24.04 LTS x86_64, root `client/`, `service/`, `installer/`, `dagger/`. Source promotion/legacy retirement does not install, stop, rename or migrate host software. Runtime `smith-next.service`, `smith-next` and XDG/keyring identities remain unchanged. [Scope](SCOPE.md), [Architecture](ARCHITECTURE.md) and [Process](PROCESS.md#approval-boundaries) govern capability/authority; [report](SMITH_LINUX_ARCHITECTURE_REPORT.md) and root `VALIDATION.md` qualify evidence. Never record credentials/serials/private topology/sensitive paths.

## Observed workstation inventory

These are **historical observations, not fresh probes** or recipient minimums.

| Area | Recorded observation/evidence |
|---|---|
| OS |2026-08-28: Ubuntu24.04.4 LTS Noble, Linux7.0.0-30-generic, x86_64; `/etc/os-release`, `uname`. |
| CPU/RAM |Ryzen9 7950X3D,16 cores/32 threads;61GiB reported RAM/8GiB swap; `lscpu`, `free`. Physical64GiB-class capacity/headroom not separately verified. |
| Storage |953.9GiB NVMe/encrypted LVM root; separate931.5GiB disk unmounted; root935GiB/771GiB available at inventory; `lsblk`, `df`. Placement/backup policy unresolved. |
| GPU |Initial PCI Navi31 family only; later approved device-only observation below identified RX7900GRE. |
| Tooling |Rust/Cargo1.98.0, Python3.12.3, CMake3.28.3, OpenCode1.18.25; Docker29.7.2 with AppArmor/seccomp/cgroup namespaces reported. These are historical developer facts, not recipient requirements. |
| Legacy service |Absent2026-08-28.2026-09-11 enabled `smith.service` observed auto-restarting without bridge, counter8091; lingering disabled. Source hardening was not installed by that task. Retirement is not host containment/repair. |

Detailed superseded inventory/service/voice/Ollama assumptions remain in the [pinned prior Host Profile](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/HOST_PROFILE.md), not active product prerequisites. [History](HISTORY.md) records the source relocation and retrieval policy.

## Current text-only recipient/builder boundary

**Implemented native source requirements, not clean-host proof:** keep `installer/dist/` complete: native CLI/optional GTK sibling, paired payload, manifest/checksums/instructions. Package defaults beside actual canonical installer executable, not cwd/alias/PATH. Neither installer persists in host command path; retain complete extracted package for maintenance.

| Recipient | Requirement |
|---|---|
| Account/OS |Prepared Ubuntu24.04 LTS x86_64, intended non-root account; production control/serve reject UID0. Native does not mean statically linked/universal Linux ABI. |
| Native runtimes |Compatible glibc/libgcc for installer/service; full Flutter native bundle/GTK3 and plugin/system libraries plus graphical session for desktop/GUI. GUI help/version/licenses avoid display initialization, not shared-library loading. CLI has no GTK/display/Python requirement. |
| Roots |Absolute owner-controlled safe existing XDG data/config/runtime roots for full setup, HOME for desktop/shim; runtime0700. Relative/unsafe/foreign roots block. Artifact-only service publication may use explicit safe data root without client HOME/config/runtime. Named subdirectories only after consent. |
| Lifecycle |Usable unprivileged `systemd --user`; current maintenance also needs kernel cgroup2 quiescence evidence. No sudo/system-unit install/user-manager provisioning. |
| Local models |Paired CPU/Vulkan runtimes plus separately explicitly provisioned curated GGUF/marker; no bundled weights/external Ollama/OpenCode. Service-only publication can lack runtime. |
| Optional cloud |Existing unlocked same-user GNOME Keyring/default collection plus explicit account/DNS/TLS egress, as below. |
| Not recipient dependencies |Python/pip, Rust/Cargo, Flutter/Dart SDKs, Dagger/Docker/build toolchains, old sidecars/audio utilities. |

Dry-run/initial decline needs only relevant artifact roots and creates no destination locks/files/Controller/provider. Full setup publishes paired artifacts then separately confirms registration/reload, persistent login Enable, Start, exact download, saved preference and optional no-chat inference. No recipient blanket yes; artifact-only `--yes` bypasses confirmation only. No OS libraries/GPU drivers/keyring/provider/service provisioning, implicit upgrades, debug-completion flag or automatic app launch. Explicit post-success Launch dispatches verified installed desktop in current graphical session, no model/service effect. Safe output for bounded setup/progress is pipe/socket/terminal, not regular-file/device redirection; artifact stdio/filesystem/fsync have no global deadline.

**Builders:** root Make/Dagger plus Docker, Rust/Flutter/Dart/Linux native development tooling, GTK/GLib/CMake/C++/Fontconfig/Pango/PNG development libraries and `glib-compile-resources` for embedded icon. Python Dagger/package tooling is builder-only. Official checksum-pinned Dagger v0.21.9 explicitly selects `--mod dagger`; engine/cache/dependency downloads are development effects. Retained compatibility locks are not proof of image-lock enforcement/hermeticity. Component source inputs are explicit; no host-home/session bus/manager/GPU/model mount to tests. Quality tests use actual Ubuntu24.04 UID/GID1000/Rust1.98, GUI isolated Xvfb; release Rust1.98 Bookworm builder. Separate Ubuntu non-root gate executes real CLI with no Python/useful PATH/toolchains. Actual native-ELF package fixtures and compiled Rust↔Dart cases are mandatory. Declared MSRV is not minimum-compiler evidence. Toolchain preparation root is not app runtime authority.

## Current login-boot recipient boundary

**Implemented source:** dedicated deterministic `smith-next.service`, `Type=simple`, `Restart=no`, `UMask=0077`, stop budget5s, `[Install]/WantedBy=default.target`. Enabled requires manager `enabled` plus exact owned wants link/target; static/runtime-linked/foreign/ambiguous facts block, not success. Binary/unit/enablement/runtime/bridge remain separate. Controller validates managed executable/unit/MainPID/socket/peer UID/PID; unsafe roots/overrides/conflicting listeners block. No ambient fake-manager command override or detached fallback.

Fresh recipient order: artifacts→register/reload→separately consented Enable→separately consented Start→fresh app/Core observation. Original60s tokens bind exact effects/facts, not renewed automatically. Enable never `--now`; Start never enables; neither invokes `loginctl`/changes lingering/legacy `smith.service`. Boot≤15s serialized read-only Core observation then separate finite app sync, no automatic effect/replay, exact enabled+safely-stopped Start only.

After-login default-target startup is the target, not proof of pre-login/post-logout availability. Smith never mutates lingering; a manual human host-policy change is separate scope/rollback. Normal shell environment does not automatically reach user service. Real manager/login/logout/reboot/stopped-manager/cgroup behavior and graphical close/reopen require exact separately approved QA; fake fixtures do not validate the workstation.

## Current bundled CPU/Vulkan runtime boundary

**Implemented source requirements:** separate SHA-pinned llama.cpp `b11429` CPU and Vulkan runtime/library/license trees in selected paired client, `$ORIGIN` runpaths, no mixed libraries. Both need compatible glibc/libstdc++/libgcc/OpenSSL3; libgomp bundled. CPU fixed `--device none --gpu-layers 0` does not load Vulkan. Vulkan needs host `libvulkan.so.1`, ICD/driver/existing unprivileged device access; installer changes none. No GPU container passthrough or AMD driver/permission/group setup is a source gate.

Auto attempts full offload only on fixed `Vulkan0`, requiring exact full-offload startup evidence/bounded normalized logs. Classified startup failure may permit one confirmed-cleaned independent CPU launch under original lease/deadline; no CPU replay after accepted runner/request failure. This is not device discovery/stable identity/resource sizing or acceleration proof. Runtime path integrity is point-in-time, not hostile-same-UID dynamic-loading isolation. Model FD binding prevents pathname substitution, not same-inode writes. Process ownership/API keys do not sandbox same-UID/GPU drivers.

### Non-model GPU diagnostic observation — GPU startup fix

**Recorded observation, not new probe/inference verification:** separately approved cleared-environment `vulkaninfo --summary` identified RX7900GRE/RADV NAVI31, Mesa25.2.8 and integrated Raphael; existing render-node read/write access was present. Installed manifest-verified runner `--list-devices` reported discrete `Vulkan0`,16368MiB/about14GiB free then, integrated `Vulkan1`. Active observed inference child was CPU-only. Device-only logging probes established verbosity/plain-log flags, loading no model. No driver/group/permission/install/service effect occurred. These refine initial PCI-family evidence, not persistent selection, usable budget, model allocation/full-offload/speed or authorization for new probes. [Pinned source-fix record](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-GPU-STARTUP-FIX.md) retains historical scope.

## Qwen3-14B installer baseline

**Implemented option, hardware suitability unverified:** fresh GUI Qwen3-14B Q4_K_M9001752960bytes≈8.38GiB weights; runtime/KV/buffers additional. Installer disk check size+64MiB is publication headroom, not RAM/VRAM fit. Seven pinned curated Apache-2.0 choices permit explicit smaller models; no free-form/import/bundled weights. Existing selection preserved. One executing slot, bounded threads/context4096/output1024/180s dispatch and600s idle expiry are provisional bounds, not CPU14B throughput. Model-default reasoning/final-only success does not promise fast output. Actual GPU and independent CPU load/inference/resource/desktop measurements remain separately approved gates; recorded64GiB-class host is not proof of current free capacity.

## Current next cloud-provider prerequisites

**Implemented source:** existing unlocked Ubuntu GNOME Keyring Secret Service/default collection for the same account as persistent service. Fixed `/run/user/<uid>/bus`, socket/peer/owner UID and unique-owner pin, `NoAutoStart`, encrypted session. No ambient D-Bus/TCP/autolaunch/credential environment/plaintext files, collection creation, activation, unlock or Prompt. CLI hidden input requires terminal but no GTK; keys never enter Flutter. Unavailable/locked session fails actual dispatch before HTTP; local operation remains independent. Saved active generation+supported model permits a new explicit Send/Validate without mandatory probe/prior validation; advisory metadata is not guaranteed access, and actual exact-generation read/revision/identity checks are mandatory.

Read-back only GNOME `text/plain` or `text/plain; charset=utf-8`, all four Smith identities optionally exact standard `xdg:schema=org.freedesktop.Secret.Generic`, no unknown extras; unchanged timestamps/secret checks. **Retained fixture evidence:** disposable Ubuntu24.04 GNOME Keyring46.1-2ubuntu0.2 with private bus/unlocked collection/no host mounts/runtime network exercised encrypted synthetic Save/read for both providers and five-attribute compatibility. A prior separately approved metadata-only host diagnostic read no secrets and proved no active-generation/API validity. Neither proves installed repair, all GNOME versions/timing or vendor access; strict timestamp-integrity concern was not waived. Historical [MIME](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/evidence/provider-save-fix/README.md)/[schema](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/evidence/provider-schema-fix/README.md) evidence preserves failures and limited outcomes at its original checkpoint.

Authorized DNS/TLS HTTPS443 only to official OpenAI Responses/Anthropic Messages endpoints in [Architecture](ARCHITECTURE.md#current-next-cloud-routing-and-credential-boundary), normal certificate checks. No proxy/custom trust/endpoint/subscription login, discovery/retry/fallback. Operator API account/billing independent from consumer apps. Fifteen static choices are not entitled-model discovery. Cloud inference does not launch local runner/require GPU, but bypasses neither normal installation/local Model setup nor service/session/native prerequisites. Cloud8192 total including reasoning/180s is a bound, not latency/cost/access guarantee; local1024 unchanged.

Disk4 retains opaque credential refs only; keyring separate. Save replacement retains old/uncertain inactive items; no P0 credential removal. Uninstall retains keyring and does not revoke vendor credentials. JSON backup excludes keys; approved migration/restore preserves exact generation relationships and compatible paired binaries, never substitutes current key. Login-session prerequisite is not headless/pre-login/logout or hostile-same-UID isolation. Real-key/API/charges/installed migration require fresh exact scope/evidence; source checks do not authorize them.

## Installer appearance resource requirements

**Implemented:** optional GUI embeds fonts/PNG icons/notices; prepared GTK3 Fontconfig/PangoCairo/PangoFT2/PNG decoder plus Linux sealed memfd/readable `/proc/self/fd`. Check support before effect worker; failure has no disk/network fallback or system font install/cache command. Builder needs development link files; recipient no generator/Python/SVG decoder provisioning. Display-free licenses/help/version still load GUI libraries. Separate CLI imports no GTK/font resources. Fixture rendering is not installed desktop or redistribution acceptance.

## Explicit maintenance runtime requirements

**Implemented source:** prepared user-manager/cgroup2 environment/safe HOME/XDG and independently verified old pair. Only exact legacy-static/current-installable managed unit, no drop-ins/transient/foreign enablement. Stop proves captured executable/cgroup identity, PID0, descendant quiescence and absent sockets; unknown blocks, no rawPID kill/stale-socket deletion/old-helper execution. Installation→Core→runner leases serialize cooperating operations. Upgrade preserves artifacts, exact static migration, exact-enabled no-churn/current-disabled separate Enable, separate Start; ordinary Install never overwrites differing binary. Uninstall disables/verifies before unregister and preserves ALL user data/releases/backups/stages/objects. Retained data consumes disk, no purge/rollback. No legacy service/linger/GPU mutation. Fake kernels/managers/Xvfb are not live integration evidence.

## Next-generation approved upgrade observation — 2026-10-11

**Historical narrow host result, not current installed inventory:** one-time explicitly approved older voice/chat paired upgrade backed up binary/state/unit/launcher metadata, replaced artifacts and started exact unchanged isolated unit; observed Core/app/MainPID ready/setup true. Unit remained static, no Enable/linger/legacy mutation, no GUI/provider/model/audio/container smoke and no selected model/chat at check. That approval is consumed; source trimming/promotion does not deploy newer text/native/cloud/disk4 candidate or prove the host still has that state. Exact historical [execution evidence](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-VOICE-CHAT-UPGRADE-EVIDENCE.json) remains retrievable; do not rewrite it as current readiness.

## Required host capabilities

**Current builder/development:** Git, POSIX shell, Python builder tooling, approved Docker/Dagger, Rust quality tools, Flutter/native tooling and storage for builds/evidence. Choose/document Docker rootful/rootless access deliberately; daemon access is privileged, never worker app capability. Desktop recipient requires only prepared native/session/manager/runtime/model contracts above; no old OpenCode/Ollama/voice dependency.

**Target/deferred orchestration workstation:** scoped repository Git/OpenSSH/tools/worker access, explicitly designed model endpoints/egress and encrypted backups, not broad default host/home mounts. Runtime Docker worker architecture/headless/search/voice must be separately planned/approved rather than inherited from legacy directories.

## Record before operation

In approved non-secret deployment record: Ubuntu/kernel; account and safe root identities; exact paired candidate/runtime/model versions/digests; CPU/RAM/swap/free disk; intended GPU/loader/ICD/access/memory/headroom and **measured** inference if authorized; user-manager/unit/enablement/socket state and separate lingering policy; keyring readiness without secret data; exact egress/destinations/firewall/non-loopback exceptions; builder Docker access/cache policy where relevant; backup encryption/retention and tested compatible restore. Never infer service environment from interactive PATH or run probes outside approval.

## Resource policy

Current chat admits4 outstanding/1 per conversation/1 actual provider,10min queue-age dispatch guard/180s cooperative inference, bounded context/state/receipts. Panes/windows/local follow-ups add no inference slots or parallel KV allocation. Preserve desktop responsiveness; bound/serialize GPU-heavy builds/inference and at most1 consequential pipeline/project/branch until a reviewed scheduler exists. Treat RAM/VRAM/disk/cache/I/O as finite; defer instead of causing host pressure. Reserve backup/model/artifact space; preserved releases/stages/uninstall objects accumulate. Source caches are reconstructable only after classification; private logs/untracked work are not in Git. No blanket cleanup or Git-history recovery claim for ignored outputs.

## Linux service policy

Unprivileged deterministic user unit/stable managed XDG binary, never register repo/debug/release build directly as permanent service. Keep registration/current runtime/persistent enablement/bridge/provider facts separate. No Start-as-Enable, `--now`, detached fallback or `loginctl` mutation. Do not claim enabled means running/pre-login. Explicit Stop/maintenance must verify quiescence and retain partial/uncertain effects. Source-only promotion needs no installed-state rollback; any later disk4 migration needs separately approved pre-state plus compatible binaries/keyring restore. Legacy host installations stay untouched unless exact separate authority covers them.

## Validation checklist

Before claiming prepared-recipient readiness: validate compatible native ELF/library/full-pair integrity; safe account/roots/user manager/private bridges; exact independent Enable/Start and installed maintenance/quiescence; controlled restart interruption/no replay; login/logout/reboot scope; explicit curated download/cancel/check/removal and GPU/CPU resource behavior; optional real keyring/account/model/charges acceptance; native graphics/IME/accessibility/window-close behavior; backup/restore/keyring relationships; egress/locality; license/icon/SBOM/provenance and independent review. Source/root relocation tests are prerequisites, not these live checks or deployment/publication permission.
