# Smith native recipient package

**Implemented source; development candidate, not release readiness.** This package targets a prepared Ubuntu 24.04 LTS x86_64 workstation and one trusted non-root operator. Source/build checks and CI artifacts do not establish clean-host, installed graphical, real-model/GPU, live-provider, licensing/SBOM or signed-provenance acceptance. See [validation history](../VALIDATION.md) and canonical [delivery/release process](../docs/PROCESS.md#release-and-deployment) in the source repository.

## Prerequisites and package contents

Keep `smith-installer`, `smith-installer-ui`, the complete `payload/`, `manifest.json`, `SHA256SUMS` and `README.txt` together. Neither installer is persistently installed: retain the entire extracted package and its path for later maintenance.

- Recipients need compatible native glibc/libgcc libraries, safe existing owner-controlled artifact roots, and existing XDG config/runtime roots plus a usable systemd user manager for full setup. Run as the intended user, not root. See [Host Profile](../docs/HOST_PROFILE.md#current-bundled-cpuvulkan-runtime-boundary) for the runtime contract.
- The GTK3 GUI and Flutter desktop need their native desktop libraries and a graphical session. GUI help/version avoids display initialization but still loads GTK3 libraries. The CLI requires neither GTK nor a display.
- No repository, Python/pip, Rust/Cargo, Flutter/Dart, Dagger, Docker or recipient compilation is required. Native does not mean statically linked or compatible with arbitrary Linux distributions.
- The paired client bundles separate CPU and Vulkan llama.cpp runtimes; no external Ollama is required. Local chat needs a separately installed curated model. No weights or voice providers are bundled.
- The installer does **not** provision OS libraries, GPU loaders/ICDs/drivers, user managers, keyrings or privileged services. Vulkan uses the prepared host's native driver stack. Smith attempts full offload on `Vulkan0`; only classified startup failures with confirmed cleanup permit one independent CPU fallback. This is not device discovery or acceleration proof.

## Acquire, verify, extract and run

Obtain a candidate from a trusted source. For a successful tagged GitHub Actions run, download the `smith-installer_vX.Y.Z` artifact and unpack the Actions download envelope if present. It contains `smith-installer_vX.Y.Z.tar.gz`, its `.tar.gz.sha256`, `build-record.json`, `validation.log` and `dagger-version.txt`. Actions candidates are retained for 14 days; they are not automatically published GitHub Releases.

Before extracting or executing, compare the archive's SHA-256 with a digest obtained through an **independently trusted** approval/provenance channel. A checksum downloaded beside an archive is not by itself authenticity: an attacker able to replace both can make them agree. After that comparison, check the supplied checksum and extract into a fresh directory. For example, replace `X.Y.Z` consistently with the acquired version:

```sh
sha256sum smith-installer_vX.Y.Z.tar.gz
sha256sum -c smith-installer_vX.Y.Z.tar.gz.sha256
tar -xzf smith-installer_vX.Y.Z.tar.gz
cd smith-installer_vX.Y.Z
sha256sum -c SHA256SUMS
./smith-installer-ui
```

The tar archive contains one same-name `smith-installer_vX.Y.Z/` folder with the **whole** package, not just an installer executable. Keep that folder intact. The inner checksum check detects changed package files, not independent authenticity. The default bundle root is beside the actual resolved installer executable, independent of cwd or a launcher symlink; `--bundle ROOT` explicitly selects the complete package root.

For display-free preflight or CLI installation from that same extracted folder:

```sh
./smith-installer --dry-run
./smith-installer
```

Dry-run is read-only artifact preflight, not a service/model readiness check. GUI/CLI installation is an intentional operator action, not a test or authority for an agent to affect the host. Approval boundaries remain in canonical [Process](../docs/PROCESS.md#approval-boundaries).

## Visual installation

The landing page offers exactly **INSTALL / UPGRADE** and **UNINSTALL**. Model/provider management is reached later through setup/result paths, not additional landing actions. Opening may perform a hidden read-only preview; selecting a flow shows **Checking Smith**, then one focused action at a time.

Normal fresh setup follows:

1. **Install Smith:** review publication of the paired app/service, app-menu entry, icon and user command. Registration/reload, persistent login **Enable**, and **Start** are requested as the product recipe, but each needed lifecycle effect has its own fresh confirmation. Enable does not start now; Start does not enable. Original Core approvals expire after 60 seconds and are not silently renewed.
2. **Choose model:** fresh setup preselects Qwen3-14B Q4_K_M (9,001,752,960 bytes, about 8.38 GiB of weights alone). Reruns preserve the saved selection. The chooser contains seven curated Apache-2.0 choices: Qwen2.5 1.5B, SmolLM2 1.7B, Qwen3 4B, Mistral 7B, Granite 8B, Mistral Nemo 12B and Qwen3 14B. Opening/preselection authorizes no download or preference change. Review disk and memory needs; weights alone exclude runtime/KV/buffer memory.
3. **Download and check:** separately confirm exact-model download, revision-checked saved preference and service-backed inference. Downloads use HTTPS with pinned revision/size/SHA and show real transfer bytes; hashing/loading/generation are indeterminate. Only verified complete final content passes; reasoning-only, empty, truncated or timed-out output is failure. The check saves no chat turn and proves neither GPU use nor performance.
4. **Optional Providers:** configure OpenAI API and/or Anthropic API separately, or skip. Save/Replace and Validate have distinct confirmations. Optional failure/skip does not undo verified local setup; see below.
5. **Result:** completion describes only verified effects. Failed checks report incomplete setup with prior effects preserved. Successful verified Install/Upgrade may offer **Launch Smith** after the worker joins. Nothing launches automatically on completion or Close. Launch validates and dispatches the installed client, not the package payload; “Launch requested” is not proof of a visible window or readiness. Launch failure leaves installation success intact and allows explicit retry.

Enable targets startup after user login through `default.target`; it changes no lingering setting and guarantees neither pre-login nor post-logout availability. Actual login/logout/reboot and installed-host behavior remain separate validation gates.

## Existing installations and preserved state

Ordinary Install/Refresh and CLI/Make artifact publication refuse differing service bytes. Use the visual flow's explicit **Upgrade**, never delete files/state to bypass that guard. Upgrade independently verifies the old managed installation, preserves artifacts/backups and separately confirms Stop, replacement and applicable registration/reload, Enable and Start. It migrates only a byte-exact legacy static unit, preserves an exact current enabled unit without enablement churn, and explicitly enables an exact current disabled unit. Enabled state alone is not a refusal; foreign/modified/unknown units, overrides, unsafe objects or unverified quiescence still block maintenance. Old installed helper code is not executed.

Upgrade does not download models. Running Core/app verification is not model readiness; choose **Manage models** explicitly afterward. Declining Start leaves upgraded artifacts with the service stopped. Before migrating installed state, retain an exact pre-migration state snapshot and compatible paired-artifact restore procedure: binary-only downgrade is not a state rollback. See [Architecture](../docs/ARCHITECTURE.md#current-next-cloud-routing-and-credential-boundary).

**Uninstall** separately confirms active integration removal: stop/quiesce, disable and verify before unregistering, then remove active menu/user-command/current/service entry points. It retains **all conversations, preferences, models, releases, backups and preserved objects**. Retained data occupies disk; there is no purge or recursive deletion. Keyring items remain and API credentials are not revoked. Uninstall does not require a candidate payload. Maintenance is independently checked even after failed/expired install preview; it is not an implicit CLI/Make upgrade.

**Manage models** uses a compatible verified running service without artifact republication. Add/select/check/removal each retain applicable exact-model consent. Removal is permanent and service-mediated; selected models cannot be removed until another installed model is explicitly selected. Any loaded runner, including idle-loaded, can block removal until its normal ten-minute idle stop. No hidden unload/Stop or history/config deletion. Partial removal can leave preserved orphan data; no automatic purge or rollback.

Decline, error, expiry or cancellation preserves prior accepted/uncertain effects, with no automatic retry or rollback. Closing/stopping joins installer-owned work, but already accepted service work may continue and is never automatically resent. Process cancellation is sticky: close/reopen before another management session.

## Optional cloud providers

**Providers / Manage providers** manages either or both API credentials independently; rerun management performs no artifact or lifecycle work. API access and billing are separate from ChatGPT or Claude app subscriptions. Save is local keyring work, not API validation. A separately confirmed Validate sends one fixed synthetic request without chat history and can incur charges; success covers only that exact model/credential generation, not all model entitlements or future availability.

Prepare an existing, unlocked same-user **GNOME Keyring Secret Service/default collection** first. Smith does not install, activate or unlock it and has no plaintext/environment credential fallback. Enter keys only in the installer's masked field or hidden terminal input—not flags, environment variables, chat, logs or documentation. Replacement retains previous keyring generations; there is no in-app credential purge. Uninstall neither removes keyring items nor revokes vendor credentials. See [provider prerequisites](../docs/HOST_PROFILE.md#current-next-cloud-provider-prerequisites).

## CLI alternative

From the complete extracted package, list the seven pinned local choices without discovery or download:

```sh
./smith-installer --models
```

Use `--model EXACT`, where `EXACT` is a complete eligible catalog name printed by that command, not an arbitrary Ollama tag or a file path. For example, choose **one** of these commands according to the intended work:

```sh
# Select an already installed catalog model during full recipient setup.
./smith-installer --model qwen3-4b:q4_k_m
# Also offer a separate download confirmation if it is missing.
./smith-installer --model qwen3-4b:q4_k_m --download-model
# Also offer a separately confirmed fixed minimal inference test.
./smith-installer --model qwen3-4b:q4_k_m --download-model --verify-inference
```

`--pull-model` is a compatibility alias for `--download-model`. These are full recipient commands, not model-only maintenance: artifacts and needed lifecycle effects are still reviewed. The CLI has no default model; the GUI's fresh preselection is not silent CLI selection. Download consent requires the exact name; failed/interrupted transfers preserve private staging files with no retry or cache deletion. Inventory/saved preference is not inference or GPU verification. The direct CLI test creates no saved chat turn and has no tools or cloud fallback.

Artifact publication, registration/reload, login Enable, Start, download, preference and optional inference have separate confirmations. Initial decline/dry-run creates no destination files/locks or manager/provider effects; later decline leaves earlier effects intact. Consent is bounded to 4096 bytes/60 seconds; EOF, interrupt and expiry cannot approve. There is no global recipient `--yes`.

Optional provider-only management is separate:

```sh
./smith-installer --providers
```

It accepts only optional `--bundle ROOT`, not artifact/model/dry-run/yes combinations, and requires a terminal for hidden key input plus the prepared unlocked keyring session.

CLI setup/progress output supports bounded nonblocking pipes, sockets or terminals, not regular-file/device redirection. The artifact publisher's separate stdio/filesystem path has no overall deadline. Cancellation/timeouts cannot guarantee real-time filesystem completion or termination of accepted manager/service work. Output failure exits nonzero without retry/rollback; reporting can be incomplete.

Developer-only component publication uses `--artifacts-only --client CLIENT_BUNDLE --service SERVICE_BINARY` (either component alone is allowed). Only explicit artifact mode permits `--yes`, without bypassing checks; it never registers/enables/starts or provisions models/providers. It is not required for prebuilt recipients.

## Open Smith after setup

Use the explicit post-success **Launch Smith**, find **smith** in the app menu, or run `~/.local/bin/smith-next`. App launch first performs bounded read-only **Boot** observations, then synchronizes saved conversations before showing chat. It never automatically installs, enables, starts, downloads, infers or resends. Only an exact installed, registered, enabled and safely stopped state offers separately confirmed **Start Smith**. Disabled/missing/conflicting states show external repair guidance, not an installer handoff/dialog. There is no Open installer action: rerun the separately retained package for installation/model/provider maintenance.

Closing the desktop detaches; accepted service work may continue. Existing conversations/preferences are preserved, not reset for setup. No implicit upgrade, lingering mutation, logout/reboot guarantee or automatic provider fallback is added. Offline model packs remain future separately reviewed distribution artifacts.
