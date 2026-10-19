# Declared ELF relocation write spans

Base `fa42f86d`; macOS ARM64, LLVM 22.1.8, object 0.39.1.

The explicit diagnostic `linked_images::elf_relocations::inspect` decodes
REL/RELA/RELR/JMPREL declarations from PT_DYNAMIC and maps their bytes through
unique PT_LOAD spans. It revalidates the linked string section and compares its
content digest with the declared load mapping. This is independent of relocation
section names: a fixture has a live dynamic relocation absent from the generic
section iterator, and the new reader still finds its overlapping potential write.

The reader supports ELF64 LE ARM64/x64 headers without processor flags. It checks
file bounds, load alignment/ranges, dynamic termination/duplicates, entry sizes,
fast relative prefix counts/types/symbols and arithmetic overflow. Entry-size
metadata without a main table remains valid for PLT-only tables. RELR is streamed
without materializing expanded address lists; bitmap-before-address and overflow
are refused. Even unaligned addresses are preserved, as allowed by the encoding.
Unknown COPY/type widths and unknown dynamic tags remain explicit. Unknown tags
cannot collide with an internal sentinel for relative-prefix metadata.

Overlap checks use complete potential write widths, so a relocation starting
before the managed section can still overlap it. There is no classification of
unknown widths as disjoint. Counters scan every record; overlap and unknown
example lists are capped at 16 per table. A 30-overlap fixture verifies that the
example cap does not stop the counts. Composed relocations can be conservatively
overcounted: these are potential per-record writes, not executed store counts.

The existing Linux ARM64 explorer matches its previous committed SHA-256. Its
managed section has 854,760 bytes and a declared PT_LOAD flags value of 4. The
metadata contains 67,445 RELA entries (67,434 RELATIVE, 11 GLOB_DAT) and 133
JMPREL/JUMP_SLOT entries. All 67,578 potential writes have reviewed widths and
none overlap the managed range. There are no unreviewed dynamic tags in this
sample, but initializer/finalizer tags are present. This is not a new build or
execution of the explorer or a certification of loader memory behavior.

Fresh Linux ARM64/x64 PIE fixtures passed with ordinary RELA and packed RELR.
The ARM64 fixture has 73 potential writes (5 encoded RELR entries when packed),
the x64 fixture 70 (3 encoded RELR entries when packed). Replacing a target with
the managed section address is detected in all four fixtures. None ran. This is
not a production size benchmark or a change to default linker flags.

All 15 linked-image tests passed, including the nine new relocation tests and
all ignored fixture/desktop prerequisites. One runnable API doctest passed;
zero-test filtered partitions are not coverage. Strict workspace/all-target and
explicit embedded-llvm Clippy and warnings-denied private docs passed. Global
formatting still fails existing drift; the changed inspection files have no
formatting diff. Full native/workspace suites, source SDK inventory rebuild,
production-profile assertions and VM/desktop/JIT executions were not repeated.
The producer/layout validators and class-pruning gates were not changed.

This audit covers declared relocation records in the file. It does not establish
validity of every destination, stable relocation metadata while the loader runs,
implicit writes such as GOT/DT_DEBUG, symbol-resolution/IFUNC code, initializers,
loaded dependencies, actual permissions or register_image bounds. No image/class
authorization or loader immutability certificate was added. Mach-O/PE fixups and
complete link-input/marker coverage remain open. No size/speed gain or resident
process/periodic task is claimed.

Normative references: [gABI dynamic entries](https://gabi.xinuos.com/elf/08-dynamic.html),
[gABI relocation encodings](https://gabi.xinuos.com/elf/06-reloc.html),
[AAELF64](https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst),
[AMD64 psABI](https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf).

Text logs normalize trailing whitespace/final blank lines. Original/stored hashes
are retained; fmt-excerpt.log contains only the first 120 lines of the original
failed formatting log, bound by its complete original size/hash.
