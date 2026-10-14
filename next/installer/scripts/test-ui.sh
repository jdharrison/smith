#!/bin/sh
set -eu
cargo fmt --all --check
cargo clippy --all-targets --locked --features gui -- -D warnings
# Private fake sessions on an isolated display, never the host desktop/manager.
xvfb-run -a env GDK_BACKEND=x11 cargo test --locked --features gui --bin smith-installer-ui -- --test-threads=1
