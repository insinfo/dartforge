# Contributing to DartForge

The initial target is Dart 3.6.2, and the original code is MIT licensed.

## Code and documentation

Write all source code in English: names of files, folders, crates, packages, types,
functions and variables; comments and documentation; user-facing messages; CLI options,
environment variables and runtime ABI symbols. Only `docs/`, `PLANO.md`, `ESTADO.md` and
the `RELATORIO-*.md` reports are written in Portuguese (see `AGENTS.md`).

Use `//!` for modules and `///` directly above functions. Explain intent, contract and
relevant limitations; avoid comments that merely restate an obvious line of the
implementation.

Public APIs must have a summary, a runnable example and an `# Errors` section when they
return failures. Document preconditions and `# Panics` where applicable. Examples are
checked by `cargo test`; do not use `ignore` to hide broken examples.

The parser rejects features outside the supported subset. Any accepted syntax must keep
semantics that tests can demonstrate, including side effects, evaluation order and scopes.

## Verification

    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace --no-fail-fast -- --include-ignored
    cargo doc --locked --workspace --no-deps --document-private-items

On macOS, the repository Cargo linker selects matching LLVM LLD for archives
containing bitcode; ordinary direct links keep the system C compiler. LLVM archive and nested
response-file inspection require Python 3. Set `DARTFORGE_MACOS_SDK_ROOT` to pin a macOS SDK; otherwise the
active SDK may be replaced by an installed compatible SDK when LLVM cannot read
its stubs. See `docs/JIT.md` for toolchain selection and explicit linker overrides.

Ignored tests have additional prerequisites: Node, the Dart source SDK, resolved
corpus dependencies, and a current native compiler for builder integrations.
The test command continues through all test executables after a failure and
still returns a failing exit status; keep the complete log when reporting it.
`DARTFORGE_COMPILER` selects that compiler; its default is
`target/release/dartforge`. For a debug compiler, build
`cargo build --locked -p dartforge-cli --features jit` and copy the executable
to `target/test-tools/dartforge` (`dartforge.exe` on Windows), creating that
directory first. Set `DARTFORGE_COMPILER` to the absolute copied path. A default
workspace test build can replace `target/debug/dartforge` with a CLI without
the native feature, so that path is not a stable external compiler for builder
tests. A compiler built in a separate target directory also avoids replacement.
Keep LLVM tools on PATH when runtime SDK
support libraries need to be built; selecting a prefix for Cargo does not add
runtime tools to the caller's PATH.
The native macro executor tests use `DARTFORGE_NATIVE_COMPILER`; set it to the
same preserved compiler when running the complete ignored-test suite.

On macOS, standalone C-link tests also need a configured C SDK. When using an
external LLVM Clang on PATH, set `SDKROOT` to an installed compatible macOS SDK
and `MACOSX_DEPLOYMENT_TARGET` to an explicit supported version for the test
process. The Cargo linker wrapper configures its own Rust-link subprocess; it
cannot change the environment inherited by Clang launched inside a test. Record
these settings with the test results. A missing `libSystem` before the native
call runs is a toolchain prerequisite failure, not ABI conformance evidence.

On PowerShell, `scripts/check.ps1` runs these checks, including ignored tests,
together and requires documentation to build without warnings. It checks that
Cargo, Dart and Node are on PATH before starting; configure the native compiler
and other ignored-test prerequisites above before invoking it. CI checks behavior only: it
does not run `cargo fmt` or `cargo doc` (the project owner's decision; see the header of
`.github/workflows/ci.yml`).
The differential comparison with Dart 3.6.2 (VM, `dartdevc` and production) is
`crates/differential` (`cargo run --release -p dartforge-differential`); see `ESTADO.md` §3.

## Commits

**No commit carries an AI-assistant trailer or signature.** No
`Co-Authored-By: Claude …`, `Co-authored-by: … Copilot`, `Generated with [Claude Code]`,
`🤖 Generated…` or the equivalents for Opus, Sonnet, GPT, Codex, Gemini, Cursor and the
like — not in regular commits, not in merges, not in pull request descriptions. The rule
applies to people and to agents, and overrides any attribution default of the tool.

The rule is enforced in two places:

* a local hook, once per clone: `git config core.hooksPath scripts/hooks` (rejects the commit);
* CI: the `messages` job of `ci.yml` fails when any commit in the history breaks the rule.

The checker is `scripts/no-ai-trailer.sh`.

## References

The whole `references/` folder stays ignored. Record sources and revisions in the catalog
under `docs/`, preserving the original licenses. Do not add clones to the history.

Measure performance separately from correctness. Never treat differences in SDK, backend or
flags as equivalent results, and do not turn known divergences into passes.
