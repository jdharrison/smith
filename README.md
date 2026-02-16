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

**Prerequisites:** Rust 1.75+ and Cargo installed. If you don't have Rust, install it from [rustup.rs](https://rustup.rs/).

### Option 2: Download Pre-built Binaries

Pre-built binaries for Windows, macOS, and Linux are available in [GitHub Releases](https://github.com/jdharrison/smith/releases).

## Quick Start

### Prerequisites

- Rust 1.75+
- Docker

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

### Core Commands

- `smith ask <question> --project <name>` - Ask a question to an agent about a project (read-only)
- `smith dev <task> --branch <branch> --project <name> [--pr]` - Execute an idempotent development task with validation and commit (read/write). Use `--pr` to create or update a pull request.
- `smith review <branch> --project <name>` - Review changes on a branch of the project (read-only)
- `smith doctor` - Validate the local environment

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

