#!/bin/sh
set -eu
cargo build --release --locked --features gui --bin smith-installer-ui
