# Resident session and production JavaScript validation — 2026-10-19

Baseline: `7eebb717`. The development session did not apply the mandatory
language/definite-assignment error predicate used by normal compile-js. A
revision reading an unassigned final could be emitted successfully. It now
returns an error before emission, retaining the last successful JavaScript on
disk. Ordinary type diagnostics retain the existing nonfatal warning policy.

Failed attempts could also update hashes and module text before a write error.
Retrying without another source edit could then skip the missing module. A
small compile wrapper invalidates hashes, dependents, fragments and in-memory
module text on any error. The next attempt recomputes and emits the revision.
The normal successful path does not clone this state. Write failures can be
partial; this is retry repair, not an atomic rollback of filesystem writes.

Both regressions failed before their respective fixes. The write-retry failure
left main.js absent while reporting a successful retry. The large original
failure dump is represented by a bounded excerpt, file lists, length and hash.
Tests now avoid dumping megabytes of runtime bytes when output comparison fails.

The edit-latency measurement example now holds a source-restoration guard
across temporary edits. It restores original bytes during panic unwinding if
compilation fails, and explicit normal restoration disarms the guard. Tests
cover byte preservation, a later change after restoration, and an actual
SDK-backed Session failure followed by source restoration and session recovery.
The guard cannot recover from abort/kill or a failed filesystem restoration.

The WebSocket test previously repeated header-building logic without calling
the writer. It now uses the real handshake and text_frame functions over a
loopback TCP connection, checks the RFC accept-key vector, literal wire headers,
UTF-8 payloads, consecutive frames and EOF. Payload byte lengths are 0, 125,
126, 128, 65535 and 65536; accented and emoji strings distinguish bytes from
character counts. Socket I/O has a five-second timeout and the thread is joined.

## Validation

- Strict Clippy for dev and emit-js-production, all targets: passed without
  new suppressions. Private SDK split arguments use a source range; RTI citation
  data has a named type. Other changes simplify conditions and initialization.
- Final dev suite: 16 passed (13 library/integration and three registered
  measurement-example tests), zero failed/ignored. Includes output preservation,
  repeated rejection, recovery equal to clean compilation, write retry without
  another edit, ordinary warnings, incremental API hashes, memory plateau and
  zero-cost behavior without builders.
- Measurement example: three also passed in an explicit example test command.
  The example is registered with test=true, so normal cargo test for dev runs
  these regressions too; --include-ignored enables its SDK-backed case.
- Production emitter: 88 passed with ignored tests enabled, including Node,
  official VM/O2 comparisons, evaluation order, typed operations and caches.
  production-suite.txt is the production portion of the combined dev/production
  run. Its earlier dev portion is superseded by the final dev suite.
- Total unique directed tests: 104. No performance improvement is claimed.
- Official Dart 3.6.2 rejects unassigned.dart with exit 254, as expected.

Commands are in checks.json. Use Dart 3.6.2 SDK sources via DARTFORGE_SDK_LIB;
Node 24.21.0 is on PATH for production tests. Example command:

```sh
cargo test --locked -p dartforge-dev --example measure_generation -- --include-ignored
```

## Workspace limits

Required full formatting and Clippy remain failures. This Clippy execution
reports parity and emit-native diagnostics; it does not prove other crates are
clean. Full tests stop at the existing Apple LLVM 21 linker incompatibility
with static LLVM 22 bitcode. Documentation builds with existing warnings.
Large logs have bounded excerpts and original hashes/lengths. Text logs normalize
trailing whitespace; full-log hashes refer to the original output. Before-fix
failures and the deliberate oracle rejection are not counted as passing tests.
