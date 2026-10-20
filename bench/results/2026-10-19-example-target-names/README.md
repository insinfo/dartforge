# Distinct workspace example targets — 2026-10-19

Cargo's workspace test build reported that the formatter and Angular generator
both wrote `target/debug/examples/oracle` (and its dSYM). Depending on the build
order, an example could replace the other package's executable. The Angular
example now has an explicit Cargo target named `ng_oracle`; its source path stays
`crates/ng_generator/examples/oracle.rs`. Its command documentation and usage
message were updated. The formatter's existing `oracle` command stays valid.

`before.json` and `after.json` project the actual output of
`cargo metadata --locked --no-deps --format-version 1` onto all workspace
example targets. Before: one duplicate name, `oracle`. After: 48 examples,
no duplicate names, with `ng_oracle` and `oracle` assigned to their respective
packages. No new regression test duplicates Cargo's own target discovery.

Both packages' examples were built together successfully without an output
collision warning. `runs.json` records the command and exits. Both executables
were then invoked independently: the Angular tool returned its expected usage
error (exit 1, no arguments), and the formatter matched official Dart 3.6.2 on a
one-file fixture (1/1 identical, exit 0). `binaries.json` binds both outputs.
This checks tool selection, not new Angular generation semantics or full formatter
parity. Strict all-targets Clippy passed for `dartforge-ng-generator`.

## Broader workspace test prerequisite

`workspace-initial.log` is a completed `cargo test --locked --workspace --
--include-ignored` run. It passed the ABI/C-call, analysis, build library and
earlier build integrations, then failed the native builder corpus. The selected
`target/debug/dartforge` had been replaced by the default CLI build without the
native backend. This is a prerequisite failure, not an approved corpus result.

`CONTRIBUTING.md` now directs users to a copied executable or separate target
directory so the test build cannot replace their external native compiler.
`native-compiler-build.log` and `native-compiler.json` record a fresh native
compiler copied to `target/test-tools/dartforge`. The broad suite was resumed
with this separate path. Its result is tracked separately from the completed
example checks; neither global formatting nor the full workspace suite is
certified by this change.

The macOS test environment selected LLVM 22.1.8, SDK 26.5 and deployment target
15.0, with source Dart SDK 3.6.2 and Node/Dart/LLVM on PATH. The LLVM archives
emitted minimum-version warnings (27.0 versus 15.0); those remain in the initial
log. No warning was suppressed. Stored text normalizes trailing whitespace.
