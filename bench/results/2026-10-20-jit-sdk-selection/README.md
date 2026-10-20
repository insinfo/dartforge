# Explicit SDK selection in native JIT execution — 2026-10-20

Base `6ab2194e`. Refreshing the real File Explorer matrix exposed two problems:

- The desktop validator placed `--sdk` and `--packages` after the Dart entry
  point. `run` forwards those arguments to the application. Its injected SDK
  environment variable, or a discoverable SDK on PATH, could hide this mistake.
  The validator now puts compiler options before the entry and supplies the SDK
  through the CLI instead of injecting a fallback environment selection.
- Even with correctly ordered options, `run` used the explicit SDK for emission
  but rediscovered the default SDK when loading its shared library. Linux failed
  after emission because no default SDK was discoverable. The loader now uses
  the same explicit SDK. Stateful reload does too, and hot restart forwards
  `--sdk` to the generated-IR child. `DARTFORGE_SDK_DLL` retains precedence.

No CLI argument-boundary rule, runtime, native emitter or public API changes.
The fix passes paths directly rather than mutating global SDK environment state.

## Reproduction and regression

`linux-ui-jit-initial-report.json` retains the original incorrectly ordered
command and missing-SDK error. `linux-ui-jit-sdk-library-failure.json` retains
the correctly ordered command that still failed during library selection.
The original host reports are retained; their JIT runs do not establish explicit
SDK selection, even though their exit status was zero.

The new CLI regression gives the child an SDK directory containing an invalid
`libraries.json`, while selecting the real SDK with `--sdk`. It fails before
the fix with the invalid-manifest diagnostic (`sdk-red`, exit 101), and passes
afterward (`sdk-green`, exit 0). It checks run, stateful reload and hot restart,
and requires the exact application arguments `["--sdk","application-owned","--"]`.
The temporary invalid SDK is cleaned up by a drop guard even on assertion failure.

The complete CLI native/JIT suite passes **83 tests in 14 executables**, with
zero failures, ignored tests or filters. This includes the new regression;
the directed run is not an additional distinct test. Workspace/all-target and
native/JIT CLI Clippy pass with warnings denied, as do private workspace docs
and global formatting. The existing LLVM deployment-version warnings are kept
in the linker logs. Python validation scripts also parse successfully.

## Desktop matrix

| Environment | Explorer AOT | Explorer JIT | Context AOT | Context JIT |
| --- | --- | --- | --- | --- |
| macOS ARM64, AppKit/CPU | pass | pass | pass | pass |
| Ubuntu ARM64, Wayland/OpenGL | pass | pass | pass | pass |
| Windows 11 ARM64, x64 compatibility, Win32/D3D11 | pass | pass | pass | pass |

The explorer opens for two frames and exits. The context smoke uses native
popup lifetimes and application-injected pointer events to verify file-to-file,
file-to-empty-area and empty-area-to-file replacement with a single secondary
press, selection updates, presented frames and an empty application error list.
Every accepted execution exits zero with empty stderr; each context run prints
`CONTEXT_RETARGET=PASS`. These are short functional checks with normal GC,
not a complete visual, physical-input, window-management or long-duration audit.

All AOT programs were compiled **on macOS**, using the preserved `6ab2194e`
backend and matching existing cross bundles. They were copied to the guests
with SHA-256 checks. The CLI correction affects JIT loading, not AOT emission.
All accepted final JIT runs use compilers containing the fix. Host JIT results
are in `macos-jit-fixed/`; only AOT records from `macos-ui/` enter this matrix.
Both guest JIT runs and the final host JIT runs have an invalid SDK in the
environment and the correct explicit CLI SDK, without a DLL override.

Run, stateful reload and hot restart also pass on each guest with the same
invalid ambient SDK and exact application arguments. Together with the host
regression, this covers nine system/mode combinations. See
`linux-sdk-modes.json`, `windows-sdk-modes.json` and the CLI regression log.

## Source and artifact identity

All three environments have the same 1,151 application/library Dart files,
four manifests/lockfiles and 427 checked SDK files after CRLF normalization.
The SDK is 3.6.2. Application and SDK aggregate hashes are respectively
`35728722d8233a0b651ad1303a7ae3ac00b86cc63a7ff28e3ddb62143f9a1ae8`
and `e53abc10817b1d1e737a44c400f9b7148e7848a9291030c91601b0bd71aaa730`.
Fonts and resolved package paths are supplied separately on each system.

The frozen fixed build-input snapshot contains 1,851 files and identifies the
three changed code/script files relative to the base. Both guests verify every
file by raw SHA-256 before building. Full source manifests, patch, archive
identities, compiler hashes, build options and executable-copy bindings are
retained. Host uses a debug compiler; guest builds use release opt-level 1,
without LTO/debug information and with one Cargo worker. Timings are execution
metadata, not comparable performance measurements.

The first Linux build snapshot omitted embedded build-executor Dart files;
its failed log and original manifest are retained. The corrected snapshot also
includes packages/native_sdk. A separate SDK-mode helper initially tried to
read a root-owned snapshot pointer as the graphical user; it now receives the
path from its wrapper. That failure occurred before compiler startup.

Runtime source binding `e0a3c8b0…` and existing cross bundles stay unchanged.
Both VMs finish suspended. The known foreign-backend pruning failure remains
open; this work does not claim complete workspace approval or solve that gate.
Its latest retained reproduction is in the preceding root-bit-iteration report.

Text evidence has trailing whitespace/final blank lines normalized, with input
encoding and original/stored hashes in `manifest.json`. The patch is losslessly
compressed to preserve whitespace required by the diff format. Large source
archives and executables stay in ignored validation outputs, identified by hash.
