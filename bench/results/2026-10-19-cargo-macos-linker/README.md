# Automatic macOS Cargo LLVM linking — 2026-10-19

Bare Cargo previously used Apple's LLVM-21 bitcode reader for the LLVM-22 static
archives, failing with `Unknown attribute kind (105)`. The earlier helper worked
only when explicitly wrapping each Cargo invocation. The repository now selects
a macOS Cargo linker wrapper for both Apple Rust architectures.

Ordinary direct links execute system cc without Python or LLVM discovery.
Nested response-file inspection also requires Python, even without LLVM. For LLVM
inputs, Python inspects archive headers and member magic, including BSD extended
names and GNU members, without reading whole archives into memory. Native-only
archives and shared LLVM selection keep system cc. Raw/wrapped LLVM bitcode
selects Clang/LLD from the configured LLVM installation and a probed compatible
SDK. Both bundled rlibs and unbundled `-lLLVM*` library search are handled.

Original arguments and response files stay intact. Explicit linker selections
take precedence; malformed inspected archives and recursive response files fail
clearly. The shared-library cases are controlled fixtures: the actual installed
LLVM distribution used here supplies static archives.

## SDK policy

Rustc supplies SDKROOT to its linker even when Cargo's caller left it unset.
Thus SDKROOT equal to the active SDK is negotiated automatically. Here, the
active SDK 27 stubs contain an architecture unknown to LLD 22; the existing
helper probes installed siblings and selects SDK 26.5. A custom nondefault
SDKROOT is preserved. `DARTFORGE_MACOS_SDK_ROOT` pins even the active SDK and
rejects an incompatible choice; contradictory explicit SDK choices fail.
The explicit Cargo helper also accepts this pin and retains its original strict
treatment of a caller-supplied SDKROOT. Neither entry point changes global
machine settings or downloads SDKs.

## Verification

- `before.log`: ordinary Cargo linking of the real cross-object test fails with
  the Apple bitcode reader at baseline edd5a9dd, even with SDK 26.5 selected.
- `llvm-tests.log`: ordinary Cargo passes all **17 LLVM tests**, invoked from
  crates/llvm, without SDKROOT or manual linker flags. This includes real object
  emission for five destinations, not execution on those foreign systems.
- `python-tests.log`: **19 tests** cover archive classification and bounds,
  explicit linker/SDK choices, rustc's injected SDK, response files and names
  beginning with a dash, paths with spaces, unbundled LLVM library lookup,
  discovery failures and preservation of unrelated arguments. A shell fixture
  with only cc on PATH proves that ordinary links need neither Python nor LLVM.
- `cli-build.log`: the real CLI with `--features jit` builds through bare Cargo,
  without SDKROOT, encoded Rust linker flags or the previous helper invocation.
- `cli-default-tests.log`: **33 CLI unit tests** pass with both LLVM prefix
  variables unset. `jit-doctests.log`: **six JIT doctests** pass without
  RUSTDOCFLAGS overrides, including compile-only examples.
- `jit.out` equals `oracle.out`: the newly built CLI executes smoke.dart in JIT
  and preserves both arguments and the typed closure result, matching Dart 3.6.2
  byte for byte. An initial development run lacked runtime ld64.lld on PATH
  (`initial-jit-missing-tool.err`); exposing the complete LLVM bin directory
  resolved that prerequisite. Cargo prefix selection does not modify the
  caller's runtime PATH. No tool-free standalone packaging claim is made here.
- `builder.log`: with DARTFORGE_COMPILER pointing at this new CLI, the selected
  native executor case cadeia_configuracao produces **22 equal outputs**, zero
  pending/different, and three edits match from-scratch execution. This is one
  native builder case, not the complete builder corpus.
- Required root checks: formatting and strict Clippy still fail, documentation
  builds with existing warnings. Workspace tests compile/link successfully and
  **227 tests pass before failure**: the native-builder test requires
  target/release/dartforge, absent in this checkout. The failed global run is
  retained as such; the configured single-case repetition does not make the
  entire workspace green. No previous test assertion was relaxed.

The macOS LLVM action no longer overrides Cargo's linker or rustdoc flags; it
runs the Python selector tests instead. Its YAML parses locally. This does not
claim a completed hosted CI run or new Windows/Linux runtime validation.

## Reproduction

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export LLVM_SYS_221_PREFIX="$DARTFORGE_LLVM_DIR"
unset SDKROOT
python3 -m unittest discover -s scripts -p 'test_*macos*.py' -v
cargo test --locked -p dartforge-llvm --all-targets --target-dir target/impl-a
cargo build --locked -p dartforge-cli --features jit --target-dir target/impl-a
cargo test --locked -p dartforge-jit --doc --target-dir target/impl-a -- --test-threads=1
```

JIT smoke and native-builder runs additionally expose the LLVM bin directory
and select the Dart 3.6.2 source SDK. Full workspace ignored tests need the
prerequisites documented in CONTRIBUTING.md, including a current compiler.
Tests ran on macOS ARM64 with LLVM 22.1.8. Deployment-version and unused native
search-directory warnings remain in passing link logs. SDK selection and caller
override booleans are recorded separately; compiler/source hashes and complete
local-log receipts bind the evidence. Checked-in excerpts omit repeated linker
warnings and bound long diagnostic lines. Elapsed times are not controlled
performance comparisons.
