# Storage boundaries for experimental class pruning — 2026-10-20

The closed-class audit previously accepted raw field reads without proving the
representation of the stored word. A scalar entry argument could be stored in
an ordinary object and loaded as a managed reference, then used as a selector
receiver. The retained `scalar-field-before.log` demonstrates that the new
regression failed against the previous implementation. No arbitrary pointer
fixture is executed: rejected cases exercise the audit only.

## Implementation

A bounded local storage witness tracks allocation identity, initial field kinds,
and every write, including later writes and writes through recognized aliases.
A managed field read requires an in-bounds local slot that never holds scalar
bits. Zero-initialized runtime allocation slots can represent null. Scalar reads
remain scalar; raw pointer heap reads and unknown layouts need separate proofs.
The total tracked slot budget is 8,192 per function. CheckNotNull and same-site
Phi aliases preserve identity; distinct or unknown allocation identities do not
merge into a layout certificate.

The witness reconciles managed/reference-pointer distinctions against the actual
emitter result representation, rather than trusting HIR annotations alone.
Erased runtime setters must have a known in-bounds index and a reference flag
matching the actual value. A field declaration identifies a declaration; it does
not establish the physical layout or authorize a managed result.

Immutable closure reads retain their existing dedicated witness. The witness
now exposes exact read IDs, and closure bodies cannot double as ordinary object
protocol targets or direct static callees. Scalar integers do not authorize native
constant getters; only the reviewed zero-getter case remains admitted. Runtime
class queries require logical managed inputs and scalar outputs.

These checks affect the opt-in closed-class proof. They do not enable closed
registration by default, authorize SDK startup/image registration, or remove the
existing production-profile backend-pruning assertion.

## Regression coverage

Nine new tests cover the original scalar-storage reproducer; GetField, EnvGet,
CellGet and runtime/declared-field getters; bounds and unknown receivers; later
writes through aliases; erased setter flags; scalar-to-reference/pointer result
fabrication; native getter capabilities; closure entry roles; and real compiled
scalar/managed field reads. The compiled fixture checks the observed scalar or
class ID, comparing eager and closed registration with normal and stressed GC:
four executables, eight executions, all requiring successful status and empty
stdout/stderr.

`storage-directed-initial.log` retains an intermediate failure caused by a
mistaken one-argument check for `dartforge_mark_constant`; its actual two-argument
ABI was restored, preserving the existing positive zero-getter case.
`storage-test-compile-initial.log` retains a test construction error (static calls
use operands, not runtime-style typed operands), corrected before final checks.
`initial-log-hashes.json` records original and stored digests; only trailing log
whitespace is normalized.

Final checks and the production platform reproduction are recorded separately
in `checks.json` and `platform-results.json`. The reproduction counts actual
foreign-backend function definitions in emitted IR; it is not a desktop execution
or a whole-workspace approval. Global formatting remains a known separate issue.

## Final results and remaining work

The final native library suite passed **939/939**, including ignored tests
(224.18 s). Strict workspace/all-targets Clippy, explicit embedded-LLVM backend
Clippy, and private documentation with warnings denied all passed. The preceding
939-test pass and Clippy type-complexity failure are retained separately; the
runtime-call tuple received a private type alias and all final checks were rerun.
The new files pass rustfmt and the patch passes `git diff --check`. Workspace-wide
formatting still fails; `formatting.json` binds the complete 15 MB observation
while `fmt-excerpt.log` retains its first 120 lines.

Environment: macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2, macOS SDK 26.5,
deployment target 15.0. `forge-class-storage-checks.py` records exact commands,
paths and selected environment, including Node and the Linux test sysroot.

The final production reproduction still emits **six foreign-backend function
definitions** in both default and opt-in modes (methods, getter adapters and
method-table factories). The trace rejects the unproved
`df.register.dart$3a_internal` entry and retains broad registration. This fix
therefore closes an unsound audit boundary; it does not complete platform pruning,
a full workspace test gate, a new desktop matrix or a performance improvement.
The production-profile test and its expected absence of foreign backends remain
unchanged. Initial and final platform records are retained with compiler/IR hashes.
