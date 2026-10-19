# Invalidate class certificates before constructing broad fallback roots

Base `84d88c27`; macOS ARM64, LLVM 22.1.8.

The regression first computes a successful class proof on complete HIR. It then
invalidates either fixed ancestry or root coverage, and compares public
`prune_hir` against a fresh, uncertified copy of that same HIR. Before the fix,
the ancestry variant retained only Used methods while the fresh copy retained
Used, Unused and Nested protocol methods. The baseline stops at that first
failure; it does not claim to have executed the missing-root variant.

The broad graph was constructed while `closed_class_tables` still held the old
active set. Resetting the set later inside `narrow` could not repair that graph
when the new audit failed or its option was disabled. `prune_hir` now clears the
old certificate before its initial summary and FFI reachability fixpoint. Only a
successful new audit may replace the broad result with a narrower graph.

The final regression exercises both invalidation reasons with the experimental
option on and off. It compares surviving function symbols with the fresh copy
and verifies preservation of every class toString body. This tests invalidation
before destructive pruning, not reconstruction of bodies already removed.
Callers changing semantics after destructive pruning still need complete HIR.
No runtime ABI or allocation/registry authorization policy changed.

Validation: 48 class-proof tests with the option enabled, the new regression
with it disabled, 14 general pruning tests and 3 pre-prune optimization tests
passed. Included compiled class-proof regressions exercise selector dispatch,
closures, runtime Type and empty event domains, including GC stress. Workspace
strict all-target Clippy and private documentation with warnings denied pass.
Global formatting still fails existing drift; its excerpt and original digest
are retained. The full native/workspace suites, production profile assertion
and desktop/VM matrix were not repeated. SDK startup certification, complete
foreign-backend pruning and measured size/performance improvement remain open.

Text test logs normalize trailing whitespace; original and stored hashes are
recorded. No resident process or recurring task was installed.
