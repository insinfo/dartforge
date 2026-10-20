# Declared PE base relocation writes — 2026-10-20

Base `59bd815a`. Imported SDK startup still registers managed images and helper
bindings, beyond the current closed-class proof. ELF and Mach-O had explicit
relocation auditors; PE only had on-disk string-section validation. This increment
adds PE base-relocation write inspection as a necessary Windows image-boundary
component. It does not authorize image registration or fix foreign-backend pruning.

## Reader contract

`linked_images::pe_relocations::inspect` reads the PE32+ optional-header base
relocation directory for x64/ARM64, independently of section names. It validates
header/section extents, file backing, alignment, non-overlapping virtual and raw
mappings, and complete blocks. ABSOLUTE entries are padding; HIGHLOW and DIR64
are counted as four/eight-byte writes. Unsupported kinds fail instead of becoming
safe or silently skipped records. Repeated targets are counted repeatedly; overlap
examples are capped at 16 independently of the full count.

The complete write span must fit one mapped section. Writes into the headers or
any part of the relocation directory are rejected so the decoder cannot assume
control data that its own writes could change. Partial blocks/directories,
overflows, conflicting stripped-relocation metadata and unknown directory layouts
are refused. Section characteristics, image flags and other nonempty directories
are reported as observations, not proofs of loader behavior.

The block layout and relocation widths follow the primary
[Microsoft PE specification](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#the-reloc-section-image-only).
No loader implementation was copied. Imports/IAT, TLS callbacks, load configuration,
other initialization/native writes, marker binding and the actual published
registration ranges remain unproved. No runtime, pruning gate or summary/cache
schema changed. The existing inspection example now reports PE audits alongside
ELF and Mach-O and continues to report `loader_memory_verified: false`.

## Validation

Nine new library tests cover x64/ARM64 synthetic PE images, renamed relocation
sections, DIR64/HIGHLOW full-width and half-open boundaries, repeated targets,
bounded examples, ABSOLUTE entries without mapped targets, malformed/truncated
blocks, unknown types, unmapped/overflowing targets, self-modifying tables,
ambiguous raw/virtual mappings, invalid alignment and stripped-table conflicts.
An absent base directory retains an observed TLS directory without claiming it safe.

Fresh x64 and ARM64 Windows images linked with Clang/LLD contain 2,100 pointer
relocations spanning five blocks. Both auditors count all 2,100 DIR64 writes and
zero managed-string overlaps. The linked images are inspected without execution.
The fixture source and exact compiler/linker arguments are in the permanent test;
its JSON records image/table digests and complete counts.

The previously recorded Windows x64 File Explorer executable has 256 blocks,
51,318 entries (128 padding), and **51,190 declared eight-byte writes**. None
overlap its **871,040-byte managed string section**. The original executable is
read unchanged; this is not a new Windows build or desktop execution. Imports,
exception/unwind, TLS and IAT directories remain explicitly reported.

The first directed run passed eight tests and failed the fresh-link test because
`lld-link` was absent from PATH. After adding the installed LLVM tools, all nine
passed. Strict Clippy then rejected `chunks_exact` where fixed arrays were
available, and a redundant bit operation in a test. Both were corrected; initial
logs and terminal statuses are preserved separately. No semantic expectation was
weakened.

Final validation passed **975/975 native library tests**, including ignored tests
(226.20 s), all nine directed tests (overlapping that suite), strict workspace
and embedded-LLVM/all-targets Clippy, warnings-denied private docs and the new
runnable documentation example. The updated inspector was built and executed on
the preserved Windows image. New-module rustfmt and `git diff --check` passed;
global formatting still fails pre-existing drift, with the full log digest/size
and a normalized excerpt retained.

An additional retained integration uses an empty permanent string and the same
2,100-pointer shape. Both targets again have five blocks, exactly 2,100 DIR64
writes and no string overlaps. `pointers.c`, exact commands, inspector output,
original artifact hashes and source/log bindings are retained; large linked
images and objects remain under ignored `target/pe-relocation-validation`.
Environment: macOS ARM64, LLVM/Clang/LLD 22.1.8, Dart 3.6.2 source SDK, macOS SDK
26.5 and deployment target 15.0; `validate-checks.py` records all selected paths.
Original log hashes are preserved separately from whitespace-normalized copies. This increment makes no performance claim,
full-workspace test approval, desktop/JIT matrix claim or new pruning authorization.
