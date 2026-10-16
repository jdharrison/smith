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

**Separate legacy candidate packaging target:** Ubuntu **24.04 LTS x86_64**, full Flutter Halo bundle with `smith_backend/smith` beside complete `lib/`/`data/`, plus a secondary standalone backend candidate; see [Chat Release](CHAT_RELEASE.md). These legacy scripts/recipient requirements are not the native `next/` installer contract below and are not migrated by it. The historical inventory above is not a new provisioning or compatibility observation.

- **Recipient:** Ubuntu runtime libraries for the backend; Bash, Python 3.12, `tar`, and `sha256sum` for install/verification; separately provisioned/running Ollama at `127.0.0.1:11434` and an exact installed local tag, either already present or explicitly downloaded through setup before `serve --chat-only --model <exact-installed-tag>`. Halo additionally needs a graphical session, GTK3 and native plugin/system libraries. Flutter is primary: guided Boot/Setup automatically validates `<actual Halo executable directory>/smith_backend/smith`, with explicit advanced absolute-path configuration, read-only structured inventory, observed-model selection and review before explicit Start, or untouched attachment to a compatible ready external bridge. No cwd/PATH scan or shell fallback exists. Opening/checking never starts the chat backend, infers or downloads; opening can execute read-only `setup inspect`. Install the entire Halo bundle, not just its executable.
- **Builder:** Rust/Cargo and Flutter/Dart toolchains plus Linux compiler/CMake/native development dependencies; candidate assembly additionally needs Python, Git, and `readelf`. Recipients do not need build tools, Docker, OpenCode, or Python voice sidecars for chat-only operation.

