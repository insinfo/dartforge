# Independent byte-edge cleanup and borrowed proof views — 2026-10-20

Base `3c7c9abd`. `Inventory::class_proof_summaries` previously returned the
unmodified summaries whenever no metadata startup was certified. That shortcut
also skipped the independent proof that a verified private byte initializer
contains no symbol addresses. A literal `@opaque` consequently retained a false
legacy summary edge, despite its bytes having been verified. The new regression
failed before the change and passes afterward; the original failure is retained.

The proof view now discharges verified byte-content edges independently of
metadata startup. Real pointer initializers and public/unverified constants keep
their original edges. This does not make the named symbol a covered function or
supply a callable value, object handle or heap-class allocation fact. Only the
entry-owned runtime metadata API edges retain their existing separate certificate.
Ordinary calls to those APIs remain visible and unproved.

The method returns one `Cow<Summary>` per selected module. Each begins by borrowing
the immutable verified snapshot, and clones only when its first actual edge
change is needed. Unchanged modules keep their original storage. The consumer
borrows both owned and unchanged views for the reachability walk; original
summaries remain available and unchanged for reconciliation and other analyses.
No cache publication schema, emitted LLVM, runtime ABI or pruning gate changes.

Four tests cover the missing-startup regression, unchanged storage identity and
repeated use, a mixed two-module snapshot with only one changed module, preserved
ordinary metadata calls, and independently loaded byte/pointer initializers.
In the mixed case, exactly one summary is owned and the other borrowed; the prior
implementation cloned both summaries. This is a demonstrated work reduction for
the proof-view path, not a measured whole-program speedup. Current production
still encounters the unproved first SDK startup before reaching this path.

Global formatting and whole-workspace test approval remain open. Compilers,
executables and IR are retained in ignored target storage; source/log/artifact
hashes bind the retained validation. No new Windows/Linux execution, desktop GUI
matrix, cross-bundle export or executable-size gain is claimed.

## Compiler and execution checks

Strict workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private documentation, the runtime UTF-8/no-Dart-allocation test and the JIT CLI
build passed. The new test file passes scoped rustfmt. The compiler retains the
preexisting warning about LLVM compression objects declaring a newer macOS
minimum; current-host execution is not older-macOS compatibility certification.

The actual platform fixture has byte-identical IR before/after, in both default
and experimental closed-class modes, with six foreign-backend definitions still
present. The first SDK startup remains unproved. The JSON/UTF-8/Unicode/RangeError/
three-isolate/base64 fixture agrees with Dart 3.6.2 under production/development
AOT and JIT macOS, normal and stressed GC. Positive closed-class native tests in
the full library suite exercise the changed proof-view consumer; the unchanged
platform fixture alone does not demonstrate that it reached the proof-view path.
Runtime sources are unchanged and no target bundle was reexported.

The full native library run passed **993/993**, including ignored tests, in
239.35 s. All four new proof-view tests passed and overlap this total. The
pre-change positive regression failed at the false byte-edge assertion; its
original command and log are retained. Source bindings confirm final validation
used the current Rust implementation, and normalized log bindings preserve the
original digests and sizes. No required check was converted from failure to pass.
