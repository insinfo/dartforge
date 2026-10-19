# Checked static string layouts before linked-image authorization

Base `8400994c`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.

Image registration uses linker section bounds. The previous inventory retained
only byte-constant initializers, so it could not inspect the managed objects
whose headers reside in those sections. Schema 4 replaces nullable
native_initializer with required initializer on every global. Loading checks the
original-line digest, symbol and recomputed kind, including unclassified globals.
Old schemas do not supply missing managed initializer evidence.

A new layout observer recognizes individual canonical permanent string
initializers emitted by LlvmEmitter. It checks the immutable linkonce_odr shape,
known string CID, PERMANENT/RAW header and word count, exact array/body sizes,
length arithmetic, zero padding, VM string hash and content-derived canonical
symbol key. UTF-16 units are little endian; Latin-1 content in UTF-16 storage is
not the emitter's canonical representation. Empty, Latin-1, UTF-16, surrogate,
escaped and padded real-emitter fixtures are covered. Altered identity, mutability,
header/hash/length/padding/alignment/section and inconsistent records are refused.

The source spellings for Mach-O, ELF and COFF are recognized as LLVM observations,
not certificates for emitted objects, relocation, comdat merging or linker inputs.
Classification static_string is separate from native_bytes. Neither exposing a
static handle nor register_image becomes authorized. Integration tests explicitly
retain both boundaries and reject re-signed misclassifications after publication
integrity passes. Existing entry-only text/nominal metadata permissions are kept.

The actual production SDK rebuild/publication/load test found 5,240 checked
static string definitions with CID 6, plus 7,753 native byte definitions and 266
unclassified initializers. Among recorded global section contributors, 15 anchors
remain unclassified. This is a census of recorded LLVM globals, not a proof of
all linked section contents; runtime objects, functions placed in the section,
anchors/markers, foreign inputs and full address/value closure still need proof.
The timing diagnostic reports definition observations before linker deduplication.
The complete SDK startups remain unadmitted.

Final verification: 58 inventory tests including actual production SDK loading,
51 class-proof tests including compiled selector/closure/Type/metadata/event and
GC-stress cases, and 14 general pruning tests passed. Strict workspace all-target
Clippy and private docs with warnings denied pass. The initial Clippy rejected
two constant-width chunks_exact uses; the final code uses array chunks and the
complete scoped suites were repeated. Global fmt still fails existing drift.
Full native/workspace suites, production-profile assertion and desktop/VM/JIT
matrices were not repeated.

Comparing schema 3 and 4 snapshots found unchanged final IR for all modules and
unchanged bodies for all 15 SDK registration startups. Cache JSON grew from
67,760,874 to 68,851,974 bytes (about 1.6%) to retain all global definitions. This
is compiler cache metadata, not an executable-size or performance measurement.
Only comparison counters, paths and hashes are committed, not large caches.
No runtime ABI or .poda format changed. Complete image/startup certification and
foreign-backend pruning remain open; no binary size/speed gain is claimed.

Text logs normalize trailing whitespace and retain original/stored digests.
No resident process or scheduled task was installed.
