# Strict workspace Clippy — 2026-10-19

Baseline: `71b35f1d`. The native package and the entire workspace now pass
Clippy with all targets and warnings denied, without new suppressions:

```sh
cargo clippy --locked -p dartforge-emit-native --all-targets --no-deps --target-dir target/impl-a -- -D warnings
cargo clippy --locked --workspace --all-targets -- -D warnings
```

These commands use the default workspace feature selection on macOS ARM64.
They do not certify `--all-features`, Windows/Linux execution or runtime
behavior by themselves. `clippy-checks.json` and the full success logs retain
the commands and terminal results. `native-clippy-before.log` retains the
previous 10 library / 25 test diagnostics.

## Change

Field-write premises, ARC insertion options and allocation-export audit
evidence have named containers. Pattern matching pairs the operand with its
source expression, and inline-copy locations pair the block/instruction
indices. The same premises, flags, rejection conditions and operands reach
the existing implementation; no proof boundary was relaxed.

Root placement enumerates the per-block Phi definitions. Async/ARC edge
creation pairs each finite edge plan with its successive block ID. The finite
plan is the first iterator, so exhaustion does not advance the ID range after
the last edge. Existing checked ARC ID conversions and insertion order remain.

Test fixtures use equivalent struct/vector initializers and borrowed slices;
the independent three-variable graph oracle retains its Floyd closure and all
512 graphs. A no-op string replacement is removed from a JIT delta fixture.
The differential harness uses equivalent Option/condition expressions and its
test module moves after production items. No acceptance expectation, golden
output, corpus gate or test inclusion rule was weakened.

## Validation

- **822 native unit tests**, including ignored tests, pass against the real
  Dart 3.6.2 source SDK. The suite includes root placement, pattern lowering,
  async/ARC transformations, compiled dispatch/closure stress-GC fixtures,
  SDK inventory rejection boundaries and a self-contained SDK executable.
- **122 native doctests** pass.
- **45 JIT library tests** pass, including delta invalidation, migration and
  emitted layout/type-identity rejection checks.
- **20 differential library tests** pass, including report normalization and
  oracle parsing. This is not a rerun of the ignored JS/DDC corpus.
- Workspace documentation passes with `RUSTDOCFLAGS='-D warnings'` and private
  items included. Workspace formatting still fails on existing repository drift.

`tests.json`, `support-tests.json` and `final-checks.json` contain exits and raw
log hashes. Full logs are retained here; `stored-logs.json` also records stored
hashes after trimming only trailing blank lines. `environment.json` records
toolchain/SDK settings, and `source-hashes.json` binds all 27 changed Rust files.
This increment makes no performance, binary-size or new VM-validation claim.
Full foreign-backend removal remains an open requirement.

## Independent workspace test run

The full workspace test run was started at `81e6762b` with all prerequisites
and no builder filter. Its native builder corpus test **passed in 2410.20 s**:
the existing gate requires complete oracle-equivalent output and incremental
state equal to a clean engine after every selected edit. The corresponding
test section is retained in `baseline-builder-excerpt.log`.

That process is still active in Angular transparency tests at this snapshot;
it has no final workspace result and does not certify the source delta here.
`baseline-progress.json` records a live process observation and verifies the
original compiler SHA-256. The compiler executable was not replaced while the
run was active. Continued logs/process metadata remain at
`/tmp/forge-workspace-configured/`.
