#!/bin/sh
set -eu
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
# Includes compiled foreground daemon and injected-manager sandbox integration tests.
# Never invoke inspect/apply here against the real user environment.
cargo test --all-targets --locked
