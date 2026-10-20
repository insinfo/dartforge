# Workspace continuation and test prerequisite checks — 2026-10-19

The preserved original workspace run completed the native builder corpus (2,428.12 s)
and all four Angular transparency tests (557.38 s), including the per-file Dart edit
comparison against independent clean runs. It subsequently stopped at the JavaScript
basic suite: all 15 tests failed because Node was absent from PATH. The complete
original log is retained; these failures have not been rewritten as passes.

The existing Node v24.21.0 installation was added to PATH for a separate execution
of the exact JavaScript test executable. All 15 tests passed in 8.32 s. The executable
hash, command and selected environment are recorded in observation.json. No compiler
or JavaScript emitter source was changed to obtain this result.

The PowerShell check runner now checks Cargo/Dart/Node availability before invoking
Cargo and includes ignored tests as required by CONTRIBUTING.md. A process check with
only Cargo/Dart stubs on PATH rejected missing Node before either stub ran. PowerShell
environment discovery also checks fallback drive existence before Join-Path, avoiding
an unrelated missing E: drive exception on this macOS host. This guard has not been
executed on Windows in this increment. The native macro compiler variable is documented.

A separate Cargo continuation includes ignored tests and excludes dartforge-build,
whose suites completed above. It was still linking when this observation was recorded.
This is not a global pass: the continuation lacks the native-macro compiler variable,
formatting remains unresolved, and the original builder run used a preserved earlier
compiler. No new desktop AOT/JIT matrix or performance improvement is claimed.
