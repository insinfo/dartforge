# Explain unproved SDK startup and helper boundaries — 2026-10-20

Base `0a9458d9`. Closed-class tracing previously stopped at a generic unproved
startup body/input-ABI message. That message did not distinguish image publication
from escaped helper addresses or identify which helper body lacks allocation
coverage. The verified import inventory now retains bounded diagnostic observations
for canonical SDK startup symbols. Rejection appends their exact outstanding
obligations and helper targets, sorted deterministically with at most three
entries per list and a total count. Missing, ambiguous, incomplete/interposable
or ABI-unknown definitions are distinct from unknown allocation closures. Local
symbol resolution has precedence over an unrelated global symbol.

Even a known empty allocation footprint is explicitly described as lacking
runtime invocation coverage. It does not establish the runtime callback ABI,
input values, escape discipline, helper-map replacement or later invocation roots.
Accepted pure/metadata entries return no rejection diagnostic. The new field
never participates in entry/call/allocation permission, summary rewriting or
image registration; the on-disk inventory protocol and runtime are unchanged.

Four import tests cover six target states, local/global resolution, accepted
entries and deterministic list limits. A class audit test starts with a valid
certificate, adds an uncovered imported helper and requires a diagnostic plus
removal of the previous active class set. Initial harness failures are retained:
an assertion used a Debug-bound helper on a non-Debug success type; a pure startup
fixture assumed a symbol namespace excluded by existing import policy. The test
was corrected to exercise an actual source-namespace pure function; production
permission was not relaxed.

`helper-observations.json` records an observational census of the prior validated
SDK cache: 41 distinct helper names with unique targets, all with unknown
transitive allocation closures. It binds the source JSON by SHA-256 and retains
ABIs and unresolved obligations. It is not a runtime invocation certificate or
an independent verification of a cache publication. The inspected baseline cache
was previously loaded by the actual selected-SDK inventory regression.

No backend pruning, speed/size gain, whole-workspace test approval or new desktop
GUI matrix is claimed. Global formatting drift remains open. Large compilers,
executables and IR stay under ignored target storage with hashes in the evidence.

## Actual compiler and execution checks

Strict workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private docs, the runtime UTF-8/no-Dart-allocation registration test, and the JIT
compiler build passed. Both new source files pass scoped rustfmt; global format
continues to fail, retained by original digest/size and a normalized excerpt.
The compiler retains the preexisting LLVM compression-object deployment warning;
current-host execution does not establish compatibility with older macOS releases.

The actual platform reproduction, default and experimental closed-class modes,
retains six foreign-backend definitions. Both IR files are byte-identical to the
preserved compiler's output (`ir-comparison.json`). The experimental trace now
names IndexError, LateError and StackOverflowError helper targets as having
unknown allocation closures, and retains broad registration. The unchanged
JSON/UTF-8/Unicode/RangeError/three-isolate/base64 fixture agrees with Dart 3.6.2
under production/development AOT and JIT macOS, normal and stressed GC. Runtime
sources are unchanged; this turn does not rebuild target bundles or repeat
Windows/Linux execution.

The full native library suite passed **989/989**, including ignored tests, in
240.45 s. The four directed import diagnostics tests overlap that suite; the
fifth new test checks class-audit integration and certificate invalidation.
All five new tests passed. Original failed harness logs are retained alongside
the successful final commands, source bindings and normalized log digests.
