# Embedded numeric clamp certificate regression

The opt-in primitive clamp certificate still required runtime numeric natives
although the reviewed bindings now lower to HIR. It therefore rejected the
selected SDK even when all source and alias premises held. The implementation
now requires the exact builtin binding state for the existing numeric census;
source hashes, ownership, signatures and alias rejection remain in force.

Reproduce with `scripts/validate-primitive-clamp.py --compiler <dartforge>
--sdk <Dart SDK root> --llvm <LLVM root> --packages <dart_ui package_config.json>
--out <empty output directory>`. The SDK used here is 3.6.2 and LLVM is 22.1.8
on macOS ARM64. The producer is hashed and unchanged throughout both profiles.

The validation compares 373 clamp and 2,736 color conversion output lines with
the official VM and official AOT. DartForge OFF/ON runs also use GC stress.
Exact IEEE fingerprints, returned object identity, evaluation counts, custom
receivers, invalid ranges and exceptions are consumed by the fixtures. Retained
LLVM confirms NaN guard counts of 0/0/0 (OFF) and 1/3/0 (ON) in `unitClamp`,
`typedUnitClamp`, `genericClamp`. This records exercised fast paths, not a speed
claim. The optimization remains experimental and disabled by default.

`validation.json` retains commands, hashes and exact-comparison outcomes;
`*-oracle.stdout` retains the reference output. Executables and full emitted IR
remain in ignored `target/primitive-clamp-embedded/`.
