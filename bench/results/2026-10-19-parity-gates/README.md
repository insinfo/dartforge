# Parity comparison and reference failure gates — 2026-10-19

Baseline: `c0158ec9`. Host: macOS ARM64. Real oracle: Dart 3.6.2.

Four added core regressions failed before the fixes (33 passed, four failed):
UTF-8 path slicing panicked; diagnostic types and inconsistent coordinates were
accepted; duplicate-key fallback consumed a later exact match. See
`core-before.txt`. The CLI also returned zero for a deliberate missing oracle
record (`probe-mismatch-before.txt` and JSON).

The scoreboard now reserves exact matches before metadata fallbacks and checks
line/column, diagnostic type, severity, message and correction. `probe_files`
retains duplicate multiplicities, rejects invalid/empty/outside/duplicate inputs,
checks recorded source hashes/counts/SDK, preserves existing package metadata,
and exits unsuccessfully for differences or analysis failures. Directory symlinks
are rejected by the probe rather than followed recursively.

The oracle checks the executable's actual version, JSON report version and
process exit, and rejects inconsistent recorded diagnostic counts. Reference
recording/classification preserve source files and recorded references on oracle
failure; an unknown/empty group selection fails. A secondary reference cannot
be reused for changed sources when its SDK is unavailable.

## Checks

- `targeted-tests.txt`: 210 passed, zero failed/ignored: analysis 153 unit + one
  doc; parity 41 unit + five integration + seven probe example + three doc.
- `targeted-clippy.txt`: strict Clippy, parity, all targets; no new suppressions.
- `cli-checks.json`: all 16 real CLI scenarios passed. `check-cli.py` reproduces
  them using isolated temporary packages/groups. A fresh real SDK warning matches;
  repeated runs and an existing custom package retain their metadata bytes.
- Wrong SDK test points the 3.13.4 configuration at actual 3.6.2 and verifies
  rejection and unchanged source/reference hashes. This does not test a positive
  3.13.4 run; that SDK is unavailable on this host.
- Invalid report preservation uses a Rust-built **test double**, explicitly
  reporting version 3.6.2 and emitting JSON version 99. It exercises the recorder's
  post-preflight failure/bisection path; it is not a successful Dart analysis.
- Required workspace checks were run: format exit 1 (existing workspace drift),
  Clippy exit 101 (emit-native: 154 library / 192 test errors), tests exit 101
  (Apple LLVM 21 linker cannot read LLVM 22 bitcode), docs exit 0 with warnings.
  `checks.json` records complete log byte counts/hashes; large logs have bounded
  excerpts. No full-workspace green claim.

Reproduction from repository root (SDK locations may differ):

```sh
export DARTFORGE_ORACLE_3_6=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk
export DARTFORGE_SDK_LIB="$DARTFORGE_ORACLE_3_6/lib"
cargo test --locked -p dartforge-parity -p dartforge-analysis -- --include-ignored
cargo clippy --locked -p dartforge-parity --all-targets -- -D warnings
cargo build --locked -p dartforge-parity --example probe_files --bin dartforge-parity
python3 bench/results/2026-10-19-parity-gates/check-cli.py
```

`source-hashes.json` identifies tested parity sources and Cargo.lock. Text evidence
has trailing whitespace normalized. No corpus source/reference was intentionally
rewritten except disposable harness fixtures, which are removed on exit.

## Limits

Comparison covers the stored `Record` projection, not every analyzer JSON field
(context messages/documentation are not stored). Recorder cross-batch deduplication
is unchanged. Source freshness hashes cover Dart text, not every dependency or
configuration; SDK version validation is not binary authentication. The general
corpus discovery and scoreboard command retain existing reporting behavior;
this change does not certify every parity subcommand as a strict gate. No
performance improvement is claimed, and these checks were not repeated in VMs.
