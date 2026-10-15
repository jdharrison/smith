# Smith Linux-First Architecture Report

> **Evidence status:** This is a dated as-built investigation and gap register for the runtime implementation inspected on `feature/autonomy`. It is not the source of target policy. For normative scope, architecture, process, design, and host requirements, read `SCOPE.md`, `ARCHITECTURE.md`, `PROCESS.md`, `DESIGN.md`, and `HOST_PROFILE.md`.

**Purpose:** Architecture input for designing a Linux-first autonomous development workstation centered around Smith.

**Scope:** Runtime implementation assessed on `feature/autonomy` at the time of investigation. This report is based on source inspection rather than README documentation alone; re-validate claims after consequential runtime changes.

**Status vocabulary:**

- **Confirmed** — directly implemented in the current source.
- **Partial** — implemented, but with meaningful limitations.
- **Intended** — documented or implied, but not fully enforced.
- **Inference** — architectural consequence of the current implementation.

### GPU startup logging correction — bounded source assessment

**Implemented source under [GPU startup fix](plans/LOCAL-SMITH-NEXT-GPU-STARTUP-FIX.md):** the pinned `b11429` library offload INFO record requires verbosity 4 and plain logging; old Smith defaults suppressed or decorated the exact record its parser required. Vulkan now explicitly normalizes logging, requires complete records within the unchanged bounded raw startup prefix, and holds acceptance through a fixed at-most-one-second post-health observation window. Child exit, capture failure/EOF, cancellation, deadline, incomplete/duplicate/invalid evidence and overflow cannot establish GPU success. Safe one-time startup fallback, CPU independence and no post-acceptance replay remain. One fixed sanitized last-startup cause is retained internally through CPU outcomes/reuse/idle, reset on fresh preparation; no UI, wire, storage or service-log reporting was added.

**Evidence boundary:** separately approved non-model host probes enumerated RX 7900 GRE as the installed runtime's `Vulkan0`, confirmed driver/access and demonstrated the logging contract; they did not load a model or prove acceleration. Synthetic validation and fresh review results belong to `next/VALIDATION.md`. No installed changes, new host probes or real inference are authorized by this source fix. Earlier first-slice validation below is historical and did not detect this logging mismatch.

### 2026-10-14 multi-chat source assessment

**Implemented source:** `next/service/src/app_v2.rs` introduces versioned conversations, strict legacy migration, isolated history/context, atomic first-submit creation, idempotent revision-bound lifecycle mutations and a four-job durable FIFO with one provider worker. Desktop `workspace_controller.dart` adopts authoritative summaries/details and current-instance inventory, retains independent process-local drafts/reading/pending/provisional state, and supplies the existing redesigned sidebar/archive UI. Exact v1 installer shapes remain, while desktop app protocol and disk storage use v2; high legacy configuration revisions use lossless decimal strings only on v2 wire. No native runner concurrency or GPU fallback changes were made.

**Observed review and evidence boundary:** independent adversarial probes exposed phase-unaware completion reservations, corrupt identity relationships, high-v1-revision migration, malformed-frame error correlation, accepted-ID loss on poll failure, legacy inert-result parsing, hidden reattach failures and stale mutation-receipt rollback. Permanent regression remediation and final command/re-review outcomes are recorded in `next/VALIDATION.md`, not inferred from this assessment. Deletion is logical current-store removal with replay identifiers retained; drafts are not desktop-crash durable. Real simultaneous generation, installed migration/rollback, graphical QA and real inference remain separate unverified gates. Earlier single-chat descriptions below are historical or refer to the separate legacy application.

### 2026-10-14 Vulkan-first runner and expanded catalog source assessment

**Implemented source, partial GPU plan:** `next/` now packages separate SHA-pinned llama.cpp `b11429` CPU and Vulkan runtime trees. The service attempts fixed `Vulkan0` full offload, accepts GPU mode only from one strict `offloaded X/Y` startup line with `X == Y > 0`, and permits one same-lease/original-deadline CPU launch only after classified startup/evidence failure and confirmed cleanup. CPU explicitly disables devices/GPU layers. Completion failure is never replayed. The catalog is seven revision-pinned Apache-2.0 text models across Compact/Balanced/Large tiers, retaining Qwen3-14B as default.

**Evidence and gaps:** downloaded runtime archives matched recorded sizes/SHA256; archive members and selected ELF dependencies/RUNPATH were inspected. Focused Rust and builder tests use fake children/synthetic payloads and do not prove real acceleration. The current fixed `Vulkan0` attempt does not discover/classify devices, persist stable identity, select dedicated over integrated graphics, size memory or support automatic partial offload. Installer preference and app execution-status contracts are deferred. No model weights, host GPU probe, driver/permission change, installed upgrade, live inference or benchmark occurred. Real GPU/CPU QA and fresh independent assurance remain blocking before release claims.

### 2026-10-13 explicit Upgrade / Uninstall source assessment

**Implemented source:** installer adds four focused actions and native maintenance for old managed installations, independent of candidate equality. Service library supplies guarded Stop/unregister and held Core lease; SystemManager gains strict enablement/cgroup facts/quiescence. Installer verifies selected release/helper/standalone/launchers, preserves artifacts, exchanges candidate binary/client under installation→Core→runner locks, and verifies readiness after separately confirmed Start. Uninstall unregisters before preserving active entries away from original names; all state/models/releases/backups remain. Existing Install/Make mismatches still block; no force flag, old-helper execution or purge.

**Focused assurance:** fresh reviewer2de5830d-079f-47b3-b7d8-7b5e56fbe6dc inspected the source and independently ran57 offline maintenance/consent/ownership/partial/lock tests, all passed, no blocking implementation finding. Aggregate caught one stale GTK assertion expecting superseded upgrade-blocked wording; assertion was corrected to actual Upgrade guidance. Parent final aggregate/artifact results are retained in `next/VALIDATION.md`, not inferred here. No installed-host, live manager/kernel-cgroup/GUI/model operation or remote effect was performed. Real QA/clean-host/release provenance remains separate.

### 2026-10-13 minimal text chat source

**Implemented source:** [minimal-chat plan](plans/LOCAL-SMITH-NEXT-MINIMAL-CHAT.md) removes desktop installer instructions and inline model configuration; the neutral dark chat uses independently scrollable history and an anchored composer. Setup/model preferences belong to the separate installer. Readiness/identity, bounded history, pending correlation, query-only Retry and exceptional expired-request recovery remain unchanged. No active voice/speech capability is added. Current validation and fresh review results belong in `next/VALIDATION.md`; no installed-host or release guarantee follows.

### 2026-10-13 focused installer/model vertical slice

**Implemented source:** native GTK installer uses focused Install Smith → Model → service-backed check → Complete stages; Qwen3-14B Q4_K_M is a fresh-only preselection with exact metadata-pinned download, not bundled weights. Manage models bypasses artifact republication, preserves existing selection, supports separately consented add/select/check/removal. Service adds revision-checked preferences and durable verify/removal jobs without chat-history append. Selected models and any loaded runner block removal; installation lock→runner lease and held-directory exact-file deletion preserve partial/orphan data. Default thinking is no longer forcibly disabled; empty/reasoning-only/truncated finals fail.

**Evidence limits:** component agents reported locked Rust/Flutter source checks; focused reviewer identified scope-overclaim and post-cancel rerun UI defects, since fixed with scope-specific completion/sticky-cancel regressions. Parent aggregate/review results are in next/VALIDATION.md, not asserted by configuration. No weights/installed-host effects. CPU14B performance, real download/cancel/inference, clean-host UX and release provenance remain open.

### 2026-10-13 bundled native runner source assessment

**Implemented source:** `next/service/src/runner.rs` / `providers.rs` replace production external Ollama with a private service-owned CPU llama.cpp child; native `catalog.rs` performs separately consented curated HTTPS downloads. The paired client bundles pinned runner/libraries/licenses, not weights. Earlier dated `next/` descriptions retain their historical scope.

**Focused evidence:**25 offline runner tests passed. Three independent findings (model pathname reopen, detached active inference on app teardown, unbounded hashing) were fixed with inherited held model FD, atomic cancellation/join and per-read deadline/cancellation checks. Reviewer `092f336f-2fec-4d2a-bda1-ac0003731980` returned scoped static PASS for those three closures, not full release assurance. Current integration results are in [validation history](../next/VALIDATION.md). No installed-host changes or weights. Real inference, GPU, clean-host, licensing/SBOM and signed provenance remain open.

### Bounded GTK3 installer UI — source-only assessment

**Confirmed source/configuration, not validation or release evidence:** [the UI plan](plans/LOCAL-SMITH-NEXT-INSTALLER-UI.md), `next/installer/src/wizard.rs` and `ui.rs` add optional-feature native GTK3 sibling `smith-installer-ui`. Full package configuration includes both installer ELFs and the unchanged complete paired payload; the default full-recipient/artifact CLI and developer Make install routes remain unchanged, without GTK/display/Python requirements. GUI execution needs GTK3 shared libraries/session; GUI help/version skips GTK initialization/display discovery, not library loading. The app handoff remains instruction-only CLI fallback, and no persistent GUI command is installed.

**Source boundaries:** one owned worker, bounded typed channels/capped reports, exact request IDs and publisher-owned opaque review bind Install to its read-only artifact snapshot. Review/Refresh creates no destination locks/root-fact writes or lifecycle/provider effects. Register/Start default off; Start requires Register. Only selected lifecycle work constructs Controller after verified artifacts; applicable Core actions use fresh inline separate confirmations and original worker-owned 60-second tokens. Stale/duplicate/expired requests cannot approve; no silent reprepare. Cancel/close waits for worker exit/join and preserves accepted/uncertain effects without rollback or a filesystem deadline. The near-black/green single-page flow reports selected steps, not chat readiness: inference is not tested, models are chosen in the existing app, and no GUI provider/model/download/inference or automatic app launch occurs. Existing native publication safeguards and legacy archives/evidence remain intact.

**Pending evidence/host limits:** Dagger configures actual Ubuntu 24.04 UID/GID-1000 native quality and isolated Xvfb GTK smoke, plus both-ELF package checks; `make installer/ui-test` exposes the UI gate and `make installer/ui` exports its binary. UI results/counts and scoped independent review are not asserted here. The native migration's final integrated assurance was cancelled, not a PASS. Manual graphical/installed-host, provider/inference, clean-host/provenance and release gates remain separate. Operator QA uses the full package from `next/` via `./installer/dist/smith-installer-ui`, building first if needed; no such live flow was executed for this documentation work. The prior installed voice candidate is unchanged and its upgrade approval consumed.

### Native Rust installer migration — source-only assessment (CLI baseline)

