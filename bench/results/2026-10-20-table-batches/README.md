# Ordered program table batches and relocated build scripts — 2026-10-20

Base `57375e6a`. Programs with at least eight active eager method tables now
publish an ordered array of generated getters through `dartforge_register_tables`.
Smaller programs keep individual calls. Closed-class filtering happens first;
SDK libraries retain their independent `_StackTrace` registration. This does
not authorize any additional class pruning, startup or helper invocation.

The runtime reads the CID from each generated table's existing header, copies
the same pairs and publishes the same per-thread registration bit. Normal
registration preserves the first table; hot reload republishes in input order.
The unsafe batch ABI requires aligned, valid immutable arrays/tables and pure,
side-effect-free generated getters. Unlike the scalar API, it calls a getter
before checking whether the embedded CID was registered. Purity makes that
extra read unobservable. Arbitrary side-effecting getters keep the scalar API.
The effects catalog remains conservative. No persistent inventory schema changes.

Three emitter tests cover thresholds 0/1/7/8/9, order, closed-class selection
and independent SDK trace startup. Three runtime tests use fresh threads and
cover first-registration parity, publication, republishing and null empty input.
They overlap the full suite totals below, rather than adding to them.

## Actual File Explorer and artifact comparison

The UI source revision is `15aff7042318d2fa56661ffed4360c81a41a86b8`.
All **3,758** eager program getters are pure constant returns. Their embedded
CIDs match the scalar call arguments, and the new array preserves their exact
order. Removing the array/declaration and expanding the one batch call back
into the old calls makes the complete IR **byte-identical**. Other runtime and
SDK registrations are unchanged. `runtime/explorer-comparison.json` and its
inspection script retain the independent checks.

| Artifact | Before | After | Difference |
|---|---:|---:|---:|
| Full LLVM IR, bytes | 293,593,137 | 293,412,654 | −180,483 |
| macOS ARM64 executable, bytes | 25,273,712 | 25,240,688 | −33,024 |
| Linux ARM64 executable, bytes | 37,610,992 | 37,611,936 | +944 |
| Windows x64 executable, bytes | 37,255,168 | 37,229,568 | −25,600 |

These are file sizes, not summed virtual segments (`__PAGEZERO` alone reserves
4 GiB on macOS). No universal size or startup-time improvement is claimed.
IR generation timings were collected with concurrent work and are not a speed
benchmark. Compiler, IR, executable, bundle and archive hashes are in
`artifact-bindings.json`; the large artifacts remain in ignored target storage.

## Correctness and execution

All nine final commands in `checks.json` passed: directed emitter/runtime tests,
strict workspace/all-target Clippy, explicit embedded-LLVM Clippy,
warnings-denied private workspace documentation, the new ABI doctest, full
runtime tests, JIT compiler build and full native library tests with ignored
tests enabled. Totals: **996 native library tests**, **324 runtime unit tests**,
**25 runtime integration tests** and **88 runtime doctests**. Directed runs and
the single ABI doctest overlap those totals. New test-file rustfmt passed.
Global formatting still fails on existing workspace drift; `fmt.json` preserves
the full output's size/digest, with a normalized excerpt retained here.

The nine-class fixture prints `parent=45` and `child=45`, agreeing with Dart
3.6.2. macOS before/after production and development AOT passed under normal and
stressed GC; updated JIT passed both. New macOS-cross-compiled Linux ARM64 and
Windows x64 AOT programs passed both, and native guest JIT compilers passed both.
Full commands, actual output, compiler hashes and guest executable-copy checks
are retained under `runtime/`. Windows runs x64 under Windows 11 ARM64
compatibility; this is not a new Windows ARM64 native target.

