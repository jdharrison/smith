# Smith next — text first

## Current bundled native runner

**Implemented source:** [native-runner plan](../docs/plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md) supersedes external Ollama setup/provider descriptions from earlier stages below for production `next/` only. Smith bundles separate CPU and Vulkan llama.cpp runtimes in the paired client release; no external Ollama is required. Weights remain separate. List choices with `smith-installer --models`; use `--model EXACT --download-model` (`--pull-model` alias) with separate exact-name download, preference and optional inference consent. The focused visual installer now provisions models and checks them through the service; Qwen3-14B is the fresh-only default selection. Neither installer is persistently installed: retain the full package.

Runtime/model verification, private Unix IPC, one runner lease, bounded context and service-owned cancellation/cleanup are described in canonical Architecture. Production loads the held verified model FD; app-owner teardown cancels/joins native inference, while desktop close only detaches. No implicit upgrades, retries, cloud/tools, bundled weights or real-inference/real-GPU/clean-host/release guarantee. Chat currently attempts fixed full offload on `Vulkan0` and falls back once to independent CPU only after classified startup failure and confirmed cleanup; Smith does not discover/provision drivers or prove acceleration. Unused installer Ollama code remains legacy library/test coverage, not a production fallback. See [validation history](VALIDATION.md) for observed checks.

**Implemented source:** a dark conversation-first Flutter desktop and independent Rust service for local text chat. No onboarding wizard, microphone/playback controls, speech providers, cloud fallback, tools or bundled model weights. Opening first observes the exact login-enabled service through a minimal Boot surface and synchronizes before mounting chat. It never installs, enables, downloads or sends a turn; only an exact safely stopped installation offers a separately confirmed **Start Smith** action.

```text
next/
├── client/          Conversation UI, typed draft and verified native helper
├── service/         Rust durable text worker, private native llama.cpp adapter and Core control
├── dagger/          Developer pipeline and builder-only package metadata tooling
├── installer/       Native Rust CLI + optional GTK3 shell; paired package under dist/
├── dagger.toml      Default developer pipeline entrypoint
└── Makefile         Developer shortcuts
```

## Application

Smith opens into a minimal Boot loader while it observes the login-enabled service. No chat controls mount until exact enabled/bridge-ready facts are observed and conversation state synchronizes. Exact enabled+safely-stopped state may offer separately confirmed **Start Smith**; disabled, missing, conflict and unknown states remain observation/setup guidance. The legacy `setup_complete` boolean is not chat readiness; executable, unit, enablement, manager, peer and saved-state integrity checks still apply.

A selected exact curated local model is required. The workspace adopts current-service-instance inventory on reattach; missing inventory is observed when work is idle or through explicit Retry, never by inference or download. Provisioning and selection belong to the separate installer.

Only explicit **Send** submits text. Accepted work belongs to the service and may continue after desktop close. Uncertain acknowledgements are queried, never automatically resent. Verified terminal answers remain visible through synchronization outages without duplicating authoritative history or inventing user messages.

**Multi-chat is implemented in source:** New chat opens/reuses an unsaved draft and becomes durable when its first message is accepted. Switch between isolated chats; Rename/Archive/Delete live in each row's menu, and Archived chats allows Reactivate/Rename/Delete. Delete requires confirmation and removes current-store content, not backups or secure memory/disk remnants. Target unfinished/uncertain work blocks archive/delete, while unrelated chats remain usable.

Up to four chat requests can be outstanding: one generating and three FIFO queued, not simultaneous inference. Histories, titles, lifecycle and accepted jobs are durable; drafts and reading positions remain process-local. Each chat retains at most16 complete turn pairs/64 KiB, with32 saved chats total,32 retained operations and1024 expired-ID tombstones. Encoded storage/completion headroom is bounded; capacity refusal never erases replay protection. Restart interrupts queued/running jobs without replay. Archive is not unlimited history.

Desktop app protocol/storage are v2 with strict valid-v1 migration; the native installer retains its exact v1 interface. Before a separately approved live upgrade, retain an exact pre-migration state snapshot and paired-artifact restore procedure: old binaries cannot read v2 state. See [multi-chat plan](../docs/plans/LOCAL-SMITH-NEXT-MULTI-CHAT.md) and [validation](VALIDATION.md).

## Developer commands

Run from `next/`:

```sh
make test             # Rust/Flutter quality + isolated publisher/installer/package tests
make build            # Export standalone service, client and recipient package
make service/test
make client/test
make installer/test   # Native/wizard quality, isolated GTK smoke and no-Python CLI gate
make installer/ui-test # Optional-feature GTK quality and isolated Xvfb widget smoke
make package/test     # Builder metadata tests with the compiled native installer
make service/build
make client/build
make installer/build
make installer/run    # Build/verify the full package, then launch the installer GUI
make installer/binary # Export the unchanged CLI without building the desktop package
make installer/ui     # Export installer/build/smith-installer-ui (binary only)
```

Formatting/lint are included in `/test`; there is no separate public check command. Dagger is developer-only. Builds run unprivileged after root-only toolchain preparation. Generated outputs are ignored. `make client/run` is an intentional native GUI launch, not a test; it still requires a compatible installed service and explicitly supplies the development helper.

## Recipient installation

`make installer/build` exports `installer/dist/` containing:

```text
smith-installer       Native CLI Linux x86_64 ELF
smith-installer-ui    Native GTK3 visual installer ELF
payload/client/       Complete Flutter bundle with compatible helper
payload/service/      Matching stable service binary
manifest.json
SHA256SUMS
README.txt
```

