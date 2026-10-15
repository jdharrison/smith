# Native installer source and recipient output

## Current bundled native runner

**Implemented source:** [native-runner plan](../../docs/plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md) supersedes external Ollama setup/provider descriptions from earlier stages below for production `next/` only. Smith bundles separate CPU and Vulkan llama.cpp runtimes in the paired client release; no external Ollama is required. Weights remain separate. List choices with `smith-installer --models`; use `--model EXACT --download-model` (`--pull-model` alias) with separate exact-name download, preference and optional inference consent. The GTK3 installer also offers explicit model setup and rerun management under [the model-wizard plan](../../docs/plans/LOCAL-SMITH-NEXT-MODEL-WIZARD.md). Qwen3-14B Q4_K_M is preselected only when no saved selection exists; opening never downloads or saves a preference. Neither installer is persistently installed: retain the full package.

Runtime/model verification, private Unix IPC, one runner lease, bounded context and service-owned cancellation/cleanup are described in canonical Architecture. Production loads the held verified model FD; app-owner teardown cancels/joins native inference, while desktop close only detaches. No implicit upgrades, retries, cloud/tools, bundled weights or real-inference/real-GPU/clean-host/release guarantee. The service attempts fixed full offload on `Vulkan0`, with one classified confirmed-cleanup CPU fallback; host loader/ICD/driver code is trusted native code and acceleration remains unverified. Unused installer Ollama code remains legacy library/test coverage, not a production fallback. See [validation history](../VALIDATION.md) for observed checks.

`next/installer` is an independent Rust 2021 crate/workspace with its own lockfile,
path-depending on the unchanged `next/service` library. Its `smith-installer` is a
native Linux x86_64 ELF, not a Python launcher or zipapp. Default features are
empty: the full-recipient/artifact CLI requires no GTK, display or Python. The
optional `gui` feature builds sibling `smith-installer-ui` with GTK3. Native does
not mean static linking or compatibility with arbitrary Linux distributions.

Run `make installer/build` from `next/` to export the complete generated recipient
package under `installer/dist/`. Keep both `smith-installer` and
`smith-installer-ui`, `payload/client`,
`payload/service`, and the package metadata together. Without `--bundle`, the
installer uses the parent of its actual canonical running executable, never cwd,
`argv[0]`, PATH, or a symlink alias. The prepared-host target is Ubuntu 24.04
LTS x86_64 and an unprivileged trusted user with appropriate native runtimes and
safe existing artifact roots. Full CLI setup/selected GUI lifecycle work needs
config/runtime roots and a usable user manager. Chat/CLI model work requires
the bundled Vulkan-first/CPU-fallback runner and a separately installed curated model; GUI model work uses the managed Smith service. Python, Rust/Cargo, Flutter, Dagger and
Docker are not recipient installer dependencies; Python may be used by builder
package tooling only.

## Terminal-inspired installer appearance

**Implemented presentation-only redesign:** the native GTK3 shell matches the app's dark-teal/cyan palette, thin bordered panels, small radii and bundled JetBrains Mono. A `> SMITH` header shows the actual installer version from Cargo metadata. Package/install and uninstall actions use the supplied outline icon geometry. Font/icon bytes are embedded in the GUI executable, so appearance does not depend on an intact client payload or system-installed fonts; no runtime asset downloads or persistent font installation occur.

All existing artifact, lifecycle and model approvals remain separate and request-bound. The model button now says **Review model**, not permission to download or select it. Exact effects are shown without an extra artifact-details disclosure click; approval buttons name their action. Preparation does not claim an action is approved, progress omits disabled pseudo-actions, and results retain the actual scope, failures, uncertainty and preserved effects with less repeated copy. Close acknowledges no effect and never launches the app.

The resizable default is 720×560 with 480×420 narrow-layout coverage, enlarged text, wrapping model rows, reachable footer actions and a bounded wide-window reading column. Synthetic screenshots and executed isolated GTK evidence are indexed in [the fixture README](tests/fixtures/ui-redesign/README.md); they are not installed desktop, screen-reader or live-service evidence.

