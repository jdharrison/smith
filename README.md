# smith

smith — open-source control plane for local agent orchestration

<img width="512" src="https://github.com/user-attachments/assets/8483313d-3d82-4c21-8751-ab6e63abd9a5" />

## Status

> **Note:** This project is complete but not yet stable. Use in production at your own risk.

**Supported platforms:** Linux and macOS. On Windows, use [WSL](https://docs.microsoft.com/en-us/windows/wsl/) to run smith.

## Installation

### Option 1: Install from Cargo (Recommended)

Install directly from the GitHub repository using Cargo:

```bash
# Install Rust & Cargo
curl --proto '=https' --tlsv1.2 -sfSf https://sh.rustup.rs | sh

# Install latest release
cargo install --git https://github.com/jdharrison/smith.git
```

**Prerequisites:** Rust 1.83+ (stable recommended). Install via [rustup](https://rustup.rs/) so the project's `rust-toolchain.toml` is used.

### Option 2: Download Pre-built Binaries

Pre-built binaries for Linux and macOS are available in [GitHub Releases](https://github.com/jdharrison/smith/releases). (Windows is not supported; use Linux binary with WSL.)

## Runtime and network

Two orthogonal settings (replacing the old `isolation` knob):

| Axis | Values | Meaning |
|------|--------|---------|
| **runtime** | `host` \| `container` | Where the brain process runs |
| **network** | `public` \| `private` | Whether the brain has general internet |

Defaults: `runtime=container`, `network=private` (former **sealed**). Legacy `isolation=host` migrates to `runtime=host` + `network=public`.

| Combo | Chat / think | Internet |
|-------|--------------|----------|
| container + private | Docker `smith-brain` on internal net | Brain offline; live search via local **SearXNG** (`127.0.0.1:4792`) then declassified snippets |
| container + public | Docker `smith-brain` with egress | Brain may reach the net |
| host + public/private | Native OpenCode + host Ollama | Private still uses SearXNG / workers for net tasks (honest policy) |

```bash
smith settings runtime get|set host|container
smith settings network get|set public|private
# deprecated alias:
smith settings isolation set sealed   # → container + private
smith settings isolation set host     # → host + public
```

Host runtime needs **native Ollama** (`127.0.0.1:11434`) and **`opencode` on PATH**. Coding/pipeline/search still need Docker. First host capability: find a file under workspaces / Pictures / Documents (or `host.roots` in config) and preview it (OS viewer + Flutter attachment).

Container runtime needs Docker. Host Ollama via `host.docker.internal` with `network=private` uses the internal net + host-gateway (Ollama only). With `network=public`, soft-seal egress is intentional — set `network=private` if you want blocked egress.

## Coding agent + ad-hoc host

Chat can ask smith to **build and host** small demos. A local `coding` agent is ensured on install/chat; demos run in `smith-sandbox` with a published port (stays up until stopped).

```bash
smith chat -m "create a web server and host an index file with just hello world on port 8080"
# → http://127.0.0.1:8080  (scratch under smith data dir)

smith settings sandbox status
smith settings sandbox stop
```

Brain emits a `:::smith code` intent; smith writes under the `scratch` workspace and serves via the sandbox container. Coding/pipeline stay in Docker even when brain runtime is host.

## Local search (SearXNG)

Web search for chat goes through a **local SearXNG** container (`smith-searxng`), not Google.

- **UI + API:** [http://127.0.0.1:4792](http://127.0.0.1:4792) (loopback only — use this instead of browser Google when you want private metasearch)
- **License:** SearXNG is AGPL-3.0 (the image); smith only starts/stops it
- **Config:** `~/.config/smith/searxng/settings.yml` (JSON format enabled for smith)
- Starts on `smith install` and on first `:::smith search` / live fact turn
- Skip with `SMITH_SEARX=0` (falls back to thin DDG Instant Answer)
- Override URL with `SMITH_SEARX_URL`

Weather still uses wttr.in / Open-Meteo as small public helpers alongside SearXNG results.

## Desktop client (prototype)

Flutter desktop UI lives in [`client/`](client/) (Windows/macOS/Linux). Text/voice chat against `smith serve`. Host runtime can show image attachments and Open.

**Always-on supervisor:** `smith install` registers OS login autostart for `smith serve` (`smith settings service autostart on|off`). Warm services (default): bridge, brain, `services.searxng`, `services.ollama`. On-demand: Whisper, TTS, coding, sandbox. Compose project stays **`smith`** with hierarchical labels (`services-searxng`, `agents-coding`, `sandbox-default`). SearXNG UI: [http://127.0.0.1:4792](http://127.0.0.1:4792).

**Async task board:** every chat message creates a durable host task. Fast replies complete in the same turn; search/files/code/pipeline ack immediately and finish in the background. The client polls `GET /v1/tasks` while work is running and **injects an assistant message** into Chat when a task completes (TTS may speak the inject). Tabs: **Chat | Tasks | Settings** (no Chat|Dev session mode — pipeline kind comes from intent).

```bash
# After smith install (Whisper + Kokoro TTS venvs/models; re-runs are idempotent)
smith serve                 # bridge :4783 (+ warm set); or rely on login autostart

# UI
cd client && flutter run -d windows   # or macos / linux
```

`smith install` sets up Whisper (`.venv`) and Kokoro TTS (`.venv-tts` + ONNX under
`client/sidecar/models/`). Kokoro weights are Apache-2.0; `kokoro-onnx` is MIT.
`smith serve` starts Whisper and TTS when not already healthy (both normal and `--echo`).
Skip Whisper with `SMITH_WHISPER=0`, TTS with `SMITH_TTS=0`. Override folders with
`SMITH_WHISPER_DIR` / `SMITH_TTS_DIR`. Default Kokoro voice: `am_onyx` (`SMITH_TTS_VOICE`).
Ctrl-C stops sidecars smith started.

Windows: allow microphone access for the app under **Settings → Privacy → Microphone**.
Host file preview reads local paths on this machine. Voice uses hold-to-talk → WAV →
local Whisper (`127.0.0.1:4790`); replies speak via Kokoro (`127.0.0.1:4791`) when up,
else OS SAPI / flutter_tts. Voice/TTS controls live under **Settings**. If Kokoro needs
phonemes, install **espeak-ng**.

Bridge extras: `GET /v1/tasks?active=1&since=<unix>`, `GET /v1/tasks/:id`; `POST /v1/chat`
returns a `task` snapshot (ack-first for async work).
## Quick Start

### Prerequisites

- Rust 1.83+
- **Docker** — container runtime used by smith.

### Build and Run

```bash
# After installing the binary
smith install   # bootstrap Docker, local model, sealed brain + validate
smith status    # monitor smith services/systems
smith settings brain status
smith help

# Development
cargo build
cargo run -- help
```

`smith install` now also:
- creates durable brain memory (`profile.md`, episodic/working/journal)
- ensures internal network `smith-brain-net`
- offers a default local Ollama profile (`qwen3:14b` — balanced for ~16GB VRAM with desktop headroom)
- optionally starts Ollama + sealed brain and checks egress is blocked
- can register **projects** (git clone or local path) and **workspaces** (existing host directories) to inject into agent containers

### Sealed local brain (chat / think)

Private, no-egress conversation and solve loop against a local Ollama model. Existing `smith run` / `smith settings agent` pipelines are unchanged.

**Prerequisites:** Docker (for sealed brain / agents). Local model via the `coding` agent (`--type local`) then `smith settings agent start coding`.

**GPU / Ollama backends:**
- **NVIDIA + Docker:** `smith settings model start` may run `smith-ollama` with `--gpus` when the NVIDIA Container Toolkit is present.
- **AMD on Windows (RX 7900 etc.):** Docker Desktop `--gpus` is NVIDIA-only; ROCm passthrough inside Desktop is not a reliable MVP path. Install [native Ollama for Windows](https://ollama.com/download), then `smith settings model start` prefers **host Ollama** and points the sealed brain at `host.docker.internal:11434` (soft-seal: no cloud keys, but egress is not blocked like the internal-net Docker path).
- **Fallback:** Docker Ollama in **CPU** mode (slow for 14B — avoid unless necessary).

```bash
# Interactive sealed chat (text REPL; /new resets the session)
# Markdown replies are rendered in the terminal (set NO_COLOR=1 for raw)
smith chat

# One-shot (same ongoing session as REPL)
smith chat -m "Summarize my profile preferences"

# Fresh session, then one-shot
smith chat --new -m "Start over"

# Iterate/kill solve loop (writes durable analysis.json)
smith think "Draft a private research outline on X" --reset

# Local chat bridge for native clients (Flutter later). Loopback only.
# Also starts Whisper STT on :4790 and Kokoro TTS on :4791 when not already up.
smith serve --echo          # no brain — protocol smoke test
smith serve                 # same session as `smith chat`
# curl -s http://127.0.0.1:4783/v1/health
# curl -s http://127.0.0.1:4783/v1/chat -H "Content-Type: application/json" \
#   -d '{"v":1,"text":"hello","session":"default"}'

# Ops
smith settings brain status
smith settings brain stop
# or: smith settings model start / stop
```

**Network private + Docker Ollama:** brain + Ollama share Docker network `smith-brain-net` (`--internal`, no external egress). The brain container has no published ports, no SSH mount, and no cloud API keys.

**Network public (container):** brain attaches to a non-internal bridge so it can egress. Still no published ports / no cloud API keys.

**Network private + host Ollama:** brain uses internal net + `host.docker.internal` for Ollama only. If egress is unexpectedly open, status reports soft-seal honestly — prefer Docker Ollama for strict private.

**Live web search (ephemeral):** under `network=private` the brain stays offline; smith opens a short-lived `smith-search-*` container on the default bridge (internet), fetches results (DuckDuckGo + wttr.in for weather), then `--rm`. Results are declassified back into the chat turn. Oneshots also work under `public`.

**Host memory** (survives container stop/recreate), under the smith data dir `…/smith/brain/`:

| Path | Purpose |
|------|---------|
| `profile.md` | Stable preferences / constraints |
| `episodic/session.jsonl` | Ongoing chat transcript (one JSON turn per line) |
| `episodic/` | Dated session notes (e.g. archived on `/new`) |
| `working/session.md` | Bounded recent chat window |
| `working/chat_session.json` | OpenCode session id for `--session` continue |
| `working/analysis.json` | Think-loop notebook |
| `working/pending_intent.json` | Host-routable intent (e.g. search ask awaiting confirmation) |
| `.opencode-share/` | OpenCode session DB (bind-mounted into smith-brain) |
| `journal/` | Per-step attempt logs |

Edit `profile.md` freely; it stays on-host and is injected into chat context only inside the sealed bubble. `smith chat` / `smith chat -m` share one continuous session. Named workers (`smith-agent-*`) and spawn agents do **not** receive brain chat history.

**Live web search (ephemeral):** under `network=private` the brain stays offline; smith opens a short-lived `smith-search-*` oneshot on the bridge, then declassifies results into the turn. Same oneshot path works under `public`.

**Native client bridge:** `smith serve` listens on `127.0.0.1:4783` (not Ollama/OpenCode ports) and starts Whisper STT (`:4790`) and Kokoro TTS (`:4791`) when needed. The host process owns the turn (intents, search, session, pipelines); the client owns UI/STT/TTS. `POST /v1/chat` with `{"v":1,"text":"…","session":"default","new":false,"mode":"chat"|"dev"}` returns text plus optional `pipeline` status. Flutter Chat|Dev toggle sets `mode`. `--echo` skips the brain so clients can integrate against the protocol first. Not a public API — loopback only.

### Pipeline commands (run plan / develop / review / release)

Run pipeline workflows via `smith run <cmd>`. Chat can also start them with a
`:::smith pipeline` intent (see Desktop client / `smith serve`); push/release
waits for explicit user signoff.

```bash
# Plan a feature (creates plan-* artifacts in the spawned agent)
smith run plan "Add login endpoint" --project myproject --branch feature/login

# Execute a plan (validate / assurance loop)
smith run develop "Add login endpoint" --project myproject --branch feature/login --plan plan-ab12

# Review plan artifacts
smith run review --project myproject --branch feature/login --plan plan-ab12

# Integrate / sync after develop (CLI); from chat this runs only after you affirm signoff
smith run release --project myproject --branch feature/login --plan plan-ab12
smith run release --project myproject --branch feature/login --plan plan-ab12 --pr
```

### Spawn pipeline lifecycle (v0.4.0)

The plan/develop/release pipeline uses project+branch scoped spawned containers (`agent_<project>_<branch>`).

- `smith run plan` auto-starts the spawned container if needed and leaves it running.
- `smith run review` fails fast when the spawned container is not running.
- `smith run develop` fails fast when the spawned container is not running (expects plan-initialized workspace/state).
- `smith run release` requires the container to be running and only stops it after full successful release.
- Use `smith run release --keep-agent` to keep the container running after success for debugging.
- If release is blocked or fails, the container remains running for diagnosis.

Use **SSH repository URLs** (e.g. `git@github.com:user/repo.git`). The pipeline mounts your host `~/.ssh` and forwards `SSH_AUTH_SOCK` when set, so host auth (e.g. `ssh-add`) works. Use `--ssh-key <path>` to supply a specific key. Projects can store an image and SSH key via `smith settings project add/update`.

### How does the pipeline choose the container image?

The **base image targets the project’s environment**, not OpenCode. When you don’t pass `--image` (and the project has no image in config), Smith clones the repo, detects the project type from manifest files (`Cargo.toml` → Rust, `package.json` → Node, `go.mod` → Go, `requirements.txt`/`pyproject.toml` → Python), and picks an env-focused image:

- **Rust** → `rust:1-bookworm` (Rust + cargo; respects `rust-toolchain.toml` e.g. nightly)
- **Node** → `node:22-bookworm-slim`
- **Go** → `golang:1-bookworm`
- **Python** → `python:3.12-bookworm-slim`
- **Unknown** → `debian:bookworm-slim`

Only **one runtime** is installed (the one for that project type); the image usually already provides it. **OpenCode** is installed by trying the [official install script](https://opencode.ai/docs) first, then `npm install -g opencode-ai` if the script fails (e.g. on minimal images), so both glibc-based and Node-based images work. Override with `--image` or project config when you need a specific image.

---

## Agent pipeline and feedback loops

Plan / develop / release share spawned workspace state under `/state/plan-*`.

### High-level ship flow (from chat or CLI)

1. **Plan** — Producer/architect/designer/planner artifacts for the request.
2. **Develop** — Implement against the plan with validate / assurance loops.
3. **Signoff (chat)** — Human affirms before release; CLI `release` can be run directly.
4. **Release** — Role reviews / integrate / sync (optional `--pr`).

All commands **fetch from the configured remote** and **reset/checkout to the latest remote ref** for the base branch when applicable. Base branch and remote name are configurable per project (see Project commands) or via `--base`.

### Summary

| Command | Role |
|--------|------|
| **plan** | Create plan artifacts for a prompt |
| **develop** | Implement + validate a plan |
| **review** | Inspect plan artifacts / issues |
| **release** | Integrate after develop (push/sync; optional PR) |

---

## Commands and options

### System commands

- **`smith status [--verbose]`**  
  Show status of dependencies, models, agents, and projects.

- **`smith install`**  
  Interactive setup: check/install Docker (Linux: get.docker.com), optionally enable Docker at boot, create config dir, add models and projects.

### Pipeline commands — `smith run <cmd>`

- **`smith run plan "<prompt>"`**  
  Run planner roles and write `plan-*` artifacts.  
  - `--project <name>` / `--branch <branch>` — Target spawned workspace.  
  - `--verbose` — Show detailed agent output.

- **`smith run develop "<task>" --plan <plan-id>`**  
  Execute a plan-driven development loop.  
  - `--project` / `--branch` / `--base` — Workspace targeting.  
  - `--plan` — Full id (`plan-xxxx`) or short id.  
  - `--max-validate-passes` — Default 3.  
  - `--pr` — Create/update PR after success (post-step).  
  - `--verbose` — Detailed output.

- **`smith run review`**  
  Review plan artifacts in the spawned container.  
  - `--project` / `--branch` / `--plan` / `--limit` / `--state` / `--reply`.

- **`smith run release --plan <plan-id>`**  
  Release / integrate a completed plan.  
  - `--project` / `--branch` / `--base` / `--pr` / `--keep-agent` / `--verbose`.

### Model profile env passthrough

Use model profile env mappings to inject host environment variables into spawned project agent containers.

- Add/update mappings with repeatable `--env` flags:
  ```bash
  smith settings model add opencode --provider openai --model gpt-5 --env 'OPENAI_API_KEY=$OPENAI_API_KEY'
  smith settings model update opencode --env 'OPENAI_API_KEY=$OPENAI_API_KEY' --env 'OPENROUTER_API_KEY=$OPENROUTER_API_KEY'
  ```
- Clear mappings on update by passing an empty env value:
  ```bash
  smith settings model update opencode --env ''
  ```
- Always quote the `--env` value so your shell does not expand `$HOST_ENV` before smith parses it.
- If a mapped host env is missing or empty at runtime, `smith settings agent start` fails fast with a clear error.
- Security note: injected values are passed to Docker as container environment variables; treat host access and Docker inspect permissions accordingly.

### Project commands — `smith settings project <cmd>`

Projects are git repos (HTTPS/SSH URL) or **local paths**. On `smith settings agent start <name>` they are cloned/fetched into smith’s project cache (when remote) and bind-mounted at `/projects/<name>` (readwrite). Spawned pipeline agents still clone the primary project to `/workspace`; extra projects appear under `/projects/<name>`.

**Default project:** install (and chat/serve ensure) registers **`smith`** → [https://github.com/jdharrison/smith.git](https://github.com/jdharrison/smith.git) on **`main`** (HTTPS, no SSH key required). That is the bootstrap for smith iterating on smith. Override or remove with the project commands below; SSH remotes remain preferred for private repos.

- **`smith settings project add <name> --repo <path-or-url>`**  
  Register a project.  
  - `--image <image>` — Docker image for this project.  
  - `--ssh-key <path>` — SSH key path for this project.  
  - `--base-branch <branch>` — Base branch for clone/compare (default: `main`).  
  - `--remote <name>` — Remote name for fetch/push (default: `origin`).  
  - `--github-token <token>` — GitHub personal access token for PR creation (per-repository).

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

Workspaces are **existing host directories** (a personal repo, a `projects/` folder, etc.) bind-mounted into agent containers at `/workspaces/<name>`. Unlike projects, smith does not clone them.

Access (Docker bind mode):
- **`readwrite`** (default) — `:rw`
- **`read`** — `:ro`
- **`write`** — treated as readwrite (Docker has no write-only mounts)

```bash
smith settings workspace add code --path ~/Develop/personal/projects
smith settings workspace add notes --path ~/notes --access read
smith settings workspace list
smith settings workspace status
```

Optional per-agent subset (omit to attach **all** configured projects + workspaces):

```bash
smith settings agent add coding --type local --model qwen3:14b \
  --project smith --workspace code
```

### Agent commands — `smith settings agent <cmd>`

Agents are identified by **name** (id). Each agent has:
- **name** — Unique id/name.
- **image** — Docker image (default: `ghcr.io/anomalyco/opencode`). Use a custom image for advanced customization.
- **model** — Model to use (e.g. `anthropic/claude-sonnet-4-5`, `openrouter/google/gemini-2.0-flash`). Baked into container.
- **small_model** — Smaller model for internal operations (reduces API costs, e.g. `openrouter/google/gemini-2.0-flash:free`).
- **provider** — Provider name (e.g. `anthropic`, `openai`, `openrouter`). Baked into container config.
- **base_url** — Custom base URL for provider (for proxies/custom endpoints).

**API Keys**: Set the corresponding environment variable on your host before starting agents. The agent will automatically forward it to the container:
- `anthropic` → `ANTHROPIC_API_KEY`
- `openai` → `OPENAI_API_KEY`
- `openrouter` → `OPENROUTER_API_KEY`

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

## GitHub Pull Requests

1. Add or update a project with a GitHub token (one per repository):
   ```bash
   smith settings project add myproject --repo git@github.com:user/repo.git --github-token <your-github-token>
   # or
   smith settings project update myproject --github-token <your-github-token>
   ```
2. Use `--pr` with `run dev` and that project:
   ```bash
   smith run dev "Add new feature" --branch feature/new-feature --project myproject --pr
   ```
   This creates a PR if none exists for the branch, or updates the existing one. The task is used as the PR title. Base branch comes from project config or `--base` (default `main`).

## Development

```bash
cargo test
cargo fmt
cargo clippy --all-targets -- -D warnings
```
