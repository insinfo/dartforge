# Classic Mach-O dyld relocation spans — 2026-10-20

Investigation started at `05a1ce7b`, whose production reproduction still retains
foreign platform method tables. Its first rejected SDK root publishes nominal
metadata, the image range, and three helper callbacks. The existing string reader
validates file bytes; that alone does not prove that declared loader fixups leave
those bytes intact. This increment audits the classic Mach-O opcode part of that
boundary. It does not authorize SDK startup or enable class pruning.

## Implementation and protocol references

The new public `linked_images::macho_relocations::inspect` API decodes rebase,
bind, weak-bind and lazy-bind tables from `LC_DYLD_INFO[_ONLY]`. It checks complete
load-command bounds/counts, disjoint virtual/file-backed segment ranges, exact
managed-section address/file mapping and uniquely mapped LINKEDIT streams.
Relocation widths cover pointer and two text-word forms, including writes that
start outside the managed section and cross its boundary. Counts cover complete
streams while overlap examples are capped at sixteen per table.

Repeated writes are counted as arithmetic progressions, not expanded into a
vector or loop proportional to the encoded count. A tiny regression encodes one
billion writes. An independent enumerating oracle checks 19,200 small combinations
of type, start, count and stride. Checked arithmetic and bounded LEB decoding
reject overflows, truncated operands and writes outside a declared segment.
Lazy DONE does not terminate the whole lazy table: later records are audited.

The implementation follows the opcode declarations in the installed Apple SDK
26.5 `mach-o/loader.h` (SHA-256
`5862aeb7fbf183453080bc887c0d502fe25a86ff5bd81291bbcc408bd7fe6e74`)
and the state/stride semantics of Apple's
[BindOpcodes](https://github.com/apple-oss-distributions/dyld/blob/main/mach_o/BindOpcodes.cpp)
and [RebaseOpcodes](https://github.com/apple-oss-distributions/dyld/blob/main/mach_o/RebaseOpcodes.cpp).
No external source files were copied into the repository.

Malformed/unknown opcodes, threaded binding, chained fixups, legacy section or
dynamic relocation tables, overlapping opcode tables and ambiguous mappings
return errors. Chained fixups remain a separate unimplemented audit, not an empty
write set. Other load commands, initialization section presence, and managed
segment protection declarations remain explicit observations.

`inspect_linked_images` now includes this audit for Mach-O, preserving its ELF
relocation report and the explicit `loader_memory_verified: false` result.
It reads the files; it does not execute them.

## Tests and observations

Eleven new tests include synthetic linked images with a real permanent string,
a mutation whose on-disk string still passes the original reader while a rebase
writes into it, every classic run/address opcode, all four streams, a cross-border
write, later lazy records, missing state, overlong/truncated LEB operands, aliasing,
legacy relocations, huge repeat counts and invalid section/segment mappings.
Fresh ARM64 and x64 images are compiled/linked with real rebases/binds; both have
zero overlapping writes. Fresh chained variants are explicitly refused. These
are foreign-format reads on macOS, not execution of those fixtures.

The previously executed macOS File Explorer executable remains byte-identical to
its earlier desktop publication. Its declared tables contain 51,472 rebases,
66 ordinary binds and 168 lazy binds: **51,706 potential writes**, none overlapping
its **850,904-byte** managed string section. No weak binds are present. The
segment's declared initial and maximum protections are both 3; this is not a
claim of runtime read-only protection. No new desktop execution is performed.

Final check commands, logs, image observations and hashes are retained alongside
this report. The full-workspace test gate and formatting remain separate pending
work. Missing source/startup/helper and complete loader-range proofs remain open;
these results do not certify marker resolution, symbol resolvers, dependency/native
initializers, arbitrary loader writes, class removal, or any size/speed benefit.

## Final verification

The native library suite passed **950/950**, including ignored tests (220.09 s).
The image suite passed **26/26**, including fresh five-format storage fixtures,
the eleven new Mach-O tests and all recorded desktop readbacks. These totals
overlap. The new API's runnable doctest passed once; the additional empty doctest
partition is not counted. Strict workspace/all-targets Clippy, explicit embedded
LLVM backend Clippy, and warnings-denied private documentation passed. New files
and the modified example pass rustfmt; the patch passes `git diff --check`.
Workspace-wide formatting still fails existing drift: its complete log hash and
a normalized 120-line excerpt are retained. No full workspace test pass is claimed.

`image-observations.json` contains the example's actual JSON output for the three
previous desktop images; `artifact-bindings.json` independently compares SHA-256
with their prior publication. The inspector itself is hashed. The example has
not been used to authorize image registration or callback ingress. Runtime ABI,
SDK inventory schema and pruning acceptance rules are unchanged.