`smith-installer-ui --licenses` prints the complete embedded font license/copyright and supplied-icon notice without initializing a display or discovering a package. Like `--help`/`--version`, it still requires the GUI ELF's native libraries. The CLI remains GTK-independent. See [asset provenance](assets/README.md), [the approved option-A plan](../../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-REDESIGN.md) and [validation history](../VALIDATION.md). Supplied-icon redistribution rights remain unresolved; local source validation is not publication approval.

## Optional visual installer

**Implemented source, validation pending:** [the bounded UI plan](../../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-UI.md) adds `src/ui.rs`, a native GTK3 one-page shell over `src/wizard.rs`. From `next/`, preferred operator QA is `./installer/dist/smith-installer-ui`; run `make installer/build` first if the full package needs export. `make installer/ui` exports only `installer/build/smith-installer-ui`, not payloads. The GUI accepts `--bundle ROOT`, help and version; it has no CLI model/artifact/`--yes` options. GUI help/version does not initialize GTK or inspect a display, but GTK shared libraries are still required; normal use needs a graphical session.

The focused flow is Install Smith → Model choice/download → service-backed check → Complete. The fixed normal paired app/service recipe requests Register/reload, then Enable at login, then Start, with each effect requiring fresh separate inline consent. Enable never starts the service, Start requires exact enablement and never enables it, and lingering is unchanged. Review/Refresh is read-only, without locks/lifecycle/root-fact writes. Install freezes choices and approves only the exact publisher-owned artifact snapshot. Core confirmations retain exact request IDs and original worker-owned 60-second tokens. Model download, revision-checked preference change, fixed-greeting verification, and destructive removal have separate exact-model confirmations. Existing selections are preserved; changed selection is never silently saved. Downloads report actual bytes; hashing/loading/checking remain indeterminate. Complete requires the service check for setup, not inventory or a saved preference. The app is never automatically launched.

Manage models bypasses artifact republication and does not implicitly register, enable, start or stop the service. Reopen management explicitly after each joined session as often as needed. Removal is service-mediated and bound to catalog name/SHA and configuration revision; selected models are refused. Any loaded runner may conservatively block removal until normal idle stop. No automatic stop, retry, rollback, recursive cleanup or history changes occur. Failed checks show setup incomplete. The service check uses model-default thinking, existing 180-second CPU generation limits and a 210-second installer observation budget; hardware suitability is unmeasured, and empty/truncated final answers are failures. Stopping observation does not cancel or resend service-accepted work.

`Session::spawn` constructs publisher/Core objects inside one owned worker. `send`, `try_event`, `is_finished` and `join` use bounded typed commands/events and capped reporting, not CLI subprocesses or stdout scraping. Runtime roots/Controller are lazy after artifact success for selected lifecycle/model work, or after explicit Manage models. Provider construction/download happens only after exact download consent. Typed provider/backend seams keep model worker tests isolated; artifact/lifecycle-only library recipes remain available. Cancel uses composed installer/Core cancellation, invalidates pending consent and keeps the window waiting until worker exit/join; no detached effect-capable worker, automatic retry/reprepare or rollback. Filesystem/fsync may delay stopping. Keep the complete package: neither GUI nor CLI is persistently installed, and the unchanged app handoff remains instruction-only CLI fallback.

## Explicit existing-installation maintenance

**Implemented native source:** `src/maintenance.rs` provides the shared Upgrade /
Uninstall orchestration under [the maintenance plan](../../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-MAINTENANCE.md).
Its public GUI boundary is `Operation::{Upgrade, Uninstall}`, `Prompt`, `Runner`,
`Outcome { complete, message, details }`, and `Native::new(bundle)`; library tests
inject `Native::with_roots(bundle, home, roots, manager)`. No new CLI maintenance
flags or recipient `--yes` bypass are added by this slice.

Read-only inspection independently verifies the selected canonical release manifest,
relative selector, exact desktop/shim and standalone/helper equality. Retained
voice-era release files are permitted; old helpers are never executed, and corrupt
app/Core data is not parsed or reset by old-install maintenance. Foreign units,
drop-ins, enablement, unknown manager facts and unsafe paths refuse maintenance.
Uninstall does not read or require a candidate payload.

