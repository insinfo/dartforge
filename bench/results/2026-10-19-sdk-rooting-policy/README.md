# Bind SDK caches to the effective callee-rooting policy — 2026-10-19

The follow-up SDK cache audit found that `DARTFORGE_ROOTS_IN_CALLEE=1` changed
emitted SDK bodies without changing their compiled cache key. The new regression
first demonstrated an SDK-shaped caller with one shadow-stack argument store
under the default policy and none with callee rooting, while both compiled
profiles still had identical keys. `before-fix.log` preserves this failure.

The emitter and compiled SDK key now share the same once-per-process decision,
including its existing tracing/shadow-stack restriction. Active callee rooting
adds `callee_argument_roots=enabled_v1` to both compiled profiles. ARC and maps
keep it inactive. Source/class catalog keys remain independent of this switch;
unset/`0` remain equivalent. This changes cache selection, not the existing
experimental rooting algorithm or its off-by-default behavior.

## Audit scope

| Control | SDK consumption | Cache treatment |
|---|---|---|
| `DARTFORGE_ROOTS_IN_CALLEE` | `LlvmEmitter::callee_roots_the_arguments` | Tag effective active policy |
| `DARTFORGE_OUTLINED_HELPERS` | SDK modules reject program helper overrides | No additional SDK tag |
| `DARTFORGE_FIELDS_BY_CALL` | Program emission assembly in `lib.rs` | No additional SDK tag |
| `DARTFORGE_PRIMITIVE_CLAMP` | Selected-source certificate explicitly excludes SDK modules | No additional SDK tag |

The exclusions follow the SDK call path and explicit guards, not merely variable
names. `Generation::description` already records an applicable object pipeline,
and `sdk_generation` fixes SDK prelink optimization independently of the program's
prelink override. This is a scoped audit, not certification of every environment
option and cache in the compiler.

## Regression and real SDK integration

`after-fix.log` records eight selected-source/cache tests passing. The new test
uses **72 isolated subprocess states**: three target selections, tracing/ARC,
four controls, and absent/`0`/`1`. In active tracing, the emitted SDK probe's
root stores are `[1, 1, 0]` while both compiled profile keys separate the enabled
state. In ARC, the probe and keys stay stable across this switch. The shared
predicate is also checked directly for inactive maps/ARC. Source keys remain
stable within every target/memory/control case. `control-observations.json`
summarizes the original rows, whose bodies remain in the raw test log.
These target selections are host-side key/emission tests, not VM execution on
three operating systems. The probe samples an SDK caller; source review supplies
the stronger exclusion rationale for program-only controls.

`validate-cache.py` used the full Dart 3.6.2 SDK in a separate empty native cache
on macOS ARM64, explicitly selecting tracing/shadow roots:

| Policy | Ready SDK entries | Observation |
|---|---:|---|
| Default | 1 | Builds default SDK |
| Callee rooting `1` | 2 | Builds separate enabled SDK |
| Callee rooting `0` | 2 | Reuses default SDK |

The `.o`/`.poda` inventories and source class-catalog TSV hashes were recorded
after every step. Creating the enabled SDK does not replace the default objects
or summaries; the final step preserves both inventories. Default/disabled IR
match byte for byte and equal the reconstructed reference from the earlier
investigation. Enabled IR differs as expected. Commands, compiler/source hashes,
inventories and assertions are in `runs.json` and `verification.json`.

`validate-execution.py` compiled the branch fixture with each rooting policy.
Both native executables printed `-86488`, matching Dart 3.6.2, normally and under
`DARTFORGE_GC_STRESS=1`. `execution.json` records all seven commands and outputs;
`executable-bindings.json` binds binaries and checks their SDK dylib dependency
against the corresponding cache key. The fixture source is retained in
`../2026-10-19-dense-dominators/branches.dart`.

This real-cache integration exercises development SDKs on macOS; production
key separation is covered by the regression. It does not certify every rooting
scenario, mapped/ARC runtime behavior, or add a new desktop/JIT matrix or speed
claim. No existing cache or compatibility manifest was rewritten.

## Checks

- Native backend library: **922 passed, zero failed**, including ignored tests.
- Selected-source/cache suite: **8 passed**, overlapping the library total.
- Strict workspace/all-targets Clippy and explicit embedded-LLVM backend Clippy
  passed.
- Workspace private documentation passed with warnings denied.
- The updated native compiler built successfully.

`run-checks.py` records LLVM 22.1.8, source Dart SDK 3.6.2, macOS SDK 26.5,
deployment target 15.0, and exact commands. The original broad workspace suite
is still executing builders with its unchanged old compiler snapshot; neither
that full gate nor global formatting is approved by these results.

Full SDKs, IR and binaries stay in ignored `target/` storage. `retention.json`
binds original/stored text with trailing whitespace normalized. Source and
evidence hashes are in `source-manifest.json` and `SHA256SUMS`.
