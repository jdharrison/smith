# Smith

Local-first text conversations for one trusted operator on a prepared **Ubuntu 24.04 LTS x86_64 workstation**.

**Implemented in source:** a Flutter desktop, an independent persistent Rust service, and a native CLI/GTK3 installer. The service owns bounded saved conversations and accepted work; closing the desktop detaches without stopping it. Local inference uses bundled, separate CPU/Vulkan llama.cpp runtimes and separately provisioned curated models—**no external Ollama is required**. Optional OpenAI API and Anthropic API routes require explicit configuration and context-sharing consent, never automatic local-to-cloud fallback.

**Status:** development candidate under active hardening, not a production-release or installed-host guarantee. Source/build checks do not establish real-model/GPU performance, clean-host/graphical acceptance, provider entitlement, licensing/SBOM clearance or deployment readiness. Broader agent orchestration, tools, roles and pipelines are **target/deferred product capabilities**, not implemented chat features. See [validation](VALIDATION.md) and [scope](docs/SCOPE.md).

## Use Smith

Obtain the **complete paired installer package**, verify its archive digest through an independently trusted channel, extract into a fresh directory and check its internal checksums before execution. Keep both installer executables, `payload/`, `manifest.json`, `SHA256SUMS` and `README.txt` together for maintenance. Follow the [recipient guide](dagger/INSTALLER.md#acquire-verify-extract-and-run); recipients do not need the repository, Python, Rust, Flutter, Docker or Dagger.

From the extracted package, choose the visual installer or CLI:

```sh
./smith-installer-ui
# Display-free artifact preflight; not a service/model readiness test:
./smith-installer --dry-run
# Interactive full recipient setup:
./smith-installer
```

Recipients need compatible native runtime libraries, safe existing owner-controlled XDG roots and a usable unprivileged systemd user manager. The GTK3 installer and Flutter desktop also need native desktop libraries and a graphical session; the CLI requires neither GTK nor a display. Native does not mean statically linked or portable to arbitrary Linux distributions. Smith does not provision OS libraries, GPU drivers, user managers or keyrings. See [host requirements](docs/HOST_PROFILE.md).

### Installation and models

The visual entry screen offers **INSTALL / UPGRADE** and **UNINSTALL**. Fresh setup publishes the paired app/service, then separately confirms registration/reload, persistent login **Enable**, and **Start**. Enable does not start now; Start does not enable. This is login-scoped operation, not pre-login/post-logout or reboot assurance; lingering is unchanged.

Local Model setup offers seven revision-pinned curated choices. Qwen3-14B Q4_K_M is preselected only for fresh GUI setup (about 8.38 GiB of weights, plus runtime/KV/buffer memory); reruns preserve saved selection. Preselection authorizes nothing. Download, checked preference and service-backed inference each need separate consent. No weights are bundled, and model inventory or a saved preference is not inference verification.

For the CLI, list exact supported identities, then select one explicitly:

```sh
./smith-installer --models
./smith-installer --model qwen3-4b:q4_k_m --download-model --verify-inference
```

The second command performs **full recipient setup**, not model-only maintenance. The CLI has no default model; `--pull-model` aliases `--download-model`. There is no blanket recipient `--yes`. Download/check failures preserve prior effects rather than implying rollback. Use **Manage models** from visual setup/results to add/select/check/remove without republishing artifacts. Selected or loaded models block removal.

The service attempts fixed full offload on `Vulkan0`; only classified startup failures with confirmed cleanup permit one independent CPU fallback. Completion failures never replay on CPU. This is not device discovery or proof of acceleration, fit or performance.

### Optional cloud providers

The optional **Providers** step follows local Model setup. **Manage providers** on rerun, or `./smith-installer --providers`, performs no artifact/lifecycle work. Configure either or both APIs independently; API access and billing are separate from ChatGPT/Claude app subscriptions.

Enter credentials only through masked installer or hidden terminal input. Keys use an existing, unlocked same-user **GNOME Keyring Secret Service/default collection**, never flags, environment variables, chat or plaintext fallback. Smith does not activate/unlock the keyring. Save/Replace and potentially billable synthetic Validate require separate confirmation. Replacement retains old generations; there is no credential purge, and Uninstall neither removes keyring items nor revokes vendor credentials.

Each chat has a **LOCAL / CLOUD** selector for future sends over a fixed fifteen-model cloud catalog. A saved active key permits an explicit attempt without prior successful validation; it does not prove key access or model entitlement. First sharing to a provider/credential generation requires context-sharing consent. Retained consent covers later explicit sends in that chat, including intervening retained history; new generations require new consent. No automatic provider/model fallback, discovery or retry. See [architecture](docs/ARCHITECTURE.md) and [provider prerequisites](docs/HOST_PROFILE.md#current-next-cloud-provider-prerequisites).

### Chat and maintenance

Launch **smith** from the app menu, the installer's explicit post-success **Launch Smith** action, or `~/.local/bin/smith-next`. Boot observes the exact enabled service and synchronizes conversations before mounting chat. It never automatically installs, enables, downloads, infers or resends; only an exact enabled, safely stopped installation offers separately confirmed **Start Smith**. Retry observes/synchronizes; it does not resend uncertain work.

Chats support rename/archive/reactivate/confirmed logical delete. History is bounded (32 saved chats, at most 16 complete turn pairs/64 KiB per chat), not an unlimited archive. Up to four service requests may be outstanding with one executing inference; restart interrupts unfinished work without replay. Waiting desktop follow-ups and unsent drafts are process-local, not durable scheduling. There is no voice, tools/search or automatic replay.

Current application protocol is **schema 3**, persisted application state **schema 4**, and Core lifecycle **schema 2**. Older binaries cannot read disk4. An approved installed-state migration/rollback needs a pre-migration snapshot and compatible paired binaries with exact keyring-generation references—not binary-only downgrade.

Use explicitly confirmed **Upgrade** for differing managed binaries; ordinary Install/Make refuses them. **Uninstall** stops/quiesces, disables/verifies and removes active integration while retaining **all conversations, preferences, models, releases, backups, preserved objects and keyring items**. It is not purge. Decline, error, expiry and cancellation preserve accepted/uncertain effects, with no automatic rollback or retry. Legacy `smith.service` installations are not migrated by this source-layout change.

## Develop from the repository root

```text
client/       Flutter desktop, assets and tests
service/      Independent Rust service workspace and lockfile
installer/    Independent Rust CLI/GTK workspace; paired package in dist/
dagger/       Builder tooling and recipient instructions
scripts/      Repository/pipeline helpers and tests
docs/         Canonical policy, active contracts, evidence and history
```

Builders need Docker Engine, the reviewed Dagger CLI, Git and Python for builder-only package tooling; native client development additionally needs Rust/Flutter and Ubuntu development libraries. Dagger uses explicit `--mod dagger` selection; current CI CLI/module is v0.21.9. `DAGGER` can select a reviewed alternate executable. Retained compatibility/lock files do not prove hermetic builds. There is **no root Cargo workspace**: do not use the retired root `cargo build/test` instructions.

Run from the repository root:

```sh
make help
make test              # Component quality, integration and isolated installer/package gates
make build             # Export service, paired client and complete recipient package
make service/test
make client/test
make installer/test    # Rust quality, isolated GTK and no-Python CLI runtime gates
make installer/ui-test
make package/test      # Builder metadata/native entrypoint tests
make service/build     # service/dist/smith-service
make client/build      # client/dist/ (including service and native runtimes)
make installer/build   # installer/dist/; includes package verification
python3 -B dagger/package.py --root installer/dist --verify
python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
```

Formatting, lint and compiled integration checks are part of `/test` gates, not separate public Make format/lint targets. For the configured aggregate source check:

```sh
env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
```

These build/test commands are not host installation approval. **Operator actions with live effects:** `make installer/run` builds/verifies then launches the full visual installer; `make client/run` launches the development desktop with an explicit helper and still needs a compatible installed service. `make install`, `make client/install` and `make service/install` publish developer artifacts only; they do not register/enable/start or provision models/providers and do not bypass differing-binary guards. `make installer/ui` exports only a GUI binary, not a complete recipient package.

## CI and authority

[CI](.github/workflows/ci.yml) validates PRs targeting `main` and pushes to `main`; [candidate automation](.github/workflows/release.yml) handles pushed `release/*` tags through the reusable CI. The release helper checks canonical annotated `release/vMAJOR.MINOR.PATCH`, exact source/tag/HEAD, fetched `main` ancestry, clean tracked source and matching component versions before accepting a verified full-package archive. Root promotion requires workflows/helper/tests to be rebased together; old root-Cargo gates are not current-product validation.

Tagged candidates retain `smith-installer_vX.Y.Z.tar.gz`, its checksum, build record and validation/CLI evidence in an Actions artifact for **14 days**. PR/main builds do not retain distributable packages. Workflows have read-only contents permissions and do not create GitHub Releases, mutate tags or deploy. Artifact upload is still remote disclosure: exact tag push, destination/visibility/retention and redistribution rights require separate approval. Checksums and green jobs are not human approval or signed provenance. Never move/reuse an existing release tag.

Read [AGENTS.md](AGENTS.md), [Scope](docs/SCOPE.md), [Architecture](docs/ARCHITECTURE.md), [Process](docs/PROCESS.md), [Design](docs/DESIGN.md) and [Host Profile](docs/HOST_PROFILE.md) before changing or operating Smith. The [pipeline plan](docs/plans/LOCAL-SMITH-INSTALLER-PIPELINE.md) records candidate scope; [VALIDATION.md](VALIDATION.md) records evidence and limitations, not inherited acceptance of a later source tree.

## History and compatibility

The former root orchestration/chat CLI and legacy client are **retired from source support**, without compatibility shims. This does not uninstall or change existing installations. Historical source, retired guides and voice/Python reference snapshots are retained through immutable Git history, not a copied legacy tree; see [History](docs/HISTORY.md).

Repository promotion does **not** rename `smith-next.service`, the `smith-next` launcher/XDG namespaces, desktop IDs, keyring identities, wire identities or deterministic ownership bytes. A runtime rebrand would require a separate migration.

[License](LICENSE) · [Security](SECURITY.md)
