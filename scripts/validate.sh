#!/usr/bin/env bash
# Validation and cleanup script for smith.
# Runs format, build, clippy, and tests.

set -e

cd "$(dirname "$0")/.."

echo "==> Formatting..."
cargo fmt

echo "==> Verifying format..."
cargo fmt -- --check

echo "==> Building..."
cargo build

echo "==> Clippy (lint)..."
cargo clippy --all-targets -- -D warnings

echo "==> Tests..."
cargo test

echo ""
echo "✓ All checks passed"