**Confirmed implemented source, not release/deployment evidence:** [the native migration](plans/LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md) provides `next/installer` as an independent Rust crate/workspace with its own lockfile and a path dependency on `next/service`. Full recipient `smith-installer` and Make's explicit `--artifacts-only --client BUNDLE` and/or `--service BINARY` now share one native implementation. Only artifact mode permits `--yes`; recipient/model flags cannot be mixed into it. Full package output is `next/installer/dist/`; component binary export is `next/installer/build/smith-installer`. Python Dagger/`next/dagger/package.py` only assemble/verify builder metadata around native outputs, not recipient runtime, publication or lifecycle logic. The exact retired Python sources/tests remain checksummed under excluded `reference/python-installer-v1/`, not a maintained fallback.

**Read-only artifact observation:** `file next/installer/dist/smith-installer` and `readelf -h -l -d next/installer/dist/smith-installer` identified the existing export as a dynamically linked Linux x86_64 ELF using `/lib64/ld-linux-x86-64.so.2`, with libc/libgcc runtime dependencies. This is static artifact inspection, not proof that the export binds the latest source, executes all recipient stages or is release-ready. Prepared Ubuntu 24.04 recipients need compatible native runtime/desktop libraries, not Python or Rust/Flutter/Dagger/Docker toolchains. Native is not static linking or universal Linux ABI support.

**Confirmed source boundaries:** `src/setup.rs` directly constructs `smith_service::Controller` with packaged `payload/service/smith-service` as `source`, never installer `current_exe`, and reuses the existing Core/app identity/manager/socket/peer authority. Client/service source and unit/wire contracts remain unchanged by the migration. Runtime roots/backend/provider are lazy after artifact acceptance; dry-run and initial decline do not write destination files/locks or contact managers/providers. Artifact publication, unit registration/reload, Start, exact-model pull, saved preference and optional direct minimal inference have separate consent. Original Core-token expiry, bounded stdin, cooperative cancellation and manager/socket budgets remain, without automatic reprepare or a global filesystem deadline. The native publisher retains held-FD/no-follow traversal, ownership/mode/single-link and bounded-tree checks, verified cooperative lock, post-consent/under-lock revalidation, byte-identical canonical release manifests, no-clobber/synced publication and final service/client dependency verification. Atomic selector exchange retains displaced/conflicting data; failures can leave current changed, with no global transaction, same-UID filesystem CAS or automatic rollback.

**Configured isolated gates and limits, not new pass claims:** `next/dagger/src/smith_next/main.py` defines native quality/parity compilation/tests on actual Ubuntu 24.04 as UID/GID 1000, plus a separate Ubuntu runtime gate with Python absent and no useful PATH/toolchain lookup. That gate directly executes actual release ELF help/version/artifact-only dry-run; builder package tests consume the actual compiled installer through `SMITH_TEST_INSTALLER` with synthetic paired payloads. These source-inspected gates do not prove full recipient setup, installed-host/graphical lifecycle or actual provider behavior. No final integrated result, new exact test count or final independent assurance is asserted here; the final native migration integrated assurance was cancelled, not a PASS. Current source/artifact evidence and fresh architecture/adversarial review remain separate work. Bounded setup/progress output accepts pipes/sockets/terminals and rejects regular files/other devices; ordinary artifact-stdio reporting and filesystem/fsync are not covered by that console deadline contract. Partial/incomplete reports preserve prior effects and never imply rollback.

**Host and authority limits:** models remain external through already-running fixed numeric-loopback Ollama `127.0.0.1:11434`, with exact tags, separate consent and no automatic provision/start, proxies, redirects, retries, cloud fallback or cache deletion. Inventory is not inference; cancellation can leave daemon/cache outcomes unknown. This source migration supplies no live install, manager/provider/model/GUI effect, commit or remote action. The October11 voice candidate remains the latest recorded installed-host observation; its one-time approval is consumed. New native-candidate host effects, real pull/inference/GPU/graphics, clean-host compatibility, provenance/license/SBOM and publication still require separate evidence/review/approval. Historical hashes and review results below retain only their original scope.

### Text-only reset source assessment — before native installer migration

**Confirmed source at the text-reset stage:** the next product became a conversation-first typed-chat client and native Rust text service, with no wizard/completion gate or audio execution. A verified 46-file voice source snapshot is excluded under `reference/voice-chat-v1/`. Narrow v1 compatibility keeps valid old preferences/history/job results/IDs/tombstones readable; retired calls cannot dispatch speech. Core unit/manager/peer safety remains. The then-separate Python3.12 stdlib recipient zipapp packaged complete paired payloads and consented native lifecycle/model actions, without bundled weights or automatic OS/Ollama installation. Its Python runtime is superseded by the native source migration above, not retained as a current requirement.

**Historical evidence scope for the text reset:** text and packaging gates passed; independent reviews reproduced/fixed recovery-display, installer schema/model validation and packaging exclusion defects. The compiled executable fixture had an intermittent ETXTBSY; a controlled CLOEXEC writable-descriptor inheritance probe reproduced the failure and a test-only spawn fence mitigates it, but the exact historical descriptor holder remains an inference. That stage's source/artifact counts and reviewed caveats are retained in [validation history](../next/VALIDATION.md), not native-candidate validation. No host deployment, downloads, provider inference, GUI, voice/model data deletion or remote actions occurred during the reset; October11's installed voice candidate remains the latest recorded host observation below.

### 2026-10-11 approved next-generation installed-host upgrade

**Observed scoped deployment:** explicit operator approval bound the known old service (`8d23bbf8…`) to reviewed voice/chat candidate (`ac53d23c…`) and paired client manifest (`30c840b1…`). Existing dedicated unit was stopped/validated, old binary/state/unit/launcher metadata backed up, reviewed replacement published with displaced data retained, and matching client installed. Exact unchanged unit then started successfully; installed helper verified Core ready/setup true and app bridge against managed MainPID. Unit/Core state unchanged, enablement static. No legacy effects, provider queries/model downloads, Compose startup, GUI launch or audio trial. This is executing-agent live observation, not independent speech/deployment assurance. [Upgrade evidence](plans/LOCAL-SMITH-NEXT-VOICE-CHAT-UPGRADE-EVIDENCE.json) records approval and narrow checks; remaining provider/audio gates are not passed.

### 2026-10-08 next-generation voice/chat source assessment

**Confirmed source:** native Rust adds a separate application bridge, bounded durable context/preferences/job metadata, single async worker and local Ollama/Whisper/Kokoro adapters. Flutter adds shared typed/dictated draft and independent spoken-output selection; Python raw-byte speech APIs and constrained Compose packaging leave host microphone access in the client. Search/cloud/provisioning/autorun remain excluded. The older debug-only/presentation-only paragraphs below describe historical stages, not current provider capability.

**Isolated evidence:** aggregate four-suite `make test` and paired `make build` passed; speech image dependency resolution/pip check and fake numeric/API tests passed. Independent reviews reproduced medium worker/state-validation and draft/output/recovery bugs, subsequently fixed and independently closed. Current counts and scope-bound evidence are recorded in [validation history](../next/VALIDATION.md). Actual speech, model inference, container network behavior and installed-service upgrade were not performed; local cached artifact presence is not successful model initialization. No existing service/unit/data/provider was changed by agents.

### 2026-10-07 next-generation artifact installer assessment

**Historical source/evidence:** the Python artifact installer (originally `next/install.py`, subsequently relocated without behavior changes to `next/dagger/install.py`, now preserved in excluded `reference/python-installer-v1/`) and Make install targets implemented full client release/menu/icon/command and standalone binary publication, without artifact execution or service lifecycle effects. The native migration above replaces its active implementation, not its recorded evidence. Isolated tests cover manifests, permissions/links, compatibility, confirmation revalidation, concurrency and partial failures. Fresh adversarial review found late selector overwrite and missing dependency revalidation; remediated source uses displaced-object-preserving atomic exchange and dependency checks before selection and success. Selector conflicts can leave current changed, report failure and retain displaced data; no same-UID filesystem CAS or global transaction is claimed. Integrated command results and final review are recorded in [validation history](../next/VALIDATION.md); no normal-host install, graphical launch or service-manager mutation occurred.

### 2026-10-07 next-generation debug boot source and sandbox assessment

**Implemented in source:** `next/client` now has a debug Loader → Setup / Home controller and strict native-helper JSON adapter. `next/service` supplies a foreground Unix-socket daemon and separate inspect/prepare/apply management operations around the isolated `smith-next.service`. Binary installation, registration/reload, start and debug setup completion have separate effect-bound consent; boot/refresh is read-only. Peer UID/PID, managed executable/unit identity, bounded I/O and safe roots are checked; the only durable domain state is an atomically synced setup flag. Desktop disposal cleans control helpers but does not stop the independently managed daemon. No autostart, lingering, providers, chat, goals, tasks or notifications are implemented.

**Sandbox/pipeline evidence:** native implementation agents reported 18 Rust and 75 Flutter tests, including a real compiled Rust↔Dart socket detach/restart fixture. Current `make test` and `make build` passed through non-root Dagger component tasks; the exported desktop includes the safely copied compatible helper. Fresh assurance identified and source-confirmed fixes for pending-reload repair, cancelled-helper manager-child cleanup and production UID-zero rejection. Abrupt helper death protects the direct manager child only, not arbitrary forked descendants or rollback of accepted systemd jobs.

