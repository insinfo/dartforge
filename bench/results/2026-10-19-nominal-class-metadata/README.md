# Nominal ancestors in the closed-class proof — 2026-10-19

The class-table proof previously required a local method table for every class
metadata record, including the synthetic Object root and abstract ancestors.
Lowering now supplies an explicit nominal-only premise for abstract classes,
mixins and the synthetic root inserted directly by `lower_program`. The premise
is preserved when lowering facts are joined.

Absence of a table is not evidence: an existing malformed-concrete-class test
caught that overly broad inference, and its rejection expectation is unchanged.
Every nominal-only CID must have a class definition and complete ancestry, no
instance table/table function, and no identity implicitly created by the runtime.
Unknown or contradictory premises reject and clear the tentative class set.
Reachable allocations still require an actual table and matching field count.
Class metadata, class names and subtype registration edges remain emitted.

The proof also admits the reviewed subtype query `dartforge_is_subclass` with
HIR I64 operands and exact `i8(i64, i64)` ABI. Catalogue attributes may follow the
matching complete prototype; a shared LLVM spelling does not make a Ref a CID.
The runtime query can allocate native cache storage, but neither creates Dart
objects nor mutates the nominal class graph (`runtime/src/core.rs`).

## Validation

- `before.log`: the new positive graph regression fails against the production
  proof at 6ae0c592.
- `directed.log`: **31 tests passed** on the final sources, including ignored
  executable cases. The new metadata regression keeps transitive subtype edges;
  13 negative variants cover allocations, runtime-created identities, missing
  ancestry/declarations, inconsistent premises and invalid subtype-query ABIs.
- The source SDK lowering test resolves actual abstract/mixin/concrete Dart
  declarations, lowers the complete program, checks the synthetic root and
  verifies premise transfer between chunks.
- The existing compiled selector test now includes a positive transitive subtype
  query and an unrelated nominal target. Eager/closed registration each compiles
  an executable and runs normally and under stress GC (**2 binaries, 4 runs**),
  preserving both selector and subtype results. The existing closure matrices
  also remain in the directed run. These ABI fixtures import no source SDK;
  they are not certification of a complete imported program.
- `native.log`: **829 passed**, none failed/ignored/filtered, with
  `cargo test --locked -p dartforge-emit-native --lib --target-dir target/impl-a -- --include-ignored`
  against the final sources and explicitly selected source SDK.
- `checks.json`, `clippy.log`, `doc.log`: strict workspace all-target Clippy and
  private documentation with warnings denied both pass. Global formatting still
  remains failing.
- `production.log`: the original SDK-backed platform pruning test remains
  **failed, Cargo exit 101**. The observed rejection now concerns `_AsyncFrame`
  (CID 1073741568) with unproved ancestry instead of Object. Async frames are
  potentially allocated runtime state and are not tagged as nominal-only to
  bypass the proof. Their layout, allocation and callback boundaries remain open.
  The assertion requiring WindowsBackend bodies to leave IR is unchanged.

The feature stays opt-in. No new whole-workspace or Windows/Linux approval and no
File Explorer size reduction is claimed. Runs used macOS ARM64, LLVM 22.1.8 and
Dart 3.6.2 sources. Correctness jobs overlap; elapsed times are not controlled
performance measurements. Logs normalize trailing blank lines only; receipts
record original and stored SHA-256 values. `sources.json` binds final sources and
the reviewed runtime/catalogue files.