After exact artifact consent, the installation lock precedes one held Core lease,
then the shared runner lease after Stop/quiescence. Separate original-token
confirmations authorize Stop, unregister/reload, candidate registration if absent,
and Start. Fresh private backups/effect records and verified retained binary/selector
exchanges preserve partial outcomes; no global transaction, same-UID CAS, rollback,
retry or crash replay is promised. Declined Start truthfully reports upgraded
artifacts with the service stopped. Running success requires fresh candidate Core
and app readiness, never model readiness. Upgrade does not provision weights.

Uninstall preserves active objects by no-clobber rename only after registration is
confirmed absent. It keeps **all** Core/app state, history, preferences, models,
releases, backups, stages, lockfiles and unrelated objects. Retained files continue
to use disk; there is no purge or recursive deletion. Missing independently owned
launchers are disclosed and skipped; ambiguous missing binary/selector ownership
blocks conservatively. Normal Install and Make differing-binary guards remain.
Synthetic tests are not installed-host evidence; fresh parent-owned assurance and
operator effect approval remain required.

## Source interfaces and authority

Artifact publication lives in `src/artifact.rs`, `src/fs.rs`, and `src/manifest.rs`.
The unchanged CLI and recipient orchestration live in `src/cli.rs` and `src/setup.rs`.
The GUI worker uses opaque `PreparedArtifacts` from read-only `prepare_review()`;
`install_reviewed()` consumes its exact approval ID and original captured facts,
reusing the same post-consent/under-lock/final-verification publication safeguards.
Orchestration directly uses `smith_service::Controller` with the packaged
`payload/service/smith-service` as source, not the installer executable. It does
not spawn a control helper or construct `Application::production`.

CLI artifacts, unit registration/reload, persistent login Enable, current-runtime Start, exact-model download,
saved model preference and optional fixed minimal inference have separate
confirmations. Enable never starts the service, Start never enables it, and neither changes lingering. Model pull requires typing the exact tag. CLI model names remain exact and explicit; fresh GUI setup preselects the pinned Qwen3-14B option, without approving effects. Inventory is not inference verification. Streaming model publication permits at most 16 GiB; artifact publisher limits are unchanged. The installer does not write
the historical setup-completion flag, change lingering,
launch the desktop, install/start Ollama, provision the OS/GPU, or perform implicit upgrades. Explicit maintenance is the separate boundary above.
Initial artifact decline and `--dry-run` do not construct runtime roots, a backend
or a provider. Later decline/failure reports prior accepted/uncertain effects,
which remain; there is no automatic retry, rollback or cache deletion.

`--artifacts-only --client BUNDLE` and/or `--service BINARY` is the separate
artifact-publication mode used by developer Make targets. Only that explicit mode
accepts `--yes`; it bypasses confirmation, not publisher safety checks. Mixing
recipient model/bundle flags into artifact mode is rejected.

Consent reads are bounded to 4096 bytes including newline and 60 seconds of actual
stdin reading, including slow drip. EOF, Ctrl-C and timeout never approve.
Cancellation chains Core's captured atomic-only handlers while startup signals
are blocked, so its manager cleanup remains intact. Direct calls retain Core's
individual manager and socket budgets; there is no promised total filesystem
45-second deadline or cancellation of work already accepted by an external daemon.
No reader/helper thread remains able to apply an effect after return.

CLI recipient setup text and consent prompts use native nonblocking console output
with a two-second message budget. Model progress implements `model::ProgressOutput`,
not arbitrary `io::Write`, and preserves the provider's exact deadline/cancellation
function. Production writes use scoped `model::FdProgress` descriptors without a
buffered flush or writer thread. All terminal error/partial-effect lines share one
250-ms budget; a failed progress sink is not reentered, cancellation skips further
reporting, and final stderr diagnostics are bounded too. If reporting cannot be
delivered, the process still exits nonzero: prior effects remain and an incomplete
report never implies rollback. Bounded console sinks accept pipes, sockets and
terminals, not regular files/devices whose writes `O_NONBLOCK` cannot bound.
The artifact publisher's separately owned stdio path is not a console/filesystem
transaction deadline and is not replaced by this reporting adapter.

