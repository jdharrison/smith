# smith

smith — open-source control plane for local agent orchestration

<img width="512" src="https://github.com/user-attachments/assets/8483313d-3d82-4c21-8751-ab6e63abd9a5" />

## Status

> **Note:** This project is complete but not yet stable. Use in production at your own risk.

**Supported platforms:** Linux and macOS. Windows is not supported (the Dagger SDK uses Unix-only APIs). On Windows, use [WSL](https://docs.microsoft.com/en-us/windows/wsl/) to run smith.

## Installation

### Option 1: Install from Cargo (Recommended)

Install directly from the GitHub repository using Cargo:

```bash
# Install cargo/Rust if needed (unix); for all others: https://rustup.rs
curl https://sh.rustup.rs -sSf | sh

# Install latest release
cargo install --git https://github.com/jdharrison/smith.git
```

**Prerequisites:** Rust 1.83+ (stable recommended). Install via [rustup](https://rustup.rs/) so the project's `rust-toolchain.toml` is used.

### Option 2: Download Pre-built Binaries

Pre-built binaries for Linux and macOS are available in [GitHub Releases](https://github.com/jdharrison/smith/releases). (Windows is not supported; use WSL.)

## Quick Start

### Prerequisites

- Rust 1.83+
- **Docker** — container runtime used by Dagger
- **Dagger** — the `ask`, `dev`, and `review` pipelines run via the Dagger Rust SDK; the engine starts automatically (e.g. `docker start dagger-engine-*`). Install the [Dagger CLI](https://docs.dagger.io/install) if you use it separately, or rely on the SDK-managed engine.

### Build and Run

```bash
# Direct install
smith --help

# Development
cargo build
cargo run -- --help
```

### Pipeline commands (ask / dev / review)

Run **ask**, **dev**, or **review** directly with `smith` (or `cargo run --`). The pipeline runs inside the Dagger engine; by default you see a spinner and then the result. Use **`--verbose`** to see full pipeline output.

```bash
# Ask a question (read-only); default branch is main, use --base to ask about another branch
smith ask "How does auth work?" --project myproject
smith ask "What's in this branch?" --project myproject --base feature/x

# Run a development task (commit + push); use --pr to create or update a PR
smith dev "Add login endpoint" --branch feature/login --project myproject
smith dev "Fix bug" --branch fix/123 --project myproject --pr --verbose

# Review a branch
smith review feature/login --project myproject
smith review feature/login --project myproject --base main --verbose
```

Use **SSH repository URLs** (e.g. `git@github.com:user/repo.git`). The pipeline mounts your host `~/.ssh` and forwards `SSH_AUTH_SOCK` when set, so host auth (e.g. `ssh-add`) works. Use `--ssh-key <path>` to supply a specific key. Projects can store an image and SSH key via `smith project add/update`.

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
6. **Commit & Push (dev only)** — Commit, `git pull --rebase` when the remote branch exists, then push. Push failures (e.g. non–fast-forward) are reported.

### Summary

| Command | Setup loop | Execute | Execute-check loop | Assurance | Commit/push |
|--------|------------|---------|--------------------|-----------|-------------|
| **ask**  | ✓ | Question → answer | — | Cleanup filter (2 passes) | — |
| **dev**  | ✓ | Task → edits | ✓ | Review → fix loop | ✓ |
| **review** | ✓ | Review prompt → text | — | — | — |

---

## Commands and options

### Pipeline commands (Dagger)

- **`smith doctor [--verbose]`**  
  Validate environment (config dir, Docker, Dagger). Without `--verbose`, only a short success line is printed.

- **`smith ask "<question>"`**  
  Ask the agent about the project (read-only).  
  - `--base <branch>` — Branch to clone and ask about (default: `main`).  
  - `--repo <url>` — Override repo (SSH URL).  
  - `--project <name>` — Use repo/image/ssh_key from config.  
  - `--image <image>` — Override Docker image (e.g. `rust:1-bookworm` for Rust repos).  
  - `--ssh-key <path>` — SSH key path (overrides project config and `SSH_KEY_PATH`).  
  - `--keep-alive` — Keep container alive after run (debugging).  
  - `--verbose` — Show full Dagger and pipeline output (default: spinner then result).

- **`smith dev "<task>" --branch <branch>`**  
  Run a development task, validate, commit, and push.  
  - `--branch <branch>` — **(required)** Target branch to work on and push.  
  - `--base <branch>` — Base branch to start from (default: `main`).  
  - `--repo`, `--project`, `--image`, `--ssh-key`, `--keep-alive` — Same as ask.  
  - `--pr` — Create or update a GitHub PR after push (requires token).  
  - `--verbose` — Full output.

- **`smith review <branch>`**  
  Review the given branch (read-only).  
  - `--base <branch>` — Base branch to compare against (optional).  
  - `--repo`, `--project`, `--image`, `--ssh-key`, `--keep-alive`, `--verbose` — Same as above.

### Project commands

- **`smith project add <name> --repo <path-or-url>`**  
  Register a project.  
  - `--image <image>` — Docker image for this project.  
  - `--ssh-key <path>` — SSH key path for this project.

- **`smith project list`**  
  List registered projects (shows repo, image, and ssh_key when set).

- **`smith project update <name>`**  
  Update a project.  
  - `--repo <url>`, `--image <image>`, `--ssh-key <path>` — Set new value; `--ssh-key ""` clears project SSH key.

- **`smith project remove <name>`**  
  Remove a project.

### Config commands

- **`smith config path`** — Print config file path.
- **`smith config set-github-token <token>`** — Set GitHub token for PR creation.

### Container commands

- **`smith container list`** — List smith-related containers.
- **`smith container stop <name>`** — Stop a container.
- **`smith container remove <name>`** — Remove a container.

---

## GitHub Pull Requests

1. Set a GitHub token:
   ```bash
   smith config set-github-token <your-github-token>
   ```
2. Use `--pr` with `dev`:
   ```bash
   smith dev "Add new feature" --branch feature/new-feature --project myproject --pr
   ```
   This creates a PR if none exists for the branch, or updates the existing one. The task is used as the PR title. Base branch is `main` unless you pass `--base`.

## Development

```bash
cargo test
cargo fmt
cargo clippy --all-targets -- -D warnings
```
