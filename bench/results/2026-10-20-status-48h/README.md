# 48-hour status audit and fresh measurements — 2026-10-20

The historical audit covers 2026-10-18 14:33:15 through 2026-10-20 14:33:15,
UTC-3. DartForge HEAD is bf4129985ec7017633a4b7371b1c0e940eca5bc8, with 102
commits in that window. `history.json` lists every commit, timestamp, subject
and changed path. `ui-history.json` independently lists 20 dart_ui commits.
The new measurements ran after the cutoff against those unchanged compiler
sources. ESTADO.md contains the Portuguese component summary, measured tables,
remaining failures and local Parallels reproduction commands.

## Fresh JavaScript measurement

Build the two optimized host tools using the checked-in release profile
(ThinLTO, one codegen unit):

```sh
cargo build --locked --release -p dartforge-cli --target-dir target/status-48h-build
cargo build --locked --release -p dartforge-emit-js-production --target-dir target/status-48h-build
python3 bench/results/2026-10-20-status-48h/measure.py
```

Both builds passed. The first build's terminal reported 1m13s; the second
reported 53.79s, with the shared Cargo target lock. These are setup observations,
not compiler performance measurements. The scripts use this installation's
explicit tool paths. `bindings.json` hashes both compiler binaries, Dart, DDC
snapshot and the shared SDK JavaScript. `commands.json` retains every measured
command, exit, output, elapsed time and load observation. `measurements.json`
contains all three samples/tool/input and hashed deployment files.

One warmup precedes three timed fresh processes per tool/input; execution order
reverses every other round. The filesystem is warm, not OS-cache cold. Timings
include startup and output writing. DDC is invoked directly with its snapshot,
without build_runner/webdev. This does not measure persistent DDC workers,
incremental edits, browser readiness, large applications or clean-checkout
generation. The two VMs were suspended and no other compiler build was active
during these measurements. No physical-memory comparison was made.
The recorded one-minute load ranged from 4.94 to 5.47 for JS commands and
4.45 to 4.89 for native commands; this was not an isolated laboratory host.

The three inputs are checked-in hello, class and JSON fixtures. Each of four
emissions per input ran under Node and matched the official VM's complete
stdout. This is 12 functional comparisons, not language-wide equivalence.
Production uses default checked options versus dart2js -O2, not omitted checks.

Development sizes include application modules and launcher plus exactly one
copy of the same dart_sdk.js, whose prebuilt availability is a prerequisite
for both. Production sizes include the standalone emitted JS file. Source maps
are excluded. Gzip9 is compressed independently per file and summed, with a
fixed timestamp. DDC's emitted import is rewritten to a local file URI and a
small Node launcher is added after compilation; those bytes are included in the
reported deployable size. Forge already emits its launcher and copies the SDK.
The scripts and large generated binaries/modules remain under ignored target/.

## Fresh native measurement and Explorer size

```sh
python3 bench/results/2026-10-20-status-48h/measure-native.py
```

`native-commands.json` records every command/exit/stdout/stderr, including retained
LLVM deployment warnings. `native-measurements.json` binds the debug-built native
driver and source inputs, with optimized executable hashes and sizes. Five
alternating process runs provide four internal samples/kernel, twenty samples
per variant/kernel. The raw samples, minima and medians are retained. All kernel
checksums match. The driver build type means its compile times must not be used
as a release compiler latency comparison. The JS/build measurements had finished
before these runs. This is one host and five kernels, not a broad runtime gate.

`explorer-upstream.json` records a successful fresh official Dart 3.6.2 compile
of the same dart_ui revision 15aff7042318d2fa56661ffed4360c81a41a86b8. That official
executable was not run here. `desktop-artifacts.json` binds the preserved Forge
Explorer AOT executables from the preceding validated matrix. External fonts,
resources, AppKit helper host and SDK shared libraries are not counted as part
of an executable's on-disk bytes. A file-size result is not total installed size,
virtual segment size, memory use or visual correctness.

## Validation and scope

`validate-powershell.ps1` parses the actual Windows desktop validator and invokes
its extracted JIT command through a recording stub. Compiler flags must precede
the entry; option-like application arguments after `--` remain unchanged. It
does not launch a compiler or guest. It passes after the one-line PowerShell
argument-order correction; replay against the pre-fix version fails at that same
assertion (use `-Baseline` for the fixed bf412998 snapshot).
`powershell-validation.json` records the pass; the original failure
is retained in `powershell-before.txt`. A new guest GUI matrix is not claimed.

The previous all-platform AOT/JIT matrix, source/SDK equality, 83 CLI tests,
strict Clippy, docs and formatting are in
`../2026-10-20-jit-sdk-selection/README.md`. Native 1,046-test and runtime evidence
is in `../2026-10-20-root-bit-iteration/README.md`. Neither is rerun or added to
the fresh benchmark sample totals. The broad workspace run previously returned
101; its remaining foreign-backend pruning failure is still open, with six
foreign definitions. Historical evidence is not relabeled as current success.
No Rust source changed, no VM was started and no resident agent was installed
for this audit. All new benchmark commands completed; both VMs remained suspended.
