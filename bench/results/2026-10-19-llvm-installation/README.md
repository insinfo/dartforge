# Portable LLVM installation selection — 2026-10-19

The shared Cargo configuration injected an absolute Windows LLVM prefix on every
host. Independently, the LLVM build script ignored compatible tools on PATH and
used a hardcoded `/usr/lib/llvm-22` fallback outside Windows. On this macOS host,
even a real LLVM 22.1.8 on PATH could not make that fallback work.

The Cargo configuration no longer supplies a machine-specific prefix. Selection
prefers nonempty `DARTFORGE_LLVM_DIR`, then `LLVM_SYS_221_PREFIX`, then versioned
and unversioned `llvm-config` names on PATH. Name precedence matches llvm-sys
221.1.0. Discovery queries `--version` and `--prefix`; subsequent link queries use
the chosen prefix. Explicit installations never silently fall through to another
toolchain. Version validation rejects release lines other than 22.1.x, and
llvm-sys now enables strict versioning for the same header release line.
PATH and the chosen llvm-config file participate in build-script invalidation.

## Verification

- `before.log`: the build script at `c624c7e5`, compiled directly with rustc,
  fails with exit 101 despite the actual LLVM being on PATH and both prefix
  variables unset. It attempts `/usr/lib/llvm-22`.
- `path-only.log`: the final script succeeds with that same real installation
  exposed only through PATH. `explicit.log` and `legacy.log` independently
  exercise both environment-variable selection paths.
- `broken-explicit.log`: a missing preferred prefix is rejected even with a
  valid legacy prefix and PATH installation available. `wrong-version.log`
  rejects a controlled fake tool reporting LLVM 21.1.8, before any library
  selection. `missing.log` reports how to configure LLVM when no tool exists.
  These three expected negative outcomes exit 101; they are not compiler
  regressions. `process-results.json` records all six actual script executions.
- `cargo-path.log`: **17 tests pass**, with neither prefix variable set in the
  Cargo child environment. This includes ten library tests, object emission for
  Windows x64 / Linux x64 and ARM64 / macOS x64 and ARM64, five selector tests,
  and the existing zstd override test. The final run includes llvm-sys's new
  strict-versioning feature. This checks actual LLVM linkage and object headers,
  not execution of foreign binaries.
- `clippy.log`: strict all-target Clippy passes for dartforge-llvm with
  `--no-deps`. No lint suppression is added.

## Reproduction and limits

The process fixtures are reproducible on this macOS ARM64 host with:

```sh
python3 bench/results/2026-10-19-llvm-installation/verify_process.py \
  /Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64 /tmp/llvm-process-verification
```

`verify_process.py` compiles the actual build.rs and invokes it as a child
process for every scenario. The incompatible-version tool is explicitly a
fixture, not another installed LLVM distribution. This script requires POSIX;
it does not certify execution on Windows.

The Cargo run used LLVM 22.1.8 and SDK 26.5. It first called
`scripts/cargo-macos-llvm.py::prepare` to select the compatible Mach-O linker and
SDK, then placed the LLVM bin directory on PATH and removed **both** prefix
variables before invoking:

```sh
cargo test --locked -p dartforge-llvm --all-targets --target-dir target/impl-a
```

The helper's encoded linker flags were retained; bare Cargo with Apple's older
LTO reader still cannot link this LLVM distribution. Explicit configuration
should keep both prefix variables consistent because llvm-sys reads the legacy
one. Homebrew-only discovery outside PATH is not implemented by this selector;
expose its bin directory or configure the prefixes explicitly.

The four required workspace checks were also attempted with both variables
pointing at the local LLVM. Formatting and Clippy still fail; workspace tests
stop at the Apple LLVM-21 linker reading LLVM-22 bitcode; documentation builds
with existing warnings. These remain unresolved. Linker warnings about a missing
`/usr/local/lib` search directory also remain in the passing package test logs.

`sources.json` binds the changed build sources; log receipts hash the full local
logs. Checked-in logs normalize whitespace, and workspace excerpts truncate long
lines. No runtime performance or new Windows/Linux execution claim is made.