## Local source checks

From this crate directory, `sh scripts/test.sh` runs formatting, warning-denied
Clippy and locked baseline/wizard tests; `sh scripts/build.sh` builds the locked
CLI release without GTK. `sh scripts/build-ui.sh` enables `gui` for the sibling
ELF; `sh scripts/test-ui.sh` adds GUI-enabled Clippy and isolated Xvfb GTK smoke.
Dagger exposes these as `make installer/ui` and `make installer/ui-test` from
`next/`, using actual Ubuntu 24.04 UID/GID 1000 for tests. Fixtures use injected
sessions/backends/providers and private roots, not normal units, providers,
downloads, home installation or the operator's display. UI results/review are
pending; the native migration's final integrated assurance was cancelled, not a
PASS. The declared
MSRV is 1.83; metadata alone is not proof of an actual 1.83 compile. The independent
lockfile reuses the service's dependency resolution rather than introducing new
versions of its provider/serialization families. Review duplicates with
`cargo tree --locked --duplicates` and target-specific MSRV metadata with
`cargo metadata --locked --filter-platform x86_64-unknown-linux-gnu`.

### Archived wrapper parity map

The archived `reference/python-installer-v1/tests/test_setup.py` is evidence, not
an active runtime fallback. Its invariant groups map as follows:

| Archived group | Native replacement/evidence |
|---|---|
| `OrchestrationTests`: decline/dry-run/preflight, separate registration/enable/start/reload, later decline/partial outcomes | `tests/setup.rs` factory/lifecycle tests; `tests/cli.rs` real publisher snapshots and partial-prerequisite test |
| `OrchestrationTests`: no default/implicit download, exact-model consent, fresh postpull observation, preference/inference decline and cancellation | `tests/setup.rs` provider-factory, postpull and separate-consent tests; no service chat submission |
| `ProtocolTests`: admission/poll IDs, real STT nullable model and retired speech schema, strict app args/results, changed instance, finite inventory, Core identity/expiry/bounds | `tests/setup.rs` typed-backend and direct `Controller<FakeManager>` tests; real `smith_service` types are used, not a generic fake schema |
| `ProtocolTests`: malformed/duplicate/nonfinite helper envelope JSON | Typed Core/app library transport replaces helper output parsing; tests assert typed envelope rejection and installer result/body/field bounds. Provider raw JSON/metadata parsing remains separately strict in `src/model.rs` |
| `HttpTests`: raw-name ambiguity before filtering, conflicting identity, fixed loopback, no proxies/redirects/retries, slow/oversized framing, postpull tags, tool-free inert inference | Model module's controlled private TCP fixtures/tests, plus `tests/setup.rs` lazy provider and orchestration correlation tests |
| `ProcessTests`: helper stdin-not-argv, pipe limits, TERM/KILL and ELF validation | Wrapper helper process is removed, not ported. Direct calls retain Core manager/socket identity and cleanup budgets; native CLI ELF/location tests, bounded real stdin tests and chained SIGINT/SIGTERM tests replace installer process mechanics. Existing Core tests retain captured manager cleanup coverage |
| `PackagingTests`: interpreter/zipapp/import entrypoint mechanisms | Obsolete for the CLI: native `main.rs`/ELF; pure Ubuntu24/nonroot preflight, inert help/version, no recipient `--yes`, actual executable-relative payload tests. Optional GTK3 sibling packaging is a separate UI extension. Full native package assembly belongs to builder-only package tests |
| `RealPublisherIsolationTests`: full artifacts dry-run/decline/revalidation and truthful partial effects | `tests/cli.rs` executes the native publisher under private roots; publisher-owned invariant/race suites remain in their own modules/tests |

This mapping does not claim a new raw-socket parser inside the installer:
`Controller::app` owns the existing typed transport and peer/manager checks.
Neither installed-host behavior nor release provenance is established by fakes.

Source scope and required package/review/host evidence are defined in
[the native migration plan](../../docs/plans/LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md).
See [operator instructions](../dagger/INSTALLER.md) for package use. Local tests and
builds do not authorize deployment, live manager/provider effects or publication.
