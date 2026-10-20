# One image-registration owner per complete SDK bundle — 2026-10-20

Base `911f850f`. Each of the 15 source SDK libraries previously registered the
same linked image range during every isolate's startup. Registration is process-wide
and idempotent, but each call takes the image registry write lock and searches the
same vector. The compiler already links all SOURCE_LIBRARIES into one SDK image
and calls their startup roots in the same fixed order before RTI/embedder code.

## Scope and ownership

Only the compiler-owned complete bundle delegates image registration to its first
startup root (`dart:_internal`). All other startup effects, static literals and
marker definitions remain unchanged. The SDK now emits one registration call
instead of 15. The program still registers its own image, since development
executables and the JIT can load the SDK from a separate shared library. No
runtime API, image layout, summary schema or manually selected cache option changed;
the compiler source fingerprint separates the rebuilt SDK cache entries.

Public `emit_sdk_libraries[_with]` still emits independently usable library startup
roots, each registering its own linked image. Delegation is an internal emitter
option with a standalone default; only the private complete-bundle path uses it.
An initial implementation applied delegation to the public per-library emitter;
that scope was corrected before final validation, which was repeated afterward.

Three unit tests require byte-for-byte identical IR except the single registration
call for a delegated module; preserve static-storage-disabled behavior and the
standalone default; and check SDK startup order plus the program's own registration.
The public development SDK emission test checks that every returned library still
owns registration. The verified production inventory checks exactly one owner
across all 15 startup roots.

Removing actual image publication from metadata-only startup functions allows the
existing unmodified certificate rules, in this tracing-GC production snapshot,
to recognize seven roots: collection, math,
convert, typed_data, nativewrappers, _http and mirrors. The inventory
regression requires exactly that set. An initial expectation incorrectly included
developer; the preserved failure reports the seven accepted roots. Developer also
publishes a generated `_AsyncFrame` name without an audited nominal declaration.
The corrected regression requires that exact open boundary to remain unproved; no
certificate rule or production backend-absence expectation was weakened. The first SDK root still publishes an image
and helper functions; it remains unproved. The production foreign-backend absence
test is unchanged. This is not a new image-registration certificate or a completed
closed-class optimization.

## Runtime comparison and limits

`static-startup.dart` exercises JSON, UTF-8, Unicode literals, a RangeError helper,
and three sequential child isolates followed by base64 encoding. The reference is
Dart 3.6.2. Before/after production and development AOT executions and updated JIT
executions are recorded separately, with explicit normal/stressed GC settings,
compiler/executable hashes, exact output and terminal status. Large executables
remain under ignored target storage.

The demonstrated work reduction is 15 to one SDK image-registration calls per
isolate startup. It is not a measured whole-program startup speedup or executable
size gain. No full-workspace test approval or new desktop AOT/JIT matrix is claimed.
The remaining image/helper coverage, foreign-backend pruning and global formatting
issues are tracked independently.

## Final local verification

The final native library run passed **978/978**, including ignored tests
(227.49 s). All three new tests passed; their counts overlap the full suite.
Strict workspace/all-targets and explicit embedded-LLVM native Clippy passed,
as did warnings-denied private docs. A separate cached production-inventory run
passed and printed the exact seven metadata roots, one image registration and
all 15 clean source-image censuses. The new test file passes rustfmt;
workspace-wide formatting still fails, with its full log digest/size and a
normalized excerpt retained.

The CLI with JIT built successfully. Its linker reported a pre-existing toolchain
boundary: some bundled LLVM compression objects declare macOS 27.0 while this
validation sets a 15.0 deployment target. The full warning is retained; runs on
the current host do not establish compatibility with older macOS releases.

`otool -rv` independently inspected relocation records in the development SDK
objects selected by the before/after executables' actual dylib dependencies.
All 15 baseline modules have a registration-call relocation; only the first
updated module does. Object paths/hashes, tool commands, output hashes and all
matching lines are retained in `owner-objects.json`. An initial attempt selected
an unavailable llvm-readobj; no inspection was performed by that missing tool.

The refreshed compiler's production platform reproduction still contains six
foreign-backend executable definitions in both ordinary and experimental modes.
The initial SDK image/helper root remains rejected. These observations do not
approve the failing absence regression or the complete workspace test gate.

## Cross-target runtime check

The same fixture was compiled on macOS for Linux ARM64 and Windows x64, then run
in Ubuntu 24.04 ARM64 and Windows 11 ARM64 (x64 compatibility), respectively.
Both normal and stressed GC runs returned zero, matched the Dart VM's output and
had empty stderr. Executable hashes were checked in the guests. This adds AOT
fixture coverage on all three systems; JIT was exercised on macOS only, and no
new File Explorer GUI/menu matrix is claimed.

The first cross attempt correctly refused the older bundles' runtime source
hash. New bundles were exported natively from frozen source copies, one VM at a
time with two Cargo jobs; existing guest source trees were not overwritten.
A small temporary example invokes the regular `cross::export` API, without
manually rewriting manifests. Both exports have runtime source hash
`23bc6f3ab5d29f8056681af73aa2c0bb668b968a0013f91a1d90d18743bb98ef`.
The source archive, scripts, native build logs, manifests, compiler commands and
artifact hashes are bound in the retained records. The `current` bundle paths
now select these validated exports; the previous directories were retained under
`target/cross-bundles/retained-before-sdk-image-owner`. A repeated rejection using
the retained old bundle also proved that an existing executable was not modified.

A Windows encoded-command controller remained live without an export process in
CIM probes. Only that controller was canceled, with the probes and cancellation
status retained. Short file transport then exposed a PowerShell harness error:
Stop-on-error treated routine Cargo stderr progress as a terminating error.
The script now allows that stream while explicitly preserving/checking Cargo's
exit status. Both failed file attempts and the final successful logged-in-user
export are retained. No VM reboot, user-application termination, persistent
execution-policy change or periodic agent was needed.
