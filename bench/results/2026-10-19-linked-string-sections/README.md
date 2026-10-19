# Linked file string section readback

Base `6a20bfb2`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.

`linked_images::inspect` reads linked 64-bit LE ARM64/x64 Mach-O, ELF and PE
images. It requires one recognized, aligned and fully file-backed managed string
section, walks every byte as zero words or validated string blocks, and returns
owned records bound to the file and section digests. The address is the preferred
file VA, not a live process address. Zero words are padding observations, not
certified anchor identities. PE reading is enabled in the existing object crate;
Cargo.lock did not change.

The storage validator is shared with LLVM source initializer observations. It
requires exact permanent RAW headers, string CIDs, bounded lengths, VM hashes,
canonical Latin-1/UTF-16 encoding and zero padding. Isolated surrogates remain
valid. Large strings use actual bounded lengths instead of saturated header word
counts. The source path still checks the symbol/key, section and storage flags;
the shared kernel returns a fixed-size digest rather than allocating a String
for every source initializer. No pruning authorization or cache schema changed.

Fresh layout fixtures were compiled and linked with Clang/LLD for Mach-O ARM64
and x64, ELF ARM64 and x64, and PE x64. All readbacks matched the expected storage
bytes. Mutating the hash in the final file was rejected for every target. These
fixtures contain C declarations of the layout and are not executions of Dart
programs or of foreign targets. Unit tests also cover truncation, altered
headers/hash/metadata/length/padding, noncanonical UTF-16, huge lengths, saturated
word counts and wrong input kinds.

The three existing File Explorer binaries match their SHA-256 values in the
previous committed desktop publication. Their sections passed complete readback:

| Image | Strings | Section bytes | Two-byte strings | Zero words |
| --- | ---: | ---: | ---: | ---: |
| macOS ARM64 | 14,396 | 850,904 | 36 | 0 |
| Linux ARM64 | 14,548 | 854,760 | 36 | 0 |
| Windows x64 | 14,534 | 871,040 | 36 | 4 |

The other strings have CID 6; two-byte strings have CID 7. These are definitions
in their respective final files, not a cross-platform performance comparison or
a new explorer build/execution. Artifacts, sections and prior manifest bindings
are recorded separately.

Six linked-image tests, 64 inventory tests (including actual source SDK rebuild,
publication and loading), 11 native-input tests and one runnable API doctest
passed. Strict workspace/all-target and embedded-llvm Clippy and warnings-denied
private docs passed. Initial Clippy failures for manual divisibility/chunk
iteration were fixed without suppression and preserved in initial-clippy.log.
Global formatting still fails existing drift; modified inspection/layout files
have no formatting difference. Full native/workspace suites, production-profile
assertions, desktop execution and VM/JIT matrices were not repeated.

The final SDK inventory, all module records and the 69,945,429-byte schema-5
payload size match the prior snapshot. The shared validation refactor did not
change observed emitted IR or inventory facts. This round does not inspect SDK
bitcode with embedded LLVM again; the inventory run uses the default generator.
The previously added native input reader shares its fallible Mach-O load-command
walk with this reader, preventing tolerated truncation from hiding sections.

This is an explicit diagnostic API, not an automatic production linker gate.
Loader rebases/binds/relocations, runtime permissions, actual register_image
bounds, marker resolution, input provenance and link options remain unproved.
Even canonical on-disk bytes do not certify what the loader registers in memory.
No managed image, static handle or class pruning is authorized, and no executable
size/speed improvement is claimed. No resident process or periodic task was added.

Text logs normalize trailing whitespace/final blank lines; original/stored
hashes are retained. The formatting log stores only its first 120 lines, with the
complete original size/hash bound by log-hashes.json.
