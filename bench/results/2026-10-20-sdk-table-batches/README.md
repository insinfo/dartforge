# Verified batches for link-time SDK registration — 2026-10-20

Base `95d8e35f`. Program table registration was already batched, but the SDK
registration emitted later by `pruning::assemble` still used individual calls.
The assembler now checks the actual generated SDK table fragment with the
constant-getter witness, then emits one `dartforge_register_tables` call for at
least eight registrations. The program emitter and assembler share that minimum.

Every getter must have a checked constant-return body, complete immutable
storage and the same embedded CID as the original scalar call. All registrations
must qualify; otherwise the original scalar path remains. This matters because
the batch reads the CID from table storage, and calls a getter even when its CID
was already registered. Constant getters make that extra call unobservable.
Effectful or unproved callbacks and mismatching CIDs do not meet that contract.
The runtime is unchanged; its first-registration and republishing rules remain
those tested with the original batch ABI.

The existing CID/name sort and deduplication happen before emission. Empty tables
remain registered. `DARTFORGE_NO_TABLE_PRUNING=1` keeps its existing pair policy;
absence of the startup declaration still adds no registration. Batch storage
avoids existing symbols, including quoted/hex-escaped LLVM names, and reuses an
existing runtime declaration. The new witness covers the generated fragment;
it is not an imported SDK-body, callback-input, image or whole-class certificate.

Four new tests cover the 0/1/7/8/9 boundaries and original CID order, live/empty
tables with both pruning settings, an absent startup declaration, namespace
collisions and declaration reuse, and preservation of scalar behavior for
unproved callbacks or mismatched CIDs. The corrected positive regression fails
on the previous scalar assembler. The initial fixture mistakenly expected every
SDK table to be live without pruning; it was corrected to explicitly reach one
getter and check both live and empty storage. Both failed attempts are retained.

## Actual assembled IR and execution

`--emit-ir` returns the program before SDK link assembly. These comparisons use
`DARTFORGE_KEEP_IR=1` files from the actual AOT driver instead. The nine-class,
two-isolate fixture has **662 SDK registrations before, one after**. All 662
getters and embedded CIDs match the previous calls in their exact order.
Removing the new array and expanding the batch back to those calls produces
**byte-identical complete assembled IR**, with ordinary and disabled pruning.
Each assembled IR is **30,861 bytes smaller**. The independent inspection script
and hashes are retained in `runtime/assembly-comparison.json`.

The same 662-entry batch appears in the actual Linux ARM64 and Windows x64
fixture, File Explorer and context smoke link inputs. These are cross-compiled
on macOS against the existing source-matched target bundles. Runtime sources
remain `a37848c6491b54570a07e4ca916ece5310d20e4c435fc32258dd2045db587ee5`;
no bundle was reexported or edited to accept a mismatch.

The fixture prints `parent=45` and `child=45`, matching Dart 3.6.2. Before/after
macOS production and development AOT, and production with table pruning disabled,
passed with normal/stressed GC. Updated JIT macOS passed both too. Updated AOT
executables passed both GC conditions in Ubuntu ARM64 and Windows 11 ARM64
running x64 compatibility code. This does not add a Windows ARM64 native target.

The real File Explorer and context-menu retarget smoke passed in AOT on macOS
ARM64/AppKit/CPU, Ubuntu ARM64/Wayland/OpenGL and Windows x64/Win32/D3D11. The
explorer runs for two frames and each context run reports `CONTEXT_RETARGET=PASS`.
All application run stderr streams are empty. UI sources are clean revision
`15aff7042318d2fa56661ffed4360c81a41a86b8`, with the source digest in
`runtime/ui-source.json`. Fonts are supplied from each native installation.
Guest executables are hash-checked after copying. Both VMs were restored to their
initial suspended state. This is a short AOT functional matrix, not exhaustive
visual or window-management testing or a new Windows/Linux JIT matrix.

## Verification and measurements

All six commands in `checks/checks.json` passed: directed tests, strict
workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private documentation, JIT CLI build and native library tests with ignored tests
enabled. The native run passed **1,006/1,006** in 232.09 s; the four directed tests
overlap this total. Scoped rustfmt passes. Global formatting and whole-workspace
approval remain open; the full fmt output size/digest and an excerpt are retained.
The unmodified foreign-backend absence test was executed again and **failed**
(exit 101). Raw platform IR stays unchanged with six foreign definitions in
both default and experimental closed-class modes. No pruning permission changes.

A separate macOS whole-process ABBA observation used five warmup cycles and
25 measured cycles (50 samples per executable), after compiler work ended and
both VMs were suspended. It includes the loader, two isolates, output pipes and
process launch, rather than isolating table registration. Medians were 6.3143 ms
before and 6.2915 ms after (−0.36%); means 6.4254/6.4095 ms. This difference is
small relative to observed variation and does **not** establish a speedup.
Every sample checked the output and exit status; raw samples are retained.

The macOS production fixture file size remains 3,425,568 bytes. The fixture with
pruning disabled changed from 10,428,368 to 10,411,856 bytes (−16,512).
Development files remain 79,760 bytes and use different content-bound SDK DLL
names. These are file sizes, not virtual segment sizes; no universal binary-size
or compilation-time gain is claimed. Large final artifacts remain in ignored
target storage with SHA-256 bindings.

## Retained harness failure

The first preservation routine copied the compiler in place again after tests
finished. Subsequent macOS launches were killed before executing user code;
three system reports identify `CODESIGNING / Taskgated Invalid Signature`.
On-disk codesign verification still succeeded. Publishing the exact same SHA-256
bytes through a fresh temporary inode and atomic rename restored execution.
The harness now uses atomic replacement, and the interrupted runtime/cross-build
checks were repeated successfully. Extracted OS reports and the original failed
results remain in `runtime/`; this was not converted into a runtime pass.
The initial `--version` probe returned the CLI's normal unknown-command error;
a subsequent supported `--help` probe exited successfully.

Source bindings cover all six changed Rust files. Log bindings retain original
and normalized sizes/digests, including failed setup and signature attempts.
The compiler log preserves the existing LLVM compression-object warning about a
newer macOS minimum; current-host execution is not older-macOS certification.
