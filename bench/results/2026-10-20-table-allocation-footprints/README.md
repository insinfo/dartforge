# Finite allocation footprints for constant table callbacks — 2026-10-20

Base `44bccfc9`. The allocation inventory can now describe calls to
`dartforge_object_new_t` and `dartforge_object_new_mt` when their callback body
and immutable table are fully present and verified together. This is an
allocation-only witness, not a new closed-class permission or a pure-import
certificate.

The accepted getter has a unique public `df.mt.*` definition with the emitter's
exact constant-return body, optionally `nounwind`. It returns its own uniquely
defined private immutable table. The complete storage has two i64 header words,
a bounded nonnegative CID, a pair count matching every field, and strictly
ordered i64 selector hashes followed by ordinary symbol pointers. The entry pointers are not invoked by registration, and this witness does not
certify that they are callable. Empty tables are valid. Mutable/incomplete storage, other callback bodies, aliases,
duplicate function/data definitions and symbol overlap do not supply witnesses.

The allocator call must have its canonical ABI, literal matching CID and
nonnegative field count. `_mt` additionally requires a literal 32-bit reference
map. The runtime contract is `selectors.rs`: registration copies pairs without
invoking their entries, and allocation uses the supplied CID. The getter itself
has an empty Dart allocation footprint; each allocator and its direct wrappers
have the singleton CID footprint. Method-address and registry exposure remain
explicit. The inventory consumer refuses ordinary scalar/Ref/ptr calls, entry
certification and covered-node status for these functions. No callback-input,
object-layout, escaped-reference or later method-invocation proof is implied.

The original positive regression failed with an unknown footprint rather than
`[900]`. Six new tests cover both allocator ABIs and transitive wrappers, opaque
callbacks/invalid operands, changed bodies/storage/linkage/ownership, complete
ordered pairs, actual emitter output with/without nounwind, and consumer refusal
of all inappropriate capabilities. The first actual-emitter fixture initially
omitted SDK-library mode and did not emit any tables; that failed setup is
retained, then corrected. The first strict Clippy pass found three style issues;
they were corrected without allowances and the full pipeline rerun.

## Production boundary remains open

The actual production SDK inventory has **32,708 functions and zero method-table
getter definitions**. Its getters are deliberately assembled later by
`pruning::assemble` from selected table summaries. A declaration identity alone
cannot be substituted for a checked callback body. Therefore the new local-body
witness does not discharge the current SDK startup, image-registration or helper
boundaries. Raw allocation fast paths, stack-overflow callbacks and invocation
inputs remain separate obligations. Source snapshots without getter definitions
use a fast empty-index path.

This is preparation for that complete proof, not a claim that foreign backends
have been removed. The actual platform reproduction must continue to retain its
six foreign definitions until the complete image and callback coverage exists.
No generated code, runtime source, runtime ABI or persisted schema changes. The
collector's source fingerprint invalidates earlier SDK inventories automatically.
No whole-program performance or executable-size gain is claimed, and no new
Windows/Linux or full File Explorer GUI matrix is part of this change.


## Final verification and retained evidence

All six commands in `final/checks.json` passed: directed tests, strict
workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private documentation, JIT CLI build and native library tests with ignored tests
enabled. The native run passed **1,002/1,002** in 227.11 s; the six directed tests
overlap this total. New Rust files pass scoped rustfmt. The global formatting
check still fails; its full output digest/size and a normalized excerpt are
retained, without claiming whole-workspace approval.

The nine-class/two-isolate fixture agrees with Dart 3.6.2 in production and
development AOT plus JIT macOS, each with normal/stressed GC. Final commands,
outputs and compiler hashes are in `runtime-final/after-runtime.json`. These are
functional checks, not speed measurements. The previous committed fixture run
supplies the pre-change behavior; `runtime/` additionally retains the first
round before the quoted-symbol uniqueness refinement. Some intermediate output
paths were reused; final retained files are bound in `artifact-bindings.json`.
The earlier compiler is separately preserved as `dartforge-table-footprints-preliminary`.

The actual platform IR is byte-identical to the preceding committed compiler
in both default and experimental closed-class modes, each retaining six foreign
backend definitions. The unmodified integration gate
`platform_constant_prunes_the_other_backends` was executed again and **failed**
(exit 101); its command/log are retained. This known failure was not converted
into a pass or removed from the suite.

The new production SDK observation again has 32,708 functions and no getter
bodies. The retained IndexError helper facts match the earlier observation and
still include unknown raw-memory, stack-overflow and table-callback obligations.
This is a comparison of those selected facts, not of the entire inventories.
The earlier cache entry was garbage-collected during rebuilding; its original
hash and extracted observations were already saved. The complete updated
inventory is preserved in ignored target storage with its SHA-256 binding.
No old inventory bytes were reconstructed or substituted for the missing file.

`source-bindings-final.json` binds all five changed Rust files. Normalized
retained logs have original/stored sizes and SHA-256 values in
`log-bindings.json`. Failed baseline/setup/Clippy runs remain alongside final
results. LLVM compression-object warnings about a newer macOS minimum remain
in the compiler log; current-host execution is not an older-macOS guarantee.
