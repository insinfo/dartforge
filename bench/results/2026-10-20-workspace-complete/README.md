# Complete workspace collection and oracle prerequisites — 2026-10-20

The unfiltered `cargo test --locked --workspace --no-fail-fast -- --include-ignored`
run completed in 2,978.67 seconds with **exit 101**. Compiler sources are at
`dac116ab`; the documentation/check-command follow-up is `c7470a1a`. No Rust
source changed during execution. The full log, terminal metadata, launch script
and selected environment audit are retained. Timings describe this execution,
not a controlled performance comparison.

Exactly two test executables failed:

- `dartforge-differential --test corpus`: both tests lacked configured oracle
  SDK roots. Dart on PATH and DARTFORGE_SDK_LIB do not configure those roots.
  The same test executable subsequently passed both tests with explicit native
  macOS SDKs 3.6.2 and 3.13.5, in a separate output directory, without modifying
  assertions or sources. Its hash, configuration, log and exit 0 are retained.
- `dartforge-emit-native --test production_profile`: the existing
  `platform_constant_prunes_the_other_backends` assertion remains failed.
  The other two tests passed. Six foreign definitions remain; no expectation
  was changed or excluded.

The native library's 1,043 tests passed, including ignored tests. The native
builder corpus passed (762.73 seconds), all four Angular transparency tests
passed (556.78 seconds), and the native JSON macro executor passed. Cargo
continued through later test executables and doctests after failures.
The baseline workspace command does not enable the CLI native/JIT features:
zero-test feature-gated executables are not coverage of those CLI paths.
The separately completed JIT suite is documented in
`../2026-10-20-jit-workspace-checks/README.md`.

`scripts/check.ps1` now checks oracle prerequisites before formatting/building.
Its separate preflight can run without a build. It requires complete SDK roots,
a host Dart executable whose version agrees with the version file, Dart 3.6.2
for the floor, and a compatible 3.13+ SDK for modern cases. Explicit selection
remains required outside the existing Windows fallbacks; no user-specific
paths are added. Direct Cargo commands still require caller configuration.
Actual PowerShell executions accepted the native SDK pair and rejected an
unset floor, a Windows modern SDK on macOS, and a wrong floor version. Missing
Node on PATH was also observed and correctly rejected before SDK checks.

Log endings alone are normalized; manifest.json preserves original and saved
hashes. Overall workspace approval, foreign-backend pruning and global
formatting remain open. No runtime, target bundle or VM state changed here.
