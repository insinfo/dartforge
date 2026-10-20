# Bind context data to the selected SDK runtime — 2026-10-19

The macOS ARM64 AOT context helper reads the external
`dartforge_context_slot` word before reading the thread's TSD slot. The JIT
published requested SDK functions but did not bind this data import from the
same SDK. Process symbol resolution could therefore use the embedded runtime's
homonymous word with the SDK runtime's thread context.

The minimal fixture compares the inline TSD lookup with the selected SDK's
`dartforge_context()` result. The retained old executor returns **1**, a pointer
mismatch, normally and under GC stress. The fixed executor returns **0** for
the exact same fixture and SDK binary. Replacing the original AOT context helper
with a runtime call also made the failing program run; that counterfactual IR
is retained, but is not the implementation of the fix.

## Implementation

`ffi::RUNTIME_DATA_SYMBOLS` lists reviewed native data imports (currently only
this word on macOS ARM64). The session inspects LLVM external-global declarations
and binds requested cells through the selected SDK library's own handle with
non-callable data flags. It keeps their names separate from callable exports.
Binding happens before ordinary module materialization and before the external
data precheck of a reload. An SDK missing required data is rejected, rather than
falling back to a homonymous process symbol. Imported cells are not module-owned
mutable globals and are not reset or versioned with program generations.

This changes neither the emitted IR, its context helper, the SDK export manifest,
runtime binaries, nor class-pruning permissions. SDKs cached before the change
continue working when they export the required runtime data.

## Verification

- The before/after probe uses the same fixture and SDK. Old executor mismatch
  exit **1** is deliberately preserved as the before control, not a passing
  execution. `execution.json` records expected and actual exit codes explicitly.
- The previous iteration's pruned development IR crashed in the JIT for both
  previous/current native compilers. Those observations are retained as
  `prior-*.json`. The fixed executor runs both unchanged IR files against their
  respective unchanged SDKs, prints the VM's **35294**, and passes normally and
  with the actual `DARTFORGE_GC_STRESS=1` setting. Regular JIT API IR also passes
  in both modes. `bindings.json` pins the executors, input IR and SDK libraries.
- Four permanent regressions pass: matching context with quoted/unquoted LLVM
  names, repeated execution across modules, the first reloadable module plus a
  hot reload, and rejection of a stub SDK without the required data. The missing
  data case also confirms that a failed addition does not install a module.
- The full Cargo JIT run passed **91 regular tests and six doctests**. A fourth
  regression was appended while it built; production code stayed unchanged.
  The final execution suite then passed **18/18**, including all four new tests,
  and the directed run passed **4/4**. The union is **92 distinct regular tests**
  plus six doctests; repeated tests are not counted as new coverage.
- Strict workspace/all-target Clippy and private workspace docs with warnings
  denied passed. The complete test/check logs and selected environment are
  retained. The first directed build had a test-helper naming error, corrected
  before execution; `directed-initial.log` preserves that compile failure.

The final executor is preserved at
`target/test-tools/dartforge-run-ir-sdk-data-fixed`; Cargo may replace the default
`target/debug/dartforge-run-ir`. Earlier validation binaries have separate
binding records. `retention.json` records original/stored hashes and any removal
of extra blank lines at EOF from stored logs; scratch originals are unchanged.

Execution scope is **macOS ARM64**. There is no new File Explorer desktop or
cross-OS matrix. Windows/Linux have no entries in this runtime-data catalog;
this increment does not claim execution checks on them. Global formatting and
the full workspace remain unapproved: the separate production backend-pruning
failure is still open. No performance or binary-size improvement is claimed.
