# Immutable HIR summary reuse — 2026-10-20

Baseline: `092b3e6f`. The FFI reachability fixed point previously rebuilt all HIR
body/table summaries each round, including registry closure and physical receiver
provenance. The module does not change during those rounds; only the entry's
trampoline/callback roots change. This increment builds the immutable body facts
once per `prune_hir` invocation and rebuilds the entry roots for each round.

The snapshot borrows the module immutably and is dropped before any retention,
class narrowing or specialization. A later invocation constructs a new snapshot.
The previous entry definition and root-only name suffix are removed before each
new entry is appended, preserving the exact identifiers and serialization of a
fresh summary. There is no global memo, changed summary schema, new pruning
permission or change to the runtime ABI. `--timings` now reports the number of
reachability rounds, including the final stable round.

## Correctness and work reduction

Four new unit tests check transitive SDK recipes/trampolines/callbacks, complete
summary equality across 24 growing/shrinking/all/empty root sets, existing entry
name collisions and the one-round no-FFI case, and recomputation after changing
allocation identities or exposing the registry. The transitive test needs four
rounds with three live signatures; the previous algorithm builds four body
summaries, while the new one builds exactly one, retaining the same functions.
The graph fixture is synthetic HIR; it is not claimed as an executable FFI test.

The separate `ffi-callback.dart` fixture calls native `abs` and invokes a Dart
callback through a native function pointer. The callback allocates an interpolated
string. Before/after optimized AOT executables both print `37` and `32`, matching
Dart 3.6.2, with empty stderr and exit zero under normal GC and actual
`DARTFORGE_GC_STRESS=1`. Commands, executable/compiler hashes and explicit stress
settings are in `runtime.json`. Its compilation timings have different object
cache hits and do not establish a performance improvement.

## Checks and application scope

All **966 native library tests**, including ignored tests, passed (225.23 s).
The four directed tests overlap that suite. Strict workspace/all-targets Clippy,
explicit embedded-LLVM backend Clippy and warnings-denied private documentation
passed. The new test file's formatting and `git diff --check` passed. Global
formatting still fails pre-existing workspace drift; the original log digest and
size plus its first 120 normalized lines are retained.

The actual File Explorer source is from clean `dart_ui`
`15aff7042318d2fa56661ffed4360c81a41a86b8`. The cold updated compiler emission
passes with two reachability rounds and produces the baseline's complete
293,593,137-byte LLVM IR, SHA-256
`0e9cff28f0f9e9a04f86aecdccdf66d4fb9c123617e02014e2bd4d5c124e212d`.
Cold SDK preparation is included in its timings and is not compared with warmed
baseline timings. The warm ABBA sequence completed with all four full IR hashes identical:

| Compiler | Wall seconds | HIR pruning seconds |
| --- | --- | --- |
| Before, first | 107.54 | 25.32 |
| After, first | 96.94 | 17.80 |
| After, second | 97.36 | 18.05 |
| Before, second | 109.37 | 26.16 |

Mean wall time was 108.45 versus 97.15 seconds (10.4% lower); mean HIR pruning
was 25.74 versus 17.92 seconds (30.4% lower). These are two observations per
compiler on one macOS ARM64 host with debug-built compiler binaries and warmed
SDK caches, using `--emit-ir --digest --timings`, not complete object/link builds.
Small log/documentation and formatting checks overlapped part of the sequence;
this was not a controlled idle-machine benchmark. The result supports a local
improvement for this invocation, not a general release-compiler speedup or a
cross-platform performance claim. Retained counts are unchanged: 38,484/146,307
functions, 39,874/176,819 pairs and 318 FFI signatures. The updated compiler
reports two reachability rounds in every emission.

`artifact-bindings.json` records large artifacts in ignored target storage;
`source-bindings.json` binds the tested Rust files, application entry and package
configuration. `environment.json`, validation scripts and original/stored log
hashes record the toolchain and evidence boundaries.

This is not a new desktop/JIT execution matrix or full-workspace test approval.
The production foreign-backend absence regression remains unresolved; its
expectations are unchanged. The observed timing improvement is limited to the invocation and environment
described above.
