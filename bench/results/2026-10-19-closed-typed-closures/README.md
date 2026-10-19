# Closed class reachability through local typed closures — 2026-10-19

The fixed-image class proof previously rejected every typed closure allocation
and indirect typed call. This increment proves confined local typed entries; it
does not complete SDK startup exposure coverage or foreign-backend removal.
The feature remains opt-in with `DARTFORGE_CLOSED_CLASS_REACHABILITY=1`.

## Proof boundary

- Both entries must have local definitions. The uniform entry retains the
  `Ref(Ref, Ptr, Ptr)` requirement and implicit-receiver protection.
- The typed body needs a Ref environment, at most 12 explicit parameters, and
  parameters/result in Ref, I64, F64 or I1. Its signature must reproduce the
  creation's ABI code. Shape validation and complete reachable-body coverage
  still apply; a familiar function name is not a certificate.
- A typed target must be a direct SSA read of the typed-code field of the same
  protected allocation, using GetField or the lowering's dartforge_object_get.
  Calls must match the body signature in HIR representations, not merely their
  shared LLVM i64 spelling. Forwarded, copied, stored or reconstructed code
  addresses are not accepted.
- Closure handles and extracted code addresses cannot escape through arguments,
  fields, globals or returns, or be mutated. The context, ABI field and readonly
  class query are allowed; the runtime class query implementation was reviewed
  in crates/runtime/src/core.rs. A code address cannot masquerade as a heap
  object for context reads. Field offsets use the runtime layout constants.
- Allocation reachability already follows both local entry bodies. A class
  created by the typed body therefore keeps its table, while unrelated tables
  may be omitted. Imported/externally supplied/stored/forwarded closure targets
  and unproved SDK/native boundaries still retain broad registration.

## Verification

- `before.log`: the new positive typed-entry regression fails before the proof
  change, against the production code at 363801f3.
- `directed.log`: all 25 class-proof cases pass with ignored cases included.
  Three new regular tests cover both read forms, scalar arguments/results and
  16 negative ABI/address/escape/mutation variants. The new executable test
  compiles **32 binaries and executes them 64 times**: direct/runtime field
  reads, no scalar argument, int/double/bool arguments, int/double/bool results,
  and a directly captured object surviving a body allocation, with eager/closed
  registration and ordinary/stress GC. Object results exercise selector dispatch
  on the allocated class; scalar results are compared with their expected values.
  The required table remains and the unrelated table leaves emitted IR.
- The executable fixture imports no SDK. As in the existing uniform fixture,
  only the emitted main wrapper is removed to use the runtime-owned main.
  This checks the actual generated typed call and runtime allocation/GC path;
  it is not certification of a complete Dart application importing the SDK.
- `native.log`: the native unit suite is run with `--include-ignored`, with the
  Dart 3.6.2 source SDK explicitly selected. The complete run includes 822 tests.
  The final captured-context matrix expansion was subsequently repeated in the
  25-case directed run; production proof sources did not change afterward.
- `doctests.log`: all 121 native doctests pass.
- `production-before.log` and `production-after.log`: the real SDK-backed
  platform-constant production test still fails because WindowsBackend bodies
  remain in IR on macOS, including with the experimental option enabled. Its
  assertion was not weakened. No new size/performance gain for File Explorer
  or complete imported-code pruning is claimed.
- Strict native Clippy remains failing with 33 library / 47 test diagnostics,
  without diagnostics in the changed class-proof files or new suppressions.
  Required root formatting, Clippy and default Apple/LLVM-22 linkage checks also
  remain failing; root documentation builds with existing warnings.

## Reproduction

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
export DARTFORGE_TEST_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --lib --target-dir target/impl-a -- --include-ignored
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --lib --target-dir target/impl-a pruning::class_reachability::tests -- --include-ignored
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --doc --target-dir target/impl-a
DARTFORGE_CLOSED_CLASS_REACHABILITY=1 python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --test production_profile --target-dir target/impl-a platform_constant_prunes_the_other_backends -- --ignored --nocapture
```

Executed on macOS ARM64 with LLVM 22.1.8 and SDK 26.5. No Windows/Linux execution
claim is made. Local-log receipts retain full hashes; checked-in log excerpts
omit repeated cached linker warnings and normalize whitespace. Sources are bound
by sources.json. Correctness runs were concurrent where independent; their
elapsed times are not controlled performance measurements.
