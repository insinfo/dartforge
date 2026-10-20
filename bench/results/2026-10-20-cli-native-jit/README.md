# CLI verification with native and JIT features — 2026-10-20

Command:

```text
cargo test --locked -p dartforge-cli --features native,jit --no-fail-fast -- --include-ignored
```

Completed on macOS ARM64 with **exit 0: 82 tests passed across 14 executables,
zero failed, ignored or filtered**, in 361.75 seconds including compilation.
The command's baseline commit is `07d5741d`; subsequent changes during execution
are reporting, checker and documentation only. Compiler Rust sources remain
at `dac116ab`. Two build jobs and two test threads were used.

Coverage includes 38 CLI unit tests, publication (4), SDK path (2), contracts (2),
destination validation (1), debugging (1), directed maps (4), generated IR (4),
native I/O (7), JIT programs (4), memory policy (3), native generated sources (2),
stateful reload (7), and subsystem options (3). The generated-source corpus has
its package configuration present, so those two tests do not take their
missing-configuration early return. Some tests also ran in the baseline
workspace: 82 is this execution's total, not 82 additional distinct tests.

The I/O suite exercises TLS/HTTPS and certificate/key formats, renegotiation
rejection, shared listeners, signal bursts and main arguments. JIT execution
checks arguments and package-resource selection through run/reload/restart.
Memory tests select tracing versus ARC explicitly. Reload covers persistent
statics, three consecutive generations, added/removed classes, changed fields
and signatures. These are existing assertions; none was weakened or skipped.

The original command log, terminal metadata, launch script and live-process
environment audit are retained. No corpus filters, compiler-test override,
pruning bypass or inherited GC stress was present. Both oracle roots were
configured. Trailing whitespace/final blank lines in the stored log are
normalized; manifest.json retains original and stored hashes.

The local checker and CONTRIBUTING now include this second command explicitly:
the ordinary workspace command alone does not enable native/JIT CLI features.
PowerShell parsing and git diff whitespace checks pass. No new compiler/runtime
code, SDK bundles or VM execution matrix are part of this increment.

The linker warning about LLVM compression objects built for macOS 27 versus
the configured minimum 15 remains visible: this run does not certify older
macOS releases. The separate workspace foreign-backend gate still fails, and
global formatting remains open. This successful CLI run does not override
either failure or constitute complete workspace approval.
