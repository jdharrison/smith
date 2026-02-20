# Agent Smith

Agent Smith — open-source control plane for coding orchestration and configuration

![46a22884bfb0164c9d70b69a5db74027](https://github.com/user-attachments/assets/b4eacebe-6161-4718-bbf2-42797d3f1ecc)

## Status

> **Note:** This project is complete but not yet stable. Use in production at your own risk.

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

Pre-built binaries for Windows, macOS, and Linux are available in [GitHub Releases](https://github.com/jdharrison/smith/releases).

## Quick Start

### Prerequisites

- Rust 1.83+ (use [rustup](https://rustup.rs/) for stable toolchain)
- **Docker** — used as the container runtime by Dagger
- **Dagger CLI** — required for `ask`, `dev`, and `review`. Install from [docs.dagger.io](https://docs.dagger.io/install) or `curl -L https://dl.dagger.io/dagger/install.sh | sh`

### Build and Run

```bash
# Build
cargo build

# Run directly
cargo run -- --help

# Or install globally
cargo install --path .
smith --help
```

### Running ask / dev / review (Dagger)

The **ask**, **dev**, and **review** commands run a phased pipeline (setup → setup check → execute → execute check → assurance) inside the Dagger engine. You must run them via **`dagger run`** so the engine is available:

```bash
# Ask a question (read-only)
dagger run smith ask "How does auth work?" --project myproject

# Run a development task and optionally open a PR
dagger run smith dev "Add login endpoint" --branch feature/login --project myproject --pr

# Review a branch
dagger run smith review feature/login --project myproject
```

Use **SSH repository URLs** (e.g. `git@github.com:user/repo.git`). By default the pipeline mounts your host `~/.ssh` and forwards `SSH_AUTH_SOCK` if set, so whatever works on the host (e.g. `ssh-add` for passphrase-protected keys) works in the pipeline. Alternatively use `--ssh-key <path>` to supply a specific key (e.g. a dedicated or deploy key with no passphrase for automation). The pipeline uses your project's `--image` (or default `alpine:latest`) as the base container. OpenCode is installed via the [official install script](https://github.com/anomalyco/opencode#installation) (no Node required). Node/npm are only installed when the project has a `package.json`. A future release may support a custom Dagger module in your repo to override the default pipeline.

- `smith doctor` — validates Docker and Dagger (run as `smith doctor`, no `dagger run` needed)

### Core Commands

- `dagger run smith ask <question> --project <name>` — Ask a question to an agent about a project (read-only)
- `dagger run smith dev <task> --branch <branch> --project <name> [--pr]` — Execute a development task with validation and commit (read/write). Use `--pr` to create or update a pull request.
- `dagger run smith review <branch> --project <name>` — Review changes on a branch (read-only)
- `smith doctor` — Validate the local environment (Docker + Dagger)

### Project Commands

- `smith project add <name> --repo <path-or-url>` - Register a project
- `smith project list` - List registered projects
- `smith project remove <name>` - Remove a project

### Config Commands

- `smith config path` - Show config file location
- `smith config set-github-token <token>` - Set GitHub API token for PR creation

### Container Commands

- `smith container list` - List all smith containers
- `smith container stop <name>` - Stop a container
- `smith container remove <name>` - Remove a container

## GitHub Pull Requests

Agent Smith supports creating and updating pull requests on GitHub:

1. **Configure GitHub token:**
   ```bash
   smith config set-github-token <your-github-token>
   ```

2. **Use `--pr` flag with `dev` command:**
   ```bash
   smith dev "Add new feature" --branch feature/new-feature --project myproject --pr
   ```

The `--pr` flag will:
- Create a new pull request if one doesn't exist for the branch
- Update an existing pull request if one already exists (only one PR per branch)
- Use the task description as the PR title
- Default to `main` as the base branch (or use `--base` to specify)

## Development

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy --all-targets -- -D warnings
```

