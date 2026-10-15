#!/bin/zsh
set -euo pipefail
cd /Volumes/ssd_dock/MyRustProjects/dartforge
source bench/results/2026-10-14-jit-private-field-identity/env.zsh
export TMPDIR="$PWD/target/allocation-closure-hardening-tmp"
export LIBRARY_PATH="$PWD/target/jit-arc-readiness/zstd:/usr/local/lib"
cargo test --locked -j2 -p dartforge-runtime --test dart_ports -- --nocapture
