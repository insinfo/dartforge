# Dense dominators for structural provenance — 2026-10-19

A two-second sample of the native builder compiler found the active worker
inside `receiver_provenance::Shape::new`, cloning and intersecting BTreeSets.
The original workspace builder run continues with its preserved old compiler;
it was not restarted or replaced for this optimization.

The structural witness now represents dominator sets with eight u64 words for
its existing 512-block budget. Entry handling, reachable predecessor filtering,
the fixed-point equation, SSA/Phi checks and conservative rejection budgets are
unchanged. Intersections no longer allocate trees. This does not authorize new
SDK/class pruning, widen the accepted HIR subset, or change runtime layout.

## Correctness and representation cost

Four new tests cover:

- All 4,165 directed graphs of one through four blocks without incoming entry
  edges. An independent oracle removes each candidate dominator and checks
  whether paths from the entry still reach the target.
- The full 512-block bitmap, reverse block order, cycles, duplicate edges and
  unreachable predecessors, checked with the same independent path oracle.
- Acceptance at the structural budget and conservative rejection above it.
- A controlled comparison with the previous implementation, checking every
  dominance pair as well as recording elapsed time.

The final isolated measurement uses a 256-block reverse chain, a pathological
order for the existing fixed-point traversal. The old sets took 6,968,352,583 ns
and the dense sets 22,768,667 ns in this debug test. This is a representation
microbenchmark, not an application runtime or whole-compiler speedup claim.
The earlier measurement and final measurement are both retained.

`native-lib.log` records **920 passed, zero failed**, including ignored tests.
`final-directed-tests.log` records **38 passed, zero failed**, including the four
new tests and real SDK receiver/canonical-cache cases. These totals overlap.
`results.json` records strict workspace/all-targets Clippy, explicit embedded-LLVM
backend Clippy, warnings-denied private documentation, and native CLI build all
passing. LLVM 22.1.8, SDK macOS 26.5 and deployment target 15.0 were selected;
`run-checks.py` records the environment and exact commands.

## Compiler output and execution

`branches.dart` is a generated branch-heavy program used as an output guard.
Both compiler snapshots emitted LLVM IR and compiled the program to a native
executable. Both executables printed `-86488`, matching Dart 3.6.2, normally
and with `DARTFORGE_GC_STRESS=1`. Commands, exits and output are in
`execution.json`; the initial compiler and updated compiler hashes are recorded
in their emission JSON records. This is a macOS AOT check, not a new desktop or
JIT matrix and not a benchmark of the entire compilation pipeline.

The initial IR byte comparison **failed**. The cached reference included one
extra `Type` method-table entry/declaration. Repeating both commands reproduced
their respective outputs. The updated compiler's first invocation reconstructed
SDK summaries; its 47-second run cannot be compared to the cached reference's
three-second run. The SDK reconstruction sample and all initial records are
retained, including `comparison.json`, `initial-comparison-check.json` and the
exact IR difference.

To distinguish the optimization from external cache inputs, the **same old
compiler** was then run with a separate empty `DARTFORGE_NATIVE_CACHE`. Its
reconstructed SDK produced IR **byte-identical** to the updated compiler:
2,349,567 bytes, SHA-256
`10c77f9a1e938e5be15521bf4550dfdb8d8ecf274270a09dda9ec159d2635399`.
No SDK compatibility manifest or cache key was rewritten. The old cached and
old reconstructed SDKs both use key `5de6da02057aa61971140a41b1bc5bba`, but have
different summary bytes. The cause of that pre-existing cache reproducibility
observation remains **unresolved**. It is not counted as a successful initial
comparison or as a size/performance result.

`cache-snapshots.json` binds preserved summary copies under ignored `target/`
storage. `core-summary-counts.json` records their hashes and record counts.
Full IR, object files, SDK caches and executable files remain in `target/`;
the retained commands, source, hashes and IR diff describe the comparison.
`validate-emission.py`, `validate-execution.py` and `emission-environments.json`
record the local reproduction steps and cache override.

## Remaining scope

The original broad workspace run is still executing native builder cases with
the unchanged old compiler snapshot. It has not certified the workspace or this
commit. Global formatting drift and the full test gate remain pending. The cache
reproducibility finding needs a separate investigation; class/SDK startup closure
coverage remains incomplete. No assertions or failure expectations were weakened.

`retention.json` records original/stored log hashes with trailing whitespace
normalized. `source-manifest.json` binds the changed source/documents and
`SHA256SUMS` binds retained evidence.
