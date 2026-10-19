# Native binary reader bounds — 2026-10-19

Baseline: `3e1aa0e5`. Host: macOS ARM64; source SDK Dart 3.6.2.

Five deterministic regressions fail against the baseline: overflow in fixed-width
reader ranges, acceptance of an overflowing u64 varint, frame-size multiplication
overflow, acceptance of inconsistent compact trace headers, and an impossible LLVM
record count causing a capacity-overflow panic. `check-readers.py` compiles the
baseline's two std-only modules verbatim, appending those five new tests. It runs
only those tests on the old implementation, avoiding hostile allocation-size
property cases on the old reader. All five fail (`readers-before.txt`).

The same script compiles the current modules verbatim with their tests: 13 pass
in both debug and optimized builds. A tiny target-selection stub selects macOS;
this harness is **not** a Windows/Linux or runtime test. It permits dead code in
the standalone harness only; no production lint suppressions were added.

The changes:

- Fixed-width reads check addition before slicing; overflowing byte ranges fail.
- Unsigned LEB128 consumes at most ten bytes, checking the last byte's payload.
- Compact DFGM/DFPC readers check table sizes before allocating, bound each
  function stream by the next, and check record indices and integer conversions.
  Root reuse needs a previous set. Frame/return/root/source-word overflow fails.
- LLVM map function/constant table sizes and record counts are bounded by section
  bytes before allocation; negative deopt counts are rejected.
- Function lookup uses subtraction for its range comparison, avoiding an overflow
  when a high-address function's declared size extends beyond u64.
- Mechanical native cleanup removes redundant casts/clones/field names and
  simplifies control flow. Strict workspace Clippy diagnostics for emit-native
  fall from 154 library / 192 test errors to 33 / 47. The remaining errors are
  not suppressed. CLI and LSP also still have Clippy errors.

`cargo-readers-before.txt` records the first four failing regressions in the full
crate (76 passed, four failed under the `reject` filter); this precedes the fifth
allocation-count regression. `first-native-suite.txt` records the intermediate
792-test successful library run. It predates the final count-bound regression and
some mechanical lint fixes; the final run is recorded separately in `checks.json`.
Do not add these overlapping test counts together.

The final full-crate attempt reports **820 passed, one failed**: 793 unit,
23 contract, two debugging and two of three production-profile tests passed.
`platform_constant_prunes_the_other_backends` fails: dead foreign constructors are
removed, but foreign getters/method tables remain in the emitted IR. Class-table
narrowing is currently opt-in and refuses imported SDK summaries. The test's
expectation has not been weakened. A frozen earlier producer exhibits the same
remaining definitions (`preexisting-platform-check.json` includes its hash); this
is not a fresh build of the baseline revision. A newly built CLI using the current
sources also emits the same remaining getters and no foreign constructors:
`current-platform-check.json` records the binary and IR hashes. The entire fixture
IR is byte-identical to the frozen producer's output; this is one fixture, not
general output equivalence. This turn does not fix complete foreign-backend
class-table removal.

Cargo stops after that failing integration binary. The remaining SDK-selection
binary is run separately (**one passed**), as are the doctests (**121 passed**). See `checks.json`
for the final statuses and `native-tests.txt` for the complete failing attempt.
No complete-crate or complete-workspace green claim is made.

## Reproduction

```sh
python3 bench/results/2026-10-19-native-reader-quality/check-readers.py
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-emit-native --target-dir target/impl-a -- --include-ignored
cargo clippy --locked -p dartforge-emit-native --all-targets -- -D warnings
```

The matching LLVM linker avoids the default Apple LLVM 21 / LLVM 22 bitcode
mismatch. Complete workspace checks still fail: format drift, Clippy errors and
the default link mismatch. Documentation builds with warnings. `checks.json`
records exit codes, full original log hashes and sizes; large workspace logs have
bounded excerpts. `source-hashes.json` identifies every changed native source.

## Limits

These are converter/reference readers, not the runtime's independent unsafe
readers. The binary format and encoder bytes are unchanged. Tests cover specified
malformed sizes/streams and valid round trips; they are not a fuzzing or arbitrary
object-file safety certification. Arithmetic elsewhere in object converters is
outside this change. No performance or full application compatibility improvement
is claimed, and this turn does not repeat VM tests. The full crate run requests
ignored tests, but the existing manual `compile_with_source_sdk` test returns
without compiling when `DARTFORGE_PROGRAM` is unset (as in this run); a reported
pass there is not evidence of an additional application execution. The Windows
link test can generate binaries on macOS; it does not prove Windows execution
without a Windows runner or Wine.

Text evidence trims trailing whitespace and trailing blank lines; recorded original
log hashes refer to the captured output before this normalization.
