# Local async-frame metadata — 2026-10-19

`_AsyncFrame` registration followed the global event-loop flag rather than local
frame allocation. A synchronous program importing async facilities therefore
registered a frame class it never allocated; a directly lowered generator module
without that flag omitted the metadata for frames it actually allocated.

Frame registration now follows an AllocObject census after source pruning. The
event-loop flag still independently generates the closure callback and loop call.
No async body, callback, event-loop requirement or pruning assertion was disabled.
Actual frame allocations still emit their class metadata with the existing
variable-layout ABI; this does not certify their ancestry for closed-class pruning.

## Validation

- `before.log`: both regression cases fail against lowering at 1725436b, for the
  phantom frame declaration and missing metadata of actual generator allocations.
- `directed.log`: both pass with final sources. The callback test explicitly
  checks that the event-loop call is retained when required. The async/generator
  fixture retains four per-resume-body allocation records and their emitted IR.
- `native.log`: **830 native unit tests pass**, with `--include-ignored`, against
  the final source and the Dart 3.6.2 source SDK.
- `results.json`, `callbacks.dart`, `frames.dart` and captured outputs: a freshly
  rebuilt native/JIT compiler executes two real programs. The first schedules a
  microtask and timer without local async state machines. The second uses an
  object live across await, sync* and async* generators and await-for. All
  **12 commands** exit 0: two Dart VM references, two optimized AOT builds, and
  eight AOT/JIT executions across normal/stress-GC modes. All executions match
  the corresponding VM stdout exactly. These are default-mode executions, not
  certification of experimental closed-image pruning.
- Compiler SHA-256: `a4251bcabb8cddf1df87f626461a368f207139fecd7a4f721e4b2f272f7c9e7e`.
  `build.log` records the successful native/JIT build and its existing macOS
  deployment-target linker warnings; this is not older-macOS compatibility proof.
- Workspace strict all-target Clippy and private documentation with warnings
  denied pass. Global formatting remains failing (`checks.json`).
- `production.log`: the unchanged platform-pruning regression still fails,
  Cargo exit 101. Without a local frame allocation its first observed rejection
  is now `_Type` (CID 1073741569), whose ancestry is unproved. This does not
  authorize allocated async frames, imported startup or unknown native boundaries.

Executed on macOS ARM64 with LLVM 22.1.8 and Dart 3.6.2. No new Windows/Linux or
complete workspace pass and no File Explorer size reduction is claimed. Executable
artifacts remain under ignored target paths and /tmp. Timings are correctness-run
metadata, not controlled performance measurements. Logs normalize trailing blank
lines except stdout/stderr, which remain byte-exact; receipts retain both hashes.
The result manifest records the source hashes and pre-commit base revision.
