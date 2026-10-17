# Smith Ubuntu Workstation Host Profile

Start here for preparation, not a hardware shopping list. [Host Profile details](HOST_PROFILE_DETAILS.md) remain authoritative for detailed requirements, dated observations and validation limits; this overview does not override them. [Scope](SCOPE.md), [Architecture](ARCHITECTURE.md) and [approval boundaries](PROCESS.md#approval-boundaries) apply.

## Prepare the recipient, not a build machine

**Supported target:** one trusted non-root operator on prepared Ubuntu 24.04 LTS x86_64. **Implemented source requirements:** compatible native libraries, owner-controlled safe HOME/XDG roots, private runtime storage, usable `systemd --user`, and cgroup2 evidence for maintenance. Flutter/GTK desktop use needs the full paired native bundle and a graphical session. The CLI needs neither GTK nor a display.

Recipients do not need Python, Rust, Flutter SDK, Dagger, Docker, OpenCode or external Ollama. Builders have separate [toolchain requirements](HOST_PROFILE_DETAILS.md#current-text-only-recipientbuilder-boundary).

## Private service and deliberate setup

**Implemented source:** unprivileged `smith-next.service`, private Unix bridges, `Restart=no` and `UMask=0077`. Artifact publication, registration/reload, Enable, Start, model download and inference checks retain separate consent. Opening or reconnecting performs no automatic setup, inference or replay. Enable never starts; Start never enables. Smith does not provision OS libraries, drivers, keyring or user managers, change lingering, or automatically launch the desktop.

**Target:** after-login startup—not guaranteed pre-login, post-logout or headless continuity. Source changes do not deploy or migrate the host. Uninstall preserves histories, models, state and retained artifacts, consuming disk. See [lifecycle](HOST_PROFILE_DETAILS.md#current-login-boot-recipient-boundary) and [maintenance requirements](HOST_PROFILE_DETAILS.md#explicit-maintenance-runtime-requirements).

## Current bundled CPU/Vulkan runtime boundary

**Implemented source:** paired SHA-pinned llama.cpp CPU/Vulkan trees require compatible native libraries; Vulkan additionally needs an existing loader, driver and unprivileged device access. Fixed `Vulkan0` full-offload startup may fall back once to CPU only after confirmed cleanup, under the original deadline—not after accepted inference failure.

One inference executes globally: bounded threads, context 4096, local output 1024 tokens, 180-second cooperative budget. Model weights require explicit download; disk headroom is not RAM/VRAM fit or speed proof. [Runtime and model limits](HOST_PROFILE_DETAILS.md#current-bundled-cpuvulkan-runtime-boundary).

### Non-model GPU diagnostic observation — GPU startup fix

**Historical observation:** device-only probes identified discrete and integrated GPUs. They prove neither current capacity, model fit, acceleration nor permission for new probes. [Recorded evidence](HOST_PROFILE_DETAILS.md#non-model-gpu-diagnostic-observation--gpu-startup-fix).

## Current next cloud-provider prerequisites

**Implemented source:** optional cloud requires an existing unlocked same-user Ubuntu GNOME Keyring session, explicit API account/billing and authorized official HTTPS egress. Keys enter private installer input only; no Flutter collection, plaintext/environment fallback, activation or unlock. Save and potentially billable Validate are separate; context transfer needs destination consent. Uninstall neither removes keys nor revokes vendor credentials.

Cloud needs no GPU; compiled choices guarantee no entitlement or availability. The 8192-total-token/180-second bounds guarantee neither cost nor latency. [Full prerequisites and evidence limits](HOST_PROFILE_DETAILS.md#current-next-cloud-provider-prerequisites). Never publish secrets or sensitive host inventory; installed acceptance needs separately approved checks.
