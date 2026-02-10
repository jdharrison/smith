# Agent Smith

Agent Smith — open-source control plane for coding orchestration and configuration

![46a22884bfb0164c9d70b69a5db74027](https://github.com/user-attachments/assets/b4eacebe-6161-4718-bbf2-42797d3f1ecc)

## Quick Start

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

### Commands

- `smith doctor` - Validate the local environment
- `smith config path` - Show config file location
- `smith project add <name> --repo <path-or-url>` - Register a project
- `smith project list` - List registered projects
- `smith project remove <name>` - Remove a project
- `smith run --project <name>` - Run orchestration (placeholder)
- `smith review --project <name> --keep-alive` - Review workflow (placeholder)

## Development

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy --all-targets -- -D warnings
```

## Roadmap

- [ ] Docker container execution
- [ ] Dagger integration for orchestration
- [ ] OpenCode integration for agentic coding
- [ ] Review workflow with keep-alive sessions

