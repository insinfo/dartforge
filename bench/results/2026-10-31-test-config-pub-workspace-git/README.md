# `dartforge test` configuration and `dartforge pub get` workspaces/git — 2026-10-31

Correctness evidence only; nothing here measures speed. macOS ARM64, Dart SDK
3.6.2 (`~/tools/dart/3.6.2/dart-sdk`) as the oracle, LLVM 22.1.8, release CLI
built with `--features native` from commits `f1a1b376` and `972f3b26`.

## `dartforge test`

- `test-differential.log` (`batch1.sh` + `compare.py`): 22 runs of `dart test -r json`
  and `dartforge test -r json` with the same arguments on fixture packages created by
  `make_fixtures.py` (the `config` fixture is also kept in
  `crates/cli/tests/fixtures/test_runner/config`). Per-test outcomes
  (`success`/`failure`/`error`/`skipped`, load failures) and exit codes were equal
  in all 22. Covered: `dart_test.yaml` `timeout`, `retry`, `concurrency`, `paths`,
  `filename`, per-tag `tags` configuration, `presets` (`-P`), `on_os`, `include`,
  `reporter`/`add_presets` errors (exit 64/65); annotations `@Timeout`, `@TestOn`,
  `@Skip`, `@Tags`, `@OnPlatform`, `@Retry` with and without import prefixes; CLI
  `-n`, `-N`, `-t`, `-x`, `--run-skipped`, `--timeout`, `--total-shards`,
  `--shard-index`, `--no-retry`, `-j`, `path?name=`; missing paths, no matches
  (exit 79), annotation errors; package:test 1.25.7 and 1.26.3.
- Error texts of load failures and the unknown-tag warning were compared by eye
  with `dart test -r expanded`: identical messages and spans.
- `test-integration.log`: `cargo test -p dartforge-cli --features native --test
  test_runner_cli -- --include-ignored` with `DARTFORGE_COMPILER` pointing to the
  release CLI: 2 passed (refusals, and 7 differential runs against `dart test`).
- `crates/cli/tests/fixtures/test_metadata/parse_metadata_oracle.json`: 55 sources
  read by test_core 0.6.4's own `parseMetadata` (`make_cases.py`); the unit test
  `agrees_with_the_official_parse_metadata_on_recorded_cases` checks the Rust
  reading against it.

## `dartforge pub get`

- `pub-real-projects.log` (`pubreal.py`): pubspec files (and existing lockfiles) of
  18 projects under `/Volumes/ssd_dock/MyProjectsDart` copied twice; `dart pub get`
  and `dartforge pub get` online with separate empty caches. 16 produced the same
  `pubspec.lock` text (apart from the header comment) and the same
  `package_config.json` (apart from generator fields and the cache prefix), plus the
  same `workspace_ref.json` files: three real workspaces (`angular`, `playwright`,
  `playwright-connect`), path and override projects and `pdfbox_dart` (git).
  Two failed in DartForge and are open: `aggr` (the backtracking search exceeds its
  100000-decision limit; Pub's conflict-driven solver finds the solution) and
  `new_sali/backend` (a hosted archive contains a link, which the extractor refuses).
- Synthetic layouts (`make_ws.py`, `probe_override_kind.py`, `probe_example.py`,
  `make_git.py` + `diff_git.py`, compared with `pubcmp.py`): workspace resolved from a
  member, override precedence, `example/`, and git dependencies (default `HEAD`,
  `ref` + `path`) gave the same lock text, configuration, mirror name, checkout
  names and `.git/pub-packages` contents as `dart pub get`.
- `ws_probe.py` records the Pub 3.6.2 behaviours reproduced (member overrides,
  incompatible sibling constraints, path siblings, stray member files, unlisted
  members, language version 3.5 requirement).
