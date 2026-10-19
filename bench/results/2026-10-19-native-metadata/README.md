# Native emission metadata types — 2026-10-19

Baseline: `134d114b`. Native Clippy diagnostics decreased from 32 to 10 for the
library and from 47 to 25 for tests, without suppressions. This is an incomplete
quality gate, not a strict-Clippy pass.

Named types now describe immutable dispatch-cache keys/results, startup method
tables, root-liveness visitors, async stub locals/success-edge writes, Ref Phi
definitions and ARC function protocols. `MethodTable` is a transparent tuple
alias with a public API example. The underlying representations, locking,
ownership protocols and control-flow algorithms are unchanged.

The debug-metadata append helper is named `add_metadata`, distinguishing it
from construction. Stack-check table metadata is initialized in its struct
literal; SDK cache pruning uses an equivalent stable descending key sort.
An emission-helper documentation paragraph is separated from its preceding
list. There is no new performance or binary-size claim.

## Validation

- All **822 native unit tests** pass with `--include-ignored`, no filter, using
  the real Dart 3.6.2 source SDK and LLVM 22.1.8. This includes emitted HIR/LLVM,
  root placement, async/ARC transformations, compiled dispatch/closure GC-stress
  fixtures, SDK inventory boundaries and a self-contained SDK executable.
- All **122 doctests** pass, including the new method-table example and the
  existing compile-fail analysis example.
- Strict workspace documentation passes with `RUSTDOCFLAGS='-D warnings'`.
- Workspace formatting and strict Clippy still fail. Remaining native Clippy
  diagnostics include CFG counters, long argument lists and test setup code.

Commands and terminal exits are in `checks.json`, `workspace-checks.json` and
`environment.json`. Full package logs and the successful documentation log are
retained here. `stored-logs.json` records original and stored hashes; only blank
lines at the end of stored logs are trimmed. `source-hashes.json` binds the ten
changed Rust sources. This validation ran on macOS ARM64, not inside the VMs.

The unit suite is not the entire native integration matrix. In particular, the
existing full foreign-backend removal requirement remains open; these changes
do not strengthen the experimental class-reachability proof or claim a smaller
File Explorer executable.

The independent full workspace test process remains active at `81e6762b`,
without builder filters. Its frozen compiler still has SHA-256
`f3284c9d610665e9325899ec0036ddfeb058adb9bc375ee741905c376b8f30f8`;
the compiler executable was not rebuilt or replaced during this run. This
ongoing result does not certify the source delta here. Metadata/logs remain at
`/tmp/forge-workspace-configured/` for continued observation.