**Gaps:** real user-manager missing-unit/loading behavior, normal-host install/start, graphical close/reopen, logout/reboot recovery, Stop/uninstall/upgrade UX, crash-stale socket repair and release provenance remain separately approved gates. No normal-host unit was installed/started and no legacy service/data was changed. See [debug boot plan](plans/LOCAL-SMITH-NEXT-DEBUG-BOOT.md), [architecture](ARCHITECTURE.md#next-generation-debug-boot-implementation), [source-bound debug evidence](plans/LOCAL-SMITH-NEXT-DEBUG-BOOT-EVIDENCE.json) and [validation history](../next/VALIDATION.md) for scope and command results. This assessment does not establish a production release or erase the historical observations below.

### 2026-09-05 LOCAL-SMITH-USER-SERVICE-P0 integrated inspection

**Confirmed in source:** `scripts/install.sh` is a binary-only Ubuntu LTS x86_64 installer with `set -euo pipefail`, effective-root refusal, an explicit canonical effective-user-owned install root, safe root/executable mode checks, private install-root provenance marker, locked Cargo installation, executable/`--help` verification, and no service, sidecar, Docker, Flutter, or lingering action. `scripts/smoke-user-service.sh` provides only `--sandbox`, isolates HOME/XDG/config/install/runtime paths, shadows `systemctl` and `loginctl`, invalidates the user bus, suppresses ambient Docker/loopback status probes, retains a transcript, monitors for unmanaged `smith serve`, and asserts installer rejection and lifecycle separation.

**Implemented and sandbox-validated:** separated unit install/reload, start, stop, restart, login enable, login disable, observed status, and stop/disable/remove/reload uninstall operations are exposed through `smith settings service`. The source validates a canonical effective-user-owned stable executable against the installer’s private root marker, refuses root registration, respects `XDG_CONFIG_HOME`, renders the deliberately limited hardened unit, has no unmanaged fallback, supports `smith --help`, and never mutates lingering. Persistent enablement, raw manager state, failed manager queries, verified bridge identity, and configured preference are reported separately in CLI/API/client surfaces. `bash scripts/smoke-user-service.sh --sandbox` passed with fake managers, including negative assertions for unsupported/root/unsafe installation, ambient service probes, `enable --now`, lingering mutation, and unmanaged spawn. `cargo build --locked`, all 77 Rust tests, strict Clippy, all 32 Flutter tests, Flutter analysis, and the `smith_halo` Linux build also passed. Repository-wide `cargo fmt --check` remains blocked by pre-existing formatting drift in untouched Rust files; directly changed Rust files other than the pre-existing unformatted `src/commands/chat.rs` passed scoped `rustfmt` checking.

**Observed host status:** no normal-path installer or live user-manager lifecycle command was run for this work record. The 2026-08-28 reference observation was `smith.service` absent/inactive and `Linger=no`. Sandbox evidence cannot establish login, pre-login, reboot, or real bridge behavior.

### 2026-09-11 CLI lifecycle failure inspection and source hardening

**Observed, not remediated:** read-only manager inspection found a separately installed legacy unit enabled and in `activating/auto-restart`, without a Smith bridge. Its restart counter reached 8,091 during QA. Each attempt performed warm/model work and then exited because host-runtime OpenCode was unavailable in the user-manager environment. The loaded unit lacks the current source hardening, and no live stop, disable, replacement, reload, or restart was authorized or performed.

**Implemented in the dirty source worktree and sandbox-validated:** host-runtime startup resolves a canonical executable from an explicit absolute `OPENCODE_BIN`, the actual process `PATH`, or stable Ubuntu user locations; validates regular-file and executable mode; on Ubuntu x86_64 requires a native x86_64 ELF; and requires bounded `--version` plus OpenCode-specific `--help` behavior. This is explicitly behavioral validation rather than cryptographic provenance. Missing/non-executable/identity-invalid configuration is classified as permanent; spawn, wait, timeout, nonzero, and unclassified probe failures remain retryable. Linux probes run in a separate process group, terminate that group after every direct-child outcome, and confirm bounded child/group cleanup. `serve` executes one deterministic startup sequence proving task/job reconciliation precedes preflight and no warm service, voice sidecar, brain readiness, or bridge-readiness effect follows a failed preflight. Only permanent configuration failures map to status 78. The rendered unit prevents restart for status 78, limits other starts to five attempts per 300 seconds, defines no disruptive start-limit action, and does not order itself after `default.target`. Status validates manager properties, treats missing/duplicate/malformed values as unknown, and reports numeric `ExecMainCode`. `smith --version` and `smith -V` are restored while `smith version` remains compatible. The source installer verifies help and version.

**Self-check evidence:** 2026-09-12 run-specific sandbox packages under `target/smoke-user-service/runs/` record revision/worktree/source-input identity, raw command results, exact and replay units, and explicit environment limitations. Passing runs established repository-wide formatting, locked build/tests, strict Clippy, shell syntax, ShellCheck, `git diff --check`, source-installer validation, version checks, malformed-manager rejection, and the complete fake-manager lifecycle; superseded run IDs remain historical and must not be cited as the current review candidate. Each current review must name one source-identical package and independently retain its digest outside the owner-controlled package. Local checksums detect stale mixing and accidental changes, not adversarial owner rewriting. The manifest records that Cargo is locked but neither network- nor cache-hermetic. This supersedes the earlier statement that repository formatting still failed; the 2026-09-05 paragraph remains a historical observation of that earlier run. Source and sandbox evidence do not prove the installed legacy service is fixed.

### LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0 source inspection

**Implemented in the current dirty worktree, not a production release:** `src/main.rs` selects the dedicated `src/commands/offline_chat.rs` profile with `serve --chat-only --model <exact-installed-tag>`; plain `serve`, `chat`, and existing systemd registration remain legacy. Direct loopback Ollama uses an exact installed local model, no ambient proxy/redirects, tools, intent dispatch, tasks, OpenCode, Docker, search, voice startup, provisioning, or cloud fallback. The restricted router exposes health/chat/shutdown. One shared default session retains bounded in-memory context, cleared on backend restart; input/output/history, inference time, and concurrent admission are bounded. This does not sandbox the Ollama service or establish actual offline inference.

**Implemented desktop/packaging subset:** the original Home/Chat/read-only Settings subset via `offline_chat_screen.dart` is now wrapped by the approved Flutter boot and subsequent wizard implementation described below, without Work/service integration/required voice. `scripts/package-chat.sh` and `scripts/install-prebuilt.sh` support separate Ubuntu 24.04 x86_64 backend and complete Flutter Halo bundle candidates. Ollama/model weights and runtime libraries are separately provisioned; recipients do not need builder toolchains. No service/default migration is supplied. [Chat Release](CHAT_RELEASE.md) separates recipient/builder requirements and install/launch authority.

**Evidence status:** this addition is source/document inspection only; no final integrated tests are claimed. Real contextual inference with external networking denied, clean-host desktop installation without build tools, final candidate-bound checks and independent review, clean-source/license/SBOM evidence, and legacy release-workflow remediation remain publication blockers. `.github/workflows/release.yml` still publishes generic multi-platform backend archives on `release/*` tags rather than the paired slice with approval/evidence gates. Historical test results below do not validate this candidate.

### LOCAL-SMITH-FLUTTER-BOOT-P0 source inspection

**Implemented in current source, not an approved release:** `client/lib/main.dart`, `client/lib/home/desktop_boot_shell.dart` and `client/lib/bridge/desktop_backend.dart` implement primary Flutter Boot/Setup → Home/Chat, observational ordinary Settings, and a four-second factual readiness pulse. At the boot-only stage Setup used manual reviewed absolute executable and exact installed `name:tag` entry without discovery or provisioning. The later wizard section below supersedes those two limitations only; shell/PATH fallback, Ollama host startup, manager integration and headless implementation remain absent. CLI remains secondary query/status/configuration with all existing commands retained. Future headless is a separate deferred deployment/lifecycle capability, not CLI.

**Ownership boundary:** readiness checks/opening never start the chat backend; positive chat-only/non-echo Smith identity, Ollama provider, exact model and affirmative provider/model availability gate readiness. Compatible external backends attach without send/reset and are never stopped. Explicit Start captures a child with a 15-second startup budget and `--desktop-session`; `src/commands/offline_chat.rs` rejects occupied-port adoption in desktop mode, exposes health `desktop_session`, and initiates graceful shutdown on parent stdin EOF/read failure. Child liveness plus session health correlates ownership, not hostile same-UID authentication. Stop/normal close closes stdin and sends TERM, waits five seconds, then KILL and two seconds for exit confirmation; cleanup unknown blocks restart and normal exit. No HTTP shutdown or port/PID kill is used. No crash-proof immediate termination or provider-work cancellation is claimed. Home remains mounted across pulses/recovery, retaining draft/transcript without automatic resend/reset.

**Reported evidence, not independently rerun here:** agents initially reported 98 Flutter tests PASS and 110 Rust tests PASS; after independent review found shutdown-readiness and preflight-budget races, fixes added regressions and the client reported 108 Flutter tests PASS. Earlier full-suite OpenCode noisy-probe checks failed twice, followed by the current backend agent's reported passing suite; this does not establish a resolved noisy-probe root cause. Fresh static assurance re-review passed the remediated local implementation. Final source-bound integrated build/test evidence remains a separate gate. Actual desktop/process integration, denied-egress contextual inference, clean-host installation, clean-source/license/SBOM evidence and publication-workflow remediation remain no-ship gates. The supplied icon's original asset is absent from the workspace; integration remains pending.

### LOCAL-SMITH-DESKTOP-WIZARD-P0 current source inspection

**Implemented in inspected source, not final runtime evidence:** `src/commands/setup.rs`, `client/lib/bridge/desktop_setup.dart` and `client/lib/home/desktop_boot_shell.dart` implement Detect backend → Local runtime → Select/install model → Review & Start. `NativeSetupRuntime.discover` uses only `Platform.resolvedExecutable`'s directory plus `smith_backend/smith`, validates with `NativeExecutableResolver` before running commands, and supports an explicit advanced absolute path. There is no cwd/PATH/shell/recursive fallback or silent substitution after invalid discovery. Opening can execute read-only `setup inspect`; health checks/pulses and inspection never start the chat backend, infer or download. Inventory readiness means eligible exact local tags observed, not inference verified. `scripts/package-chat.sh` copies the release backend into full Halo `lib/`/`data/` payload and records its digest/version/source identity; existing bundle copies must match the release digest. `install-prebuilt.sh` copies the full payload install-only to `ROOT/halo/`, retaining `smith_backend/smith`. Standalone backend packaging is secondary. Assembly does not execute the resolver or authenticate/build-attest artifacts.

**Implemented separate setup boundary:** schema-v1 JSON inspection reads fixed-loopback Ollama `/api/tags`, rejects ambiguous inventory and excludes cloud/remote entries. `setup pull --model <exact-name:tag>` emits operation/sequence-bound NDJSON, normally starting with `approval_required`; exact model/operation stdin approval is required within 60 seconds before POST. Decline/EOF/mismatch/expiry cannot authorize a pull. Sanitized provider stages and known layer byte counts precede terminal status; success requires provider success and fresh exact eligible-tag verification. The no-proxy/no-redirect fixed-loopback client has two-second connect/inventory probes, 256 KiB tags, 16 KiB frames, 15-minute overall pull and 30-second idle deadlines. No automatic retries, sudo, shell/Ollama CLI, legacy install helper, daemon installation/start, manager mutation, cloud fallback, persistent configuration or cache rollback/deletion exists. The chat-only bridge still has no provisioning tools/routes.

**Partial setup and actual concurrency guards:** absent Ollama receives guided instructions for separate host installation/start. Explicit pull may cause external daemon internet use and persistent cache writes, with unknown sizes until reported. Cancel/EOF drops local request/observation and cleans its captured setup child; daemon continuation/partial cache outcome may remain unknown. `DesktopSetup` serializes one captured setup child and retains cleanup-unknown ownership. Wizard editing/selection requires idle setup, configurable non-ready backend, no owned backend and no active chat. Pull additionally requires backend absence and rechecks consent permission; conflicting backend/chat changes cancel locally. Start also requires backend eligibility and an inventory-observed selection. Read-only detection/inspection may run with an active backend. This is neither global provider locking nor cross-process scheduling. Owned/external lifecycle remains under `desktop_backend.dart`; external backends are never stopped. Home/draft/transcript remain mounted without automatic resend/reset.

**Implemented client follow-up, inspected source:** `PullProtocol` accepts a fully validated schema-v1 sequence-zero terminal `error` as preflight failure without consent or approval; terminal success/cancelled first remain rejected and the exception cannot authorize a pull. The wizard preserves validated failure code/message with control/bidi stripping and slash-prefixed path-like redaction, retaining no-auto-retry/unknown-cache guidance. Automatic detection and advanced-path resolution are bounded by the inspection timeout (10 seconds by default). These targeted fixes supersede the earlier minor protocol/error-handling gap, not real provider compatibility gates.

**Reported evidence and pending gates:** agents reported **127 Rust / full 136 Flutter tests PASS**, plus client analyzer/Linux build PASS; these are not final source-bound integrated evidence and were not rerun for this docs-only update. Fresh static assurance was reported **source-limited PASS with no blockers**; the subsequent low-severity client fixes above also passed focused independent static re-review. No final integrated checks or release approval are claimed. Actual provider pull/cancel and provider/backend/client version compatibility, desktop integration, denied-egress contextual inference, clean-host installation, final candidate-bound validation/fresh review, clean-source reconstruction, license/SBOM and publication-workflow gates remain pending. Source restrictions do not sandbox the external Ollama daemon. The supplied icon's original asset is unavailable; integration remains pending. See the [approved wizard plan](plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md), [Architecture](ARCHITECTURE.md#desktop-setup-command-boundary), and [Chat Release](CHAT_RELEASE.md).

## 1. Executive summary

**Separate legacy assessment boundary:** sections 1–14 below assess the legacy orchestration runtime and its dated as-built evidence, not the native `next/` text recipient contract. The installer migration does not migrate legacy `src/`, `client/`, install scripts or releases, or erase their observations and blockers.

Smith currently consists of a Rust control plane, primary Flutter desktop interface, Docker-managed workers, host/container OpenCode execution, Ollama-backed local inference, local search, and Python voice sidecars.

The current architecture is viable for a Linux-first single workstation, but Linux should be treated as the primary deployment target rather than simply another supported OS. The most important design implications are:

1. Smith’s supervisor is implemented for management by a **per-user systemd service**, not a system service; a legacy unit is now observed on the reference host but is failing, and the hardened source has not been deployed.
2. Autonomous work depends on the supervisor process remaining alive. Pipeline jobs are not independently durable.
3. Persistent state is mostly stored under the user’s config/data directories, but pipeline workspace state is stored inside Docker container writable layers.
4. Docker is used through the host CLI. Smith does not use a Docker socket inside worker containers.
5. AMD GPU support is currently expected through **host Ollama**, not AMD GPU passthrough into Docker.
6. The repository has no resource scheduler: no maximum agent count, CPU limits, memory limits, GPU scheduling, priorities, or backpressure.
7. A 64 GB / 16 GB VRAM workstation can support the target workload, but concurrent local inference, Docker builds, IDEs, game engines, and desktop applications will require host-level resource policy.
8. Legacy execution can run without Flutter, but the approved desktop policy makes Flutter primary and retains CLI as secondary query/status/configuration; future headless deployment/lifecycle is separate and deferred. Voice, file opening and browser-visible functionality require an interactive desktop session.
9. Linux avoids Windows’ executable-lock problem during Cargo rebuilds, but development and service binaries should still be separated operationally.
10. The largest future portability constraints are localhost assumptions, process-local background jobs, container-local pipeline state, hardcoded ports, and implicit access to a single host filesystem.

The ideal Phase-1 host should therefore be a continuously available Linux user environment with Docker, native AMD-capable Ollama, a systemd user service with an explicit login-only or separately approved lingering policy, fast local storage, encrypted backups, and enough headroom to preserve desktop responsiveness during autonomous work.

## 2. Current Smith process topology

### Topology

```text
                         Optional graphical session
                    ┌─────────────────────────────────┐
                    │ Flutter smith client             │
                    │ Chat / Tasks / Settings / Voice  │
                    └──────────────┬──────────────────┘
                                   │ HTTP loopback
                                   ▼
                    ┌─────────────────────────────────┐
                    │ smith supervisor / bridge        │
                    │ Rust process                      │
                    │ 127.0.0.1:4783                  │
                    └───────┬───────────────┬─────────┘
                            │               │
                            │               ├── Whisper sidecar :4790
                            │               └── Kokoro TTS :4791
                            │
                            ▼
                 ┌─────────────────────┐
                 │ Brain execution     │
                 │                     │
                 │ Container mode:     │
                 │ smith-brain         │
                 │ OpenCode + Ollama   │
                 │                     │
                 │ Host mode:          │
                 │ host OpenCode       │
                 │ + host Ollama       │
                 └─────────┬───────────┘
                           │ structured Smith intents
           ┌───────────────┼────────────────┬─────────────────┐
           ▼               ▼                ▼                 ▼
       SearXNG        Host file worker   Coding worker    Pipeline worker
       :4792          host filesystem   Docker           Docker
       Docker         only in host      smith-agent-    agent_<project>_<branch>
                      runtime           coding          container
                                          :4098            dynamic :4096-8191
                                                   │
                                                   ▼
                                             smith-sandbox
                                             node:22
                                             configurable host port,
                                             default :8080
```

### Smith supervisor and bridge

**Confirmed:** `smith serve` is the Rust supervisor and HTTP bridge in the same process.

- Bind address: `127.0.0.1:4783`.
- HTTP endpoints include health, chat, tasks, services, sidecar management, shutdown, and host file opening.
- The bridge is loopback-only and has no authentication layer.
- The client is not required; CLI and HTTP clients can call the bridge directly.
- `POST /v1/shutdown` triggers graceful shutdown.
- Ctrl-C and Unix SIGTERM are handled through Tokio shutdown signals.
- Background work runs in detached threads owned by this process; the bridge is not a durable job executor.
- References: `src/commands/serve.rs`.

**Lifecycle:**

- Reserves the loopback listener first; if occupied, accepts only an exact healthy Smith `/v1/health` identity and otherwise fails closed.
- Reconciles persisted queued/running task and job records before warm-service or bridge readiness side effects.
- Then starts configured warm Docker services and optionally starts Whisper and TTS as child processes.
- Stops sidecars it owns when the bridge exits.
- Does not itself run as a separate daemon from the bridge.

### Brain

#### Container runtime

**Confirmed:**

- Container name: `smith-brain`.
- Image: `ghcr.io/anomalyco/opencode`.
- No host port is published.
- Host memory is bind-mounted at `/memory`.
- OpenCode session data is bind-mounted at `/root/.local/share/opencode`.
- The container entrypoint is replaced with an idle shell loop; individual prompts are executed with `docker exec`.
- Reference: `src/docker/brain_runtime.rs:315-443`.

The brain is therefore a long-lived container, but each chat/think request starts a separate `opencode run` process inside it.

#### Host runtime

**Confirmed:**

- OpenCode runs as a native host subprocess.
- An Ollama-compatible API must be reachable at `127.0.0.1:11434`; this can be native Ollama or Docker Ollama published to that address.
- The host OpenCode configuration is generated under the Smith brain memory directory.
- Reference: `src/brain/host_runtime.rs:17-379`.

**Important distinction:** host runtime does not mean that all Smith work becomes native. Coding, pipeline, and search workers remain Docker-based.

### Ollama

**Confirmed:**

- API port: `11434`.
- Host Ollama is preferred when available, especially for AMD or when NVIDIA Docker support is unavailable.
- Docker Ollama container: `smith-ollama`.
- Docker model data uses named volume `smith-ollama`.
- Docker Ollama is published as `11434:11434`, without a loopback host prefix, so it can bind all host interfaces unless host firewall rules restrict it.
- Docker GPU passthrough only uses `--gpus all`, which the code treats as NVIDIA-only.
- References: `src/docker/model_runtime.rs:12-268` and `src/docker/model_runtime.rs:317-429`.

Default local model:

```text
qwen3:14b
```

Ollama is configured as a warm service by default, but Smith does not independently manage a native Ollama system service.

### OpenCode

OpenCode appears in several roles:

1. Brain image inside `smith-brain`.
2. Host-native OpenCode in host runtime.
3. Named agent containers such as `smith-agent-coding`.
4. Per-project pipeline containers named `agent_<project>_<branch>`.
5. One-shot coding containers for scratch demos.

There is no separate embedded model runtime in Smith. Smith delegates model execution to OpenCode and Ollama.

### SearXNG

**Confirmed:**

- Container: `smith-searxng`.
- Image: `docker.io/searxng/searxng:latest`.
- Host port: `127.0.0.1:4792`.
- Configuration is bind-mounted from Smith config data.
- JSON search API is enabled.
- The container has internet access.
- Reference: `src/docker/searx_runtime.rs:15-185`.

SearXNG is the primary live-search worker. Direct weather and DuckDuckGo fallback requests are also made by the host Rust process in `src/docker/search_runtime.rs`.

### Whisper

**Confirmed:**

- Host Python sidecar.
- Loopback port: `127.0.0.1:4790`.
- Default model: `base.en`.
- Default device: CPU.
- Default compute type: `int8`.
- Uses `faster-whisper`, FastAPI, and Uvicorn.
- The model is serialized behind a Python thread lock.
- Reference: `src/commands/whisper.rs` and `client/sidecar/whisper_server.py`.

Whisper is on-demand by default. The Flutter client can request sidecar startup through the bridge.

### Kokoro/TTS

**Confirmed:**

- Host Python sidecar.
- Loopback port: `127.0.0.1:4791`.
- Uses Kokoro ONNX.
- Models are stored under `client/sidecar/models`.
- Default voice: `am_onyx`.
- Synthesis is serialized behind a thread lock.
- Audio output is written to the system temporary directory.
- Reference: `src/commands/tts.rs` and `client/sidecar/tts_server.py`.

Kokoro is on-demand by default. Flutter falls back to platform TTS if Kokoro is unavailable.

### Coding agent

**Confirmed:**

- Named container: `smith-agent-coding`.
- Default port: `4098`.
- Image: `ghcr.io/anomalyco/opencode`, unless a generated local image exists.
- Uses the configured local Ollama model.
- Shares the configured scratch workspace.
- Reference: `src/docker/sandbox_runtime.rs:13-217`.

The coding agent is on-demand by default. It becomes warm only when explicitly configured or requested. There is an implementation mismatch: sealed-brain installation currently calls `ensure_coding_agent_ready()` during bootstrap, so a fresh install may start this worker even while its service policy is on-demand. Reference: `src/commands/system.rs:1152-1164`.

### Sandbox

**Confirmed:**

- Container: `smith-sandbox`.
- Image: `node:22-bookworm-slim`.
- Scratch directory is bind-mounted to `/app`.
- Default host port: `8080`.
- Runs a long-lived idle container, then launches generated web content inside it.
- Reference: `src/docker/sandbox_runtime.rs:257-335`.

The sandbox is on-demand and scratch files remain on the host when the container is removed.

### Pipeline workers

**Confirmed:**

- Pipeline commands run inside Rust background threads.
- Each project/branch gets a Docker container named `agent_<project>_<branch>`.
- The container clones the configured repository into `/workspace`.
- Pipeline artifacts are written under `/state`.
- OpenCode runs inside that container.
- References: `src/docker/agent_runtime.rs:628-856` and `src/brain/pipeline_jobs.rs:164-370`.

Plan, develop, review, and release are workflow stages, not separate persistent services.

Pipeline workers normally use Docker’s default bridge rather than the brain’s private network. The primary repository is cloned into the container’s writable `/workspace`, not mounted from the host. A configured local-path project therefore cannot be accessed by a pipeline container merely by existing on the host; it must be explicitly mounted or represented by a remote Git source.

Pipeline and worker ports are generally published with ordinary `host:container` mappings rather than explicit loopback bindings. Named workers and spawned workers may therefore be reachable from other host interfaces, depending on Docker firewall behavior.

### Persistence summary

- **Config and Smith data:** survive Smith and OS restart if the home storage survives.
- **Brain container:** usually remains present, but its restart policy is `no`.
- **Ollama Docker volume:** survives container recreation and OS restart.
- **SearXNG configuration:** survives container recreation.
- **Whisper/TTS child processes:** do not survive supervisor restart.
- **Coding containers:** may remain present, but are not guaranteed to restart automatically.
- **Scratch files:** survive container removal because they are host-bound.
- **Pipeline container workspace/state:** survives only while the container remains; removal loses it.
- **Pipeline Rust threads:** do not survive supervisor or OS restart.
- **Pipeline metadata:** survives; on the next successfully claimed supervisor startup, persisted queued/running metadata is reconciled to `interrupted` before readiness.

The 2026-09-05 source implements idempotent startup reconciliation for persisted task and pipeline-job records: queued/running records become interrupted with appended recovery guidance while prior results/summaries/errors and forward-compatible unknown JSON fields are preserved; awaiting-signoff and terminal records remain byte-identical. Reconciled JSON uses synced same-directory temporary files and atomic rename, with deterministic pre-rename failure tests proving the original remains intact and temporary files are cleaned. `handle_serve` reserves/identifies the bridge port, performs both reconciliations, and exits on reconciliation failure before warm-service startup and bridge readiness. State-preservation, repeat-run, atomic-failure, DTO-label, valid-Smith identity, and non-Smith listener tests pass. A real controlled restart remains unvalidated and requires later explicit approval.

## 3. Linux readiness

Linux support is present and meaningful, but not yet Linux-primary in every detail.

### Systemd integration

**Implemented and sandbox-validated; hardened source not deployed:** the source defines an XDG-aware per-user unit at `$XDG_CONFIG_HOME/systemd/user/smith.service`, falling back to `$HOME/.config/systemd/user/smith.service`. The rendered unit captures a canonical stable executable, runs `smith serve`, and uses the deliberately limited directives `Restart=on-failure`, `RestartPreventExitStatus=78`, `RestartSec=15`, `StartLimitIntervalSec=300`, `StartLimitBurst=5`, `TimeoutStopSec=30`, `KillMode=control-group`, `UMask=0077`, and `NoNewPrivileges=true`. It defines no `StartLimitAction` and no longer orders itself after the login target that selects it.

The service module defines separate install/reload, start, restart, stop, enable, disable, status, and stop/disable/remove/reload uninstall operations without `--now` and without an unmanaged spawn fallback. Status separately collects unit registration/load, active state/substate, manager result/restart/main-exit evidence, persistent enabled-at-login, raw enablement state, lingering, and Smith bridge identity/health; failed manager queries remain unknown, nonpersistent enablement remains separately labeled, host-brain health reuses verified bridge identity, and configured-port identity probes reject redirects. The hermetic fake-manager sandbox lifecycle uses a temporary run beneath a stable per-user QA root, requires the same installer provenance marker with no production runtime path override, passes, and retains evidence under `target/smoke-user-service/`; actual user-manager, login, logout, and reboot behavior remains unvalidated.

This remains a **systemd user service**, not a system service. Smith does not mutate `loginctl` lingering. Enabled-at-login does not establish pre-login operation; lingering is a distinct observed host policy and requires a separate manual approval if changed.

### Linux-specific paths and commands

- Native Whisper virtual environment: `client/sidecar/.venv/bin/python`
- Native TTS virtual environment: `client/sidecar/.venv-tts/bin/python`
- Host file opening: `xdg-open`
- Ollama discovery: `which ollama`, `/usr/local/bin/ollama`, `/usr/bin/ollama`
- GPU probing: `/dev/nvidia0`, `/dev/kfd`, `/dev/dri`, and `lspci`
- Systemd registration: `systemctl --user`
- Docker installation helper: `sudo systemctl start/enable docker`
- References: `src/commands/whisper.rs:173-198`, `src/commands/tts.rs:377-410`, `src/brain/files.rs:164-191`, `src/docker/model_runtime.rs:182-204`, and `src/main.rs:3598-3687`.

### XDG behavior

Most Smith configuration/data/cache paths delegate resolution to `directories::ProjectDirs` or `dirs`. The in-progress user-service implementation now reads `XDG_CONFIG_HOME` directly for the unit path and rejects a relative value, with an absolute `$HOME/.config` fallback. Re-validate all other paths independently; XDG-aware unit placement does not prove every legacy path follows the same rule.

Expected Linux paths are approximately:

```text
~/.config/smith
~/.local/share/smith
~/.cache/...
~/.config/opencode
~/.config/systemd/user/smith.service
```

### Executable discovery

Linux is relatively favorable for executable discovery:

- Host OpenCode is resolved to a canonical executable from an explicit absolute `OPENCODE_BIN`, direct inspection of the process `PATH`, or `$HOME/.opencode/bin/opencode` / `$HOME/.local/bin/opencode`; the executable bit and a bounded `--version` probe are required.
- `ollama` uses `which` and common system paths.
- `git`, `docker`, `curl`, `python3`, `xdg-open`, and `lspci` are expected to be discoverable by name.

However:

- An explicit `OPENCODE_BIN` must be delivered deliberately to the user-service environment; an interactive-shell export alone is not sufficient. Stable Ubuntu user locations avoid that dependency for normal installations.
- `SMITH_WHISPER_DIR` and `SMITH_TTS_DIR` are important for installed binaries.
- A released or `cargo install` binary does not automatically carry the repository’s `client/sidecar` directory.

### Linux-primary concerns

1. **Sidecar packaging is repository-location dependent.**  
   `resolve_sidecar_dir()` searches the current directory, executable ancestors, and the compile-time Cargo manifest path. A standalone installed binary may not find `client/sidecar`. References: `src/commands/whisper.rs:325-349` and `src/commands/tts.rs:336-365`.

2. **Systemd does not import a deliberate environment file or working directory.**  
   Variables such as `OPENCODE_BIN`, `SMITH_*`, provider credentials, and custom paths may not exist in a user service environment even if they exist in an interactive shell.

3. **`host.docker.internal` is not uniformly available on native Linux Docker.**  
   Some containers explicitly add `host-gateway`; others do not. This is particularly relevant to named coding and sandbox workers.

4. **Systemd SIGTERM behavior still needs real-manager validation.**
   The bridge now handles Unix SIGTERM alongside Ctrl-C and its HTTP shutdown notification, then stops owned sidecars. The code path is implemented, but an actual `systemctl --user stop` and journal/cleanup check has not been authorized or observed.

5. **Docker access assumes the invoking user can use the Docker CLI.**  
   The source does not select between rootful Docker, rootless Docker, or a dedicated service account.

6. **Shell usage is mostly Linux-compatible.**  
   Pipeline containers use `sh -lc`, `apk`, `apt-get`, Git, and OpenSSH. This is more natural on Linux than Windows.

7. **Linux TTS fallback is not fully confirmed.**  
   `flutter_tts` is declared in `client/pubspec.yaml`, but the generated Linux plugin registration
   currently includes audio recording/playback plugins and not `flutter_tts`. Kokoro should be
   treated as the dependable Linux TTS path until the fallback is validated.

8. **Concurrent sidecar startup is not fully serialized.**  
   Port checks occur before sidecar ownership is locked, so simultaneous ensure requests can race
   to start duplicate Whisper or TTS processes.

## 4. Container architecture

### Images

Current images include:

- `ghcr.io/anomalyco/opencode`
- `ollama/ollama`
- `docker.io/searxng/searxng:latest`
- `node:22-bookworm-slim`
- User-configured custom project images

There are no checked-in Dockerfiles for the runtime stack. Local agent Dockerfiles are generated dynamically under:

```text
<config_dir>/agents/<agent>/Dockerfile
```

Reference: `src/main.rs:2630-2727`.

The generated local-agent Dockerfiles use the selected image as `FROM` and set
`ENTRYPOINT ["opencode"]`, but the current generation path installs Git and OpenSSH rather than
installing OpenCode itself. The selected base image must therefore already contain a working
`opencode` executable. This differs from documentation that describes OpenCode as automatically
installed.

### Networks

#### Private brain network

```text
smith-brain-net
```

Created with Docker’s `--internal` flag.

Used by:

- `smith-brain`
- Docker Ollama when applicable

This prevents ordinary external egress from the brain network.

#### Public brain network

```text
smith-brain-public
```

Created as a normal non-internal Docker network.

Used when `network=public`.

#### Other containers

Named agents, coding workers, sandbox containers, and spawned pipeline containers generally use Docker’s default networking plus host-gateway configuration where explicitly added.

The generated Compose file is descriptive/grouping metadata, not the authoritative orchestrator.
Runtime containers are created through `docker run`; the Compose file does not fully reproduce
dynamic mounts, generated commands, all worker instances, or public/private runtime variants.

### Mounts

#### Brain

- Host Smith brain directory → `/memory:rw`
- OpenCode session directory → `/root/.local/share/opencode:rw`

The brain intentionally does not mount the host global OpenCode config, reducing accidental cloud credential exposure.

#### Named agents

- Global or generated OpenCode config → `/root/.config/opencode:ro`
- Configured workspaces → `/workspaces/<name>`
- Configured projects → `/projects/<name>`
- Project mounts are always read/write.
- Workspace access can be read-only or read/write.
- Reference: `src/main.rs:3441-3524`.

#### Spawned pipeline agents

- Their current project is cloned into the container’s `/workspace`.
- Other configured projects and workspaces may be mounted separately.
- Pipeline state is stored inside `/state`.
- No host bind mount is used for `/workspace` or `/state`.

This is a major durability distinction: pipeline source and artifacts are container-local unless separately copied out.

### Git behavior

Spawned containers:

- Install Git and OpenSSH inside the container at startup.
- Clone the repository if `/workspace` is empty.
- Fetch origin.
- Create or select the target branch.
- Configure commit identity.
- Push during release/develop flows where requested.
- Reference: `src/docker/agent_runtime.rs:692-772`.

Local repositories can also be mounted through configured projects or workspaces.

Pipeline command strings currently hardcode the Git remote name `origin`, even though project
configuration stores a remote field. Projects using another remote name require normalization
before these workflows can operate reliably.

### SSH behavior

Supported mechanisms:

1. Explicit SSH key mounted at `/root/.ssh/id_rsa:ro`.
2. `SSH_AUTH_SOCK` environment propagation.
3. Host `.ssh` directory mounted read/write if `SSH_AUTH_SOCK` exists.

The current code does not visibly mount the SSH socket itself, so agent forwarding depends on external Docker/environment behavior.

Project SSH key paths are not consistently passed through the same `~` expansion logic as workspace paths. Linux deployments should use stable absolute paths.

### Credentials and secrets

Credentials can enter through:

- Provider API key environment variables for named agents.
- Model profile environment mappings.
- Project GitHub token stored in Smith config.
- Project SSH keys.
- Mounted OpenCode configuration.
- Cloud MCP URLs.

The brain explicitly avoids mounting global OpenCode configuration and cloud keys. Other agent containers may receive them.

The implementation does not provide a dedicated secret store. Some spawned-container environment values are passed as concrete Docker command arguments, which may expose them to process inspection during startup.

### Docker socket access

**Confirmed:** Smith invokes the Docker CLI from the host Rust process.

There is no visible `/var/run/docker.sock` mount into OpenCode, coding, or pipeline containers.

This is a favorable boundary: worker containers do not automatically control the host Docker daemon.

### Lifecycle and cleanup

- Most Smith-managed containers use `--restart no`.
- Named and spawned OpenCode workers use `--restart on-failure:1`.
- Existing named containers may be removed and recreated to change mounts, ports, or networks.
- Stopped spawned containers can be pruned.
- Release normally stops spawned agents unless `--keep-agent` is used.
- Docker images are removed only when explicitly requested during uninstall.
- Docker volumes are not generally removed during uninstall.
- References: `src/docker/agent_runtime.rs:604-625` and `src/commands/system.rs:1227-1298`.

### Many-agent behavior

The architecture can create many containers, but currently assumes relatively low concurrency.

Likely contention points:

- Concurrent Ollama model calls competing for 16 GB VRAM.
- Model loading and context memory competing for 64 GB system RAM.
- Docker overlay filesystem and build caches competing for NVMe I/O.
- Multiple project clones and dependency installs.
- Parallel Git fetches and package downloads.
- Host IDE, browser, meeting, Unity, Unreal, Steam, and media workloads.
- Dynamic port allocation in the 4096–8191 range.
- Global container names and project/branch-derived names.
- Shared host workspaces mounted read/write by multiple workers.

No container CPU or memory constraints are configured.

There is no application-level admission limit or queue. A task is normally created directly in
`Running` state; the `Queued` task/job states are not a general scheduler. Concurrent requests
can therefore start immediately and compete for the same Ollama model, Docker daemon, ports,
shared workspaces, and project/branch container.

Cancellation is also limited:

- There is no bridge endpoint for cancelling a task or pipeline.
- The Docker execution path uses a process-global cancellation flag rather than per-job state.
- Killing the local Docker command does not explicitly track or terminate every descendant
  process inside the container.
- The pipeline watcher’s six-hour limit is an observation limit, not a guaranteed termination
  timeout; the underlying pipeline may continue after the client marks it failed.

Task and pipeline metadata use load-modify-write operations without cross-process file locks.
Pipeline job identifiers use Unix-second precision, so simultaneous starts can collide. Port
allocation also performs a check-then-start sequence and is not atomic.

## 5. Filesystem and storage map

### Configuration

Expected Linux location:

```text
~/.config/smith/config.toml
```

Contains:

- Projects
- Workspaces
- Agent/model profiles
- Runtime/network mode
- Service policies
- Host roots
- GitHub tokens
- Project settings
- Install marker

Other configuration data:

```text
~/.config/smith/.smith-installed
~/.config/smith/compose/compose.yaml
~/.config/smith/searxng/settings.yml
~/.config/smith/agents/<name>/
```

Agent directories may contain:

```text
Dockerfile
opencode.json
opencode.jsonc
prompt.md
mounts.md
```

### Brain data

Expected Linux location:

```text
~/.local/share/smith/brain/
```

Contents:

```text
profile.md
episodic/
  session.jsonl
  <timestamp>.md
working/
  analysis.json
  session.md
  chat_session.json
  pending_intent.json
  opencode.json
journal/
```

This includes sensitive conversational and behavioral data.

Reference: `src/brain/memory.rs:1-218`.

### Tasks

```text
~/.local/share/smith/tasks/<task-id>.json
```

Tasks include user prompts, results, errors, timestamps, and pipeline IDs.

### Pipeline runs

```text
~/.local/share/smith/pipeline-runs/<job-id>/
  meta.json
  log.txt
```

These are durable metadata/log files, but the actual project workspace and `/state` artifacts are inside the spawned Docker container.

### Projects

Remote projects are cloned under:

```text
~/.local/share/smith/projects/<project-name>
```

Local projects remain at their configured host paths.

Reference: `src/main.rs:3321-3410`.

### Scratch workspace

```text
~/.local/share/smith/sandboxes/scratch
```

This is bind-mounted into:

- Coding agent: `/workspaces/scratch`
- Sandbox: `/app`
- One-shot coder: `/workspaces/scratch`

Scratch files survive container removal.

### Model storage

#### Native Ollama

Smith does not choose the native Ollama storage path. Native Ollama uses its own platform defaults, typically under the user’s home directory.

#### Docker Ollama

```text
Docker named volume: smith-ollama
Container path: /root/.ollama
```

This is potentially the largest Smith-managed cache.

### Sidecar environments and models

Under the repository:

```text
client/sidecar/.venv/
client/sidecar/.venv-tts/
client/sidecar/models/
  kokoro-v1.0.onnx
  voices-v1.0.bin
```

Whisper model downloads are managed by the Python dependency/runtime stack rather than explicitly by Rust.

### Temporary files

- Flutter recordings are written under the platform temporary directory.
- Kokoro generated WAV files are written under the Python system temporary directory.
- Pipeline file writes use temporary files before `docker cp`.
- Sandbox server logs are written to `/tmp/smith-serve.log` inside the sandbox.
- References: `src/brain/pipeline_jobs.rs:298-319`, `client/sidecar/tts_server.py:168-199`, and `client/lib/listen/voice_turn.dart:419-471`.

Temporary voice audio and generated TTS files may accumulate unless the client or operating system cleans them.

### Docker data

The repository does not configure Docker’s data root. A normal Linux Docker Engine will use its system-managed Docker data directory, commonly:

```text
/var/lib/docker
```

This can contain:

- Image layers
- Build cache
- Container writable layers
- Named volumes
- JSON logs
- Overlay filesystem data

### Storage classification

#### Critical persistent state

- `~/.config/smith/config.toml`
- Brain profile and memory
- Tasks
- Pipeline metadata/logs
- Local project repositories with uncommitted changes
- Scratch content considered valuable

#### Sensitive

- Chat transcripts
- Brain profile
- Task prompts/results
- GitHub token
- SSH key paths and potentially mounted SSH material
- Cloud API environment values
- Project source

#### Large reconstructable cache

- Ollama model data
- Docker images and build cache
- Whisper model cache
- Python virtual environments
- TTS ONNX models
- Node dependencies generated in scratch/projects

#### Write-heavy

- Chat JSONL
- Working session files
- Journal logs
- Task JSON files
- Pipeline logs and artifacts
- Docker overlay layers
- Game-engine and compiler build directories

### Storage implication

The fast NVMe should prioritize:

- Linux operating system
- Active repositories
- Smith config/data
- Docker active layers/build cache
- Compiler and game-engine working trees
- Swap or equivalent emergency memory

The secondary SSD is suitable for:

- Model caches
- Docker data if configured deliberately
- Large generated artifacts
- Archived project clones
- Local backup staging

The current 1 TB + 1 TB arrangement is workable, but the combination of Docker, game engines, SDKs, models, build trees, and project repositories can consume both disks quickly.

## 6. AI and GPU architecture

### Ollama abstraction

Smith is hardware-agnostic at the API boundary:

```text
OpenCode → Ollama-compatible HTTP API → model
```

It supports:

- Native host Ollama
- Docker Ollama
- CPU fallback
- NVIDIA Docker GPU passthrough when detected

### AMD ROCm relevance

The current source detects AMD-like Linux hardware using:

- `/dev/kfd`
- `/dev/dri`
- `lspci`

However, Smith does not:

- Install ROCm.
- Configure ROCm.
- Pass `/dev/kfd` or `/dev/dri` into containers.
- Pass AMD GPU devices to Docker workers.
- Detect VRAM capacity.
- Schedule GPU usage.
- Validate Ollama’s actual AMD acceleration.

References: `src/docker/model_runtime.rs:126-220`.

For the RX 7900 GRE, the intended architecture is therefore:

```text
Linux host
  └── native Ollama with working AMD/ROCm support
        └── HTTP API on 127.0.0.1:11434
              ├── host-mode OpenCode
              ├── brain container via host gateway
              └── coding workers via host gateway
```

This is materially better aligned with the current source than attempting AMD GPU passthrough into every Docker worker.

### Docker GPU assumptions

The only explicit Docker GPU path is:

```text
docker run --gpus all
```

The code treats this as NVIDIA-specific and refuses to use it without an NVIDIA container runtime.

Therefore:

- NVIDIA Docker support is implemented.
- AMD Docker GPU support is not implemented.
- AMD acceleration should be provided by native host Ollama.
- OpenCode workers themselves do not need direct GPU access if they call host Ollama.

### Whisper

Whisper currently defaults to:

```text
device=cpu
compute=int8
model=base.en
```

Configuration is available through:

```text
SMITH_WHISPER_MODEL
SMITH_WHISPER_DEVICE
SMITH_WHISPER_COMPUTE
```

The repository does not provide AMD-specific Whisper acceleration. CPU execution should be considered the supported baseline.

### Kokoro

Kokoro runs through `kokoro-onnx`. The repository does not select a GPU execution provider or configure ROCm. Treat it as CPU-oriented unless independently validated on the target machine.

### OpenCode and cloud models

Cloud-capable agent profiles can use:

- Anthropic
- OpenAI
- OpenRouter
- Other provider names
- Custom base URLs

Local profiles use Ollama.

The brain itself is generated with an Ollama provider and does not mount cloud credentials by default.

## 7. Concurrency and resource-management behavior

### Actual mechanisms

Smith uses:

- Tokio async runtime for the HTTP bridge.
- `spawn_blocking` for blocking HTTP handlers.
- Standard Rust threads for search, file, code, and pipeline work.
- Dedicated stdout/stderr reader threads around Docker/OpenCode processes.
- Docker containers and subprocesses for workers.
- Filesystem JSON files as the task/job database.
- `Mutex` around sidecar ownership and Python-side model inference locks.
- Global atomic cancellation state for foreground spawned prompts.

### Background tasks

Search, files, code, and pipeline intents return acknowledgements and execute in background threads.

Pipeline jobs:

- Write metadata to disk.
- Spawn a Rust thread.
- Run Smith CLI subprocesses synchronously.
- Write logs.
- Update metadata.
- Potentially wait up to six hours in the client-side watcher.

References: `src/brain/pipeline_jobs.rs:164-246` and `src/commands/chat.rs:615-770`.

### Missing controls

The current implementation has no explicit concepts for:

- Maximum concurrent agents
- Maximum concurrent pipeline jobs
- CPU quotas
- RAM quotas
- VRAM quotas
- GPU scheduling
- Job priority
- Desktop-vs-background priority
- Queue backpressure
- Central job queue
- Persistent worker ownership
- Job cancellation API
- Pipeline-wide timeout
- Automatic job recovery after supervisor restart
- Per-job filesystem locking

The `--max-validate-passes` option is workflow validation, not resource management.

### Durability limitations

Pipeline job execution is process-bound:

- The Rust thread disappears if the supervisor exits.
- The child Smith CLI process disappears with it.
- The task/job files remain on disk, potentially marked `running`.
- Startup does not scan and reconcile abandoned jobs.
- There is no durable queue consumer.

The job ID is currently generated from Unix seconds:

```text
job-<timestamp>
```

Two pipeline jobs started in the same second can collide.

Task IDs use nanoseconds, but task file updates are still ordinary whole-file writes without a lock.

### Shared-state races

The following state is globally shared:

- One default chat session.
- One `pending_intent.json`.
- One chat transcript.
- One task directory.
- One current pipeline status.
- One sidecar hub.
- One global cancellation atomic.

The Flutter client serializes ordinary sends, but multiple clients or simultaneous HTTP callers are not isolated.

### Resource implications for the target workstation

The main risk is not CPU capacity alone. It is contention between:

- A 14B local model and multiple contexts.
- Docker workers.
- Unity/Unreal compilation.
- Browser and meeting applications.
- Cursor and language servers.
- File indexing.
- Game-engine shader compilation.
- Steam and media workloads.

The external host design should reserve interactive headroom rather than allowing Smith to consume all 32 threads, 64 GB RAM, or 16 GB VRAM.

## 8. Autonomous and headless operation

**Historical/as-built legacy assessment, not current product priority or headless approval.** Flutter is the primary desktop interface under the approved boot plan. CLI remains secondary with existing commands retained; non-GUI commands do not establish a supported headless deployment/lifecycle. Future headless work is separate and deferred. The observations below do not authorize new service/provisioning/headless implementation.

### What works without the Flutter client

**Confirmed:**

- `smith serve`
- CLI chat
- Pipeline jobs
- Search workers
- File workers in host runtime
- Coding workers
- Sandbox workers
- Docker management
- Task persistence and HTTP task querying

The Flutter client is a UI and voice front end, not the execution engine.

### What works without a graphical login

Potentially:

- Rust supervisor
- Docker services
- Ollama
- Brain
- Pipeline workers
- Search
- Host file operations that do not open files
- CLI workflows

But current systemd registration is user-scoped and tied to `default.target`. Smith does not configure user lingering.

### Graphical-session dependencies

Require an interactive desktop session:

- Flutter UI
- Microphone capture
- Audio playback
- `xdg-open`
- Browser-visible SearXNG UI
- Desktop TTS fallback
- Any future automation of Unity, Unreal, or GUI-only tools

Host-mode file search itself does not require a display. Opening a file does.

### Headless limitations

1. Pipeline jobs are not independently supervised.
2. Systemd user service boot behavior requires deliberate user-session/lingering configuration.
3. Sidecars are owned by the bridge process.
4. No native systemd units exist for Whisper, TTS, Ollama, or individual agents.
5. No job recovery occurs after supervisor termination.
6. No external event queue exists to submit jobs while Smith is down.
7. Shutdown handling is designed more around Ctrl-C and HTTP shutdown than Unix service signals.

The desired headless state is achievable on Linux, but the host must provide a continuously running user service manager and Docker daemon.

## 9. Network and privacy behavior

### Bridge and sidecars

- Bridge: loopback `127.0.0.1:4783`.
- Whisper: loopback `127.0.0.1:4790`.
- TTS: loopback `127.0.0.1:4791`.
- SearXNG: host published only on `127.0.0.1:4792`.

These are not exposed on the LAN by the explicit port bindings.

### Private mode

Private mode means:

- Brain container is placed on `smith-brain-net`.
- Network is Docker-internal.
- Brain is intended to have no general internet access.
- Brain uses Ollama only.
- Search occurs through a host-side SearXNG worker.
- Search results are declassified into brain prompts.

However, host Ollama mode creates a soft boundary:

```text
brain container → host.docker.internal → host Ollama
```

The code itself reports that this may allow host-path leakage in private mode.

Private mode is therefore a policy and container-network boundary, not a complete host firewall or data-loss guarantee.

The private network applies primarily to the brain container. SearXNG, coding agents, named
agents, sandbox containers, spawned pipeline workers, host-side search fallbacks, and Docker
Ollama can still have external connectivity through their own networks or through the host.

### Public mode

Public mode places the brain on:

```text
smith-brain-public
```

This is a normal egress-capable network.

The generated public brain configuration still primarily uses Ollama, but the container can reach the internet.

### Direct host network access

The Rust host process can directly access:

- SearXNG
- DuckDuckGo
- wttr.in
- Open-Meteo
- GitHub API
- Git remotes
- Model download URLs
- Docker registries
- TTS model download URLs

Private mode does not prevent these host-side operations. Search is intentionally allowed to use the public internet.

### Cloud credentials

Cloud credentials can enter through:

- `OPENAI_API_KEY`-style environment variables.
- Named-agent provider environment passthrough.
- Project GitHub token.
- SSH keys.
- OpenCode config mounts.
- Cloud MCP URLs.
- User-provided project setup scripts.

The brain’s sealed configuration avoids global OpenCode credentials, but other agent containers are deliberately more permissive.

### Linux host network design implication

The host should support:

- A clear outbound firewall policy.
- Loopback-only bindings for Smith APIs.
- Explicit allowlisting or user confirmation for cloud providers.
- Separate environment handling for systemd and interactive shells.
- Encrypted storage for Smith data and credentials.
- Easy inspection of Docker networks and egress.

The explicit loopback bindings for the bridge, Whisper, TTS, and SearXNG should not be assumed
for every Docker service. Docker Ollama, named agents, spawned agents, and the sandbox use
ordinary host-port mappings in the current source and may bind beyond loopback. This should be
verified and restricted by the Linux host firewall or by deployment configuration.

## 10. Linux development workflow

### Rust

Current repository requirements:

- Rust `1.83` minimum.
- Stable toolchain.
- `rustfmt`.
- `clippy`.
- Cargo build/test workflow.
- `scripts/validate.sh` runs formatting, build, clippy, and tests.

References: `Cargo.toml`, `rust-toolchain.toml`, and `scripts/validate.sh`.

Linux is a natural development platform for this Rust code because:

- Docker CLI and shell execution are native.
- Pipeline scripts use POSIX shell syntax.
- `systemd --user` is directly available.
- Linux does not normally prevent replacing an executable while it is running.

### Running the supervisor during development

There are two operational modes:

1. Run `cargo run -- serve` interactively.
2. Run a built or installed binary through systemd.

The systemd unit captures the exact `current_exe()` path at registration time. Registering a `target/debug/smith` binary can therefore couple the service to a development build path.

Recommended operational separation:

```text
Development binary:
  repository/target/debug/smith

Service binary:
  stable user installation path or release path
```

Linux permits replacing an executable while an old process is running, so it avoids the Windows file-lock failure. However, the running process continues using the old executable image until restarted.

### Flutter

The current Linux Flutter project uses:

- GTK 3.
- CMake.
- C++14.
- `pkg-config`.
- Native Linux plugins for `audioplayers` and `record`.
- References: `client/linux/CMakeLists.txt` and `client/linux/flutter/generated_plugin_registrant.cc`.

The Flutter client is built, packaged, launched, and validated independently of Smith’s supervisor. It is not installed by `scripts/install.sh`, embedded in the Rust binary, or registered by `smith.service`; it shows offline/unknown state until a verified Smith bridge is available.

### Sidecars

Sidecar development is repository-location-sensitive:

```text
cd client/sidecar
python3 -m venv .venv
python3 -m venv .venv-tts
```

The current installer automatically creates environments and installs requirements, but the sidecars remain under the source tree rather than a system-wide application data directory.

### Logs

- Supervisor: systemd journal when run under systemd.
- Pipeline logs: `~/.local/share/smith/pipeline-runs/<job>/log.txt`.
- Docker workers: `docker logs <container>`.
- Sandbox server: `/tmp/smith-serve.log` inside the sandbox.
- Sidecar logs: inherited by the supervisor or displayed during manual execution.

There is no centralized Smith log store.

### Hot reload

- Flutter supports normal Flutter development hot reload.
- Rust supervisor has no hot reload.
- Docker workers require container restart/recreation for image or mount changes.
- Sidecars require process restart for code/model changes.
- Service changes require systemd reload/restart.

## 11. Linux development requirements

### Core host/system requirements

Derived from current source:

- Linux x86_64.
- Rust stable, minimum Rust 1.83.
- Cargo.
- Rustfmt and Clippy.
- Docker CLI.
- Running Docker daemon.
- Git.
- OpenSSH client.
- `curl`.
- POSIX `sh`.
- `python3`.
- Python virtual-environment support.
- `systemd --user` for automatic supervisor startup.
- `sudo` and `systemctl` only for Docker installation/boot management.
- `xdg-open` for host file opening.
- `lspci` for best-effort GPU detection.

### Desktop development requirements

For the Flutter Linux client:

- Flutter SDK.
- Dart SDK compatible with `^3.10.7`.
- CMake.
- C++ compiler/toolchain.
- Ninja or the normal Flutter Linux build backend.
- `pkg-config`.
- GTK 3 development headers.
- GTK runtime.
- Linux audio stack compatible with `record` and `audioplayers`.
- Microphone permissions and working PipeWire/PulseAudio/ALSA integration.
- Wayland or X11-compatible graphical session.

The repository itself does not mandate a specific desktop environment or Linux distribution.

### AI requirements

Required for local inference:

- Ollama, native or Docker.
- Enough disk for model data.
- Network access for initial model pulls.
- Native AMD-compatible Ollama acceleration for the RX 7900 GRE if GPU inference is desired.

Optional:

- ROCm components, depending on the chosen Ollama build and kernel/userspace stack.
- CUDA/NVIDIA Container Toolkit only for NVIDIA hardware, not relevant to the specified AMD GPU.

### Voice requirements

Whisper dependencies:

```text
fastapi
uvicorn
faster-whisper
python-multipart
pydantic
```

Kokoro dependencies:

```text
fastapi
uvicorn
kokoro-onnx
soundfile
pydantic
numpy
```

Also potentially required:

- `espeak-ng`
- Working audio libraries
- Python build/runtime dependencies required by the selected wheels

### Optional components

- Flutter client.
- Whisper.
- Kokoro.
- SearXNG, if web search is disabled.
- Host OpenCode, if using container brain runtime.
- Cloud provider credentials.
- GitHub token and PR automation.
- Additional named agents.
- Unity, Unreal, Godot, embedded SDKs, and other user development tools.

### Current documentation divergences

The following differences between current source and older documentation matter to deployment
planning:

- Documentation describes short-lived `smith-search-*` containers; current code uses persistent
  SearXNG plus host-side HTTP fallbacks.
- Documentation describes host runtime as requiring native Ollama; current host runtime accepts
  any Ollama-compatible API reachable at `127.0.0.1:11434`, including Docker Ollama.
- Coding is described as on-demand, but sealed-brain installation currently starts the coding
  worker during bootstrap.
- Binary release/install paths do not package the repository’s Python sidecars, virtual
  environments, models, or `espeak-ng`.
- Linux TTS fallback is documented through `flutter_tts`, but the generated Linux plugin
  registration does not currently include that plugin.

## 12. Phase-1 migration concerns

### High priority

1. **Systemd service availability without graphical login**  
   Decide whether Smith runs after login only or through a lingering user service manager.

2. **Native AMD Ollama path**  
   The current code strongly favors host Ollama for AMD. The Linux installation must validate actual ROCm-compatible inference before depending on it.

3. **Docker access model**  
   Smith assumes direct Docker CLI access. Decide rootful Docker group, rootless Docker, or a dedicated service account.

4. **Storage capacity**  
   Models, Docker layers, game engines, SDKs, repositories, and build artifacts will compete for two 1 TB drives.

5. **Pipeline durability**  
   Current pipeline execution is not crash-resilient. The host should have reliable power, storage, and backups, but this does not eliminate the software-level recovery limitation.

6. **Interactive responsiveness**  
   Docker and Ollama are currently unlimited. Host-level resource governance is needed to preserve Unity/Cursor/browser responsiveness.

7. **Sidecar deployment path**  
   Keep the repository and sidecar directories at stable absolute paths, or plan explicit `SMITH_WHISPER_DIR` and `SMITH_TTS_DIR` configuration.

8. **Private-mode expectations**  
   Private mode blocks ordinary brain-container egress but does not create a complete host air gap.

### Medium priority

- Ensure `host.docker.internal` works on the chosen native Docker setup.
- Ensure systemd receives required environment variables.
- Keep project paths free of unusual filesystem semantics.
- Decide whether Docker data belongs on NVMe or SATA.
- Define encrypted backup coverage.
- Validate Linux audio and microphone support before relying on voice-first interaction.
- Avoid registering a development `target/debug` binary as the permanent service binary.

### Lower priority

- Desktop file associations.
- Browser access to local SearXNG.
- Optional platform TTS fallback.
- Additional named agents.
- Automated installation of optional cloud providers.

## 13. Future portability considerations

### No concern

#### Structured intent protocol

`SmithIntent` provides a reasonably portable boundary between brain reasoning and host workers.

Reference: `src/brain/intent.rs`.

#### Explicit project/branch metadata

Pipeline jobs record project, branch, base, plan, and task identifiers. This is useful for eventual worker separation.

#### HTTP bridge

The loopback bridge is already a service boundary, even though it currently assumes localhost.

### Keep in mind

#### Hardcoded ports

Ports include 4783, 4790, 4791, 4792, 11434, 4096, 4098, and 4096–8191 for spawned workers.

This is acceptable for Phase 1, but remote workers will eventually need advertised endpoints rather than implicit localhost ports.

#### Host gateway assumptions

Containers use `host.docker.internal` to reach host Ollama. This is host-specific and not a portable worker address.

#### Host path bind mounts

Agents receive paths from one machine’s filesystem. Remote workers cannot consume these paths without a workspace transfer or shared filesystem contract.

#### Global Smith names

Container names such as `smith-brain`, `smith-ollama`, and `smith-agent-coding` assume one Docker namespace.

#### Environment-variable credentials

Provider credentials are resolved from the local host environment. Remote execution would require credential scoping and transport rules.

#### Single default chat session

The bridge protocol currently supports only session `"default"`.

#### Local service discovery

Health checks use `127.0.0.1` and fixed ports. This is correct for one machine but not for heterogeneous workers.

### Eventually needs abstraction

#### Pipeline state inside container writable layers

The current `/state` and `/workspace` model makes the container both worker and storage authority. Remote workers will require durable, externally addressable artifact storage.

#### Process-local background execution

Pipeline jobs exist as Rust threads inside the supervisor. A future worker model will require durable job ownership and recovery.

#### Filesystem JSON as the task database

Tasks and pipeline metadata are ordinary files with no cross-process locking or transactional store.

#### Implicit repository availability

Workers either clone from Git or rely on host bind mounts. A heterogeneous worker system will need an explicit repository/workspace transfer model.

#### Host-specific executable assumptions

The code directly invokes Docker, Git, curl, Python, OpenCode, systemctl, and platform file-open tools.

#### Global mutable pending intent

`pending_intent.json` is a single host-level continuation state, not a per-session or per-job object.

These constraints do not justify distributed execution in Phase 1. They should simply be preserved as architectural seams rather than expanded into more host-specific logic.

## 14. Recommended questions for the external architect

1. Should Smith run under the primary interactive user, a dedicated service user, or both?
2. Should the user service run after login only, or continue at boot with systemd lingering?
3. Should Docker be rootful, rootless, or isolated behind a dedicated service account?
4. Where should Docker’s data root live?
5. Where should native Ollama models live?
6. Can the selected Linux graphics/kernel/userspace stack provide stable AMD inference for the RX 7900 GRE?
7. Is 64 GB RAM sufficient for the intended model and parallel-agent workload, or should memory be expanded?
8. How much CPU and RAM should be reserved for interactive Unity/Cursor/game-engine work?
9. Should model storage, Docker storage, active repositories, and backups use separate filesystems?
10. Which Smith data must be encrypted at rest?
11. What is the backup and restore plan for brain memory, task state, project clones, models, and Docker configuration?
12. How should systemd receive Smith environment variables and credentials?
13. Should sidecar virtual environments/models remain inside the repository or move into application data directories?
14. What audio stack should support Linux microphone capture and playback?
15. Is a graphical session expected to be continuously available, or must Smith normally run fully headless?
16. Which tools may Smith invoke on the host, especially Unity, compilers, embedded SDKs, browsers, and MCP servers?
17. Should SearXNG and other loopback services be protected with local authentication despite loopback binding?
18. Which cloud integrations are intentionally allowed, and how should credentials be isolated?
19. What filesystem should hold high-write Docker/build data while preserving recoverability?
20. How should the machine behave during long autonomous jobs: performance-first, energy-saving, or adaptive?

## What Smith Needs From Its Ideal Phase-1 Linux Host

### REQUIRED

1. **Stable 64-bit Linux x86_64 environment** suitable for long-running user services and Docker workloads.
2. **Working systemd user services**, with a deliberate boot/login strategy and user lingering if headless operation is required.
3. **Direct Docker CLI access** for the Smith execution user without repeated interactive privilege prompts.
4. **Native Ollama installation capable of using the AMD GPU**, because current Smith code does not provide AMD Docker GPU passthrough.
5. **At least 64 GB RAM**, with sufficient swap or equivalent recovery headroom.
6. **Sustained cooling and power delivery** for prolonged CPU compilation, model inference, and Docker activity.
7. **Fast local storage for active repositories, Smith data, Docker activity, and builds.**
8. **Enough total storage for models, Docker layers, game engines, SDKs, dependencies, and generated artifacts.**
9. **Reliable Git, OpenSSH, curl, Python 3, POSIX shell, and compiler toolchains.**
10. **Encrypted backups** covering `~/.config/smith`, `~/.local/share/smith`, repositories, scratch work, and credentials.
11. **Loopback-oriented firewall/network policy** that keeps Smith APIs local while permitting intentional outbound search, Git, registry, and model traffic.
12. **Operation without Flutter, Cursor, or an open terminal**, including supervisor startup and Docker availability.

### STRONGLY PREFERRED

13. Separate or deliberately managed storage for Docker data and model caches so they cannot consume all active-development space.
14. Host-level resource controls capable of reserving CPU/RAM responsiveness for the interactive desktop.
15. Stable absolute paths for the Smith repository and `client/sidecar` directory.
16. Working `host.docker.internal` or equivalent host-gateway behavior on native Linux Docker.
17. PipeWire/PulseAudio-compatible microphone and playback support.
18. Wayland and X11 compatibility for Flutter, browsers, game engines, and development tools.
19. Easy access to systemd journal, Docker logs, Smith pipeline logs, and hardware monitoring.
20. UPS or equivalent protection for long-running autonomous jobs and model downloads.

### OPTIONAL

21. Expanded RAM beyond 64 GB for larger models, more agents, and game-engine workloads.
22. Larger or additional NVMe storage for model and Docker growth.
23. Rootless Docker, provided Smith’s direct CLI assumptions remain compatible.
24. Dedicated backup disk or NAS target.
25. A separate graphical desktop profile for interactive work while Smith continues headlessly.
26. Additional network bandwidth for model/image downloads and repository operations.

## Final assessment

The current hardware target is directionally appropriate for Phase 1. The most important architectural decisions are not distribution-specific; they are native AMD Ollama support, Docker placement/access, systemd user-service longevity, storage allocation, encrypted recovery, and host-level resource governance.
