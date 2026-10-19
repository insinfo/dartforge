# Hosted metadata cache validation — 2026-10-19

The CLI used to publish downloaded JSON before validating the package identity,
version rows, pubspec identity, archive digest and archive transport. A rejected
response could therefore replace the last usable offline snapshot. Publication
now happens after candidate validation, using the existing atomic writer.
Online failures remain errors; there is no silent fallback. A validated update
still replaces the snapshot. Valid metadata followed by an archive download
failure is outside this change's preservation guarantee.

## Evidence

- `regressions-before.log`: the previous production implementation fails two
  regressions; all eight invalid update variants destroy offline usability.
  Two other selected tests pass, including the valid-update control.
- `cli-tests.log`: 33 unit tests and six integration tests pass with default
  CLI features and `--include-ignored`. Feature-gated integration binaries that
  report zero tests do not count as coverage.
- `cli-clippy.log`: strict Clippy passes for the default CLI features, all
  targets, without new suppressions. The adjacent condition simplifications
  preserve evaluation order.
- `cli-build.log`: the actual release compiler was built with `--features jit`
  (including native), optimization level 1, LTO off and 16 codegen units.
- `cli-cases.json`: 17 actual compiler invocations pass against a temporary
  loopback registry. One valid resolution downloads an archive; eight rejected
  metadata updates preserve cache files, lockfile and package configuration;
  eight subsequent offline resolutions succeed without HTTP requests.
  SHA-256 snapshots and the tested compiler hash are recorded in the report.
  No Dart/Pub subprocess or public registry is used.
- `source-hashes.json` binds the changed source files and reproduction script.

Three new Rust tests cover invalid updates, an invalid first response (which
must not create a snapshot), and a valid replacement. Local TCP fixtures read
complete request headers and use bounded accepts, reads and writes.

## Reproduction

Run on macOS ARM64 with LLVM 22.1.8, macOS SDK 26.5 and Dart source SDK 3.6.2:

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
export PATH=/Applications/ChatGPT.app/Contents/Resources/cua_node/bin:$PATH
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-cli --all-targets --target-dir target/impl-a -- --include-ignored
cargo clippy --locked -p dartforge-cli --all-targets -- -D warnings
CARGO_PROFILE_RELEASE_OPT_LEVEL=1 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 python3 scripts/cargo-macos-llvm.py build --locked -p dartforge-cli --features jit --release --bin dartforge --target-dir target/impl-a
python3 bench/results/2026-10-19-pub-cache-validation/check-cli.py --compiler target/impl-a/release/dartforge --sdk-lib /Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib --out bench/results/2026-10-19-pub-cache-validation/cli-cases.json
```

## Limits and workspace checks

This cache regression was exercised on macOS, not repeated inside the VMs.
The preceding desktop matrix is independent; see the desktop-refresh evidence.
The CLI's all-features Clippy still reports existing native/JIT and debugging
integration-test lints (`cli-all-features-clippy.log`); default-feature success
does not certify all configurations.

The four required workspace checks were run. `workspace-checks.json` records
exit codes, byte lengths and SHA-256 of the complete local logs, with bounded
excerpts committed here. Formatting fails on existing repository drift;
Clippy fails in native/LSP code; the test command fails during linking because
Apple's LLVM 21 reader cannot consume LLVM 22 bitcode. Documentation builds
with existing warnings. These failures remain unresolved and are not passes.
