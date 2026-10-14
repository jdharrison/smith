# Smith native recipient package

## Current bundled native runner

**Implemented source:** [native-runner plan](../../docs/plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md) supersedes external Ollama setup/provider descriptions from earlier stages below for production `next/` only. Smith bundles separate CPU and Vulkan llama.cpp runtimes in the paired client release; no external Ollama is required. Weights remain separate. List choices with `smith-installer --models`; use `--model EXACT --download-model` (`--pull-model` alias) with separate exact-name download, preference and optional inference consent. GUI model provisioning/check/removal is implemented in the focused wizard; fresh setup preselects Qwen3-14B, preserving existing selection on reruns. Neither installer is persistently installed: retain the full package.

Runtime/model verification, private Unix IPC, one runner lease, bounded context and service-owned cancellation/cleanup are described in canonical Architecture. Production loads the held verified model FD; app-owner teardown cancels/joins native inference, while desktop close only detaches. No implicit upgrades, retries, cloud/tools, bundled weights or real-inference/real-GPU/clean-host/release guarantee. Chat attempts fixed full offload on `Vulkan0`; classified startup failure may fall back once to independent CPU after confirmed cleanup. Smith does not discover/provision drivers or prove acceleration. Unused installer Ollama code remains legacy library/test coverage, not a production fallback. See [validation history](../VALIDATION.md) for observed checks.

The full package contains native Rust Linux x86_64 `smith-installer` (unchanged CLI) and `smith-installer-ui` (optional GTK3 visual interface). Keep both beside the complete `payload/`, `manifest.json`, `SHA256SUMS` and `README.txt`. No Python, Dagger, Docker, Cargo/Rust, Flutter, pip or recipient compilation is required. The CLI requires neither GTK nor a display; GUI installation needs GTK3 shared libraries and a graphical session. GUI help/version skips GTK initialization/display discovery, but still requires those libraries.

Supported candidate: Ubuntu 24.04 LTS x86_64 and a trusted non-root account with compatible native glibc/libgcc and safe existing artifact roots. GTK3/Flutter libraries are for the GUI/client, not the default CLI. Full CLI setup or selected GUI lifecycle work requires existing config/runtime roots and a usable systemd user manager. The paired client bundles separate CPU/Vulkan llama.cpp runtimes with CPU fallback; chat needs an explicitly installed catalog model, not an external Ollama daemon. Vulkan loads the prepared host's loader/ICD/driver as trusted native code. Config/runtime roots and a user manager are needed only when GUI lifecycle work is selected. Native does not mean statically linked or universal Linux ABI support. The installer does not provision OS libraries, GPU drivers or privileged system services. No models or voice providers are bundled.

Obtain the package from a trusted source and verify an independently retained candidate digest/provenance. From the extracted package, `sha256sum -c SHA256SUMS` checks accidental changes—not authenticity against someone able to rewrite checksums. The default bundle is located beside the actual resolved running executable, independent of cwd or a launcher symlink. `--bundle ROOT` explicitly selects a root containing `payload/`.

## Visual installation / operator QA

Preferred from `next/` after the full package has been exported:

    ./installer/dist/smith-installer-ui

If needed, run `make installer/build` first, then that command. Inside an extracted recipient package, use `./smith-installer-ui`. `make installer/ui` exports only a binary, not paired payloads. These are manual operator QA actions, not an agent test shortcut or host approval.

The entry screen offers **Install / Upgrade / Manage models / Uninstall**:

- **Install** is for a fresh/current matching installation. It does not overwrite differing service bytes.
- **Upgrade** independently verifies an old managed installation, preserves old artifacts and separately asks for Stop, replacement, applicable registration/reload and Start. Choose this for an existing older service. Original consent expires; no automatic renewal/retry/rollback. It does not execute old helper code or provision models. Successful running verification is Core/app readiness, not model readiness; continue with Manage models explicitly.
- **Manage models** uses a compatible running service and has separate download/preference/check/removal confirmation.
- **Uninstall** stops/unregisters Smith and removes active menu/user-command/current/service entry points. **All conversations, preferences, models, releases, backups and preserved objects are kept.** Retained data occupies disk; no purge/recursive deletion. Uninstall does not require a candidate payload.

Upgrade/Uninstall remain available if install preview fails or expires, but independently refuse foreign/modified/unknown or unsafe objects, enabled/overridden units and unverified quiescence. These are visual-installer operations; unchanged CLI/Make still refuse implicit maintenance. No installed-host action is authorized by building or reading these instructions.

The near-black/green flow focuses on one action at a time:

