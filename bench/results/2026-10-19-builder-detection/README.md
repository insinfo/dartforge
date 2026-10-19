# Builder detection uses configuration YAML semantics — 2026-10-19

The previous line scanner recognized only unquoted, unindented keys ending at
the colon. Valid flow mappings and quoted keys could therefore disable source
generation. Unreadable or malformed configuration could also be treated as
absent, allowing a stale generated file to compile. Conversely, empty sections
and definitions in later YAML documents could activate an unnecessary engine.

Detection now uses `config::load_yaml`, including its selected first-document
semantics. Nonempty definition maps require the engine; missing/null/empty
sections and nested target options do not define builders. Invalid YAML, wrong
root/section shapes and unreadable files require the engine's strict validation.
The explicit resolved `build_runner` trigger is retained. This predicate is not
a complete schema validator; other schema rules still belong to the engine.

## Verification

- `detection-before.log`: four of five new detector tests fail on the original
  scanner. The resolved-runner control passes.
- `detection-after.log`: all five pass. Valid definitions are also parsed by the
  full configuration reader, so an unsupported fixture is not counted as a
  missed valid builder. Cases cover quoted/flow/indented/explicit/anchored keys,
  post-process definitions, empty sections, nested options, selected documents,
  bad shapes, malformed text, invalid UTF-8 and a directory named `build.yaml`.
- `cli-before.log`: the two new CLI regressions fail against an immutable copy
  of the preceding compiler; the two existing generated-IR tests remain passing.
  Compiler SHA-256 values are recorded in `compilers.json`.
- `cli-after.log`: all four integration tests pass, with ten actual compiler
  invocations. The two new tests exercise generation from quoted and flow
  definitions, plus rejection of malformed/ill-typed/non-UTF-8 configuration in
  the presence of a stale source. Previous IR and contract files stay intact.
  The isolated i18n fixture uses synthetic package metadata and a native port;
  it is not an ecosystem compatibility oracle or an external builder execution.
- `zero-cost.log`: the existing structural gate now includes empty definition
  sections and no lockfile. No engine or stage is constructed, and each body
  edit has exactly the allocation count of the control session without stages.
  This does not measure initial YAML parsing cost, elapsed time or memory usage.
- `build-tests.log`: 60 tests pass with ordinary all-target selection; 18 are
  ignored by that command. The new detector tests are included in the 60.
- `doctests.log`: all 16 build-engine doctests pass, including the public
  detection example.
- `corpus.log` / `corpus-counts.json`: the additionally selected ignored corpus
  test passes over 11 projects, with ten equal plans and one no-engine control.
  Outputs: 15 equal, 45 pending, zero different. Pending outputs lack an executor
  or fallback; this does not certify execution of those builders. No Dart/native
  builder executor was attached in this corpus run. The remaining ignored
  VM/native-executor integrations were not run in this increment.
- `build-clippy.log` and `cli-clippy.log`: strict all-target Clippy passes with
  `--no-deps`; CLI uses all features. No new lint suppression is added.

## Reproduction and limits

Executed on macOS ARM64, Rust 1.98.1, LLVM 22.1.8 and macOS SDK 26.5. Native CLI
tests use the Dart 3.6.2 source SDK. No new Windows/Linux runtime claim is made.

```sh
cargo test --locked -p dartforge-build --all-targets --target-dir target/impl-a
cargo test --locked -p dartforge-build --doc --target-dir target/impl-a
cargo test --locked -p dartforge-dev --test zero_cost --target-dir target/impl-a
cargo test --locked -p dartforge-build --test corpus --target-dir target/impl-a corpus_builders_plan_and_scoreboard -- --ignored --nocapture
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
export DARTFORGE_TEST_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
unset DARTFORGE_TEST_COMPILER
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-cli --features jit --test generated_ir --target-dir target/impl-a -- --nocapture
```

The old CLI replay uses `DARTFORGE_TEST_COMPILER` and deliberately fails two
tests. Eleven corpus package configurations were present; the selected corpus
test did not skip missing prerequisites. `sources.json` binds changed sources;
`log-hashes.json` records complete local-log hashes and exits. CLI excerpts omit
cached linker warnings. Root-check log excerpts and `workspace-checks.json`
record the four required workspace checks: existing formatting, Clippy and
Apple/LLVM-22 linking failures remain; documentation builds with warnings.
These failures remain unresolved.