The real UI smoke opens the File Explorer for two frames, and the application
context smoke checks file-to-file retargeting and empty-area menus. macOS uses
its existing AppKit host/CPU presentation; Ubuntu uses Wayland/OpenGL; Windows
uses Win32/D3D11. These are short functional checks, not exhaustive visual,
window-management or long-duration certification. GUI runs use normal GC;
the separate fixture supplies GC stress coverage. UI Dart sources match the
host: `ui-source-{host,linux,windows}.json` all report 1,151 files with the
same full relative-path ordering and CRLF-normalized SHA-256
`35728722d8233a0b651ad1303a7ae3ac00b86cc63a7ff28e3ddb62143f9a1ae8`.
The earlier host/Linux `*-path-parts.json` pair used component ordering and also
matches; its different aggregate hash reflects ordering, not a source mismatch.
Fonts and resolved package configurations are separately supplied per system.

The final UI matrix passed in every cell. The original explorer exits after two
frames, and both menu runs report `CONTEXT_RETARGET=PASS` with empty stderr.
The previous AOT explorer binary also passed in all three environments.

| System | Updated explorer AOT | Updated explorer JIT | Updated context AOT/JIT |
|---|---|---|---|
| macOS ARM64 | pass | pass | pass / pass |
| Ubuntu ARM64, Wayland/OpenGL | pass | pass | pass / pass |
| Windows 11 ARM64, x64 compatibility, D3D11 | pass | pass | pass / pass |

The guest reports are `runtime/{linux,windows}-ui-report.json`; the macOS report
and separate command streams are under `macos-ui/`. Both VMs were returned to
their initial suspended state after terminal checks. The `current` bundle links
now select the verified new bundles; old destinations are preserved and recorded
in `runtime/current-bundle-update.json`.

## Real build-script fault and bundle integrity

Reusing Cargo's compiled emitter build script from a new source checkout exposed
a real error: `env!("CARGO_MANIFEST_DIR")` retained the old checkout path. The first
Linux snapshot contained the new runtime but exported the old runtime source
hash. Both source-root users now read Cargo's execution-time environment.
`scripts/test-build-script-relocation.py` runs one compiled build script against
three temporary source trees: changed contents must change emitter/runtime hashes
and all three runtime artifact names; identical contents moved elsewhere must
preserve them. The old binary fails; the updated binary passes. The fake-source
hashes in that regression are not the actual runtime hash.

Both final guest snapshots exported runtime sources
`a37848c6491b54570a07e4ca916ece5310d20e4c435fc32258dd2045db587ee5`.
The final source archive and eight changed-source hashes are bound in
`runtime/snapshot-final.json` and `runtime/source-bindings-final.json`.
The first wrong export/snapshot is retained separately. A stale-bundle rejection
also verifies that an existing output executable remains unchanged. No manifest
was edited to make stale content pass. Old bundle directories remain available.

## Retained failed attempts and limits

The initial Windows fixture command accidentally invoked an old validation
script; its wrong marker/hash was rejected, then the script transport was fixed.
The initial Linux JIT compiler copy had root-owned permissions; only that owned
copy was corrected. The initial Wayland GUI attempt used the X11 presentation
name and failed; the explicit `wayland-opengl` configuration passed afterward.
These original failures remain in the runtime observations and scripts.

Two Windows release builds failed with librarian `LNK1104` opening a `ring`
archive. C: had approximately 34 GB free. A fresh, shorter isolated Cargo target
directory produced a working compiler; this observation alone does not prove
path length was the cause. Original logs and the successful command are retained.
Guest compiler builds use one job, no Rust LTO, no debug info and reduced release
optimization to fit 4 GB VMs; Dart AOT still uses `--optimize`. No heavy Rust build
and Dart JIT are run simultaneously within one VM. No periodic agent was added.

The actual platform reproduction still contains six foreign-backend definitions
in both default and experimental closed-class modes. Its first SDK image/helper
startup remains unproved. This change does not satisfy foreign-backend absence,
whole-workspace test approval or global formatting. macOS linker warnings about
LLVM compression objects declaring a newer OS minimum are retained; execution on
the current host does not certify older macOS compatibility.
