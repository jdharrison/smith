# smith

smith — open-source control plane for local agent orchestration

<img width="512" src="https://github.com/user-attachments/assets/8483313d-3d82-4c21-8751-ab6e63abd9a5" />

## Status

> **Note:** This project is under active hardening and is not yet stable. Use in production at your own risk.

**Supported target:** one self-hosted Ubuntu LTS x86_64 workstation. Smith's current runtime contains useful local-first functionality, while its target approval, credential, worker-isolation, recovery, service-lifecycle, and release guarantees are documented but not all yet enforced.

Read the canonical project policy before operating or changing Smith:

- [`SCOPE.md`](docs/SCOPE.md) — supported target and non-goals
- [`HOST_PROFILE.md`](docs/HOST_PROFILE.md) — verified workstation baseline and required provisioning
- [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — trust/capability boundaries and recovery model
- [`PROCESS.md`](docs/PROCESS.md) — evidence, triangulation, and approval workflow
- [`DESIGN.md`](docs/DESIGN.md) — operator task, failure, and signoff experience

Existing artifacts for other environments are legacy implementation facts, not target-policy requirements.

## Local text-chat slice

**Implemented in the current source; local candidate, not a production release.** The approved [offline-chat plan](docs/plans/LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0.md) adds an explicit profile:

```text
smith serve --chat-only --model <exact-installed-tag>
```

Replace `<exact-installed-tag>` with the exact local model name returned by Ollama's `/api/tags`; no default model or automatic pull is supplied. After separate launch approval, this binds the bridge to `127.0.0.1:4783` and uses only direct Ollama at `127.0.0.1:11434`, without tools, intent dispatch, tasks, OpenCode, Docker, search, voice sidecars, provisioning, or cloud fallback. One shared default session has bounded in-memory context, cleared on backend restart.

**Implemented Flutter-first guided setup:** the approved [desktop wizard plan](docs/plans/LOCAL-SMITH-DESKTOP-WIZARD-P0.md) extends [boot](docs/plans/LOCAL-SMITH-FLUTTER-BOOT-P0.md): Detect backend → Local runtime → Select/install model → Review & Start → Home / Chat; ordinary Settings remains read-only. The wizard automatically validates only `<actual Halo executable directory>/smith_backend/smith`, never cwd/PATH or a recursive scan. An invalid candidate does not silently fall back; **Advanced · explicit backend path** accepts a reviewed absolute executable. Opening can run read-only `smith setup inspect` for eligible installed-model inventory; it does not start the chat backend, infer, install or download. Inventory `ready` is not verified inference. Choose an observed exact `name:tag`, then explicitly **Start packaged backend**, or attach to a compatible ready external backend untouched.

**Partial empty-host setup:** missing Ollama receives guided instructions to provision/run it separately at `127.0.0.1:11434`; no host installation/start, sudo, legacy install helper, service integration or cloud fallback is supplied. **Review model download** invokes the separate `smith setup pull --model <exact-name:tag>` command. Explicit consent is bound to that exact model and operation through stdin; bounded NDJSON progress/cancellation has finite deadlines and no automatic retries. Ollama may use internet and persistent disk outside Smith; sizes are unknown until reported. Cancel stops the local request/observation, not necessarily daemon work or partial cache. The chat-only bridge still has no provisioning tools/routes. See [Architecture](docs/ARCHITECTURE.md#desktop-setup-command-boundary) for guards and protocol limitations.

Only a captured owned child is stopped on normal close, using health `desktop_session` plus child liveness for correlation, stdin EOF and bounded TERM/KILL cleanup; no crash-proof termination is promised. Readiness recovery preserves Home, draft and transcript without automatic resend/reset. CLI is secondary query/status/configuration with existing commands retained. Future headless operation is a separate deferred deployment/lifecycle capability, not CLI. Voice, Work, uploads, saved history, integrations and streaming remain deferred. The supplied icon is pending because its original asset is not in the workspace.

Ubuntu **24.04 LTS x86_64** Halo candidates bundle `smith_backend/smith` alongside the **full Flutter `lib/` and `data/` payload**; the standalone backend candidate is secondary. Recipients need Ubuntu runtime libraries, a graphical session for Halo, and separately provisioned/running loopback Ollama with an installed or explicitly downloaded model—not Rust/Cargo, Flutter/Dart, or compiler tools. Builders need those toolchains. See [Chat candidate installation and release guide](docs/CHAT_RELEASE.md) for exact prerequisites, install-only boundaries, and provenance limitations.

**Publication remains blocked:** final source/candidate-bound tests/review, actual provider pull/cancel integration and version compatibility, real contextual inference with external networking denied, clean-host desktop installation without build tools, clean-source build binding, license/SBOM review, and replacement of the legacy release workflow are not established by this documentation update. Plain `smith serve`, `smith chat`, and the existing systemd unit remain legacy orchestration paths; no default or service migration is implied. The following orchestration instructions describe those legacy paths.

## Binary installation

**Implemented:** [`scripts/install.sh`](scripts/install.sh) validates Ubuntu LTS and x86_64, refuses effective-root execution, installs the reviewed checkout with locked dependencies into an explicit canonical root owned by the invoking user, rejects unsafe group/world-writable roots and executables, writes the private install-root marker required by unit registration, verifies the executable with `smith --help` and `smith --version`, and stops. It does **not** install/start a unit, sidecar, Docker container, or Flutter client, and it never changes lingering.

```bash
# From a reviewed local checkout; installs $HOME/.local/bin/smith
bash scripts/install.sh --root "$HOME/.local"
```

`SMITH_INSTALL_ROOT` can supply the root non-interactively. The root must be absolute. The script runs exactly `cargo install --locked --path <checkout> --root <install-root>` for the build/install step. Rust stable (minimum 1.83) and Cargo are installation prerequisites; runtime capabilities such as Docker, Git, OpenSSH, Python, and Ollama are provisioned separately under [`docs/HOST_PROFILE.md`](docs/HOST_PROFILE.md).

Pre-built artifacts, when used, require independent release/checksum verification and are not validated by the source installer.

### User-service lifecycle

**Target CLI contract:** after binary installation, each service effect is an explicit command. Confirm the commands exposed by `smith settings service --help`; the dated implementation status and gaps are in [`docs/SMITH_LINUX_ARCHITECTURE_REPORT.md`](docs/SMITH_LINUX_ARCHITECTURE_REPORT.md).

```bash
# Stage the user unit and reload the user manager; do not start or enable it
$HOME/.local/bin/smith settings service install

# Observe unit load, runtime, login enablement, linger, and bridge health separately
$HOME/.local/bin/smith settings service status

# Current runtime only
$HOME/.local/bin/smith settings service start
$HOME/.local/bin/smith settings service restart
$HOME/.local/bin/smith settings service stop

# Login policy only; neither command changes the current runtime
$HOME/.local/bin/smith settings service autostart on
$HOME/.local/bin/smith settings service autostart off

# Explicitly stop, disable, remove, and reload the user-unit registration
$HOME/.local/bin/smith settings service uninstall
```

Installed, running now, enabled at login, and available before login are different claims. Status also reports manager result, restart attempts, and the last main-process exit independently. A permanently missing, non-executable, non-native, or behaviorally invalid host OpenCode configuration is checked before warm resource startup and is not automatically retried. The Ubuntu x86_64 check requires a native executable, a bare semantic version, and OpenCode-specific help markers; this detects accidental/impostor configuration but is not cryptographic package provenance. Operational probe failures such as spawn, wait, timeout, or nonzero exit remain retryable but are limited to five starts per five minutes. Enabling the unit does not start it and does not change lingering. Smith never invokes `loginctl enable-linger` or `disable-linger`; pre-login operation is an optional manual host-policy decision requiring separate review and approval. With `Linger=no` or unknown, report login-scoped behavior only.

The supervisor reconciles persisted queued/running tasks and jobs to interrupted/recovery-required before new bridge readiness in the target architecture. After any restart, inspect status and retained task/job evidence rather than assuming work resumed. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/DESIGN.md`](docs/DESIGN.md).

### Service smoke validation

The repository smoke harness has only a sandbox mode:

```bash
bash scripts/smoke-user-service.sh --sandbox
```

It uses temporary HOME/XDG/config/install roots, activates deny-by-default fake `systemctl`/`loginctl` before installer code runs, invalidates the user-bus address, and suppresses external Docker/loopback status probes. It checks the complete Rust/shell gates, installer preflight failures, help/version behavior, lifecycle separation, restart containment and diagnostics, malformed manager output, persistent versus runtime-only enablement, offline systemd unit parsing before and after sandbox cleanup, no `enable --now`, no linger mutation, no ambient service-state contamination, and no unmanaged `smith serve` fallback. Every invocation retains a fresh checksummed, owner-read-only evidence package under `target/smoke-user-service/runs/`, including raw command results, revision/worktree/source-input identity, the exact rendered unit, and a replayable retained-binary unit fixture. The manifest truthfully labels Cargo network/cache assumptions. Local checksums detect stale mixing and accidental changes, but they are not tamper-proof against the owner; release review must anchor the package digest in an independent review or approval record. If the required separated CLI is not in the tested binary, it exits blocked before invoking lifecycle commands.

Sandbox success does not validate the workstation's real user manager, login/logout behavior, lingering, reboot recovery, or runtime dependencies. Installing to a normal path or running any live-manager smoke requires later explicit human approval bound to the exact account, paths, commands, and effects; see [`docs/PROCESS.md`](docs/PROCESS.md).

## Runtime and network

Smith separates the brain execution location from the brain network setting:

| Axis | Values | Meaning |
|---|---|---|
| **runtime** | `container` \| `host` | Where the brain process runs |
| **network** | `private` \| `public` | Whether a container brain uses internal or ordinary Docker networking |

The current default is `runtime=container`, `network=private`. Use the settings commands to inspect the configured state:

```bash
smith settings runtime get
smith settings network get
```

### What private mode means

- **Container + private:** `smith-brain` uses an internal Docker network, so ordinary brain-container egress is blocked. Search may still use a separate networked worker and return declassified text to the brain. That text remains untrusted.
- **Container + public:** brain egress is intentional.
- **Host runtime:** OpenCode is a native process using an Ollama-compatible endpoint. `network=private` is **not** an operating-system filesystem or network sandbox for host think-mode tools.

Private mode is a useful boundary for the container brain; it is not a complete host air gap or authorization system. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before enabling tools, external endpoints, credentials, or host access.

Coding, pipeline, and search workers still rely on Docker when the brain runs on the host. On the reference Ubuntu workstation, native AMD-capable Ollama must be separately provisioned and verified; it is not currently installed by default.

## Coding agent and sandbox

Smith can create scratch demos through a Docker-based coding worker and `smith-sandbox`. The current implementation can publish sandbox and worker ports; do **not** treat those ports as loopback-only unless you have inspected the effective Docker publication. The target policy requires loopback-only defaults and explicit approval for any broader exposure.

```bash
smith settings sandbox status
smith settings sandbox stop
```

The brain emits a structured `:::smith code` intent; the supervisor decides whether and how to act on it. An intent is untrusted data, not permission to expose a service, access additional host paths, or perform external changes. Coding and pipeline workers remain Docker-based even when the brain runtime is host.

## Local search (SearXNG)

Smith can use local SearXNG at [http://127.0.0.1:4792](http://127.0.0.1:4792), with host-side DuckDuckGo, weather, and other fallbacks in the current implementation. SearXNG is loopback-bound; external search destinations necessarily receive the submitted query.

Search output may contain hostile instructions, inaccurate claims, or sensitive material. Smith and its operators must treat returned text as untrusted reference material—not authority to change scope, permissions, approval state, or host configuration. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Desktop client (prototype)

The primary Flutter entry point in [`client/`](client/) uses Boot/Setup and the chat-only slice described above; see [client instructions](client/README.md). The following plain-`serve` example is retained solely as a **legacy orchestration** example, not a compatible launch path for Halo:

```bash
# Start the local bridge after required services are provisioned
smith serve

# In a separate terminal, run the Linux client
cd client && flutter run -d linux
```

The bridge listens on `127.0.0.1:4783`; optional Whisper and Kokoro sidecars use loopback ports `4790` and `4791`. Their virtual environments, models, audio integration, and service lifecycle require Ubuntu-host validation before routine use. A separately installed legacy `smith.service` has been observed failing in a restart loop on the reference workstation; the hardened source unit has not been deployed, and user lingering remains disabled.

In the legacy bridge, every message creates a task record; chat-only messages do not. An acknowledgement means work was accepted, **not** that it completed. The operator must be able to inspect task state, retained evidence, failures, and pending approval effects as defined in [`docs/DESIGN.md`](docs/DESIGN.md).
## Quick Start

### Before first operation

1. Review [`docs/HOST_PROFILE.md`](docs/HOST_PROFILE.md) and provision the required Ubuntu host capabilities.
2. Confirm the current runtime configuration with `smith status --verbose` and the runtime/network settings commands.
3. Treat `smith install`, agent startup, and service configuration as local host changes. Run them only after reviewing their expected effects.
4. Keep external credentials out of shell history, agent prompts, repository files, and generated artifacts.

### Build and run from source

```bash
cargo build --locked
cargo run -- help
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

### Local brain and bridge

The container brain is tool-constrained by default and can use an internal Docker network. It is not a complete host-security boundary. The host runtime runs native OpenCode, so it must be treated as a privileged local capability and used only with explicitly approved tools and roots.

```bash
# Interactive chat; /new resets the current session
smith chat

# One-shot chat
smith chat -m "Summarize my profile preferences"

# Start the local bridge; --echo is useful for protocol smoke checks
smith serve --echo
smith serve

# Inspect local state
smith settings brain status
smith settings sandbox status
```

Smith retains brain data under its local configuration/data directories, including conversation and pending-intent state. That state can be sensitive. Do not treat a pending intent as approval, and do not treat container-local pipeline state as durable audit evidence.

### Local model on Ubuntu

The reference host has an AMD Navi 31 GPU but does not yet have Ollama or ROCm tooling on `PATH`. Install and validate a native Ollama path before depending on GPU inference. Smith's Docker GPU path is designed around NVIDIA `--gpus`; do not assume it provides AMD acceleration. CPU fallback may be possible but requires explicit capacity and responsiveness acceptance.

### Legacy local bridge and desktop boundary

`smith serve` listens on `127.0.0.1:4783`. Whisper and Kokoro sidecars use loopback ports when started. The bridge is local-only but has no separate authentication layer, so local processes are within its trust boundary. Flutter is the primary Ubuntu desktop interface; the retained CLI remains secondary query/status/configuration and preserves task/evidence/approval meaning. This legacy bridge is not the desktop chat-only lifecycle; future headless deployment is separate and deferred.

## Delivery pipeline

Smith has `plan`, `develop`, `review`, and `release` stages that use a project/branch-scoped Docker worker. The durable process is defined in [`docs/PROCESS.md`](docs/PROCESS.md): independent producer, architect, and designer analysis; planner reconciliation; scoped local implementation; self-check; fresh-context assurance/adversarial review; then explicit human approval for every external effect.

```bash
# Local planning with an approved local work record
smith run plan "Add login endpoint" --project myproject \
  --branch feature/add-login --base main

# Local implementation and evidence production
smith run develop "Add login endpoint" --project myproject \
  --branch feature/add-login --plan plan-ab12

# Inspect/review the plan and evidence artifacts
smith run review --project myproject \
  --branch feature/add-login --plan plan-ab12
```

`develop`, `--pr`, and `release` can cause Git or GitHub effects in the current implementation. Do **not** invoke them against a real remote unless the active approval scope in [`docs/PROCESS.md`](docs/PROCESS.md) permits the exact action. In particular, branch push/draft PR, merge, release, and deployment require separate approval scopes.

Pipeline state beneath `/state/plan-*` belongs to the worker container and is not durable audit storage. Preserve the required evidence package before removing a worker. The current SSH and worker-mount handling is being hardened; do not rely on broad host SSH-directory mounts or implicit credential forwarding as a safe default.

### Worker images

Smith selects a project-oriented base image from detected manifests or explicit project configuration. A worker image must contain the expected project tooling and a working OpenCode runtime. Treat image selection, network access, mounts, and credentials as an elevated configuration change and review them under [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## Commands and options

### System commands

- **`smith status [--verbose]`**  
  Show status of dependencies, models, agents, and projects.

- **`smith install`**  
  Interactive setup: check/install Docker (Linux: get.docker.com), optionally enable Docker at boot, create config dir, add models and projects.

### Pipeline commands — `smith run <cmd>`

- **`smith run plan "<prompt>"`** — creates planning artifacts in a scoped worker.
- **`smith run develop "<task>" --plan <plan-id>`** — implements an approved plan and produces self-check/assurance inputs.
- **`smith run review`** — inspects plan artifacts and review state.
- **`smith run release --plan <plan-id>`** — integration/release stage; may have external Git effects and is never an implicit local-only command.

Use `--project`, `--branch`, and `--base` to bind a command to its approved work record and revision. Jira ticket keys are supported when the optional Jira integration is enabled. The `--pr` option creates or updates a pull request and requires `REVIEW_REQUEST_APPROVED`; it is not a routine validation flag.

### Model profiles and credentials

Current model-profile environment mappings can pass host values into worker containers. This is a sensitive capability: container environment values may be observable to processes with Docker inspection access, and the current implementation does not yet provide the target secret-store isolation.

Do not place literal tokens in shell history, repository files, agent prompts, plan artifacts, PRs, work-tracking comments, or logs. Do not grant a worker credentials, MCP endpoints, or external network access unless the approved work record, architecture review, and approval scope require them. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`SECURITY.md`](SECURITY.md).

### Project commands — `smith settings project <cmd>`

Projects are Git repositories (remote URLs or local paths). Named agents can receive configured project/workspace mounts; spawned pipeline workers clone their primary project into `/workspace` and may receive additional configured mounts.

**Important:** configured project mounts are currently read/write. Add only approved paths and repositories, use read-only workspace access wherever possible, and do not assume a worker is safe to give broad host access. The default Smith project uses `jdharrison/smith` on `main`; `feature/autonomy` is the documented one-time self-hosting bootstrap exception.

- **`smith settings project add <name> --repo <path-or-url>`**  
  Register a project.  
  - `--image <image>` — Docker image for this project.  
  - `--ssh-key <path>` — SSH key path for this project.  
  - `--base-branch <branch>` — Base branch for clone/compare (default: `main`).  
  - `--remote <name>` — Remote name for fetch/push (default: `origin`).  
  - `--github-token <token>` — current per-repository PR credential input. Do not provide literal secrets to an agent or commit them; prefer an approved secret-reference process while the target secret-store design is implemented.

- **`smith settings project list`**  
  List registered projects (shows repo, image, ssh_key, base_branch, remote, and whether github-token is set).

- **`smith settings project status [--project <name>] [--verbose]`**  
  Validate project configuration and connectivity.

- **`smith settings project update <name>`**  
  Update a project.  
  - `--repo <url>`, `--image <image>`, `--ssh-key <path>`, `--base-branch <branch>`, `--remote <name>`, `--github-token <token>` — Set new value; pass `""` to clear optional fields.

- **`smith settings project remove <name>`**  
  Remove a project.

### Workspace commands — `smith settings workspace <cmd>`

Workspaces are existing host directories bind-mounted into agent containers at `/workspaces/<name>`; unlike projects, Smith does not clone them.

- **`read`** creates a read-only mount.
- **`readwrite`** creates a read/write mount and is the current default.
- Docker does not provide a write-only mount; `write` is therefore read/write.

Treat every workspace mount as an elevated capability. Add only narrowly scoped approved paths, prefer `read`, and explicitly select project/workspace subsets for each agent rather than attaching everything by default.

### Agent commands — `smith settings agent <cmd>`

Agents are identified by **name** (id). Each agent has:
- **name** — Unique id/name.
- **image** — Docker image (default: `ghcr.io/anomalyco/opencode`). Use a custom image for advanced customization.
- **model** — Model to use (e.g. `anthropic/claude-sonnet-4-5`, `openrouter/google/gemini-2.0-flash`). Baked into container.
- **small_model** — Smaller model for internal operations (reduces API costs, e.g. `openrouter/google/gemini-2.0-flash:free`).
- **provider** — Provider name (e.g. `anthropic`, `openai`, `openrouter`). Baked into container config.
- **base_url** — Custom base URL for provider (for proxies/custom endpoints).

**Credentials:** current agent configuration can forward provider environment variables into a container. Treat this as a privileged exception, not a default. Provider/model choice must remain replaceable and may never expand a worker's filesystem, network, tool, or publication authority.

- **`smith settings agent add <name>`**  
  Register an agent.  
  - `--image <image>` — Docker image (default: `ghcr.io/anomalyco/opencode`).  
  - `--model <model>` — Model to use (e.g. `anthropic/claude-sonnet-4-5`).  
  - `--small-model <model>` — Small model for internal ops.  
  - `--provider <name>` — Provider name (`anthropic`, `openai`, `openrouter`, etc.).  
  - `--base-url <url>` — Custom base URL for provider.  
  - `--project <name>` / `--workspace <name>` — Repeatable; omit to attach all configured.

- **`smith settings agent list`**  
  Show named agents and spawned project/branch agents.

- **`smith settings agent remove <name>`**  
  Remove a named agent from config (does not stop a running container).

- **`smith settings agent start <name>`**  
  Start a named agent (`smith-agent-<name>`). Injects configured projects (`/projects/<name>`) and workspaces (`/workspaces/<name>`).

- **`smith settings agent start --project <name> --branch <branch>`**  
  Start a spawned project/branch agent (`agent_<project>_<branch>`). Primary repo clones to `/workspace`; extra projects/workspaces are also mounted.

- **`smith settings agent stop <name>`** / **`--all`** / **`--all-named`**  
  Stop named or spawned agent containers.

- **`smith settings agent logs --project <name> --branch <branch>`**  
  Stream logs from a spawned agent.
---

## Optional Jira, GitHub, and pull-request integration

Jira/GitHub synchronization is available for work that elects to use it; it is not required for local planning, development, validation, or review. A local approved task/plan record is sufficient when work tracking is not enabled.

When Jira/GitHub integration is enabled, Jira owns intent, acceptance criteria, priority, and lifecycle; GitHub owns branches, commits, checks, review, merges, tags, and releases. A ticket-key branch and draft PR should link the work item and use [the repository template](.github/pull_request_template.md).

Whether work tracking is enabled or not, the PR/evidence package must state the objective, acceptance criteria, validation evidence, and risks. An agent must not push, create/update a PR, transition Jira, merge, tag, release, or deploy without the exact approval scope for that external effect.

## Development

Run the established Rust checks from the repository root:

```bash
cargo fmt --check
cargo build --locked --verbose
cargo test --locked --verbose
cargo clippy --all-targets -- -D warnings
```

When changing the Flutter client or Python sidecars, add the applicable Ubuntu validation described in [`docs/PROCESS.md`](docs/PROCESS.md). Do not report an unrun check as passed.
