# Mach-O chained fixups and immutable control metadata — 2026-10-20

Base `a6e89a47`. The SDK's first rejected registration root still includes an
image range and helper callbacks. A local-class-only shortcut is not justified by
the current SDK allocation inventory: raw memory allocation/exposure obligations
remain explicit. This increment extends the image boundary inspection, without
changing that pruning gate or declaring the production-profile assertion fixed.

## Corrected mapping/control assumptions

The new regression failed against the preceding implementation. It accepted all
six tested cases: SG_HIGHVM, SG_FVMLIB, protected/unknown segment flags, a classic
rebase into Mach-O load commands and a rebase into LINKEDIT. File bytes and a
linear section address calculation alone did not establish an immutable decoding
context. `control-before.log` preserves the failure and all accepted-case labels.

The reader now refuses unreviewed segment flags and classic writes into headers
or LINKEDIT. It records the managed segment flags; SG_NORELOC and SG_READ_ONLY
remain explicit declarations, not a promise of runtime permissions or absence of
implicit loader effects.

## Chained locations

`macho_relocations::inspect` now decodes `LC_DYLD_CHAINED_FIXUPS` for ordinary
64-bit/64-bit-offset pointers and the ARM64E, ARM64E_USERLAND and
ARM64E_USERLAND24 formats (2, 6, 1, 9, 12). Pointer authentication is observed,
not verified. Segments/pages, chain starts including overflow lists, import
formats 1/2/3, reserved bits, ordinals, names, file backing and every eight-byte
write span are checked. All metadata regions must be ordered, bounded and
non-overlapping. Each segment record must agree with the independently parsed
load commands. Mixed classic/chained records and duplicate commands are refused.

A visited-address set rejects repeated/overlapping targets, preventing one fixup
from rewriting a later chain word. Target writes into loader headers/control
metadata are refused. Chain progress stays within its page and file-backed
segment. Unknown/kernel/32-bit formats, compressed symbols and malformed metadata
return errors, never a partial clean audit. Import-name validation sorts named
offsets and reuses terminators for shared suffixes; unused zero padding needs no
per-byte terminator index.

The layouts and strides are checked against the installed Apple SDK 26.5
`mach-o/fixup-chains.h` and Apple's
[ChainedFixups implementation](https://github.com/apple-oss-distributions/dyld/blob/main/mach_o/ChainedFixups.cpp).
No external implementation was copied. Binding targets, authentication, arbitrary
loader effects, native initializers, marker binding and actual registered ranges
remain unproved. These facts grant no image-registration or class-pruning authority.

## Validation scope

Permanent tests cover all five pointer formats, authenticated and ordinary
bindings, 16/24-bit ordinals, three import layouts, valid static string headers
with overlapping fixups, section-boundary writes, multiple starts, aliases,
unterminated metadata, structural mutations, compressed/unknown layouts,
conflicting commands and writes to decoder control data. Fresh ARM64/x64 images
are linked with both classic and chained fixups, then inspected without execution.
The prior classic File Explorer artifact is also read back with unchanged bytes.

A separate integration builds ARM64/x64 images with 2,100 additional pointers to
exercise chains spanning several pages; source, exact build commands, inspector
output and input hashes are retained. These fixtures are not run as programs.
No fresh desktop AOT/JIT matrix, binary reduction or compilation speedup is claimed.

Full check results are retained in `checks.json`; diagnostic/source hashes bind
the observed revision. Global formatting and the full-workspace test gate remain
separate pending work. Complete SDK startup/helper and image-range proofs, and
removal of the foreign backends, remain open.

## Final results

All **959 native library tests** passed, including ignored tests (218.01 s).
The Mach-O suite passed **20/20**, and all image suites passed **35/35**; these
counts overlap. Nine tests are new in this increment. The API doctest passed
once; the empty filtered partition is not counted. Strict workspace/all-targets
Clippy, explicit embedded-LLVM backend Clippy and private documentation with
warnings denied all passed. Scoped rustfmt and `git diff --check` passed;
workspace formatting still fails existing drift, whose original log digest and
normalized excerpt are preserved.

The multipage images each contain exactly **2,101 rebases and one bind**, with
zero overlapping managed-string writes. The ARM64 data chain spans two 16 KB
pages; x64 spans five 4 KB pages. Exact counts/page coverage are asserted by
`check-multipage.py`. The prior File Explorer still has 51,706 classic writes,
none overlapping managed strings. The diagnostic example successfully handles
both formats in one invocation. Artifacts were read, not executed.

All observations are diagnostic facts. No SDK startup certificate, runtime ABI,
SDK schema or production pruning acceptance rule was changed. The missing image
range/helper and raw-memory proofs remain pending. This is not a new desktop
matrix, a full-workspace test approval, or a measured binary size/speed benefit.
