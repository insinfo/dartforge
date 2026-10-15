#!/bin/zsh
set -euo pipefail
cd /Volumes/ssd_dock/MyRustProjects/dartforge
source bench/results/2026-10-14-jit-private-field-identity/env.zsh
export TMPDIR="$PWD/target/allocation-closure-hardening-tmp"
export LIBRARY_PATH="$PWD/target/jit-arc-readiness/zstd:/usr/local/lib"
unset DARTFORGE_TEST_SDK_LIB
export DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
python3 bench/results/2026-10-15-jit-callback-sdk-discovery/run_full.py
