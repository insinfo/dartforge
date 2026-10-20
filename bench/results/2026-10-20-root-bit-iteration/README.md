# GC root bit iteration — 2026-10-20

Base `cc71a2b5`. Sampling warm LLVM IR emission for the actual dart_ui file
explorer identified GC root analysis, including the private bit-set iterator,
in the call stacks. The retained late sample is diagnostic evidence, not a
before/after timing comparison. Its logs and sample are stored losslessly as
gzip files with original and stored SHA-256 bindings.

The iterator now finds the lowest set bit and clears it before continuing,
instead of testing all 64 positions of every word. Empty words return
immediately. Ascending indices, liveness, interference, GC eligibility and
slot coloring are unchanged. There is no runtime or public API change.
The added equivalence test compares iterator output with membership across
word boundaries, empty inputs, single bits, dense and alternating patterns,
and 256 deterministic pseudo-random word vectors.

## Complete explorer emission measurement

Both preserved compilers are debug CLI builds with JIT enabled. The input is
the actual file_explorer.dart application, with Dart 3.6.2 source SDK,
`compile-native --optimize --emit-ir --timings`, on macOS ARM64. Separate
warmups precede eight measured runs in ABBAABBA order. Both VMs remain
suspended throughout; no sampler or validation build runs during measurement.
Compiler identities, exact commands, source URI/digest pairs, per-process
resource usage, logs and all ten results are retained under `benchmark/`.

| Median of four measured runs per version | Before | After |
| --- | ---: | ---: |
| Complete command wall time | 99.62749 s | 95.57189 s |
| LLVM IR emission stage | 30.43604 s | 26.54618 s |

Wall time decreases by **4.1%**; the LLVM stage decreases by **12.8%**.
Before wall times range from 99.42315 to 99.79130 s; after wall times from
95.42047 to 95.78070 s. Every run has the same 1,082 source input bindings
and the same 293,412,654-byte IR, SHA-256
`f655bd72fb69227c5d6e153c7ef0ea2c3d309dc07d6b77881ed54f7d1f132b36`.

These numbers cover warm IR emission by debug compilers. They do not measure
a release compiler, cold SDK compilation, Clang/link time, application
execution, guest performance or executable size.

## Isolated tradeoff

The release-optimized iterator-only checksum harness improves empty/sparse
words, but **all-set words become approximately 3.09 times slower** in that
particular reduction. Both outputs match. This is retained in
`iterator-runs.json` and `iterator-summary.json`; the checksum reduction is
not the complete compiler workload. The change is not a universal speedup
claim. A separate prefix-specialization prototype improves its all-set case
but still regresses nearly dense words; it was not adopted. Its source and
measurements are retained as a rejected experiment.

## Validation and outstanding scope

All 1,046 native-library tests pass, including ignored tests. Workspace and
embedded-LLVM Clippy pass with warnings denied, as do private documentation
and the full runtime unit/integration/doctest suites. Formatting and diff
whitespace checks pass. Logs and commands are retained under `checks/`.

The table-batch fixture matches Dart 3.6.2 in macOS AOT production,
development and unpruned modes, and JIT, normally and under GC stress.
All three assembled AOT IRs remain byte-identical to the previous compiler.
`runtime/` retains commands, compiler/executable hashes, stdout/stderr and
IR comparisons. Both VMs remain suspended; this change does not refresh
cross-runtime bundles or execute guest tests.

The original foreign-backend production test still exits **101**, because
WindowsBackend bodies remain in macOS IR. Its assertion is unchanged; this
optimization does not solve class-proof coverage or complete workspace
approval. No new guest GUI or guest JIT validation is claimed. Existing
LLVM deployment-version warnings remain in the build log.

Text evidence has trailing whitespace and final blank lines normalized.
`manifest.json` records both original and stored hashes.
