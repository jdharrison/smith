#!/bin/sh
set -eu
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
# Uses typed fake backends/providers and private HOME/XDG/IPC fixtures only.
# Never register/start a normal unit or contact a normal model/provider here.
cargo test --all-targets --locked
