# Closed-image uniform closure witnesses — 2026-10-19

The experimental class-table analysis previously refused every closure creation
and invocation. It now covers local uniform closures and canonical tear-offs
whose code-bearing handles cannot escape or be mutated. This extends the proof
toward closure-based backend factories; it does not complete SDK startup coverage.
The opt-in flag remains `DARTFORGE_CLOSED_CLASS_REACHABILITY=1`.

## Proof boundary

- Each created closure names an actual local function with ABI
  `Ref(Ref, Ptr, Ptr)` and has a `Ref` context and result.
- Its SSA handle may be invoked or have its captured context read. Other uses
  (including arguments, aliases, stores and returns) refuse narrowing. Context
  field identity is derived from the runtime's layout constants.
- The target entry's implicit receiver is also protected from writes and escape.
  It is not an invocation witness: uniform ABI bodies can also be called through
  a selector or static call with a different receiver.
- The normal reachability graph follows the closure code reference and retains
  all classes transitively allocated by that body. Existing runtime class-table
  protection covers implicit closure allocation.
- Typed, forwarded, imported and externally supplied closures remain unknown.
  Source SDK startup still has additional allocation, registry and exposure
  obligations. The default behavior remains unchanged.

## Tests and evidence

`before.log` records the new positive regression failing against the previous
implementation. Three tests were added: the positive witness, 16 negative
variants, and compiled execution. `class-tests.log` records all 21 class-analysis
tests passing on the final source, including ignored execution fixtures.

The new compiled fixture creates six executables: uniform closure, canonical
tear-off and captured context, each with eager and narrowed registration. Each
executable runs under both normal and forced GC, for 12 executions. A selector
call on the returned value checks that its required class table remains. IR
assertions check that unrelated tables disappear. These are SDK-free ABI
fixtures, using the runtime-owned `main` instead of the normal SDK wrapper; they
are not complete imported-application or cross-platform validation.

`unit-tests.log` records 818 passing native unit tests with ignored tests included.
The existing manual-program test returns without running an external application
when `DARTFORGE_PROGRAM` is absent, as it was here. `doctests.log` records 121
passing doctests. `native-clippy.log` retains the existing 33 library / 47 test
diagnostics, with none in the changed class-analysis files and no new suppression.

`production.log` records the separate production requirement, explicitly run
with closed-class analysis enabled. It still fails because WindowsBackend bodies
remain in the macOS IR. That requirement has not been weakened or counted as a
pass. Earlier desktop validation is independent of this experiment.

## Reproduction

macOS ARM64, LLVM 22.1.8, macOS SDK 26.5, Dart source SDK 3.6.2:

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --lib --target-dir target/impl-a -- --include-ignored
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --doc --target-dir target/impl-a
DARTFORGE_TEST_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib DARTFORGE_CLOSED_CLASS_REACHABILITY=1 python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --test production_profile --target-dir target/impl-a platform_constant_prunes_the_other_backends -- --include-ignored --nocapture
```

`sources.json` binds the implementation and test sources. The four required
workspace checks were also run; `workspace-checks.json` records their exit codes
and complete local-log hashes, with bounded excerpts committed alongside it.
Formatting and strict Clippy retain repository failures; default test linking
fails because Apple's LLVM 21 reader cannot consume LLVM 22 bitcode. Workspace
documentation builds with warnings. These are unresolved failures, not passes.
