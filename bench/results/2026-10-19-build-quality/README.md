# Build engine validation — 2026-10-19

The SCC traversal now moves its suspended edge cursor rather than cloning the
entire adjacency vector when visiting each new child. Private contexts collect
invalidation events and application-planning dependencies. No new Clippy
suppression was added.

Correctness is checked against 192 results from Dart `graphs` 2.3.2, including
component and node order, and a 50,000-node chain. Regenerate the oracle with:

```sh
dart --packages=corpus/builders/cadeia_configuracao/.dart_tool/package_config.json \
  crates/build/tests/scc_oracle.dart crates/build/tests/scc_graphs_2_3_2.json
```

Use Dart 3.6.2 and the locked dependencies. The fixture input was generated with
Python Random(10419), eight cases for each node count 1 through 24. For each
node, the outgoing count is `randrange(min(n+2,10))` and targets are
`randrange(n)`. Roots are `randrange(n)` repeated `randrange(n+2)` times.

The benchmark script extracts the SCC function from baseline `ee6e289a` and the
working source, compiles both in one Rust executable with `rustc -O`, asserts
identical results, and alternates execution order over nine samples per shape
and size. Run from the repository root:

```sh
python3 bench/results/2026-10-19-build-quality/scc-benchmark.py
```

`csv` times include traversal, input adjacency clones requested by the callback,
result construction and destruction. They measure synthetic SCC traversals,
not whole builds, and were taken on a shared macOS ARM64 development machine
while integration checks were running. They are not isolated performance gates.
The star case with 10,000 nodes had medians 22.723 ms before and 0.810 ms after.
Chain and cycle cases showed much smaller differences; see all paired samples.
No total-memory improvement is claimed.

The native executor check uses the unchanged frozen producer
`target/analysis-quality-producer/dartforge` from the preceding validation;
its hash and limited case selection are recorded in checks.json. Current build
engine code controls the run. This is not a rebuild of the compiler for all
platforms. Earlier desktop AOT/JIT and true cross-compilation evidence remains
in `../2026-10-19-cross-desktop/`.

Workspace failures are retained as failures: global formatting differs, strict
Clippy reports diagnostics in analysis, world and emit-js, and the default Apple linker cannot
read LLVM 22 bitcode. Documentation builds with existing warnings. Large
formatting/linker logs have bounded excerpts with full-log hashes and lengths.
No check was weakened or marked successful because of those environment issues.

Text logs normalize trailing whitespace. Full-log hashes in workspace-failures.json
refer to the original command output before that normalization.
