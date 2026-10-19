# JavaScript emitter and closed-world validation — 2026-10-19

Baseline: `ec1c44c8`. Source changes clear strict Clippy diagnostics in world
and emit-js without new suppressions. Generic bounds retain their existing Box
allocation, extension lookup borrows the receiver type, and complex parameter,
assignment and memo types have names. Redundant branches and string replacement
are removed. These are not measurements of whole-compiler time or memory.

The corpus utility previously wrote an error placeholder and exited successfully
when a program failed compilation. It now returns a failing status, removes an
old combined output for that failed input, rejects empty/missing inputs and
colliding output names before writing, and rejects empty module results.
Determinism checks require both shipped inputs and nonempty emitted modules.

## Checks

- `cargo clippy --locked -p dartforge-world -p dartforge-emit-js --all-targets -- -D warnings`: passed.
- `cargo test --locked -p dartforge-world -p dartforge-emit-js -- --include-ignored`: 25 passed.
- `cargo test --locked -p dartforge-emit-js --example emit_corpus -- --include-ignored`: two passed.
- `cargo test --locked -p dartforge-emit-js-production -- --include-ignored`: 88 passed.
- Five actual invalid CLI invocations failed: no arguments, no input files,
  nonexistent file, duplicate output stem and syntax error. The last also
  removed a deliberately stale output. Details: negative-checks.json.

Tests use Dart 3.6.2 and Node 24.21.0. `DARTFORGE_SDK_LIB` selects SDK sources;
Node's directory is on PATH. The 15 development-profile differential tests run
against the official VM. Production tests additionally cover optimized emission,
accessor effects, typed reads, arithmetic, evaluation order, caches and identity.
All ignored tests in the two requested test commands and in the example command
were enabled; no successful total is inferred from the failed workspace run.

## Before/after emission

The 208 top-level `corpus/js/*.dart` inputs were emitted by both binaries. All
207 accepted programs produced byte-identical concatenated, sorted modules.
The remaining input, `229_nome_indefinido_e_local_duplicado_erro.dart`, is an
intentional negative fixture rejected by both emitters and by official Dart
(exit 254). The old helper returned 0 despite that rejection; the new helper
returns 1 and creates no fake JavaScript artifact for it. This is not a claim
that all 207 programs were executed against Dart.

The baseline crate was exported with `git archive ec1c44c8 crates/emit_js` under
ignored `target/js-quality-baseline`, not a branch or Git worktree. Its manifest
uses explicit workspace metadata, absolute dependency paths into this checkout,
and a standalone workspace. No baseline Rust source was changed. It was built
with `cargo build --offline --manifest-path target/js-quality-baseline/crates/emit_js/Cargo.toml --example emit_corpus --target-dir target/js-quality-baseline-build`.
The current example was built with `cargo build --locked -p dartforge-emit-js --example emit_corpus`.
Each binary received an absolute output directory and the same sorted absolute
input paths recorded in corpus-inputs.json. Output hashes and the complete
comparison are in emission-comparison.json; binaries are hashed in checks.json.

## Workspace limits

Required workspace formatting and strict Clippy still fail. Clippy advances
past these crates and reports diagnostics in analysis, dev, emit-js-production
and emit-native. Full workspace tests stop at the existing Apple LLVM 21 linker
incompatibility with static LLVM 22 bitcode. Documentation builds with existing
warnings. Large formatting/linker logs have bounded excerpts plus original
lengths and SHA-256 hashes. Text logs normalize trailing whitespace; full-log
hashes refer to the original command output. No failure is counted as a pass.