Neither archive bundles Ollama/model weights or installs dependencies, starts applications/services, changes PATH/desktop registration, or migrates the legacy systemd unit. Empty-host setup remains partial: missing Ollama gets guided separate installation/start instructions, with no sudo or legacy install helpers. The approved [wizard plan](plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md) narrowly permits operator-confirmed model pull through already-running Ollama, not OS/runtime provisioning. Exact-model/operation-bound stdin approval, bounded NDJSON progress/cancel and finite deadlines do not constrain external daemon internet/disk effects; sizes may be unknown until reported and cancellation may leave continuing daemon work/partial cache. No automatic retries or cache deletion. Read-only inventory is not inference proof, and setup does not add provisioning tools/routes to the chat-only bridge. See [Architecture](ARCHITECTURE.md#desktop-setup-command-boundary) for mutation guards; inspection may run with an active backend, and no global daemon lock is claimed. Provisioning and launch/install effects need separate approval. Desktop Start does not provision/start Ollama, register services or mutate the legacy unit. CLI remains secondary query/status/configuration with existing commands retained; future headless operation is a separate deferred deployment/lifecycle capability, not CLI. Owned-child session correlation, stdin EOF and bounded TERM/KILL normal-close cleanup are described in [Architecture](ARCHITECTURE.md); no crash-proof guarantee is made. Actual provider pull/cancel integration and version compatibility, model resource/acceleration requirements and transitive runtime libraries remain to be validated; real network-denied inference and clean-host desktop installation without build tools are publication blockers, not completed checks.

## Next-generation persistent service target

**Target plus implemented source subset; no new host inventory or installed-service claim:** the reset requires Smith service lifetime to be independent of desktop lifetime. The current `next/` source implements a dedicated unprivileged systemd user unit, private Unix bridge, exact persistent default-target enablement and bounded client Boot/attachment after user login. Close/quit detaches without stopping already-authorized work. Live-host validation, actual login/logout/reboot recovery, pre-login/post-logout operation and lingering still require fresh exact approval. This is not permission to install/enable/start a unit, alter the existing legacy service or change lingering. See [the product reset](plans/LOCAL-SMITH-PRODUCT-RESET.md) and [login-boot plan](plans/LOCAL-SMITH-NEXT-LOGIN-BOOT.md).

## Next-generation debug recipient/builder boundary

**Implemented source requirements, not clean-host proof:** the paired client bundle includes `smith_backend/smith-service`; recipients need the Ubuntu desktop runtime plus a usable systemd user manager and existing absolute, owner-controlled XDG data/config/runtime roots. The runtime root must be 0700. Control and serve run unprivileged and reject UID zero. The flow acts only on the isolated `smith-next` namespace, with no sudo, `loginctl`, lingering mutation, legacy `smith.service` migration, provider installation or implicit model download. Strict schema 2 accepts login enablement only when manager state and the exact owned `default.target.wants` link agree. A stopped/unavailable manager, conflicting files or unsafe roots are explicit repair/blocking states.

Builders need Rust, Flutter/Dart and Linux GTK/GLib native tooling. Dagger prepares toolchains as container root, then runs source builds/tests as UID/GID 1000. Native tests use injected fake managers and captured temporary foreground daemons, not the real user manager. Live unit install/enable/start, actual login/logout/reboot and graphical close/reopen remain separately approved integration gates.

## Next-generation client icon build inputs

**Implemented in the scaffold:** the user-supplied PNG is embedded in the GTK runner as a GLib resource. Builders need C/C++ compilation and `glib-compile-resources` from the GTK/GLib development tooling; recipients do not need that compiler tool. The Linux bundle includes desktop launcher/icon inputs under `share/`, but building/exporting it does not register them on the host. GNOME/Wayland dock/launcher display requires separately installed desktop integration and is not established by source/build tests.

## Next-generation developer artifact installation

**Implemented source contract, not a host inventory:** `next/` Make install targets require Dagger/Docker and builder toolchains, then invoke the native Rust ELF exported by `make installer/binary` to `installer/build/smith-installer`. Python is builder-only, not the host artifact publisher or a desktop runtime dependency. Make uses explicit `--artifacts-only --client BUNDLE` and/or `--service BINARY`; only that mode permits optional `--yes`, without bypassing safety checks or authorizing lifecycle/provider effects. The invoking Ubuntu 24.04 x86_64 account must be non-root with an existing safe owner-controlled XDG data root, plus HOME for client/menu/command publication; an explicit service-only data root does not require client HOME or config/runtime roots. Named subordinate destinations may be created after confirmation. Installed complete bundles retain executable-relative helpers; the stable standalone binary matches the managed service's XDG data path. No OS dependencies, units, login policy or providers are installed/started. The menu entry uses the integrated icon; actual GNOME/Wayland appearance remains a separately approved host check.

## Next-generation approved upgrade observation — 2026-10-11

**Observed narrow host result:** the operator explicitly approved one-time upgrade of the isolated `smith-next.service` and matching desktop artifacts. The previous binary and existing state/unit/launcher metadata were preserved in a private backup; reviewed voice/chat binary and paired client were installed. The exact unchanged unit was explicitly started and observed active/running, with verified Core and app Unix bridges and preserved setup completion. Enabled-at-login remains **static**, with no enable/linger or legacy-unit mutation. No GUI/provider/model/audio or container smoke occurred; no selected model or saved chat was present at validation. This supersedes source-only installation wording for this specific candidate/bridge scenario, not broader runtime guarantees. See [upgrade execution evidence](plans/LOCAL-SMITH-NEXT-VOICE-CHAT-UPGRADE-EVIDENCE.json). That one-time host approval is consumed; it does not authorize installation of the native installer/text candidate or further live effects.

## Current text-only recipient/builder boundary

**Implemented native source requirements, not a new installed-host observation:** [the native migration](plans/LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md) supplies the unchanged Linux x86_64 CLI plus optional GTK3 `smith-installer-ui` sibling from the independent `next/installer` Rust crate, with complete paired Flutter/Rust payload and metadata under `next/installer/dist/`. Keep the package together; its default root is the actual canonical executable directory. Recipients need prepared Ubuntu 24.04 LTS x86_64, compatible native installer/service runtime libraries, GTK/native desktop libraries and a graphical session for the client, safe owner-controlled existing XDG data/config/runtime roots for full setup, a usable unprivileged user manager, and separately installed/running Ollama at `127.0.0.1:11434` for chat/CLI model work, not GUI artifact review/publication. Native is not a static-linking or universal Linux ABI guarantee. Python, pip, Rust/Cargo, Flutter/Dart, Dagger and Docker are not recipient installation/use dependencies. Help does no operational discovery; dry-run/initial decline require only relevant artifact roots and do not inspect a manager/provider or create locks/files.

Full recipient mode publishes Smith artifacts, then separately confirms exact isolated-unit registration/reload, persistent login Enable, Start, optional exact-model download, saved preference and optional direct minimal inference. Enable runs separately before Start, never uses `--now`, and neither operation invokes `loginctl` or changes lingering. There is no recipient `--yes`. Direct `smith_service::Controller` reuse validates against the packaged service executable, not installer bytes, without changing the client/service. It does not provision OS libraries, Python, Ollama or GPU drivers, automatically launch the desktop, change lingering, perform an implicit upgrade or write the old setup-completion flag. The GUI's explicit post-success Launch Smith action now opens only the verified installed client in the current graphical session; it adds no service/model action or recipient dependency. See [installer launch](plans/LOCAL-SMITH-NEXT-INSTALLER-LAUNCH.md). Models remain external to the default app bundle; inventory is not inference, and offline packs are future separately reviewed artifacts. Bounded setup/progress output needs a pipe, socket or terminal, not regular-file/device redirection; this does not promise a total filesystem/fsync or artifact-stdio deadline.

**Optional GUI requirements:** [the visual slice](plans/LOCAL-SMITH-NEXT-INSTALLER-UI.md) needs compatible GTK3 shared libraries and a graphical session. GUI `--help`/`--version` avoids GTK initialization and display access but still loads those libraries. The default CLI requires neither GTK nor a display nor Python. Both installer ELFs are included in the full package, but neither is persistently installed; keep the package. Review requires only artifact roots, with no locks/lifecycle/root-fact writes; config/runtime roots and a user manager are needed only for selected GUI lifecycle work.

Builders retain Dagger/Docker, Rust and Flutter/native development toolchains; Python Dagger/package tooling only assembles/verifies metadata around supplied native artifacts. Dagger's native quality/parity tests compile/run on actual Ubuntu 24.04 as UID/GID 1000 with Rust 1.98 tooling and typed fake managers/providers/private fixtures. Release binaries use the Rust 1.98 Bookworm builder. A separate Ubuntu 24.04 non-root runtime gate executes the actual ELF's help/version/artifact-only dry-run with no Python or builder toolchains, and builder package tests consume the actual compiled installer ELF with synthetic paired payloads. GUI builds enable Cargo feature `gui` with GTK3 development libraries; GUI quality/widget tests use actual Ubuntu 24.04 UID/GID 1000 and isolated Xvfb, not the operator's display. The current login-boot component `make installer/test` PASS includes that isolated GTK gate; fresh independent review remains pending. These component gates are not a claim of aggregate candidate validation, independent assurance or full clean-host setup. The declared MSRV is not actual minimum-toolchain compile evidence. Retired Python installer sources/tests are excluded checksummed reference, not a fallback or recipient dependency. Archived speech dependencies, images, model mounts, parecord/paplay, audio session and voice GPU devices are not active text-product prerequisites. The migration leaves the October11 installed voice candidate and host data/services unchanged; text deployment and real pull/inference/graphics need fresh approval/evidence.

## Current next cloud-provider prerequisites

**Implemented source requirements under [cloud-provider plan](plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md), not live-host validation:** optional OpenAI/Anthropic use adds a prepared Ubuntu 24.04 x86_64 **GNOME Keyring Secret Service with an existing unlocked default collection**, available to the same unprivileged account as the persistent Smith service. `next/service/src/cloud_keyring.rs` connects only to the fixed `/run/user/<uid>/bus` Unix socket, verifies socket/peer/service-owner UID and addresses the existing unique Secret Service owner. It does not use `DBUS_SESSION_BUS_ADDRESS`, TCP/autolaunch addresses, shell API-key variables or plaintext credential files. Calls prohibit autoactivation; no keyring start/install, collection creation, unlock or Prompt is attempted. An unavailable/locked session fails an actual cloud attempt closed before HTTP, while local operation remains separate. A saved active reference is sufficient to attempt Send/Validate under the [saved-key contract](plans/LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS.md); stale or absent observations and previous validation results do not require a preliminary probe or disable the attempt. The [provider-save MIME correction](plans/LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX.md) was exercised with synthetic keys against real GNOME Keyring `46.1-2ubuntu0.2` in a disposable Ubuntu 24.04 container: encrypted Save/read succeeded once for each provider. The container had its own bus/collection, no mounts and no runtime network. This is compatibility evidence for that fixture/version, not observation or repair of the installed user's session. The subsequent [schema follow-up](plans/LOCAL-SMITH-NEXT-PROVIDER-SCHEMA-FIX.md) accepts only the standard optional `xdg:schema=org.freedesktop.Secret.Generic` metadata alongside four exact Smith identity fields. A separate disposable run of the same GNOME version read encrypted synthetic five-attribute items for both providers and saved/read fresh normal identities once each; all observed timestamps remained equal. A prior separately approved metadata-only host diagnostic supports the compatibility finding but read no real secrets and established no active-generation/API validity. Neither fixture proves installed-host repair, arbitrary GNOME-version/timing compatibility or provider access. This is a login-session prerequisite, not a pre-login/headless/logout guarantee or authentication against hostile same-UID processes.

Cloud inference needs authorized DNS/TLS connectivity to the two exact HTTPS/443 endpoints in [Architecture](ARCHITECTURE.md#current-next-cloud-routing-and-credential-boundary), normal certificate validation, and operator-provided API access/billing independent of consumer chat subscriptions. Ambient proxies, custom endpoints/trust and subscription login are unsupported. Cloud dispatch itself does not launch the local runner or depend on a local GPU, but it does **not** replace or bypass normal installer/local Model setup or the existing app/service/GTK prerequisites. No OS, driver, keyring, user-manager or lingering provisioning is added. Local generation remains capped at 1024 tokens; cloud requests cap total generation at 8192 including reasoning under the 180-second service budget. These are bounds, not throughput, account-access or cost guarantees.

Saved keys live in the keyring; disk3 stores opaque references, not keys. Replacement retains prior generations and uncertain publication may retain inactive items. P0 provides no credential removal/purge; Uninstall keeps keyring items and does not revoke provider credentials. A state backup does not include keyring items, so approved disk3 restore must preserve exact generation relationships rather than resolve another key automatically. CLI provider management remains GTK/display-free but still requires that accessible unlocked session and a terminal for hidden key entry. Apart from the separately scoped metadata-only diagnostic described above, no normal-host keyring access, provider inference/billing, installed migration or deployment was performed by these source corrections; final source/package gates and fresh assurance belong in `next/VALIDATION.md`, not inferred host readiness.

The [static-catalog expansion](plans/LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION.md) adds fifteen fixed text-model choices and a local picker filter, not host prerequisites or vendor discovery. No real key/model-access check is implied. Existing keyring and two inference endpoints remain unchanged; strict older clients/installers require the rebuilt paired catalog-aware artifacts.

## Installer appearance resource requirements

**Implemented source, no new host provisioning:** [the installer redesign](plans/LOCAL-SMITH-NEXT-INSTALLER-REDESIGN.md) embeds JetBrains Mono and PNG icon derivatives in the GUI ELF. The supported GTK3 stack already supplies Fontconfig, PangoCairo/PangoFT2 and a PNG decoder; builders need their development linker files. GUI font registration additionally uses Linux sealed `memfd_create` files and readable `/proc/self/fd`. Denied support gives a startup error before the worker, not a disk/network fallback. No fonts, SVG decoder, Python generator or new system package are installed for the recipient by Smith. GUI help/version/`--licenses` remain display-free, not native-library-free; the separate CLI stays GTK-independent. Fixture rendering is not installed desktop acceptance.

## Current login-boot recipient boundary

**Implemented source requirements, no live-host observation:** `smith-next.service` now has deterministic `[Install]/WantedBy=default.target` and retains `Type=simple`, `Restart=no`, `UMask=0077` and its five-second stop budget. Strict schema 2 reports independent exact enablement facts; only manager `enabled` plus the exact owned default-target wants link counts as enabled. The installer separately confirms Enable then Start, without `--now`, `loginctl` or lingering mutation. The client performs serialized read-only Boot inspection for at most 15 seconds, synchronizes conversation only after verified bridge readiness, and offers separately confirmed Start only for the exact enabled safely stopped state.

This targets startup after the intended user logs in. With lingering unchanged, it does not guarantee pre-login or post-logout operation. Observed component tests are recorded in `next/VALIDATION.md`; real manager behavior, login/logout/reboot and installed Upgrade/Uninstall are unverified and require fresh exact approval. No aggregate build/test, independent assurance, release or deployment claim follows.

## Current bundled CPU/Vulkan runtime boundary

**Implemented source requirements, with a separately approved non-model observation below:** [native-runner plan](plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md), extended by the first [GPU-preferred slice](plans/LOCAL-SMITH-NEXT-GPU-PREFERRED.md), supersedes the external Ollama requirement for `next/` above. The paired client includes separate SHA-pinned CPU and Vulkan llama.cpp `b11429` runtime/library/license subtrees. Both need compatible glibc/libstdc++/libgcc/OpenSSL3; libgomp is bundled. The CPU executable explicitly disables devices/GPU layers and does not load the Vulkan backend. The Vulkan subtree additionally needs compatible host `libvulkan.so.1`, ICD/driver and existing unprivileged device access; Smith installs or changes none of them. Models are separately provisioned curated GGUF files.

The earlier PCI-only AMD Navi31 inventory did not establish exact model, usable dedicated memory, `Vulkan0` suitability, driver support or performance; the separate observation below refines only the observed capability facts. The current Auto path attempts full offload only on `Vulkan0`; missing/unusable Vulkan can fall back to CPU under the source policy. Device discovery/classification, stable explicit selection, numerical memory/desktop headroom and partial offload remain unresolved. Prepared Ubuntu24.04 x86_64, safe existing XDG roots, user manager and desktop/GTK prerequisites otherwise remain. The original first GPU-preferred source slice included no host GPU probe, driver provision, permission/group change, real inference or installed upgrade.

### Non-model GPU diagnostic observation — GPU startup fix

**Observed, not inference verification:** during the separately approved read-only investigation preceding [the source fix](plans/LOCAL-SMITH-NEXT-GPU-STARTUP-FIX.md), `vulkaninfo --summary` with a cleared environment identified RX 7900 GRE / RADV NAVI31, Mesa 25.2.8, and the integrated Raphael device. Existing render-node read/write access was available. The exact installed manifest-verified Vulkan runner's `--list-devices` identified the discrete card as `Vulkan0`, reporting 16368 MiB device memory and approximately 14 GiB free at that instant; `Vulkan1` was integrated graphics. This refines the earlier PCI-family-only observation, not a persistent device identity or resource budget. The observed active inference child was CPU-only (`--device none --gpu-layers 0`).

Device-only logging probes confirmed default suppression/decorations and the supported normalization flags. No model was loaded by these probes; no driver/group/permission/install/service change occurred. Actual GPU model allocation, full offload, inference and speed remain unverified. The subsequent source-fix approval does not authorize additional host probes or deployment; source/fake tests do not replace those separate live gates.

## Qwen3-14B installer baseline

**Implemented source option, hardware suitability unverified:** model wizard defaults to Qwen3-14B Q4_K_M (9001752960 bytes ≈8.38GiB weights alone). Runtime/KV/buffer memory is additional; installer checks disk size+64MiB during publication, not a guarantee of RAM suitability. CPU-only, one slot, context4096/output1024/service180s provisional budgets. Recorded64GB-class hardware does not prove actual available memory or adequate throughput. Thinking uses pinned runner/model defaults, not forced zero. No weights, live model test, GPU/device change or installed-service upgrade under this source task. Smaller curated models remain selectable; app/service/runtime host requirements above remain.

## Explicit maintenance runtime requirements

**Implemented source, not a new installed-host observation:** optional visual installer performs explicit Upgrade/Uninstall using independently verified older managed artifacts. It requires the prepared Ubuntu/systemd-user/cgroup2 environment, safe XDG/HOME roots and only a byte-exact legacy static or current installable managed unit, with no overrides/transient/foreign enablement. Upgrade migrates exact legacy static to current, preserves exact current enabled state without churn, and separately enables exact current disabled state before separately confirmed Start. Uninstall disables and verifies before unregistering. Stop proof reads kernel cgroup hierarchy and verifies socket absence; unknown quiescence blocks. No sudo/GPU/`loginctl`/linger/legacy-`smith.service` changes or old-helper execution. All user data and release/backup objects are retained. Current live user-manager/kernel behavior is not established by fake fixtures; real operation remains separate approved QA. The recorded older voice candidate is unchanged by this source work.

## Archived next-generation voice/chat runtime prerequisites

**Implemented source requirements; no real-provider readiness claim:** local Ollama must already run on numeric loopback11434 with an exact eligible installed model. Speech providers use separate loopback4890/4891 and operator-provisioned CPU CTranslate2 Whisper and matching Kokoro v1.0 ONNX/voice artifacts. Compose/Docker is a voice deployment dependency, not a desktop build tool substituted for runtime. Models must be readable by the declared container UID through narrow read-only mounts; startup/inference never pulls weights. No GPU passthrough is required or validated for voice; Ollama acceleration remains an independent host concern.

The client requires `/usr/bin/parecord` and `/usr/bin/paplay` from Ubuntu `pulseaudio-utils`, plus a usable PulseAudio-compatible user session (including PipeWire Pulse). Missing audio access leaves text chat usable. Microphone capture and playback occur in the client only after explicit actions; providers receive bytes, not device/host-path access. Python/native dependencies are built into the speech image; no microphone, Docker socket, broad home or audio-session socket is mounted. Locality does not authenticate against other trusted-host processes. Real weights, speech quality, loopback/container isolation and installed-host lifecycle remain separately approved integration gates.

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

**Implemented next chat admission:** [multi-chat](plans/LOCAL-SMITH-NEXT-MULTI-CHAT.md) permits four outstanding chat requests but only one provider invocation/native runner slot. Queue age is checked at dispatch (ten minutes); the existing180s provider and CPU/thread/context budgets remain. Conversation/state/receipt bounds and completion reservations are enforced independently of model memory. No new host dependency, GPU probe, parallel KV allocation or measured throughput guarantee follows. This chat FIFO does not relax consequential-pipeline serialization below.

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
