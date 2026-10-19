# Analyzer diagnostics and dependency-cache validation — 2026-10-19

Baseline: `d63e7ae3`. The analysis crate passes strict Clippy on all targets
without new suppressions. Private field-initialization context, directive
predicates and memo types have names; redundant branches and recursion-only
arguments are removed. Public diagnostic contracts are unchanged.

## Dependency cache

The thread-local dependency cache used an owned `(String, bool)` key even for
hits. It now uses one map per dependency mode and looks up each map with `&str`.
Only insertion copies the source. The combined entry limit retains the old
`> 64` clearing condition. Cached failures remain distinguishable from misses,
and callers still receive owned vectors. Two regressions cover normal/dev
separation, repeated hits, failed inputs, result mutation and eviction.

The allocation probe extracts the actual cache function before and after this
change and compiles both with a counting System allocator. A parser stub only
primes entries; its counter proves no parsing occurs during measured hits. The
valid synthetic YAML and stub results both describe demo/runtime_dep/test_dep.
For 10,000 warm production lookups of the 16,464-byte source, allocation count
fell from 40,000 to 30,000 and allocated bytes from 165,270,000 to 630,000.
Development lookups retain the extra owned result string. Cached None results
fall from 10,000 key allocations to zero for a 16,393-byte source. This measures
allocation traffic in cache hits, not live/peak memory, YAML parsing cost or
whole-analyzer time.

Reproduce from the repository root:

```sh
python3 bench/results/2026-10-19-diagnostics-quality/cache-probe.py
```

## Diagnostic checks

`cargo test --locked -p dartforge-analysis -p dartforge-parity -- --include-ignored`
passed 193 tests, with zero failures and zero ignored. The four analyzer parity
tests now require SDK sources instead of returning successfully when discovery
fails. An explicit DARTFORGE_SDK_LIB override is honored directly by their test
helper, so it cannot silently fall back to another installed SDK. The main
parity test also rejects panics and diagnostics beyond its expected set.

The deliberate invalid-SDK invocation failed with status 101, as expected,
while the configured-SDK suite passed. That negative check is retained as a
failure proving the gate, not included among the 193 successes.

The 89 source cases from analyzer_rules.rs were freshly analyzed by official
Dart 3.6.2. All 417 diagnostics match the test expectations in file, code, line,
column and length. Messages and severity are not compared by this check. Dart
analyze returns 3 because these fixtures intentionally contain errors; the
comparison script succeeds only when the complete diagnostic multiset matches.
It requires a nonempty corpus and expectation set and records source hashes.

```sh
DARTFORGE_DART=/path/to/dart-sdk/bin/dart \
  python3 bench/results/2026-10-19-diagnostics-quality/verify-oracle.py
```

Use DARTFORGE_SDK_LIB for the Rust tests and DARTFORGE_ORACLE_3_6 for the parity
package's SDK root. The tested SDK version is 3.6.2. Commands, logs, source
fingerprints, oracle output and allocation counts are recorded in checks.json.

## Workspace limits

Full workspace formatting and Clippy still fail. This Clippy run reports dev
and emit-native diagnostics; it does not establish that every other crate is
clean. Full tests stop at the existing Apple LLVM 21 linker incompatibility
with static LLVM 22 bitcode. Documentation builds with existing warnings.
Bounded excerpts and original full-log hashes/lengths retain these failures.
Text logs normalize trailing whitespace; original-log hashes precede that
normalization. No global green result or general performance gain is claimed.
