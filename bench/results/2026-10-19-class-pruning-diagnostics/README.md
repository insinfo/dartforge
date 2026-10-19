# Closed class pruning diagnostics — 2026-10-19

The experimental class-table proof now reports its first unproved boundary when
both `DARTFORGE_CLOSED_CLASS_REACHABILITY=1` and
`DARTFORGE_TRACE_CLASS_REACHABILITY=1` are set. Without the trace, rejection
remains silent. Every rejection discards the provisional class set and retains
broad registration. No proof condition was relaxed and the feature remains opt-in.

Diagnostics distinguish configuration, duplicate declarations, missing or
inconsistent class/table definitions, SDK inventory reconciliation, imported roots,
reachable symbols, SSA/closure boundaries, instructions, protocol ABIs and exposed
function addresses. Available symbols and instruction values identify the boundary.
Only the first failure is reported; its ordering is not a complete list of blockers.

## Actual production failure

`production.log` is a real source-SDK production test with both options enabled,
using the final proof source identified in `sources.json`. It exits **101** on the
unchanged assertion that WindowsBackend bodies must leave the IR on macOS.
Before reaching the inventory/entry checks, the declaration audit rejects an
imported SDK table function without a local table. The observed symbol is
`df.mt.dart$3aio._ProcessImplNativeWrapper`, class 867. Other imported declarations
may be encountered first because the map iteration order is not fixed.

Lowering declares table functions for compiled SDK libraries as well as the local
program (`lower/sdk_source.rs`), while the proof requires those declarations to
match local method tables. The trace makes this mismatch visible. This increment
does not authorize imported tables, establish closed SDK startup, or remove the
foreign backend methods. Any future import support must validate the actual
selected SDK definitions instead of trusting a familiar symbol name.

`ir-comparison.json` records the baseline IR and an intermediate diagnostic build:
the sorted backend definitions are identical. Foreign constructors are already
absent, but their getters/adapters and table functions remain. Full IR artifacts
remain in `/tmp`; their hashes and the relevant definitions are recorded.

## Verification

- `directed.log`: **26 passed**, including executable eager/closed registration
  comparisons and stress GC. This run uses the final source. A new regression
  verifies that a failed re-audit clears previous coverage and names an unproved
  root or allocation instruction.
- `native-before-detail-refinement.log`: **823 passed** with `--include-ignored`.
  This complete native unit run used the intermediate diagnostic implementation;
  declaration messages were subsequently split into more precise reasons without
  changing acceptance conditions. The final 26-case run covers that refinement.
- `clippy.log` and `doc.log`: final workspace strict all-target Clippy and private
  documentation with `RUSTDOCFLAGS=-D warnings` both exit **0**.
- `checks.json`: global formatting remains **exit 1** from existing drift.
- `production.log`: final production pruning requirement remains **failed**.
  No new complete workspace pass or Windows/Linux validation is claimed.

Logs normalize trailing blank lines only; `logs.json` retains original and stored
SHA-256 receipts. Validation used macOS ARM64, LLVM 22.1.8 and Dart 3.6.2 sources.
The production regression command was:

```sh
DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64 \
LLVM_SYS_221_PREFIX=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64 \
DARTFORGE_TEST_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib \
DARTFORGE_CLOSED_CLASS_REACHABILITY=1 DARTFORGE_TRACE_CLASS_REACHABILITY=1 \
cargo test --locked -p dartforge-emit-native --test production_profile \
  --target-dir target/impl-a platform_constant_prunes_the_other_backends \
  -- --ignored --nocapture
```
