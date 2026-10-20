# Typed local stack subobjects in allocation observations — 2026-10-20

Base `f711ef5b`. Raw stores previously kept every destination allocation-unknown,
including exact local argument buffers and GC frame slots from alloca. The
producer now recognizes bounded typed subobjects of local stack allocations.
The reference contracts are LLVM's [alloca](https://llvm.org/docs/LangRef.html#alloca-instruction)
and [GEP](https://llvm.org/docs/GetElementPtr.html) semantics. This does not
change the separate HIR stack-slot/class-storage proof or its acceptance rules.

The witness follows scalar/array/struct types and constant nonnegative i32/i64
indices. A region retains its element type and remaining sibling count; stores
must have the exact scalar element type. Full aggregate GEPs, chains and direct
array-to-element GEPs keep the address within the tracked allocation/subobject.
No byte offset, pointer width or target structure padding is guessed. Unrelated
layout reinterpretation, pointer loads/casts, dynamic counts/indices, negative
indices, one-past stores, atomics, metadata and unsupported forms stay unproved.

SSA names are normalized across quoted/escaped spellings. Duplicate definitions,
including collisions with named formals, invalidate the witness. Unnamed formal
arguments are unsupported by this witness. Limits are 8,192 body lines, 65,536
tokens, 1,024 tokens per instruction, type depth 16, 64 fields per structure and
1,048,576 abstract storage units. Exhaustion grants no local-store facts. The
pass analyzes compiler-emitted LLVM; it is not an independent full CFG/SSA or
lifetime verifier. It proves neither initialized words nor valid Ref contents.

Complete writes within these regions cannot be Dart heap allocation state.
Exposure remains separate: alloca now also explicitly records raw pointer/memory
exposure, including bodies without a later store. This fixes an omitted exposure
on a returned bare stack address. The consumer continues refusing stack-address
functions as entries/imports/nodes. Ordinary calls and finite allocator CIDs
remain in the allocation closure; opaque effects are not ignored.

## Regressions

Two positive regressions failed before implementation: typed local writes were
unknown, and bare alloca omitted pointer exposure. Five new tests cover valid
subobjects/SSA spellings, invalid bounds/types/unknown destinations/hidden effects,
finite callee allocations, budget refusal, bare allocation exposure and consumer
refusal. The directed filter also includes the existing independent HIR storage
test (six directed tests total); those counts overlap the full suite.
Clang 22.1.8 accepted all six positive examples as real object compilations.
An initial observation JSON had a literal trailing backslash-n; the original is
retained separately alongside its corrected parseable record.

## Final verification

All **1,043 native library tests**, including ignored tests, passed in 286.60 s.
Strict workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private workspace documentation and the JIT CLI build passed. Scoped rustfmt and
git diff checks passed. Global formatting remains open; its output size/digest
and excerpt are retained. Source bindings identify five final Rust files.

The two-isolate fixture matches Dart 3.6.2 in macOS production, development and
unpruned AOT plus CLI JIT, normally and under actual GC stress. All three complete
assembled IRs are byte-identical to the retained baseline. Baseline reuse and
artifacts are bound to paths/hashes. Runtime sources, bundles, schema and ABI are
unchanged. The LLVM compression-object deployment warning is preserved.

## Actual SDK observations and limits

The SDK comparison retains 32,708 identical function identities/body digests,
module records and global initializers. Calls, dependencies and direct classes
are unchanged. No function's final observation record changed: **zero** complete
raw-memory obligations disappeared, and known allocation closures remain **649**.
There are other unproved destinations in the same bodies, so the per-function
unknown flag remains. This does not yet remove foreign backends or demonstrate
a real application size/performance gain.

A standalone observation harness runs the same stack witness on the actual
hashed SDK LLVM modules, reusing the actual tokenizer, symbol decoder and header
parser. It analyzes 32,693 parseable multiline definitions, including 24,047
with alloca, and recognizes **200,544 local stores** and **88,310 local GEPs**.
These are instruction-level observations, not removed function obligations.
The extracted helper source, build command, input hashes and per-module counts
are retained. The harness adds no alternative storage algorithm or authority.

The unchanged foreign-backend absence gate was rerun and still fails (exit 101).
Default and experimental closed-class reproductions both retain six foreign
definitions. Full-workspace approval and global formatting remain open. Both VMs
remained suspended; this is not a new guest AOT/JIT or explorer GUI matrix.
