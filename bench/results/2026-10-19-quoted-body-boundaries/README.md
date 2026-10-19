# Quoted LLVM header delimiters must not hide body effects

Base `410250f4`; macOS ARM64, LLVM/Clang 22.1.8.

While inspecting the SDK registry boundary, a separate inventory soundness bug
was reproduced: the body extractor used the first literal `{` and last literal
`}` in a definition header. A quoted parameter named `{ ret i64 0 }` made the
collector audit that text instead of the actual multiline allocating body. A
quoted string attribute can create the same ambiguity. The first collector
regression observed an empty class set instead of CID 399. A second regression
published and loaded an artifact-bound test inventory, then observed incorrect
acceptance of this allocator as an empty scalar import. Both failed before the
fix. The collector baseline stops at the first parameter-name case; it does not
claim to have executed all final variants before the fix.

The final body scanner locates braces outside quoted identifiers/attributes and
strips comments only outside quotes. It recognizes a multiline closing brace
followed by a comment. Tests cover parameter and attribute braces, semicolons,
hex-escaped quotes, inline returns referencing quoted names and the boundary of
the following function. Allocated CID 399 propagates to the wrapper; the verified
import consumer now refuses the allocating function as an empty closure.

All five retained LLVM fixture files compiled to objects using Clang 22. The
commands, results and object hashes are in llvm-validation.json. Its target
triple override warnings are retained. These are valid LLVM counterexamples,
not a demonstrated Dart application crash. Import publication tests use synthetic
object/runtime artifact bytes to exercise metadata integrity and admission; they
are not executable ABI tests. The separate real production SDK publication/load
test passed and still refuses opaque registry startups.

Final verification: 41 inventory tests (including real SDK rebuilding/loading),
48 class-proof tests including compiled selector/closure/Type and GC-stress
regressions, strict workspace all-target Clippy and warnings-denied private docs
passed. Global fmt still fails existing drift. Full native/workspace suites,
production profile assertion and the desktop/VM matrix were not repeated.

Inventory schema remains 2: its record format did not change. The collector
source participates in the compiler fingerprint, invalidating old SDK caches.
No new SDK registry/image/helper authorization was granted. Full backend pruning
and measured executable size/performance gains remain unproved.

Logs normalize trailing whitespace and retain original/stored digests. Objects
stay in /tmp; their LLVM sources and hashes are retained here. No resident task
was installed.
