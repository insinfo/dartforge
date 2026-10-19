# Native IR uses the project's generation engine — 2026-10-19

`compile-native` used the generation engine when producing an executable, but
the `--emit-ir` / `--digest` branch passed no generator. A project importing an
in-memory generated source failed in these modes. With an old generated file on
disk, they could instead succeed while ignoring an invalid builder configuration.
Both modes now use `engine::with_generator`, preserving their resolved options,
worker stack, digest stdout, contract report and output paths. Resolved IR options
are grouped in a private owned request instead of eleven positional arguments.

## Regression and execution evidence

- `regressions-before.log`: the two final regression cases fail when replayed
  against the recorded previous compiler, SHA-256
  `d94913f8b04055a37eda0e53895cd7a1f5fbb2805f7e1ec4ca6004e02326888d`.
  `DARTFORGE_TEST_COMPILER` selects that executable explicitly; normal Cargo tests
  use `CARGO_BIN_EXE_dartforge`. It was built before this change, independently
  of the debug CLI used for the IR control below.
- `generated-ir.log`: both regressions pass against the current Cargo-built CLI.
  Three successful invocations prove fresh in-memory generation, an input edit
  reaching digest-only mode, equal IR/digest summaries for the updated input,
  unchanged output files in digest-only mode and a matching contract digest.
  Two rejected invocations prove that invalid configuration cannot use the stale
  file or replace existing IR/manifest files. Source outputs remain absent from
  disk in the successful case.
- The isolated fixture supplies synthetic package metadata, the i18n 4.2.1
  descriptor and minimal unused helper declarations. It exercises the native
  port without a Pub cache, network, VM or Dart builder executor. It is not an
  i18n compatibility oracle. The native executor is explicitly disabled.
- `native-generated.log`: both existing tests against the real resolved
  `corpus/builders/i18n` package pass: AOT executable and JIT output match the
  saved VM oracle. The required package configuration was present; neither test
  returned early for a missing dependency setup. No generated source or
  build_runner tree existed beforehand.
- `ir-control.json`: a program without builders produces exactly the same
  166,722 IR bytes before/after; stdout digest is unchanged and contract JSON is
  equal after normalizing only the requested artifact path. Compiler and IR
  hashes are recorded. This control uses optimize/assert/debug/timings/language
  and SDK options. Timings are not a performance comparison: cache state differs.
- `jit-cases.json` / `check-jit.py`: six real CLI cases pass across `run`,
  `reload --once`, and `reload --once --restart`. Unicode/empty/option-like
  arguments survive, and program exit 37 remains exit 37 in each mode. This
  checks the equivalent JIT guard simplification alongside the generation fix.
- `cli-unit-tests.log`: all 33 default-feature CLI unit tests pass.

The previous native/JIT CLI Clippy diagnostics were removed without new lint
suppression. `cli-all-features-clippy.log` records strict all-target CLI Clippy
with `--all-features --no-deps`; it does not certify dependencies. The default
feature check is recorded separately. The GDB-only integration test's equivalent
`split_once` cleanup was compiled, but GDB execution was not tested on macOS.

## Reproduction and limits

Run on macOS ARM64, LLVM 22.1.8, macOS SDK 26.5 and Dart source SDK 3.6.2:

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
export DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
export DARTFORGE_TEST_SDK_LIB="$DARTFORGE_SDK_LIB"
export DARTFORGE_BUILD_NATIVE=0
unset DARTFORGE_TEST_COMPILER
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-cli --features jit --test generated_ir --test native_generated --target-dir target/impl-a -- --nocapture
cargo clippy --locked -p dartforge-cli --all-targets --all-features --no-deps -- -D warnings
python3 bench/results/2026-10-19-generated-native-ir/check-jit.py --compiler target/impl-a/debug/dartforge --sdk-lib "$DARTFORGE_TEST_SDK_LIB" --out bench/results/2026-10-19-generated-native-ir/jit-cases.json
```

The old compiler replay deliberately exits with failing tests. Other native
fixtures and a full all-features CLI runtime suite were not run in this increment.
No new Windows/Linux execution is claimed. Existing engine fallback rules and
nontransactional IR/manifest filesystem writes are unchanged.
Integration log excerpts omit repeated cached linker warnings;
`test-log-hashes.json` records the complete local logs' hashes and exit codes.

`sources.json` binds changed implementation/tests and the runtime fixtures.
The four required workspace checks were run; `workspace-checks.json` records
complete local-log hashes and exit codes, with bounded excerpts committed here.
Formatting and Clippy retain repository failures; default workspace test linking
cannot read LLVM 22 bitcode with Apple's LLVM 21 reader. Documentation builds
with warnings. These failures are not counted as passes.
