# Smith Ubuntu Workstation Host Profile

Start here for preparation, not a hardware shopping list. [Host Profile details](HOST_PROFILE_DETAILS.md) remain authoritative for detailed requirements, dated observations and validation limits; this overview does not override them. [Scope](SCOPE.md), [Architecture](ARCHITECTURE.md) and [approval boundaries](PROCESS.md#approval-boundaries) apply.

## Prepare the recipient, not a build machine

**Supported target:** one trusted non-root operator on prepared Ubuntu 24.04 LTS x86_64. **Implemented source requirements:** compatible native libraries, owner-controlled safe HOME/XDG roots, private runtime storage, usable `systemd --user`, and cgroup2 evidence for maintenance. Flutter/GTK desktop use needs the full paired native bundle and a graphical session. The CLI needs neither GTK nor a display. The candidate package's full directory ancestry must also be safe; a private package leaf cannot compensate for a writable parent ([requirements](HOST_PROFILE_DETAILS.md#explicit-maintenance-runtime-requirements)).

Ordinary Chat/metadata recipients do not need Python, Rust, Flutter SDK, Dagger, Docker, OpenCode or external Ollama. The source-implemented Projects editor, context/read-only metadata and fresh-profile-only Smith starter add no repository connection, Git/SSH, model or agent/container prerequisite; read-only is not enforced security. Existing profiles are not backfilled. Projects validation and fresh assurance remain parent-owned/pending; editor source is not installed graphical acceptance. The source disk9 change is not live migration: later rollout/restore requires an approved pre-migration snapshot and compatible paired artifacts, not binary-only downgrade. See the [recipient boundary](HOST_PROFILE_DETAILS.md#projects-metadata-recipient-boundary) and separate [builder requirements](HOST_PROFILE_DETAILS.md#current-text-only-recipientbuilder-boundary).

## Agents recipient boundary

**Implemented source, acceptance pending:** Dev → Agents creates/edits up to32 saved metadata profiles, retaining two permanent built-ins. It adds no OpenCode, Git, Docker, model/provider, keyring, network or filesystem-root prerequisite, execution permission or automatic Dev start. Fixed Material icons (one generic custom icon) need no new asset contract. App3/Core2 remain; disk10 must preserve strict disk9/earlier state and existing migration semantics. Preferences write v4/read strict v1/v2/v3/v4, adding only the inert Dev root after Agents' v3 destination. Disclosure is session-local; session-only windows still avoid the store. No preference path, host prerequisite or service schema changes follow from Dev navigation. No source edit deploys or migrates installed state. [Migration and approval boundary](HOST_PROFILE_DETAILS.md#agents-definitions-recipient-boundary).

## Optional Dev initialization preview

**Implemented source only:** receipt-only Dev initialization separately needs `/usr/bin/docker`, an explicitly addressed same-user rootless daemon and a prepared `smith-dev-worker:1.19.3` image. OpenCode stays inside the image; no automatic provisioning, rootful fallback, repository mounts, worker network or model/provider access follows. Source changes do not prepare or validate this host capability. [Prerequisites and pending acceptance](HOST_PROFILE_DETAILS.md#dev-initialization-review-slice).

**Implemented local-runner source; live acceptance pending:** the [one-shot runner](plans/LOCAL-SMITH-LOCAL-OPENCODE-RUNNER.md) reuses Smith's selected installed model through a private adapter. Its [approved rootful follow-up](plans/LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) deliberately uses existing local Docker at `/run/docker.sock`, not a rootless/ambient fallback. The service remains non-root, but daemon access is root-equivalent host authority explicitly accepted by the operator; that socket/authority is never passed into the worker. Docker, existing service-account access and `smith-dev-worker-local:1.19.3` are operator prerequisites; no installer provisioning or permission repair occurs. Image/model interoperability, setup and live acceptance remain separately approved/unverified; ordinary Chat requirements are unchanged.

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
