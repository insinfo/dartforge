# Cross compilation desktop evidence

Sources: DartForge cross implementation `986b16d4` plus `98ee596b` (Windows
JIT bounded-layout/symbol fixes and compiler architecture guards); dart_ui `15aff70`;
Dart SDK sources 3.6.2; LLVM 22.1.8.

Native means the compiler runs on the destination OS. Windows is x64 on Windows
11 ARM64 through compatibility; Linux and macOS are ARM64. Cross builds execute
the compiler on macOS for Windows/Linux, and on Linux for macOS. Bundles share
runtime source fingerprint
`5d6bcd9a8178d40fde4250933289fec03204d44fa6d5b66ea3c2278edcf357a8`.

JSON files retain commands, process exit codes and timings. Timings are functional
run metadata, not comparable performance measurements. The initial Windows JIT
failure is retained alongside results after its fix. Native AOT entry points run
for two frames. Context smoke programs use the real app and their own native
popup events, requiring `CONTEXT_RETARGET=PASS`; no third-party input injection
is used. Fonts and the existing macOS helper host are supplied separately.

Executable binaries and target bundles remain in ignored `target/` directories;
`artifacts.json` records their hashes. No live user documents are modified by the
context smoke (it uses an in-memory filesystem).

See the root `RELATORIO-CROSS-COMPILACAO-DESKTOP.md` for the result matrix,
limitations and pre-existing workspace check failures.
