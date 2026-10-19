# Entry-only nominal subtype startup certificates

Base `c40fe42c`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.

The runtime implementation of dartforge_register_subclass publishes a direct
nominal edge and invalidates host subtype caches. It does not allocate Dart
objects or call Dart helpers. This contract is separate from the generic native
effect catalog, which remains conservative for this API. Runtime artifact and
compiler/source fingerprints remain bound by verified inventory publication.

The new certificate accepts a complete, unique, non-interposable void() module
startup with the exact generated registration symbol. Its only observed external
effect must be ABI-matched register_subclass calls with literal valid CIDs, a
child nominally declared in that module, unambiguous unique ownership and edges
present in the verified summary. Shared zero requires the existing universal
parent certificate. Additional calls, allocations, raw/unknown effects, missing
ancestry, symbol overrides and incomplete premises do not gain authorization.

Admission is entry-only. The startup is not an ordinary pure import, and the
native API is not added to the general covered-runtime set. After reconciling
the selected original summaries, a temporary graph discharges only references
owned by the certified startup. Other users of the same API retain their edges.
Original summaries and executable bodies are preserved. With no certificates,
the inventory lends its verified summaries without cloning them.

Validation includes a complete HIR class-proof acceptance case; rejection of an
ordinary call to the startup and a direct runtime API call; graph ownership and
original-summary preservation; 18 unsupported operand/effect/identity variants;
nominal ownership, ambiguity, missing graph facts and shared-zero premises.
The compiled regression builds broad and certified executables. An observation
oracle at entry completion checks transitive 900 <: 902 and rejects the inverse,
in normal and GC-stress processes. The startup body is the same one certified by
the inventory. The oracle is test instrumentation, not an additional production
registration capability. Initial harness linking used two C mains and failed;
that log is preserved. The final harness uses the runtime-owned main and the
matching runtime CID vector. No failure was reclassified as a pass.

Final results: 45 allocation-inventory tests (including actual production SDK
rebuild/publication/load), 51 class-proof tests (including the compiled subtype
regression and existing selector/closure/Type/empty-event/GC cases) and 14 general
pruning tests passed. Strict all-target workspace Clippy and warnings-denied
private docs pass. Global fmt still fails existing drift; its excerpt and original
hash are retained. Full native/workspace suites, the production-profile assertion
and desktop/VM/JIT matrices were not repeated.

The real SDK test explicitly confirms all of its complete startups remain
unadmitted: they require name, argument, managed-image, table and helper contracts
beyond subtype publication. Inventory schema remains 2; no serialized field or
runtime ABI changed. Complete foreign-backend pruning and executable size or
performance gains are not claimed. This is one registry proof domain, not the
completion of SDK startup certification.

Text logs normalize trailing whitespace and retain original/stored hashes.
No resident process or scheduled task was installed.
