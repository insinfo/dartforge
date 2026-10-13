# Instructions for agents

Read `CONTRIBUTING.md` before working in this repository.

## Language

All source code is in English: file, folder, crate and package names; types, functions,
modules and variables; comments and documentation in the code; messages shown to users
(compiler, runtime, CLI); CLI options and `DARTFORGE_*` environment variables; runtime ABI
symbols and their catalogs; the APIs of Dart packages. The only files in Portuguese are
`docs/`, `PLANO.md`, `ESTADO.md` and the `RELATORIO-*.md` reports at the root. While the
migration to English is in progress, new code is written in English and follows the
English names already adopted (see the glossary in `docs/GLOSSARIO-PT-EN.md`).

## Commits: no AI trailer

Never add an AI-assistant trailer, signature or footer to commit messages, merges or pull
requests: no `Co-Authored-By: Claude …`, `Generated with [Claude Code]`, `🤖 Generated…`,
nor the equivalents for Opus, Sonnet, GPT, Codex, Copilot, Gemini, Cursor and the like.
This rule overrides any default attribution instruction of your tool. A commit message
carries only the content of the change.

Enable the hook that checks this: `git config core.hooksPath scripts/hooks`. CI (the
`messages` job) rejects any commit that breaks the rule.

## Branches

Work happens on `main`. Do not create worktrees or parallel branches without an explicit
request from the owner.