Keep both installer ELFs, complete payload and metadata together. The target is prepared Ubuntu 24.04 LTS x86_64 with appropriate native runtime libraries and safe existing artifact roots; selected lifecycle work needs config/runtime roots and a user manager. The unchanged CLI needs no GTK/display or Python, repository, Dagger/Docker, Rust, Flutter or pip. The GUI is built with optional Cargo feature `gui` and needs GTK3 shared libraries plus a graphical session; its help/version avoids GTK initialization but still requires those libraries. Native is not static linking or arbitrary Linux compatibility. The paired client bundles separate CPU/Vulkan llama.cpp runtimes with CPU fallback; chat requires a separately installed catalog model. Vulkan uses the prepared host loader/ICD/driver as trusted native code. Neither installer provisions OS/GPU dependencies.

**Preferred operator QA**, from `next/` with the full package exported:

```sh
./installer/dist/smith-installer-ui
```

To build, verify and launch in one step, use `make installer/run` (or `make -C next installer/run` from the repository root). The GUI launches only after the full package build and verification succeed, with all installer confirmations unchanged. `make installer/ui` exports only a binary, not its paired package. These are manual operator actions, not tests or agent host approval.

The minimal entry screen offers only **INSTALL / UPGRADE** and **UNINSTALL**. Model management appears later from setup/results rather than the landing page. Under the [maintenance plan](../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-MAINTENANCE.md), an older differing managed installation must use **Upgrade**, not Install/Refresh. Upgrade independently verifies old ownership, preserves backups, separately confirms Stop and applicable lifecycle effects, publishes/verifies the new paired artifacts and offers Start. It does not download models; continue with Manage models afterward. Declined Start leaves upgraded artifacts with service stopped. Foreign/modified/unknown installations still refuse.

**Uninstall** explicitly stops/unregisters and removes active entry points, keeping **all history, preferences, models, releases, backups and preserved objects**. Retained data occupies disk; there is no purge or recursive deletion. Both maintenance actions remain available after failed/expired artifact preview and have independent confirmation. Neither uses old installed helpers. No automatic retry/rollback or normal-host effect follows from source validation. CLI/Make artifact modes are unchanged; maintenance actions are in the visual installer.

The [focused visual flow](../docs/plans/LOCAL-SMITH-NEXT-MODEL-WIZARD.md) is Install Smith → Choose model → Download/check → Complete. Smith is one paired product. Registration, login Enable and Start are the fixed product recipe, with fresh separate Core confirmations when needed. Qwen3-14B Q4_K_M (9.00GB) is preselected for fresh setup; reruns preserve saved selection. Download, checked preference and service-backed inference each require exact-model consent. Only verified complete final content passes the check; no chat turn is saved. Cancel joins installer-owned work but accepted service work may continue.

Use **Manage models** to add/select/check or remove catalog models without republishing artifacts. Selected models and any loaded runner block removal; no hidden unload/Stop, history deletion or rollback. Management completion describes only its own model outcome. After cancellation, close/reopen before rerunning. Neither installer is persistently installed; retain the full package.

The default full CLI is unchanged, with no GTK/display requirement:

```sh
./installer/dist/smith-installer --dry-run
./installer/dist/smith-installer
```

For CLI model work, use the documented `--models`, `--model`, `--download-model` (`--pull-model` alias) and optional `--verify-inference` flags. No arbitrary recommended model is silently selected. Artifact publication, registration/reload, login Enable, service Start, model pull, saved selection and inference each obtain separate consent. Downloads use HTTPS/private disk with pinned size/SHA; interrupted staging data is preserved. Full instructions: [recipient installer](dagger/INSTALLER.md).

Neither installer is persistently installed. The app does not launch the installer; concise setup guidance refers to the separately retained package. Models remain outside the app bundle; offline model packs are future separately reviewed artifacts.

Developer artifact-only targets remain `make install`, `make client/install` and `make service/install`; they build `installer/build/smith-installer` and invoke its explicit `--artifacts-only` mode with client and/or service inputs. They do not register/start services or provision models, and service-only installation does not require building the desktop. Both CLI modes use the same native publisher. Existing different service bytes still block ordinary Make/Install; choose explicitly confirmed Upgrade in the visual installer, never bypass the guard by deleting data. The already-installed voice candidate was not changed by this source migration.

Default user destinations are XDG data `smith-next/client/releases/<digest>` with verified relative `current`, `smith-next/bin/smith-service`, XDG applications entry **smith**, and HOME `.local/bin/smith-next`. Roots must already exist and be safe/non-root-owned; conflicts are preserved. Full recipient installation separately enables the exact user unit for `default.target` login startup; developer artifact-only targets do not. No lingering mutation, sudo, broad legacy migration or implicit rollback.

## Voice archive and evidence

The exact 46-file voice source snapshot is preserved under [reference/voice-chat-v1](../reference/voice-chat-v1/README.md). It is outside all explicit default build/test/package inputs. Active speech execution/dependencies and the sidecar pipeline are removed, not hidden. Minimal v1 compatibility data remains so existing voice-bearing preferences/results/IDs do not corrupt or replay saved text state.

Source/fake/compiled-socket/build evidence is not a real-model, graphical, installed-host, clean-host or release guarantee. New host upgrade and live install→chat→close→reopen validation require separately bound approval; no host services, models or containers were changed during this reset.

The earlier Python installer is preserved only as a checksummed, excluded [historical reference](../reference/python-installer-v1/README.md). Python in Dagger/package tooling is builder-only, with no active recipient fallback. The native migration's historical final integrated assurance was cancelled, not a PASS; current source/build and focused review results are recorded separately in validation history. See [native installer source](installer/README.md), [UI plan](../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-UI.md), [migration plan](../docs/plans/LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md), [text reset plan](../docs/plans/LOCAL-SMITH-NEXT-TEXT-RESET.md), [validation history](VALIDATION.md), and [repository policy](../AGENTS.md).
