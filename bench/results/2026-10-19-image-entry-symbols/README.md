# Image entrypoint namespace and RTI reload compatibility

Base: `311115f6`. The emitter previously gave two program-generated functions
names in the reserved native runtime namespace. The unchanged closed-class audit
correctly refused those images. The emitter now names its RTI initializer
`df.prepare_rti` and event-loop callback `df.call_dart0`. Runtime C exports and
runtime source files are unchanged. Names live in `dartforge-abi::image_symbols`,
shared with JIT consumers. The emitter SDK code fingerprint now includes ABI
sources. Cargo.lock has two new local dependency edges; because runtime bundle
fingerprints include the lockfile, old target bundles may require refreshing.

The before logs show actual source lowering expected the new name and failed,
and a reload counter stayed zero instead of one before the JIT recognized the
new initializer. The full hot-reload integration suite passes 17 tests, including
three-generation current/current, legacy/legacy and legacy/current transitions.
Each unchanged-body reload must republish its initializer and update preserved
global state from zero to one to two. This does not certify arbitrary migrations
of live callbacks across compiler versions.

The full native unit suite passes 835 tests. The 36 directed class-proof tests
include rejection of native runtime shadowing and unproved initializer bodies.
ABI tests pass two unit tests, the native C/Wasm integration and one new doctest;
`DARTFORGE_CLANG=/usr/bin/clang` selects the installed configured C compiler.
LLVM 22.1.8 and Dart source SDK 3.6.2 are used on macOS ARM64.

The real production pruning test still fails: inventory reconciliation now
rejects the selected SDK image and retains broad class registration. No audit
condition was relaxed and no foreign backend or explorer size reduction is
claimed. Full workspace tests are not approved: the prior global invocation
stopped at an unconfigured standalone Clang/sysroot prerequisite. Global fmt
still fails existing formatting drift. The CLI build retains existing macOS
deployment-target linker warnings. No new Windows/Linux execution is included.


The rebuilt compiler also passed 33 real-program commands: three official Dart
references, six optimized AOT builds, and 24 executions (three programs, tracing
and ARC, AOT and JIT, normal and GC stress). Every execution exactly matches its
Dart reference output. Programs exercise microtasks/timers, async functions plus
sync/async generators, and canonical Type identity. Commands, exit status,
compiler SHA-256 and stream hashes are in `smoke-results.json`; the source and
raw streams are retained. Strict workspace Clippy and private documentation pass;
format failure is recorded with a bounded excerpt and full original digest.

An additional full JIT unit run initially ended with 41 passes and four failures.
All four failed before their callback/migration assertions because the test
process could not find `ld64.lld` to link the source SDK DLL. The failure log
is retained as `jit-lib-missing-linker.log`. The configured rerun adds LLVM's bin
directory to PATH and explicitly selects Dart SDK 3.6.2; tiny native driver C
compilations continue using `/usr/bin/clang`. LLVM prefix variables alone do not
change the PATH of tools launched by a test process.

The configured rerun passes all 45 JIT unit tests, including both real native
callback lifetime tests and both source-generated field migration tests.

Text logs normalize trailing whitespace and final blank lines for repository
checks; `log-normalization.json` preserves original and stored digests. Raw
program stdout/stderr streams are unchanged.
