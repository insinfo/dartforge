# Byte literals do not create LLVM symbol edges — 2026-10-20

Base `ef8170f3`. The pruning reference scanner previously interpreted `@name`
inside an LLVM `c"..."` byte literal as a symbol dependency. If that name
belonged to a real definition, ordinary pruning retained uncalled code.

The scanner skips complete byte literals, preserving relocatable addresses
outside them and quoted symbol identifiers. Other quoted text, including opaque
assembler text, keeps its conservative handling. Incomplete byte literals also
keep the conservative scan. Tracking ordinary quoted strings prevents a trailing
`c` inside an assembler string from being confused with a byte-literal prefix.

The class-proof projection now only discharges certified metadata publication
edges. It no longer repairs invented byte-data edges after parsing. Public byte
data remains uncovered; managed returns, raw regions, image registration and
helper protocols gain no new admission rule.

## Regression and SDK observations

Both new regressions failed before the scanner change and pass afterward. One
assembles a complete module: the bytes `@dead` stay intact, the uncalled `dead`
definition leaves, and a real `live` function pointer in the same aggregate
preserves its definition. The other covers escaped byte contents, adjacent
addresses, quoted identifiers, opaque assembly and an incomplete literal.

A standalone observer embeds the exact old/current scanner functions. On 15
preserved SDK LLVM files, references decrease from 450,652 to 450,481 across
171 changed lines. All changed lines contain byte literals and remaining
references preserve their original order. These are reference occurrences,
not 171 removed functions. Inputs and scanner versions are hash-bound.

A separate pair of SDK caches has identical IR digests for all 15 modules.
All 32,713 complete function-observation records match as a multiset, including
body digests, obligations, call arguments and allocation closures. Known
allocation closure coverage stays at 635 within this matched pair. The pair's
configuration differs from other previously reported SDK snapshots; this is
not a new global SDK coverage count. Global facts differ only in five textual
reference lists. After translating name indices, all other serialized summary
facts match, including tables, pairs, selectors, allocation audits and ancestry.
Exactly 171 summary definitions lose false edges: `dart` (166), `Native` (3),
and the upper/lowercase alphabet strings (one each).

The production isolate fixture's actual assembled graph drops four nonexistent
symbols (46,798 to 46,794; live 6,081 to 6,078). Its table/pair selection and
pruned function/global counts stay equal. Production, development and unpruned
assembled IRs remain byte-identical. No executable-size or performance gain is
claimed; recorded timings are functional execution metadata.

## Validation and retained failures

The directed run passes all nine selected tests. The first full native-library
run passed 1,044 tests and failed one older assertion that explicitly required
the invented byte reference to exist in the original summary. That assertion
now requires an empty original reference list and still verifies snapshot
immutability. The failed log is retained. The final complete run passes
**1,045 tests, zero failed/ignored/filtered**. Existing proof-view tests verify
that actual pointer edges and uncovered public data remain conservative.

Workspace/all-target Clippy and embedded-LLVM Clippy pass with warnings denied;
workspace/private documentation passes with warnings denied. Runtime unit,
integration and documentation tests pass. Workspace formatting and diff
whitespace checks pass. The current JIT-enabled CLI is published through a
fresh inode and atomic rename, preserving its predecessor.

The isolate fixture matches Dart 3.6.2 in macOS AOT production/development/
unpruned and JIT, normally and under GC stress. The current compiler also
cross-compiles it for Windows x64 and Linux ARM64. Both destination VMs pass
normal/stress execution, with executable transfers checked by SHA-256 and
empty stderr. Linux console execution uses root. This is not a new explorer
GUI or guest JIT matrix. Both VMs finish suspended. Runtime sources and their
`e0a3c8b0…` binding remain unchanged; existing matching bundles are reused.

The original foreign-backend test still returns **101** because WindowsBackend
bodies remain in the macOS IR. Default and experimental closed-class modes
retain six foreign definitions and byte-identical fallback IR. The startup
image/helper boundary remains unproved. No test expectation, filter or fallback
is changed to turn this failure into a pass; complete workspace approval remains
open. LLVM's existing deployment-version warning is retained in the compiler log.

Complete logs have trailing whitespace/final blank lines normalized; manifest.json
records original and stored digests. The cache-comparison and observation
scripts, source bindings, compiler identity, execution records and failed runs
are retained alongside this report.
