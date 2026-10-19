# Closed class coverage validation — 2026-10-19

Baseline: `0fa55ed5`. Host: macOS ARM64. Source SDK: Dart 3.6.2.

The existing experimental gate could publish a closed class set with a live
selector target lacking a body, a statically called body with an incompatible
signature, or native scalar memory writes. Three new regressions failed before
the changes (`before.txt`: four passed, three failed). The gate now preserves the
wide registration regime for these inputs.

## Implementation

- All live graph references need a known definition or one of the explicitly
  admitted runtime APIs. Roots, function/table/data namespace collisions,
  duplicated selector hashes, uniform entry ABIs and protocol references are
  checked. Local calls match arguments/results against actual callee signatures;
  admitted runtime calls match the canonical declaration and operand types.
- The existing bounded structural SSA/CFG witness is shared with the class gate.
  This is not a general HIR type verifier. Unknown bodies and unsupported exposure
  do not become empty allocation facts.
- Native memory operations, function-address constants/tables and external helper
  bindings stay unknown. Pointer loads/stores need a direct local alloca with the
  matching representation; Ref pointer loads remain unsupported. Global accesses
  need a local declaration and matching representation/root metadata.
- A failed re-audit clears both the tentative and previous active-table sets.
- Declared tables of canonical runtime CIDs stay active even when the module's
  registration vector omits them. Primitive boxing, strings and exceptions can
  create these values without explicit object allocation instructions.
- When the proof succeeds, inactive table data/functions are omitted by the
  emitter as well as registration calls. Nominal class metadata remains.

The uniform toString fixture was corrected to use a separate three-argument
adapter rather than putting a one-argument legacy protocol body directly in the
uniform table. An ignored native test (explicitly run here) compiles eager and
closed modules and executes each normally and with `DARTFORGE_GC_STRESS=1`. It
checks selector dispatch and successful completion, plus absence of inactive
table definitions. The SDK-free fixture uses the runtime-owned main: the exact
SDK-style main wrapper is removed to avoid defining main twice. Runtime CIDs come
from the actual canonical ABI catalog. It is not an application/SDK pruning test.

## Verification

`checks.json` records final statuses: **805 unit tests and 121 doctests passed**,
with zero ignored reported in those runs. The repeated production-profile test
still fails on retained foreign-backend definitions. That is **926 passes and one
failure** in non-overlapping runs; the crate/workspace is not fully green.

`checks.json` records final statuses. The directed component run passed 15 tests
before the added implicit-boxing regression; the final library run includes all
16 component tests. Twelve tests were added to the crate; native execution is
included, not silently skipped. Component runs overlap the full library run and
must not be added to its count. The existing manual `compile_with_source_sdk`
test returns without compiling if `DARTFORGE_PROGRAM` is unset; that remains
unset in this run and is not evidence of another application execution.

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --lib --target-dir target/impl-a -- --include-ignored
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --doc --target-dir target/impl-a
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --test production_profile --target-dir target/impl-a platform_constant_prunes_the_other_backends -- --include-ignored
```

The modified files add no Clippy diagnostics or lint suppressions. Workspace
checks still fail for format drift, existing native/CLI/LSP Clippy diagnostics,
and the default Apple LLVM 21 linker versus LLVM 22 bitcode. Documentation builds
with warnings. Full original workspace log hashes/sizes are recorded; large logs
have bounded excerpts. Source hashes identify this revision's changed code.

## Remaining scope

The gate remains opt-in (`DARTFORGE_CLOSED_CLASS_REACHABILITY=1`) and still refuses
imported SDKs. This turn does not implement imported allocation/exposure closure,
does not consume the allocation-inventory sidecar as permission to prune, and
does not weaken the production-profile regression requiring foreign backends to
disappear. That larger requirement remains open. No performance, full foreign
backend removal, VM execution, or complete-workspace green result is claimed.

Text evidence trims trailing whitespace and blank lines; log hashes record the
original captured output before normalization.
