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

## Quick Start

### Prerequisites

- Rust 1.83+
- **Docker** — container runtime used by smith.

### Build and Run

```bash
# After installing the binary
smith install   # one-time setup wizard
smith status    # monitor smith services/systems
smith help

# Development
cargo build
cargo run -- help
```

### Pipeline commands (run ask / dev / review)

Run pipeline workflows via `smith run <cmd>`.

```bash
# Ask a question (read-only); default branch is main, use --base to ask about another branch
smith run ask "How does auth work?" --project myproject
smith run ask "What's in this branch?" --project myproject --base feature/x

# Run a development task (commit + push); use --pr to create or update a PR
smith run dev "Add login endpoint" --branch feature/login --project myproject
smith run dev "Fix bug" --branch fix/123 --project myproject --pr --verbose

# Review a branch
smith run review feature/login --project myproject
smith run review feature/login --project myproject --base main --verbose
```

Use **SSH repository URLs** (e.g. `git@github.com:user/repo.git`). The pipeline mounts your host `~/.ssh` and forwards `SSH_AUTH_SOCK` when set, so host auth (e.g. `ssh-add`) works. Use `--ssh-key <path>` to supply a specific key. Projects can store an image and SSH key via `smith project add/update`.

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

All pipeline commands (ask, dev, review) use the same high-level flow.

### Phases

1. **Setup**
   - **Setup Run** — Clone repo, install deps (cargo check / npm install / go mod / pip), bootstrap opencode-ai. If install fails, the run fails with a clear message.
   - **Setup Check** — Build and run tests. If this fails, the agent is given the failure output and asked to fix the project (deps/config/code); we re-run install and re-check. Repeats up to 3 times.
3. **Execute**
   - **Execute Run** — Run the agent (ask: answer the question; dev: run the task; review: run the review prompt).
   - **Execute Check** — Validate the output (for dev, format and build). If this fails, the agent is given the failure, asked to fix the code, then re-checks. Repeats up to 3 times.
5. **Assurance** — Depends on the command:
   - **Ask:** **Ask assurance** — Filter step: the raw answer is passed through a cleanup prompt (trim preamble/cruft); that can run up to 2 passes (feed back into itself). Does not fail the run.
   - **Dev:** **Assurance loop** — Agent reviews recent changes. If the review reports issues, the agent is asked to address them; we re-run execute check, then assurance again. Up to 3 attempts; then we continue to commit.
6. **Commit & Push (dev only)** — Commit, `git fetch` + rebase onto remote branch when it exists, then push. Push failures (e.g. non–fast-forward) are reported.

All commands **fetch from the configured remote** and **reset/checkout to the latest remote ref** for the base branch. Features are created from up-to-date remote base; reviews compare against the latest remote base. Base branch and remote name are configurable per project (see Project commands) or via `--base` for the branch.

### Summary

| Command | Setup loop | Execute | Execute-check loop | Assurance | Commit/push |
|--------|------------|---------|--------------------|-----------|-------------|
| **ask**  | ✓ | Question → answer | — | Cleanup filter (2 passes) | — |
| **dev**  | ✓ | Task → edits | ✓ | Review → fix loop | ✓ |
| **review** | ✓ | Review prompt → text | — | — | — |

---

## Commands and options

### System commands

- **`smith status [--verbose]`**  
  Show status of dependencies, models, agents, and projects.

- **`smith install`**  
  Interactive setup: check/install Docker (Linux: get.docker.com), optionally enable Docker at boot, create config dir, add models and projects.

### Pipeline commands — `smith run <cmd>`

- **`smith run ask "<question>"`**  
  Ask the agent about the project (read-only).  
  - `--base <branch>` — Branch to clone and ask about (default: `main`).  
  - `--repo <url>` — Override repo (SSH URL).  
  - `--project <name>` — Use repo/image/ssh_key from config.  
  - `--image <image>` — Override Docker image (default: auto from repo: Rust/Node/Go/Python image; must be glibc-based or have Node for OpenCode).  
  - `--ssh-key <path>` — SSH key path (overrides project config and `SSH_KEY_PATH`).  
  - `--keep-alive` — Keep container alive after run (debugging).  
  - `--timeout <sec>` — Timeout for agent run (default: 300).  
  - `--verbose` — Show detailed pipeline output.

