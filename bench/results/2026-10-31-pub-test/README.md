# `dartforge pub` (PubGrub, everyday commands) and `dartforge test -r expanded` — 2026-10-31

Correctness evidence only; nothing here measures speed. macOS ARM64, Dart SDK 3.6.2
(`/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk`) as the oracle, release CLI of this
branch (`--features native` for `test`). Every run uses its own empty `PUB_CACHE`
(`cache-dart` for `dart`, `cache-df` for DartForge) under the session scratchpad, never
the user's cache. Network access to pub.dev and github.com was used.

## `pub get` on real projects

`runall.sh` runs `pubreal.py` over the 51 projects of `/Volumes/ssd_dock/MyProjectsDart`
that have a top-level pubspec (the 18 projects of
`bench/results/2026-10-31-test-config-pub-workspace-git` plus 33 more) and
`new_sali/backend`. Only the pubspec files (and, in the second pass, the existing
lockfiles) are copied, twice; `dart pub get` and `dartforge pub get` run in the two
copies. Compared byte for byte: every `pubspec.lock` (headers included), every
`package_config.json` (with the `generated` timestamp and the cache prefix normalized),
every `workspace_ref.json`, stdout (cache prefix normalized) and the exit code.

| Pass | Result |
|---|---|
| Without lockfiles (fresh resolution) — `pub-get-nolock.log` | **51/51 identical** |
| With the projects' lockfiles — `pub-get-lock.log` | 49/51; `aggr` and `pg8000` differed |
| Same two after the fix — `pub-get-lock-recheck.log` | **2/2 identical** |

The two differences with lockfiles were locked ids whose pubspec now has another
version (`_macros 0.3.2` from the SDK, which ships 0.3.3; a path package that moved from
3.1.2 to 4.0.0). Pub's `SystemCache.describe` rejects such an id, which makes the
solver move on; DartForge does the same now.

`aggr` (previously over the 100000-decision limit) and `new_sali/backend` (archive
`eloquent 3.4.3` with GNU long names in a `ustar` archive) are among the identical ones.

## Other commands

`addcmp.sh` (log `pub-commands.log`): 32 scenarios run with `both.sh` in two copies
of the same pubspec — `add` with sorted and unsorted maps, constraints that need quotes,
`dev:`, `override:`, `{path: ..}`, `{git: ..}`, `{hosted: ..}`, empty and flow
`dependencies`, an existing dependency, a missing package, an unsatisfiable constraint
with overrides, `add dev:` of a regular dependency; `remove` (including the last entry
of a section and a missing package); `upgrade --major-versions`, `upgrade <pkg>`,
`upgrade --tighten --dry-run`, `downgrade`; `deps` in every style, `--no-dev`, `--json`,
`--executables`; a solve failure and an old-SDK resolution. Same pubspec, lockfile,
stdout, stderr and exit code in all of them, except the suggestions block Pub appends to
a solve failure ("You can try one of the following suggestions…"), which DartForge
does not print.

`addexp.sh` records the `yaml_edit` behaviours the editor reproduces.

`outcmp.sh` (log `pub-outdated.log`): 10 `outdated` scenarios (without and with a
lockfile, `--json`, `--no-dev-dependencies`, `--show-all --no-transitive`,
`--prereleases`, an up-to-date package): identical.

## `dartforge test -r expanded`

`tstsetup.sh setup -r expanded -j 1` creates a package with two suites (pass, failure
in a group, skip with a reason, print) and runs both tools: `test-expanded-dart.txt` and
`test-expanded-dartforge.txt`. The lines are the same (colors, `loading`, counters,
`[E]`, indentation, `Skip:`, summary, chain-stack-traces notice) except the elapsed time
(DartForge's includes the native compilation of the second suite) and the stack frames
(`dart:native main` instead of the VM's frames).

## Unit and integration tests

`cargo test --release -p dartforge-cli` and `--features native`: all pass, including the
new `pub_get` tests against a local registry (conflict learning and Pub's lockfile text,
failure explanations, archive links and GNU long names, workspaces and Pub's workspace
rules, relative local Git repositories, pubspec editing) and the `version`, `tree` and
`yaml_edit` unit tests. `cargo clippy -D warnings` clean with and without `native`.
