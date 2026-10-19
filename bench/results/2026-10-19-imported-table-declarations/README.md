# Verified imported table declarations — 2026-10-19

The experimental closed-class proof previously required every table function
referenced by the program to have a local method table. Lowering also declares
SDK table functions, whose virtual definitions and data are assembled from the
selected SDK summaries. The declaration check now recognizes those separately.

## Certificate boundary

The inventory builds an indexed symbol/CID map from its verified summary bytes.
Acceptance requires a unique external declaration and virtual function definition,
exactly one reference to its matching `$d` table node and no selector calls.
The data node and CID must be unique across the selected summaries; a function/data
namespace collision or an ordinary definition of the table data is rejected.
Module-local names are excluded. Indexing is linear in definitions/tables rather
than scanning all definitions for every imported declaration.

The class proof uses these identities **only after** inventory reconciliation with
the actual graph inputs and local symbol/class universe. An absent SDK graph,
missing/stale inventory or local collision still rejects. This admits an unused
imported declaration; it does not classify that function as a safe call or covered
reachable node and does not permit allocating its class. Independent body,
allocation, input, protocol and startup checks remain in force.

## Evidence

- `before.log`: the new positive regression fails against production code at
  648a5594 before the declaration certificate was implemented.
- `directed.log`: **28 class-proof tests passed** with final sources. The positive
  graph fixture retains the used local class while accepting an unused verified
  SDK declaration. Eight negative cases cover wrong CID/name, missing or changed
  inventory inputs, calling/allocating the imported class, local symbol collision
  and an absent selected SDK graph. The existing executable fixtures still compare
  eager/closed registration with ordinary/stress GC.
- `inventory.log`: **9 tests passed**, including the new declaration identity test
  with ten malformed/duplicate variants and cross-summary duplication. The actual
  Dart 3.6.2 SDK yielded **662 verified table declarations**, 32,676 unique function
  records and 428 reviewed empty closures. Every accepted declaration remained
  unauthorized as a call/reachable node. SDK startup remains unproved.
- The synthetic graph fixtures use explicitly nonexecutable artifact bytes. They
  validate graph and cache binding, not execution of imported SDK code.
- `native-before-final-guard-order.log`: **826 native unit tests passed** including
  ignored tests. This run preceded moving the declaration check after reconciliation
  and adding the missing-graph negative case/actual SDK declaration assertions.
  Final class and inventory runs above validate those refinements.
- `checks.json`, `clippy.log`, `doc.log`: final strict all-target workspace Clippy
  and private documentation with warnings denied pass; global formatting remains
  failing from existing drift. No new complete workspace or Windows/Linux pass is
  claimed.
- `production.log`: the unchanged real SDK platform-pruning requirement still
  **fails** (Cargo exit 101). With the final check order, the first observed
  rejection is `class Object (0) has no method table`. Nominal metadata versus
  allocatable class coverage is a remaining boundary. This rejection occurs before
  the imported-declaration check in that real program; the actual SDK identities
  are independently verified by `inventory.log`. Other boundaries may remain.

No reduction of the File Explorer binary is claimed. The feature remains opt-in
with `DARTFORGE_CLOSED_CLASS_REACHABILITY=1`; the production run also uses
`DARTFORGE_TRACE_CLASS_REACHABILITY=1`. Runs used macOS ARM64, LLVM 22.1.8 and
Dart 3.6.2 sources. Timings are correctness metadata, not controlled performance
measurements. Logs only normalize trailing blank lines; receipts preserve original
and stored SHA-256 hashes. `sources.json` binds the final proof/test sources.
