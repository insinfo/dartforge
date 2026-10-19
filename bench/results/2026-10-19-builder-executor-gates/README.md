# Builder executor acceptance gates — 2026-10-19

The VM/native execution tests previously discovered cases by requiring both an
oracle plan and `package_config.json` in a filter. Missing configurations were
silently omitted. A nonexistent native case filter consequently passed with
zero cases. The VM test also accepted pending outputs, so an unavailable Dart
executable passed with 15 equal, 45 pending and zero different outputs.

Both execution tests now use a shared prerequisite selector. It requires at
least one planned case and a configuration file for every selected case. Native
filters reject empty or unknown names, including partially valid filters.
Configuration contents are still validated by the engine when loaded. The
shared output check rejects pending and different outputs while leaving optional,
unrequested outputs alone. The separate plan-only corpus test retains its
informative pending scoreboard; it is not an execution acceptance test.

## Regressions and results

- `empty-native-before.log`: the original test passed a nonexistent filter with
  zero cases. `empty-native-after.log` rejects it with `unknown builder case`.
  `missing-config-after.log` exercises the actual native test against a temporary
  planned case with no package configuration and confirms its prerequisite error.
- `unavailable-vm-before.log`: with the prerequisite selector already fixed but
  the original output acceptance loop still present, a nonexistent Dart executable
  passed the ten-case corpus with 45 pending outputs. The final test fails in
  `unavailable-vm-after.log` and lists the pending assets and reasons. These two
  negative runs deliberately use an invalid tool path; they are not real compiler
  failures.
- `selection-tests.log`: six regular tests cover missing/empty roots, sorted
  discovery, the no-builder control, missing configurations, unknown/partial/empty
  filters and rejection of pending or different outputs.
- `vm.log` / `vm-counts.json`: all four ignored VM integrations pass with Dart
  3.6.2. The corpus covers ten projects and 60 equal outputs, zero pending and
  zero different. Twelve edits compare incremental and from-scratch states.
  The other integrations cover `json_serializable` demand, state persistence and
  post-processors. Fixtures are copied without build_runner outputs, so those
  outputs cannot serve as disk fallback. Native generator ports can still handle
  supported actions; this is the engine with its VM executor, not a claim that
  all 60 outputs execute Dart builder code.
- `native-initial.log`: the native executor passes `cadeia_configuracao`, with
  22 equal outputs, no pending/different and three incremental edits. This run
  followed the prerequisite fix and preceded consolidation of the already-strict
  native output check into the shared helper. `native.log` records its final
  repetition after that consolidation.
- `build-tests.log`: 66 regular tests pass; 18 tests are ignored by ordinary
  selection. `doctests.log`: 16 pass. `build-clippy.log`: strict all-target Clippy
  passes with `--no-deps`, without new suppressions.

## Reproduction

```sh
cargo test --locked -p dartforge-build --all-targets --target-dir target/impl-a
cargo test --locked -p dartforge-build --doc --target-dir target/impl-a
cargo clippy --locked -p dartforge-build --all-targets --no-deps -- -D warnings
export DARTFORGE_BUILD_DART=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart
cargo test --locked -p dartforge-build --test vm_executor --target-dir target/impl-a -- --ignored --nocapture
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
export DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
export DARTFORGE_COMPILER="$PWD/target/impl-a/debug/dartforge"
export DARTFORGE_CASES=cadeia_configuracao
cargo test --locked -p dartforge-build --test native_executor --target-dir target/impl-a -- --ignored --nocapture
```

The VM rejection can be reproduced by setting `DARTFORGE_BUILD_DART` to a
nonexistent executable and selecting `corpus_builders_through_the_vm`. The native
selection rejection uses `DARTFORGE_CASES=does_not_exist`; no executor compilation
is attempted for that invalid selection.

Executed on macOS ARM64. `tools.json` binds the compiler and identifies the Dart
VM; `sources.json` binds changed Rust test sources. Log receipts hash the original
local logs; checked-in logs normalize trailing whitespace. Workspace excerpts
are bounded and receipts retain full-log hashes. The four required root checks
were attempted: formatting and Clippy remain failing, workspace tests stop at
the Apple LLVM-21 linker reading LLVM-22 bitcode, and documentation builds with
existing warnings. An initial attempt inherited a Windows LLVM path; the recorded
root checks were repeated with the local LLVM directory explicitly selected.

This validates ten builder projects through the VM and one through the native
executor, not all native builders or Windows/Linux execution. Timings in logs
are observations, not controlled performance comparisons. No production compiler
or engine behavior is changed by this test-gate correction.