- **`smith run dev "<task>" --branch <branch>`**  
  Run a development task, validate, commit, and push.  
  - `--branch <branch>` — **(required)** Target branch to work on and push.  
  - `--base <branch>` — Base branch to start from (default: `main`).  
  - `--repo`, `--project`, `--image`, `--ssh-key`, `--keep-alive` — Same as ask.  
  - `--pr` — Create or update a GitHub PR after push (requires token).  
  - `--timeout <sec>`, `--verbose` — As above.

- **`smith run review <branch>`**  
  Review the given branch (read-only).  
  - `--base <branch>` — Base branch to compare against (optional).  
  - `--repo`, `--project`, `--image`, `--ssh-key`, `--keep-alive`, `--timeout`, `--verbose` — Same as above.

### Model profile env passthrough

Use model profile env mappings to inject host environment variables into spawned project agent containers.

- Add/update mappings with repeatable `--env` flags:
  ```bash
  smith model add opencode --provider openai --model gpt-5 --env 'OPENAI_API_KEY=$OPENAI_API_KEY'
  smith model update opencode --env 'OPENAI_API_KEY=$OPENAI_API_KEY' --env 'OPENROUTER_API_KEY=$OPENROUTER_API_KEY'
  ```
- Clear mappings on update by passing an empty env value:
  ```bash
  smith model update opencode --env ''
  ```
- Always quote the `--env` value so your shell does not expand `$HOST_ENV` before smith parses it.
- If a mapped host env is missing or empty at runtime, `smith agent start` fails fast with a clear error.
- Security note: injected values are passed to Docker as container environment variables; treat host access and Docker inspect permissions accordingly.

### Project commands — `smith project <cmd>`

- **`smith project add <name> --repo <path-or-url>`**  
  Register a project.  
  - `--image <image>` — Docker image for this project.  
  - `--ssh-key <path>` — SSH key path for this project.  
  - `--base-branch <branch>` — Base branch for clone/compare (default: `main`).  
  - `--remote <name>` — Remote name for fetch/push (default: `origin`).  
  - `--github-token <token>` — GitHub personal access token for PR creation (per-repository).

- **`smith project list`**  
  List registered projects (shows repo, image, ssh_key, base_branch, remote, and whether github-token is set).

- **`smith project status [--project <name>] [--verbose]`**  
  Validate project configuration and connectivity.

- **`smith project update <name>`**  
  Update a project.  
  - `--repo <url>`, `--image <image>`, `--ssh-key <path>`, `--base-branch <branch>`, `--remote <name>`, `--github-token <token>` — Set new value; pass `""` to clear optional fields.

- **`smith project remove <name>`**  
  Remove a project.

### Agent commands — `smith agent <cmd>`

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

- **`smith agent add <name>`**  
  Register an agent.  
  - `--image <image>` — Docker image (default: `ghcr.io/anomalyco/opencode`).  
  - `--model <model>` — Model to use (e.g. `anthropic/claude-sonnet-4-5`).  
  - `--small-model <model>` — Small model for internal ops.  
  - `--provider <name>` — Provider name (`anthropic`, `openai`, `openrouter`, etc.).  
  - `--base-url <url>` — Custom base URL for provider.

- **`smith agent status`**  
  Show status of all configured agents (active/inactive, image, port, model, provider).

- **`smith agent build [<name>] [--all] [--force] [--verbose]`**  
  Build Docker image for one agent or all. Generates Dockerfile if missing, then runs `docker build`. Use `--all` to build all configured agents, `--force` for clean build (remove image, build with `--no-cache`), `--verbose` to print Dockerfile path and docker build command.

- **`smith agent update <name>`**  
  Update an agent.  
  - `--image <image>`, `--model <model>`, `--small-model <model>`, `--provider <name>`, `--base-url <url>` — Set new value; pass `""` to clear.

- **`smith agent remove <name>`**  
  Remove an agent.

- **`smith agent start [--verbose]`**  
  Start all configured agents. Each agent runs in its own container with the configured model/provider. Skips agents that already have a running container. Use `--verbose` to print docker command and health-check details.

- **`smith agent stop`**  
  Stop all running agent containers (smith-agent-*).

- **`smith agent logs <name>`**  
  Stream live logs from an agent container (`docker logs -f`).
---

## GitHub Pull Requests

1. Add or update a project with a GitHub token (one per repository):
   ```bash
   smith project add myproject --repo git@github.com:user/repo.git --github-token <your-github-token>
   # or
   smith project update myproject --github-token <your-github-token>
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
