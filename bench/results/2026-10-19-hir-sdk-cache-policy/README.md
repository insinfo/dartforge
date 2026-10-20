# Separate compiled SDK caches by HIR optimization policy — 2026-10-19

The cache investigation from the dense-dominator change is resolved for the
observed mismatch: `DARTFORGE_OPTIMIZE_HIR=0` changes SDK emission, but the
compiled SDK key previously omitted this policy. A disabled-HIR build could
populate the same cache entry later requested by a normal optimized build.

The key now calls the optimizer's actual `disabled()` policy and includes
`hir_optimization=disabled_v1` for disabled passes. This applies to development
objects/shared SDKs and production bitcode/summaries. Absent and `1` share the
enabled policy. The source/class catalog key stays independent of this switch.
Linux's link-only RELR tag still affects only the shared development SDK.
The source fingerprint of the updated compiler invalidates old SDK entries;
no existing cache directories or compatibility manifests were edited.

## Reproduction before the fix

The new subprocess regression first failed with identical enabled/disabled
production keys; `before-fix.log` preserves the failing assertion (exit 101).
It uses one shared fake SDK root for all subprocesses, avoiding path salts or
process-global environment mutations. Existing RELR assertions are retained.

The retained old compiler, SHA-256
`0945d0980bc86c875156713f4b00c28f1063d40d2442a602730a05c76ffcfbcd`, was then run
with `DARTFORGE_OPTIMIZE_HIR=0` and a separate empty native cache. It generated
the same SDK key `5de6da02057aa61971140a41b1bc5bba` and reproduced **all 15
pruning summaries byte for byte** from the previously unexplained cached SDK.
`old-disabled-sdk-reproduction.json` records every hash and equality result.
For example, `core.poda` is 859,332 bytes with SHA-256
`a2ca14a2d779c94d1fc7fb1746c123004c75d33cbdc206c61141920c0c930d5f`.

This ties the earlier cached-versus-reconstructed IR difference to the missing
policy, rather than to the bitmap representation. The earlier artifacts and
failed comparison remain in `../2026-10-19-dense-dominators/`.

## Fixed key regression and real SDK integration

`after-fix.log`: seven selected-source/cache tests pass, including the new
regression across Linux ARM64, macOS ARM64 and Windows x64 target selection,
with RELR absent/enabled. It checks both compiled profiles, absent/`1` equality,
disabled-mode separation, and unchanged source keys. These are key tests on the
host, not new VM execution tests for three operating systems.

`validate-cache.py` then used a fresh, isolated cache with the updated compiler
and the full Dart 3.6.2 SDK. It emitted the branch fixture in three subprocesses:

| HIR policy | Ready compiled SDK entries | Result |
|---|---:|---|
| Absent (enabled) | 1 | Builds enabled SDK |
| `0` | 2 | Builds separate disabled SDK |
| `1` | 2 | Reuses enabled SDK |

`runs.json` binds every `.o` and `.poda` file after each step. The original
enabled object's/summary's hashes stay unchanged while the disabled entry is
created, and the third step preserves both inventories. Class-catalog TSV names
and hashes remain identical throughout. Default and explicit-enabled IR are
byte-identical; disabled IR differs as expected. `verification.json` records
these assertions. This integration exercises development SDK artifacts; the
production-key separation is covered by the subprocess regression and the native
library suite's existing production integrations.

The fixed default IR also matches the reconstructed old-compiler reference:
SHA-256 `10c77f9a1e938e5be15521bf4550dfdb8d8ecf274270a09dda9ec159d2635399`.
`reference-equivalence.json` records this comparison. The fixture source is
`../2026-10-19-dense-dominators/branches.dart`; its hash is recorded in each run.
This change adds no new executable/JIT/desktop execution matrix or performance
claim. Cold SDK creation and warm reuse timings in the logs are not equivalent
benchmark scenarios.

## Checks and remaining scope

- Native backend library: **921 passed, zero failed**, including ignored tests.
- Selected source/cache tests: **7 passed, zero failed**, overlapping the above.
- Strict workspace/all-targets Clippy and explicit embedded-LLVM backend Clippy
  passed.
- Workspace private documentation passed with `RUSTDOCFLAGS=-D warnings`.
- The updated native compiler built successfully.

`run-checks.py` records commands/environment (LLVM 22.1.8, source Dart SDK 3.6.2,
macOS SDK 26.5, deployment target 15.0). The original broad workspace suite is
still executing builders with its unchanged old compiler snapshot; it is not a
pass for the updated workspace. Global formatting drift remains unresolved.
This is a fix for the identified policy omission, not a proof that every other
code-generation environment option is covered by all caches.

Raw SDKs, full IR and compiler binaries stay in ignored `target/` storage.
`retention.json` binds original/stored records with trailing whitespace normalized.
`source-manifest.json` and `SHA256SUMS` bind the changed source and retained evidence.
