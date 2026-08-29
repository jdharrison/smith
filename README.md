# smith

smith — open-source control plane for local agent orchestration

<img width="512" src="https://github.com/user-attachments/assets/8483313d-3d82-4c21-8751-ab6e63abd9a5" />

## Status

> **Note:** This project is complete but not yet stable. Use in production at your own risk.

**Supported target:** one self-hosted Ubuntu/Linux workstation. Smith is still being hardened: the current runtime contains useful local-first functionality, while its target approval, credential, worker-isolation, recovery, and release guarantees are documented but not all yet enforced.

Read the canonical project policy before operating or changing Smith:

- [`SCOPE.md`](docs/SCOPE.md) — supported target and non-goals
- [`HOST_PROFILE.md`](docs/HOST_PROFILE.md) — verified workstation baseline and required provisioning
- [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — trust/capability boundaries and recovery model
- [`PROCESS.md`](docs/PROCESS.md) — evidence, triangulation, and approval workflow
- [`DESIGN.md`](docs/DESIGN.md) — operator task, failure, and signoff experience

Existing artifacts for other environments are legacy implementation facts, not target-policy requirements.

## Installation

### Option 1: Install from Cargo (Recommended)

Build the checked-out source with Cargo:

```bash
# Install Rust and Cargo if they are not already present.
curl --proto '=https' --tlsv1.2 -sfSf https://sh.rustup.rs | sh

# From a reviewed local checkout
cargo install --path .
```

**Prerequisites:** the supported host needs Rust stable (minimum 1.83), Docker, Git, OpenSSH, Python, and an intentionally provisioned local Ollama-compatible endpoint. Review [`docs/HOST_PROFILE.md`](docs/HOST_PROFILE.md) before installation. `smith install` may create local configuration, start services, and offer host setup; inspect its effects before running it.

### Option 2: Download Pre-built Binaries

Pre-built Linux binaries are available in [GitHub Releases](https://github.com/jdharrison/smith/releases). Verify the release notes and the supported Ubuntu host requirements before installation.

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

The optional Flutter client in [`client/`](client/) talks to `smith serve` over the local bridge. The supported target is Ubuntu Linux:

```bash
# Start the local bridge after required services are provisioned
smith serve

# In a separate terminal, run the Linux client
cd client && flutter run -d linux
```

The bridge listens on `127.0.0.1:4783`; optional Whisper and Kokoro sidecars use loopback ports `4790` and `4791`. Their virtual environments, models, audio integration, and service lifecycle require Ubuntu-host validation before routine use. `smith.service` and user lingering are not currently provisioned on the reference workstation.

Every message creates a task record. An acknowledgement means work was accepted, **not** that it completed. The operator must be able to inspect task state, retained evidence, failures, and pending approval effects as defined in [`docs/DESIGN.md`](docs/DESIGN.md).
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

### Local bridge and optional client

`smith serve` listens on `127.0.0.1:4783`. Whisper and Kokoro sidecars use loopback ports when started. The bridge is local-only but has no separate authentication layer, so local processes are within its trust boundary. The optional client is for Ubuntu Linux and must not be required for task visibility, evidence review, or approvals.

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
