# Separate read-only allocation effects and reject hidden writes — 2026-10-20

Base `a06d3ae6`. The final-LLVM auditor treated every pointer read/address
calculation like a possible heap allocation. It also inspected only the first
opcode of a source line, so an arithmetic instruction followed by an inline
store could incorrectly receive an empty footprint. Clang accepts adjacent
instruction tokens after numeric constants, quoted SSA names and unnamed SSA
IDs; source line boundaries are not instruction boundaries.

The auditor now distinguishes complete canonical scalar loads, volatile loads,
primitive GEPs and scalar pointer/bit casts from unproved writes. Reads and
address arithmetic preserve raw-pointer exposure. A strict operand/type/shape
parser must consume the whole instruction; unsupported aggregate, atomic,
metadata and literal forms keep their old open obligation. Stores and heap bump
allocation remain unknown, and direct calls still contribute their complete
allocation dependencies.

An independent token scan rejects embedded memory operations, calls, EH effects
and additional assignments. It skips quoted names, handles numeric SSA IDs and
decimal boundaries, and fails closed on unsupported hexadecimal forms when an
embedded effect needs inspection. Assignment prefixes must be a single SSA name;
a write or label before an assignment cannot disappear in an RHS-only audit.
This does not introduce pointer validity, Ref/value-region, callback ABI or
registry permissions. The consumer still refuses these read-only functions as
known nodes, callable pointer/Ref imports and runtime entries.

## Regressions and real LLVM

All five final directed tests fail against the previous allocation auditor and
pass after the change. They cover empty read footprints with exposure, raw and
hidden writes, unknown/indirect calls, finite and unknown callee footprints,
real Clang acceptance of hidden writes, and consumer capability refusal.
The original three-test failure and a later counterfactual with the complete
five-test fixture are retained. The old auditor is pinned by commit; unrelated
compiler code is not reverted for this counterfactual.

The real `df.list_len` emitter helper now has an empty allocation footprint and
keeps pointer exposure. Its previous test was updated to check that distinction.
The independent inline heap-bump allocation rejection is unchanged and passes.
An existing context test likewise checks the newly allocation-neutral unrelated
reader rather than requiring a read to allocate. No foreign-backend absence
assertion was weakened.

The first strict Clippy run rejected a constant-size `chunks_exact`; this was
replaced with `as_chunks`, and full verification was repeated. An intermediate
successful round preceded the numeric/SSA boundary guards. Only `checks/` and
`source-bindings.json` describe final approval. Earlier logs remain separate.

## Actual SDK observations

| Production SDK | Functions | Known footprints before / after | Gained / lost |
| --- | ---: | ---: | ---: |
| macOS ARM64 | 32,708 | 443 / 649 | 206 / 0 |
| Linux ARM64 | 32,708 | 444 / 650 | 206 / 0 |
| Windows x64 | 32,693 | 428 / 634 | 206 / 0 |

All newly known functions retain direct or transitive exposure obligations.
The complete `(module, symbol, body_digest)` multisets are unchanged. The first
comparison incorrectly joined by the global key, which is not unique for
ambiguous/interposable definitions, reporting 18 apparent differences. The
failed expectation and script are preserved; the corrected comparison verifies
multiplicity and module identity rather than discarding duplicate definitions.

These are allocation observations, not evidence that the enclosing SDK error
helpers, runtime invocation roots or image startup are fully covered. The actual
platform reproduction still has six foreign-backend definitions in both modes.
The unmodified `platform_constant_prunes_the_other_backends` gate was rerun on
final sources and fails with exit 101 because Windows backend bodies remain.

## Final verification and execution

The complete native library suite passed **1,026/1,026**, including ignored tests,
in 306.40 seconds. The five directed tests overlap that total. Strict workspace
all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied private docs
and the JIT compiler build passed. New/scoped files pass rustfmt. Global format
still fails; its output size/digest and normalized excerpt are retained.
Whole-workspace test approval remains open. The existing LLVM compression-object
minimum-macOS warning is retained; current-host execution does not certify older
macOS versions.

The nine-class, two-isolate fixture matches Dart 3.6.2 before/after in macOS AOT
production, development and disabled pruning, and after in macOS JIT, under normal
and stressed GC. Before/after cross AOT builds succeeded for Linux ARM64 and
Windows x64. Updated binaries passed both GC conditions in Ubuntu and Windows 11
ARM64's x64 compatibility environment, with guest copy hashes checked and empty
stderr. Both VMs were restored to their initial suspended state.

All five complete assembled IR comparisons are byte-identical: three macOS
profiles and Linux/Windows cross builds. No runtime source, target bundle,
inventory schema or callee-effects catalog changed. No new explorer GUI/guest
JIT matrix, speedup or binary-size reduction is claimed.

Source bindings identify the final seven Rust files. Log bindings preserve
original and normalized evidence, including failures. Large compilers, LLVM IR,
executables and inventory snapshots remain under ignored target storage with
artifact hashes. The compiler is published through a fresh inode and atomic
rename, preserving executable identity while it is in use.