1. **Install Smith:** one paired app/service product with menu/icon and user command. Normal recipe requests registration and Start; each applicable effect retains fresh inline confirmation and original60-second Core expiry. No autorun/linger or implicit upgrade.
2. **Choose model:** Qwen3-14B Q4_K_M is preselected on fresh setup (9001752960 bytes). Existing saved selection is preserved; smaller curated alternatives remain. No weight download or preference change merely by opening.
3. **Download and check:** separate exact-name-bound download, revision-checked preference and service-backed inference confirmations. Real transfer-byte progress; hashing/loading/generation are indeterminate. Uses service-selected Vulkan-first/CPU-fallback execution and model-default thinking, provisional180s service/210s observer budgets; unmeasured performance, reasoning-only/truncated/timeout output is not success. No saved chat turn.
4. **Complete:** verified outcomes for this scope only. Failures say Setup incomplete with prior effects preserved, no automatic retry or rollback. No automatic desktop launch.

**Manage models** skips artifact republication and lets you add/select/check or remove a catalog model. Removal is permanent and separately confirmed through the service. Selected models cannot be removed; choose another installed model first. Any loaded runner—including idle-loaded—can block removal until its normal10-minute idle stop. No implicit runner/service stop or chat/history/config deletion. Failed partial removal may preserve orphan data; there is no automatic purge/rollback.

Model management needs an already-running compatible verified Smith service. It is not an upgrade bypass; use the separately confirmed Upgrade action for differing installed service bytes. Management success reports Model check complete or Model removal complete, not installation verification. Stop/close joins installer-owned work; already accepted service work may continue and is never automatically resent. Cancellation is permanent for that process: close/reopen before another management session. Keep the entire package; installer commands are not persistently installed.

## CLI alternative

Read-only artifact preflight:

    ./smith-installer --dry-run

Install artifacts and separately review needed registration/start effects:

    ./smith-installer

Select an exact observed model (replace MODEL:TAG with an eligible actual tag):

    ./smith-installer --model MODEL:TAG

Request an additional exact-model download confirmation if missing:

    ./smith-installer --model MODEL:TAG --download-model

Optionally offer a separately confirmed fixed minimal inference test:

    ./smith-installer --model MODEL:TAG --verify-inference

No recommended/default model is silently invented. Select a tag compatible with available workstation resources and review its license/provenance. Download requires typing the exact model; the installer uses HTTPS and private disk. Exact size/SHA are pinned; failed or interrupted transfers preserve private staging files. No retry or cache deletion occurs. Inventory and saved preference are not inference or GPU verification. The direct test creates no saved chat turn and permits no tools/cloud fallback.

CLI artifact publication, native registration/reload, Start, pull, preference save and inference have separate confirmations. Initial decline/dry-run creates no destination files/locks or manager/provider effects; later decline/error leaves prior accepted effects and reports partial/uncertain outcomes without rollback. Consent input is bounded to 4096 bytes/60 seconds; native lifecycle tokens retain their original 60-second binding. EOF/interrupt/expiry cannot approve. There is no global recipient --yes.

CLI setup/progress output is bounded through nonblocking pipes, sockets or terminals; the publisher's separate stdio path has no overall console/filesystem deadline. Regular-file/device output sinks are rejected rather than falsely promising bounded writes; use a supported pipe/terminal to observe progress. Cooperative cancellation/timeout does not guarantee real-time filesystem completion or termination of external daemon/accepted manager work. If output becomes unwritable, the program fails nonzero without retry/rollback; reporting may be incomplete.

Developer component installs use the same native publisher without lifecycle/provider work:

    smith-installer --artifacts-only --client CLIENT_BUNDLE --service SERVICE_BINARY

Either component may be selected alone. `--yes` is permitted only in explicit artifact-only mode and approves the displayed artifact plan, bypassing no safety checks. Mixed recipient/artifact options fail. Existing different service/helper bytes remain blocked in artifact-only mode; use the visual Upgrade flow for separately confirmed stop/preservation/replacement/start. Never delete files/state to bypass a mismatch.

After artifact publication, manually find **smith** in the app menu or run `~/.local/bin/smith-next`; installation does not automatically launch it or prove service/model readiness. The app opens the conversation directly, does not provision/start services or pull models, and never uses the legacy completion flag as chat readiness. Closing detaches; accepted work may continue. No autostart/linger or logout/reboot guarantee is added.

Keep the entire extracted package for repair. Open installer in the app remains an unchanged instruction-only CLI terminal/package fallback, not a launch of the new GUI, a terminal or an installed command. Neither installer is persistently installed. Offline model packs remain future separately reviewed distribution artifacts.

This is a development candidate, not a production release. UI/Xvfb results and scoped independent review are pending; the native migration's final integrated assurance was cancelled, not a PASS. Source/fake/build and interpreter-free CLI checks are not real inference, graphical/installed-host, clean-host, licensing/SBOM, GPU or signed-provenance evidence. The prior installed voice candidate is unchanged and its one-time upgrade approval is consumed.
