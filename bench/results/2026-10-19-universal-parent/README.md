# Shared terminal subtype parent certification

Base: `6c577488`, macOS ARM64, LLVM 22.1.8 and Dart source SDK 3.6.2.
The real production proof previously stopped at the overlapping SDK nominal CID
zero (`before.log`). Lowering inserts that synthetic terminal parent in every
module; it is distinct from heap class identity.

A shared declaration is now explicit: valid HIR emits `; df.universal-parent 0`;
the version 9 pruning summary preserves `U\t0`. The summary reader rejects older
versions rather than consuming caches without the new contract. Updated version
rejection tests still reject the immediately previous version. The compiler SDK
code fingerprint includes the changed source and invalidates its cached objects.
Runtime source, ABI exports and Cargo.lock are unchanged.

The local certificate requires one synthetic Object declaration, complete nominal
and metadata-only premises, no ambiguity, fields, method/table function, physical
layout, program identity, runtime CID, own parent or explicit HIR allocation of
zero. Summary certificates require exactly one nominal zero and no contradictory
tables, layouts, ambiguous CID, parent edges or known zero allocations. Duplicate,
wrong-value and malformed U records fail decoding.

Inventory reconciliation shares only nominal zero when both declarations are
certified. Table, ambiguous and other CID collisions keep their original checks.
The marker grants no allocation, field layout, native call, startup or callback
permission. The integrated proof test admits matching roots and keeps the used
class table; missing SDK/local premises and explicit zero allocations fail.
Actual source lowering and IR/HIR summary round trips are also checked.

The first directed/pruning run and production observation used the original
vector-based declaration check; the final helper uses an iterator with the same
conditions and tests three additional negative declaration mutations. Final
unit tests, compiler build and live programs use that final implementation.

The production observation advanced past inventory reconciliation and now rejects
`df.call_dart0` as an unproved entry body/input ABI. The event-loop callback still
receives an external Ref; its domain needs a separate proof. No callback rule was
relaxed, the production test remains failing, and no complete foreign-backend
pruning or explorer size reduction is claimed. Global fmt and full workspace
approval remain pending; no new Windows/Linux validation is included. CLI build
logs retain existing macOS deployment-target linker warnings.


Final results: 839 native unit tests passed. The initial directed/pruning run
passed 142 tests. Strict workspace Clippy and private docs pass. The rebuilt CLI
passed 11 real-program commands: one official Dart reference, two optimized AOT
builds and eight executions (tracing/ARC, AOT/JIT, normal/GC stress). All eight
outputs exactly match the reference for Type equality, canonical identity and
runtimeType behavior. Compiler hash, commands, stream hashes and exit statuses
are retained in `live-results.json`. Final production still exits 101 at the
unproved callback entry, preserving the original assertion on foreign backends.
Text logs normalize trailing whitespace and final blank lines; raw program
stdout/stderr are unchanged and original/stored log digests are recorded.
