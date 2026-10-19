# LSP request types and strict Clippy — 2026-10-19

The LSP now passes strict Clippy for all targets without lint suppressions.
The baseline is `e6d683d6`; `clippy-before.log` contains its 16 library/test
diagnostics and `clippy-after.log` contains the successful final check.

Completion input, deferred auto-import metadata, member occurrence queries and
import rewrite options now have named types/fields instead of long positional
argument lists or opaque cache tuples. Named aliases describe export filters,
written parameters, async conversion edits, prefix references and inline local
occurrences. Rename dispatch uses `RenameKind`, distinguishing the operation
kind from a Dart class. Annotation lookup uses `find`, and two parameter-name
loops use `enumerate`; traversal order and early return behavior are preserved.

The completion algorithm, rename/reference family rules, import rewriting
policies and LSP JSON schema are unchanged. No performance improvement is
claimed for this structural cleanup.

## Validation

```sh
cargo clippy --locked -p dartforge-lsp --all-targets --no-deps --target-dir target/impl-a -- -D warnings
cargo test --locked -p dartforge-lsp --target-dir target/impl-a -- --include-ignored
```

- Strict Clippy passes, including integration tests and examples.
- All 258 unit/protocol/integration tests and 17 doctests pass. The existing
  suite checks completion resolution/auto-imports, references, rename families,
  quick fixes, async conversion, method/inline/move refactorings, cancellation
  and retained/incremental sessions. No acceptance expectation was weakened.
- `lsp-tests.log` retains the full test output; `source-hashes.json` binds the
  changed Rust sources; `checks.json` records command exits and log hashes.

This validation ran on macOS ARM64, not inside Windows/Linux VMs. The LSP test
harness uses its existing SDK fixtures; this cleanup does not establish new
whole-SDK compatibility or parity with all official server responses.

## Workspace scope

The required formatting check still fails on repository-wide drift. Workspace
strict Clippy still fails in emit-native (33 library / 47 test diagnostics).
Documentation builds with existing warnings, with LLVM 22.1.8 selected through
`DARTFORGE_LLVM_DIR` and `LLVM_SYS_221_PREFIX`.

The separate full workspace test run remains active at baseline `81e6762b`,
with all native builder cases and the current compiler configured. It does
not certify this source delta and has no final result yet. No second full
workspace run or replacement compiler was started. Process metadata and logs
remain under `/tmp/forge-workspace-configured/` for continued observation.
