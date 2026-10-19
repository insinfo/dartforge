# Source image section census and explicit marker protocols

Base `d16e74c8`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.

Schema 5 adds image_census to each module, bound to its final IR digest. The
scanner visits all source lines, distinguishes quoted symbols/strings/comments,
records image-section globals and functions, and retains open obligations for
unreviewed assembly/attributes, multiline constructs, marker overrides, missing
or duplicate roles and mixed formats. It recognizes canonical pointer-free
string definitions and exact zero anchors. The explicit-format marker producer
is shared with target::image_markers; host state is not used to inspect the three
protocol spellings. The global portion is replayed against stored initializers.

A clean source census is an observation within the reviewed lexical subset,
not proof of LLVM grammar validity or complete linked contents. Function/body
obligations remain compiler-owned observations tied to final IR and immutable
publication; bodies are not duplicated for independent replay. Object section
layout, relocation, comdat merging, runtime archives, foreign inputs and linker
options still require independent coverage. No image, static handle or Ref/Ptr
return is authorized by this new state.

The initial real SDK census reported one unreviewed assembly instruction in
each of 15 modules. It is the producer's exact macOS ARM64 TSD register read:
`mrs $0, TPIDRRO_EL0`, with its fixed i64 output, constraints and memory attributes.
This instruction does not switch sections. Its source line is now a constant
shared with the TSD producer. The existing exact no_dead_strip stackmap keep
line is also reviewed. Other assembly stays open. Tests alter the assembly,
constraints and sideeffect flag and confirm rejection; the raw memory/context
helper still has unknown allocation/exposure closure. This is not permission to
call that helper or manufacture a packet pointer.

Final tests cover all three marker protocols, functions/foreign data in image
sections, source continuations, unreviewed assembly and attributes, encoded
marker aliases, missing/duplicate roles, format mixing and quoted decoys. Re-signed
module/global census mismatches are refused after publication integrity passes.
An existing integration test now confirms that even a clean source image does
not authorize register_image or a managed static return.

64 inventory tests including actual SDK rebuild/publication/loading, 51 class
proof tests including compiled normal/GC-stress scenarios, and 14 general pruning
tests passed. Strict workspace all-target Clippy and warnings-denied private docs
pass. Optional context lib/doctest filters selected zero tests and are not counted
as coverage. Global formatting still fails existing drift. Full native/workspace
suites, production-profile assertion and desktop/VM/JIT matrices were not repeated.

All 15 final SDK modules have no opaque source contribution in this subset:
5,240 string definitions plus 15 anchors, with 45 marker declaration lines.
Their complete startup functions remain unadmitted. Final module IR digests and
all 15 startup body digests match the prior schema-4 snapshot: the TSD producer's
emitted instruction is unchanged. Cache JSON grew from 68,851,974 to 69,945,429
bytes (about 1.6%). This is compiler metadata cost, not executable size or speed.
Large inventories remain in ignored caches; only counters, paths and hashes are
committed. Runtime ABI and .poda format did not change. Complete linked-image/
startup certification and backend pruning remain pending.

Text logs normalize trailing whitespace and preserve original/stored digests.
No resident process or scheduled task was installed.
