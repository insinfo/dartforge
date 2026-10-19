# SDK inventory consumption

The experimental fixed-image class analysis now reads the allocation inventory
from the exact selected production SDK cache. It verifies the compiler, runtime,
object and summary hashes; parses the verified summary bytes; and reconciles them
with the graph inputs. Allocation SCCs are recomputed. Only complete,
noninterposable, allocation-free source closures with direct ABI-matched calls
and no exposure obligations are admitted. Native calls, raw memory, registry
mutation, managed globals and ambiguous definitions remain opaque.

This is not complete SDK exposure coverage. SDK startup remains unproved, the
feature remains opt-in (`DARTFORGE_CLOSED_CLASS_REACHABILITY=1`), and the foreign
backend table-removal production requirement is not relaxed. Normal compilation
does not load this additional inventory. The cache is compiler-owned and must
remain immutable through linking; hashes are not authentication against coherent
replacement of the entire cache.

`actual-sdk.log` records an explicitly enabled test against the real Dart 3.6.2
source SDK, production runtime and generated objects: 32,676 unique function
records, 428 allocation closures and 428 reviewed empty closures. It also checks
that SDK startup is not authorized. The first attempt exposed repeated global
declarations across modules; those now remain opaque instead of invalidating
unrelated scalar facts.

The seven regular inventory tests and two class-graph tests use explicit
nonexecutable object/runtime bytes to exercise cache binding and graph queries.
They cover stale summaries, altered objects/runtime/profile/key, missing records,
ABI disagreement, recomputed-ready semantic tampering, recursive components,
weak/duplicate definitions, symbol interposition and native/managed obligations.
They are not imported-code execution tests. The preexisting class test separately
executes SDK-free binaries with eager/closed registration and normal/stress GC.

The native suite is run with `scripts/cargo-macos-llvm.py`, LLVM 22.1.8 and macOS
SDK 26.5. Source SDK: `/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib`.
`workspace-checks.json` records the required root checks and bounded excerpts:
formatting and strict Clippy retain existing failures; default workspace linking
cannot consume LLVM 22 bitcode with the Apple linker; documentation builds with
warnings. Strict native Clippy still reports 33 library / 47 test diagnostics,
with none in the new inventory or class-analysis modules. No lint suppression
was added.

Final native unit run: **815 passed, 0 failed, 0 ignored**. The separate
production regression still fails because WindowsBackend bodies remain in IR.
See the complete logs and `sources.json` for source fingerprints.
All 121 native doctests also passed.
