# Allocation-only witnesses for thread-context readers — 2026-10-20

Base `179af970`. The allocation inventory previously treated every emitted
`df.context` body as unknown because it reads raw pointers and, on macOS ARM64,
uses inline assembly. This prevented an empty allocation observation even for
the exact register/TSD lookup or native TLS pointer-cache body.

The auditor now recognizes independently reviewed, complete instruction shapes.
The macOS shape reads the CPU thread register and runtime-published slot. The
native TLS shape addresses a private zero-initialized pointer cache, reads it,
and fills it through the existing canonical runtime context API on the slow path.
That ordinary runtime call remains in normal SCC closure. The TLS intrinsic is
allocation-neutral only inside the checked body, with its exact declaration and
storage. Other raw memory operations keep their existing open obligations.

Reviewed instruction strings are intentionally separate from the generator and
its assembly constant: a future generator change cannot automatically extend
this proof. Changes in effects, storage, linkage, calls or instruction text must
invalidate the witness until independently reviewed. The generator's native TLS
body was factored out for cross-host fixtures; its emitted bytes are unchanged.

The witness checks symbol/storage uniqueness, including quoted/hex-escaped aliases,
and rejects shadowed native boundaries, duplicate owners, malformed/multiline
quoted operands and opaque assembly. Module assembly with alternate whitespace
and inline assembly in another function cannot hide a runtime alias. The one
reviewed TSD register instruction is accepted exactly; assembly-like bytes in
ordinary strings do not count as assembly.

## Scope and observed SDK inventories

These are **Dart allocation footprints**, not pointer-validity, purity, runtime
callback/input-ABI, image-registration or class-coverage certificates. Raw-memory,
global-address and thread-context pointer exposure remain explicit. The TSD
reader also retains inline-assembly exposure; native TLS retains both ordinary
call records. The consumer refuses node/call/entry capabilities and `ptr`/`Ref`
results for the lookup and a source wrapper. No inventory schema, runtime ABI,
callee effects catalog or import-pruning permission changed.

Actual production inventories, bound in `runtime/sdk-footprint-comparison.json`:

| SDK target | Functions | Known footprints before / after | Changed bodies |
| --- | ---: | ---: | ---: |
| macOS ARM64 | 32,708 | 428 / 443 | 0 |
| Linux ARM64 | 32,708 | 429 / 444 | 0 |
| Windows x64 | 32,693 | 428 / 428 | 0 |

All 15 changed observations in macOS and Linux are local context readers with
unchanged body digests. Linux records the TLS intrinsic and runtime context call
in every reader. The Windows SDK currently enables no TLS-context transformation;
it continues using the direct runtime call and has zero such helper definitions.
The first inspection script incorrectly expected 15 there; its failed assertion
and original script are retained. The corrected expectation follows emitted IR
and the generator's Unix-only condition, without changing production behavior.
The final macOS inventory's entire `inventory` payload also matches the
intermediate payload from before the independent-review refactor.

## Regressions and final checks

Six directed tests cover exact TSD/TLS variants, altered bodies/storage,
namespace and ownership ambiguity, opaque assembly, independent modules, actual
host emission and consumer capability refusal. Five tests fail against the old
allocation auditor with unchanged helper templates retained as fixtures; one
negative test passes. This counterfactual does not revert unrelated compiler
changes. The actual preserved old compiler and its SDK inventory also show the
previous unknown context footprints.

The final complete native library run passed **1,021/1,021**, including ignored
tests, in 288.26 seconds. Directed tests overlap that total. Strict workspace
all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied private docs,
and the JIT compiler build passed. New files pass scoped rustfmt. Global
formatting remains open, with full output size/digest and a normalized excerpt.
Two earlier check rounds and the pre-independent runtime checks are retained
separately; only `checks/` and `source-bindings.json` describe final approval.
The initial LLVM compression-object deployment warning remains; current-host
execution is not certification for older macOS versions.

The actual platform reproduction still emits six foreign-backend definitions in
both modes. The unchanged `platform_constant_prunes_the_other_backends` gate was
executed on the final sources and **fails** with exit 101 because Windows backend
bodies remain in the IR. Foreign-backend pruning, enclosing error-helper/startup
proofs and whole-workspace test approval therefore remain incomplete.

## Execution and code-generation parity

The nine-class, two-isolate fixture matches Dart 3.6.2 (`parent=45`, `child=45`)
before/after in macOS AOT production, development and disabled table pruning,
and after in macOS JIT, under normal and stressed GC. Before/after cross AOT
builds succeeded for Linux ARM64 and Windows x64; final guest executables passed
both GC conditions with hash-checked copies and empty stderr. Windows executes
x64 compatibility code inside Windows 11 ARM64. Both VMs are suspended again.

All five complete assembled IR comparisons are byte-identical: three macOS
profiles and the Linux/Windows cross builds. This validates that the template
factor and added observations preserve the fixture's code-generation inputs.
No runtime or target bundle was changed/reexported, no new GUI/guest JIT matrix
was run, and no speedup or binary-size reduction is claimed.

Large compiler, executable, LLVM IR and inventory snapshots remain under ignored
target storage, bound by `artifact-bindings.json`. `log-bindings.json` binds
original and normalized logs/scripts, including the old-auditor failures and the
incorrect Windows inspection expectation. Final source bindings are rechecked
before collection. Compilers are published through fresh inodes and atomic rename.
