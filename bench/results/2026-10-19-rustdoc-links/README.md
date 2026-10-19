# Warning-free workspace documentation — 2026-10-19

Baseline `08726521` emitted 28 rustdoc diagnostics: 21 unresolved links, six
public links to private items and one ambiguous function/macro link. It also
emitted one compiler dead-code warning for a helper used only by unit tests.
The original complete log is retained in `warnings-before.log`.

Outdated names from the English migration now link to existing items, including
constant-cycle state, graph traversal, flow inference and source SDK modules.
The production JavaScript emitter links to the qualified member-name function.
Public API documentation mentions private implementation helpers as code text
instead of publishing links that require private documentation. The `write()`
reference explicitly selects the function rather than the macro.

`FieldDeclaration::identity` is compiled only for tests: all its consumers are
in the declared-field HIR/pruning test modules. There are no lint suppressions,
and no executable statements or runtime ABI declarations were changed.

## Checks

With LLVM 22.1.8 explicitly selected through `DARTFORGE_LLVM_DIR` and
`LLVM_SYS_221_PREFIX`, both commands pass with no warnings:

```sh
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps --document-private-items
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
```

`private-doc.log` and `public-doc.log` retain their output. `checks.json` records
the terminal exits, exact commands, environment and log hashes. These are the
default workspace features on macOS ARM64, not certification of all feature
combinations or Windows/Linux documentation builds.

The nine native tests selected by `declared_field` pass, including HIR emission,
effect preservation, corrupted-summary rejection and the actual SDK Base64
declaration roundtrip. There are 813 filtered tests: this targeted result must
not be described as a complete native unit test run. `field-tests.log` and
`test-check.json` retain the command and result; `source-hashes.json` binds all
changed Rust sources. Documentation examples were not edited.

Workspace formatting still fails on existing drift. Strict workspace Clippy
still fails in emit-native: 32 library diagnostics (previously 33) and 47 test
diagnostics. The independent full workspace test process remains active at
baseline `81e6762b`, without builder filters. It has no terminal result yet
and does not certify this source delta. Its compiler executable was not
replaced; continued process metadata/logs are under
`/tmp/forge-workspace-configured/`.
