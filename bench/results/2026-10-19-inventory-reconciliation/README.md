# SDK inventory reconciliation rejection details

Base compiler: `ce4c48aa`, macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.
The existing experimental class proof rejected the real production image with
only `SDK inventory does not match the selected image` (`before.log`).

Reconciliation now returns a reason for the same rejection conditions: selected
module count, repeated module, snapshot mismatch, SDK symbol collision with the
local image/native runtime/LLVM, invalid CID or collision with a local class.
The lexically first rejected symbol is reported independently of hash iteration
order. The caller keeps broad registration on every error; successful partial
checks do not expose partial inventory facts. Allocation-free closure, startup,
table and native exposure requirements are unchanged.

One new test covers each boundary, negative/out-of-range CIDs in table, nominal
and ambiguous records, a non-colliding class control, and deterministic symbol
diagnostics. Existing inventory tests continue using the same acceptance and
rejection expectations. The final wording test passes. The first full native
suite and inventory suite were launched before the final cosmetic wording
change from `nominal class class 0` to `nominal class 0`; their retained logs are
labeled accordingly. No runtime ABI or generated program behavior changed.

The real production test identifies the overlapping SDK nominal CID 0 from
`dart:_internal`. `lower/mod.rs` inserts synthetic `Object` class 0 in each
module and marks zero as a terminal universal parent. This is evidence for the
next identity-contract change, not permission to ignore arbitrary CID collisions.
No condition is relaxed here. Production foreign-backend pruning still fails;
there is no claimed explorer size improvement or complete closed-class proof.

Strict workspace Clippy and private docs pass; global formatting still fails.
No full workspace approval, performance measurement or new Windows/Linux run
is claimed. Logs retain configured LLVM and source-SDK test context; the optional
production proof uses `DARTFORGE_CLOSED_CLASS_REACHABILITY=1` and
`DARTFORGE_TRACE_CLASS_REACHABILITY=1`. Runtime binaries and cross target bundles
are unchanged by this diagnostic-only change.


Results: 836 native unit tests and 10 inventory tests passed before the final
wording cleanup; the final diagnostic test passes against the final source.
The final production test still exits 101, now explicitly reporting
`SDK dart:_internal nominal class 0 overlaps a local class`. No acceptance
expectation of that existing production regression was changed. Text logs
normalize trailing whitespace and final blank lines; original/stored digests
are retained in `log-normalization.json`.
