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
    cargo test --locked --workspace -- --include-ignored
    cargo doc --locked --workspace --no-deps --document-private-items

Tests ignored by default require Node. On PowerShell, `scripts/check.ps1` runs these checks
together and requires documentation to build without warnings. CI checks behavior only: it
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
* CI: the `mensagens` job of `ci.yml` fails when any commit in the history breaks the rule.

The checker is `scripts/sem-trailer-ia.sh`.

## References

The whole `references/` folder stays ignored. Record sources and revisions in the catalog
under `docs/`, preserving the original licenses. Do not add clones to the history.

Measure performance separately from correctness. Never treat differences in SDK, backend or
flags as equivalent results, and do not turn known divergences into passes.
