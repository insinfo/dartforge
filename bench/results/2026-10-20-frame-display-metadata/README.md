# Reserved asynchronous frame display metadata — 2026-10-20

Base: `30e7cee3`. Complete SDK startup certification previously rejected
`dart:developer` because its generated `_AsyncFrame` class name has no nominal
class declaration. The shared frame CID describes variable per-resume storage;
interpreting it as an ordinary fixed-layout class would be unsound.

The entry-only metadata certificate now recognizes exactly the reserved frame
CID paired with the exact private UTF-8 initializer `_AsyncFrame`. All existing
body, ABI, ownership, binding, pointer and content checks remain required.
Any nominal declaration, ambiguous class, table, field layout or parent edge
involving that CID in any selected summary rejects this exception. Ordinary
class-name registrations still require their existing nominal ownership proof.
The reserved CID is excluded from nominal subtype certificates even if nominal
facts are supplied. No runtime ABI, summary schema or allocation rule changed.

This is deliberately not a generic exemption for names. The runtime searches
class names to choose the allocatable StackTrace class. The reserved display
label has no such role. Four import tests cover the exact entry-only permission,
ordinary calls and allocation refusal, ten malformed/unsafe variants, twelve
local/imported metadata conflicts, and idempotent display publication without
subtype authority. Two class reachability tests check that the active class set
stays unchanged, that frame allocations remain unproved, and that real native
subtype queries agree under eager/closed registration with normal/stressed GC.
The initial positive test failed before the implementation; its log is retained.

The selected production SDK inventory now requires exactly eight certified
metadata startup roots, adding developer to the prior seven. Frame publication
remains outside the nominal universe and ordinary call capabilities. The first
SDK root still registers its image and helpers and remains unproved. The
production backend-absence expectation is unchanged.

Validation records distinguish the full native library suite, targeted tests,
workspace and embedded-LLVM Clippy, documentation, executable comparison and
actual explorer IR comparison. Executables and large IR remain in ignored
`target/frame-name-validation`; their hashes are recorded. Global format drift
is retained by original digest/size and a normalized excerpt. No general speed,
binary-size gain, full workspace approval or new desktop GUI matrix is claimed.

## Verification completed

All **984 native library tests**, including ignored tests, passed in 246.01 s.
The 17 directed frame-related tests overlap that suite. Strict workspace/all-target
Clippy, explicit embedded-LLVM Clippy and warnings-denied private workspace docs
passed. A separate runtime test confirms name registration preserves UTF-8 without
allocating Dart objects. The cached real SDK inventory test passed in 17.10 s and
printed exactly eight certified roots; the original positive regression failed
before this change and is retained separately.

The unchanged JSON/UTF-8/Unicode/RangeError/three-isolate/base64 fixture agrees with
Dart 3.6.2 before/after production/development AOT and updated JIT on macOS, normal
and under GC stress. This turn does not repeat Windows/Linux execution. Runtime
sources are unchanged and existing bundles are not reexported. The production
platform reproduction still contains six foreign-backend definitions in both
default and experimental closed-class modes; this is an open gate, not a pass.

The CLI build succeeds with a linker warning: bundled LLVM compression objects
advertise macOS 27 while the deployment target is 15. Current-host execution does
not establish compatibility with older macOS releases. Global formatting still
fails; `fmt.json` binds the complete original output and `fmt-excerpt.txt` stores
only a normalized excerpt. Both new test files pass scoped rustfmt.

The full real File Explorer IR is byte-identical before/after: **293,593,137 bytes**,
SHA-256 `0e9cff28f0f9e9a04f86aecdccdf66d4fb9c123617e02014e2bd4d5c124e212d`. The UI source revision is
`15aff7042318d2fa56661ffed4360c81a41a86b8`, with a clean worktree. These runs
validate unchanged emission, not performance: other validation processes ran
concurrently. Compiler and artifact hashes are retained independently.
