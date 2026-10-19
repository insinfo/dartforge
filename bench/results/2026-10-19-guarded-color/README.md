# Guarded color context specialization

The existing identity certificate does not prove immutable heap field contents.
The new pass keeps each enum index load and tests the observed value. A miss
calls the original body with the real ABI arguments and forwards result/pending
exception. Replay is restricted to a unique effect-free entry prefix; calls,
allocations, stores, joins, cycles and unknown receivers are barriers. Removed
switch edges are removed from successor phis. Analysis has explicit 32-guard,
32-block and 256-prefix-instruction limits; existing growth budgets are unchanged.
The experiment remains disabled by default.

`validate.py --compiler <dartforge> --sdk <SDK root> --llvm <LLVM root>
--packages <dart_ui package_config.json> --out <directory>` compares 89 constant
format outputs, the enum-pattern fixture and 2,736 color outputs with official
VM/AOT, OFF/ON, normal and GC-stress. The ON fixture must retain a guarded clone.
Use `python3` to invoke the scripts. Native hit/miss and pending-exception tests
also execute a hand-built HIR module with a deliberately changed observed index.

`performance.py` accepts the same options and `--iterations` (default 1,000,000).
It builds all three artifacts, checks the measured caller/clone IR, rotates
OFF/ON/official order over three process rounds and retains all five samples in
each process. First in-process samples are excluded; the result is the median
of the three per-process warm medians. Checksums are consumed and must agree.
Correctness and performance are separate validations on macOS ARM64, SDK 3.6.2,
LLVM 22.1.8 with SDKROOT pointing to macOS SDK 26.5.

The final medians are 32,373 / 30,338.5 / 5,743 microseconds (OFF/ON/official).
This is about 6.3% less time in this kernel, still about 5.3x the official time.
Both DartForge files have 3,096,624 bytes; native __text increases by 1,368 bytes.
Official Mach-O launcher text excludes its appended AOT snapshot and is not used
as a code-size comparison. The first measurement is retained separately; emitted
application hashes match the final round. No universal speed or application-size
claim is made. Binaries/full LLVM stay in ignored target directories.

All 777 default native unit tests and 15 focused specialization tests pass;
nine native tests remain ignored. Workspace formatting, Clippy and the default
Apple linker gate remain non-green; documentation builds with warnings.
