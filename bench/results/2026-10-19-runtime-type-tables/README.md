# Runtime-created Type tables — 2026-10-19

The canonical Type object is created by the runtime RTI cache, outside ordinary
program AllocObject instructions and the positional runtime CID vector. Its
lowering already emitted a source Type parent edge and method table, but omitted
the complete-ancestry premise and table-function mapping. Both are now published;
ancestry is marked only when a distinct resolved parent exists. The closed-image
proof additionally protects the fixed Type CID unconditionally when its table is
present, even without explicit program allocations or a runtime CID vector.

This is table retention and declaration modelling, not permission to call unknown
native factories. The existing source/body/ABI checks remain. Treating the runtime
Type identity as nominal-only is rejected; the RTI factory is still outside the
proof's admitted native calls.

## Verification

- `before.log`: the new implicit-Type retention regression fails against 9cf1d3ba.
- `directed.log`: **34 class-proof tests pass**. The source lowering case checks
  the real Type parent edge, ancestry premise and table-function binding. The new
  compiled ABI fixture calls the actual runtime factory, checks its emitter CID,
  scalar field 0 and canonical identity across another allocation. It runs normally
  and under stress GC. This executable fixture does not apply closed-image proof;
  a separate negative test retains the factory's rejection.
- `native.log`: **833 passed**, none failed/ignored/filtered, with ignored native
  unit tests included against the final proof source and selected Dart 3.6.2 SDK.
- `identity.dart`, `results.json` and byte-exact outputs: fresh compiler
  `77853bc1f4d8b31dad97ff5e2e3544977c6ca71ee80763e64b73ed1ca2b61361`
  passes a real RTTI program in optimized AOT/JIT, normal/stress GC. Equality,
  canonical identity, Type text, nominal membership and stable identity hash match
  the Dart VM stdout. All **six commands** exit 0, including reference and build.
- Final strict workspace all-target Clippy and private docs with warnings denied
  pass. Global formatting still fails. The CLI build succeeds with existing
  macOS deployment-target linker warnings, not older-macOS compatibility proof.
- `production.log`: the unchanged platform pruning requirement still fails,
  Cargo exit 101. Type no longer causes the initial ancestry rejection; the
  namespace check rejects generated `dartforge_rti_init`/`dartforge_call_dart0`
  definitions. Their ownership/ABI and other startup/callback boundaries remain
  unproved. No prefix exemption or original assertion was weakened.

## Whole-workspace result and C toolchain prerequisite

The complete run begun at **9cf1d3ba** is terminal, **exit 101**. It completed Rust
compilation, passed the two ABI library tests, then stopped in the ignored native
C-call integration test before exercising the call: external LLVM Clang had no
configured C sysroot and the linker could not find libSystem. The frozen compiler
and preflight had succeeded; Cargo's Rust-link wrapper cannot configure the parent
environment inherited by a separate Clang process inside a test.

`workspace-baseline.json` retains the actual run metadata, exit, complete-log hash
and the settings of the subsequent configured ABI test. `workspace-failure.log`
keeps the failure section; the original full log remains at its recorded /tmp path.
With the installed SDK 26.5 and deployment target 14.0, the **unchanged** ABI test
passes its native call (result 42) and wasm32 object-header check (`configured-abi.log`).
These settings only configure child test processes; no machine defaults, scripts
or resident jobs were installed. CONTRIBUTING now documents the prerequisite.
The global suite has **not** been approved or rerun after that configuration fix;
remaining packages were not executed by that failed run.

No new complete workspace or Windows/Linux certification and no File Explorer
size reduction are claimed. This increment used macOS ARM64, LLVM 22.1.8 and
Dart 3.6.2. Timings are correctness metadata, not controlled performance figures.
Logs only normalize trailing blank lines, except stdout/stderr which stay exact;
receipts preserve original and stored hashes. Source hashes include the reviewed
runtime factory and unchanged ABI integration test. The live result manifest
records the pre-commit base revision and exact newly built compiler hash.
