# `dartforge test` on package:test's own runner; `pub cache`; `pub global` — 2026-11-02

Correctness evidence only; nothing here measures speed. macOS ARM64, Dart SDK 3.6.2
(`/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk`) as the oracle, release CLI of this
branch built with `--features native`. Every pub run uses its own empty `PUB_CACHE`
under the session scratchpad, never the user's cache. Network access to pub.dev was
used for reading packages; nothing was published.

The scripts are in this directory; `env.sh` holds the paths of the session
(`$SP` is the scratchpad, `$DF` the DartForge binary, `$DART` the oracle).

## `dartforge test`

Fixture (`$SP/pt/tst`, `test: 1.25.7`, test_core 0.6.4): four suites —
`a_test.dart` (pass, failure inside a group, skip with a reason, print),
`b_test.dart` (one pass), `c_test.dart` (`@Skip('whole suite')` on the library) and
`d_test.dart` (`setUpAll`/`tearDownAll` that print, nested groups, a test that throws
`StateError`, an unknown tag, a skipped group, a multi-line print).

`reporters.sh` and `reporters2.sh` run each scenario with `dart test` and `dartforge
test` (`cmp.sh`), keeping stdout, stderr and the exit code. Comparison rules:

* JSON (`jsondiff.sh` / `jsonnorm.py`): only `time`, the `pid` of `start` and the
  `stackTrace` text of `error` events are normalized; `line`, `column`, `url` (and the
  `root_*` fields when present) are replaced by a marker because DartForge emits them
  null (see below). Every other field — ids, `suiteID`, `groupIDs`, `parentID`,
  `testCount`, `metadata.skip`/`skipReason`, `hidden`, `skipped`, `result`,
  `isFailure`, `messageType`, `allSuites.count`, `done.success` — and the event order
  are compared as they are.
* Text reporters (`textdiff.sh` / `textnorm.py`): elapsed times `MM:SS` and the
  github reporter's `(N ms)` become placeholders and the compact reporter's identical
  consecutive repaints are collapsed (it repaints its line every second while a suite
  loads, so the count depends only on time). Stack frames are left in, so every
  remaining difference is listed.

| Scenario | Exit (dart / dartforge) | Result |
|---|---|---|
| `-r json -j 1` | 1 / 1 | **59 events identical**, stderr identical |
| `-r json` (default concurrency) | 1 / 1 | interleaving of the concurrent suites differs (timing); `jsonsem.py` (per-test view, order-independent) identical, 28 lines |
| `-r json -j 1 -N passes` | 0 / 0 | 22 events identical |
| `-r json -j 1 -n nomatch` | 79 / 79 | 16 events identical, stderr (`No tests match ...`) identical |
| `-r expanded -j 1` | 1 / 1 | identical except the two stack traces |
| `-r compact -j 1` | 1 / 1 | identical except the two stack traces and the executable in "To run this test again" (see below) |
| `-r github -j 1` | 1 / 1 | identical except the two stack traces |
| `-r failures-only -j 1` | 1 / 1 | identical except the two stack traces |
| `-r silent -j 1` | 1 / 1 | identical (empty) |
| `-r expanded -j 1 --fail-fast` | 1 / 1 | identical except the stack trace |
| `-r nope` | 64 / 64 | identical usage error |
| `-r compact -j 1 --file-reporter json:<file>` (`reporters2.sh`) | 1 / 1 | the JSON file: **59 events identical**; stdout identical except the two stack traces and the rerun executable |
| `-r json -j 1 --fail-fast` | 1 / 1 | 31 events identical (the run stops after the first failure in both) |
| `-r github -j 1 --fail-fast test/a_test.dart test/b_test.dart` | 1 / 1 | identical except the stack trace |
| `-r json -j 1 --test-randomize-ordering-seed 42 test/a_test.dart` | 1 / 1 | 19 events identical (same shuffled order) |
| `-r expanded -j 1 -t slow` | 0 / 0 | identical, including the unknown-tag warning |
| `--version` | 0 / 0 | identical (`1.25.7`) |
| `--help` | 0 / 0 | package:test's usage identical; DartForge appends the 7 lines of its own options |
| `--watch` | 64 / 64 | identical usage error |
| `--coverage out` | 1 / 64 | refused by DartForge (`coverage collection is not supported by the native runner`) |

Known, documented differences:

* **Stack frames.** DartForge's native executables have no stack frames yet
  (`dart:native  main` where the VM prints `test/a_test.dart 6:25  main.<fn>.<fn>`).
  The JSON reporter takes `line`/`column`/`url` from the trace captured when a test is
  declared; with an empty trace package:test's `JsonReporter` would throw
  (`frames.first`). The host detects the empty trace at start-up and tells the suites
  not to collect declaration traces, so the reporter emits null `line`, `column` and
  `url`, exactly what package:test emits when there is no trace. These three fields
  are what IDEs use to place tests in the file; names, ids and results are complete.
* **Rerun hint of `compact`.** package:test prints `Platform.executable`; the host
  runs with `argv[0]` set to the DartForge executable, so the hint is
  `<dartforge> test test/a_test.dart -p vm --plain-name 'g fails'`, the command that
  reruns the test, where `dart test` prints `<dart> test ...`. (The run above predates
  that change and shows the host's own path.)
* **Time.** Each suite is compiled natively (20–45 s each here), so the `loading`
  test of a suite lasts that long.

## `--coverage`

Not implemented, deliberately: see "`--coverage`: por que não há implementação" in
`docs/PUB-E-TEST.md`. The measurement behind it: `dart test --coverage cov-out` on a
two-test suite (`$SP/pt/cov`) writes `cov-out/test/add_test.dart.vm.json` with 223
scripts (every library of the isolate); `package:tst/tst.dart` has
`"hits": [1, 1, 2, 1, 5, 1, 3, 0, 8, 0]` although `add` ran four times — counts are per
VM coverage range, not executions, and the coverable lines come from the VM
compiler's PC descriptors.

## `pub cache`

`pubscen.sh <name> <scenario>` runs each line of the scenario with `dart pub` and
`dartforge pub` in two empty caches, then compares the transcript (stdout and stderr
interleaved, exit codes, cache and work paths normalized) and the tree of the cache;
`cachefiles.sh` compares the contents of `README.md`, `hosted-hashes/*`,
`.cache/*-versions.json` (without `_fetchedAt`), and every `pubspec.yaml`,
`pubspec.lock` and `package_config.json` in the caches.

| Scenario | Commands | Transcript | Cache tree | Files |
|---|---|---|---|---|
| `cache1.txt`: `add` (exact, again → "Already cached", `--version` range `--all`), `list`, missing package (69), usage errors (no package, extra argument, bad version), no match, `repair`, `preload` of a missing file, `clean` without and with `-f`, `list`/`repair` of an empty cache, `cache`, `--help` of `cache`/`add`/`clean`, `repair x` | 20 | **identical** | identical | — |
| `cache2.txt`: `add` of `path` and `args` (+ a range `--all`), `list`, `repair` (5 packages), `preload` of `meta-1.15.0.tar.gz` and of a non-archive, `list`, `repair`, unknown subcommand, `list --help`, `preload` without file | 12 | identical except the gzip error text (fixed afterwards, see below) | **identical (189 entries)** | **31/31 identical** |

The one difference of `cache2.txt` was the message for a file that is not gzip:
Dart's `GZipCodec` says `Filter error, bad data`, `flate2` said `invalid gzip header`;
`preload` now reports zlib's text. Rerun with the fix (`batch2.sh`): **12/12 commands identical, cache tree identical (189 entries), 31/31 files identical**.

## `pub global`

Fixture `$SP/pt/gp`: a package `gp 1.2.3` depending on `path`, with
`executables: {gp: main, other: }`, `bin/main.dart` and `bin/other.dart` (exit code 3),
committed in a local Git repository. Hosted package: `markdown 7.2.2` / `7.1.1`.

| Scenario | Commands | Transcript | Cache tree | Files |
|---|---|---|---|---|
| `global1.txt`: `list` (empty); `activate --source path` (+ `-x gp`, `--no-executables`); `list`; `run gp:gp` (66), `run gp:other` (re-resolution + exit 3), `run gp`, `run gp:missing`; `deactivate` twice (65); `activate --source git` (relative repository) twice ("already activated"); `run gp a`; `activate markdown 7.2.2` twice, `run markdown:markdown --version`, downgrade to `'>=7.0.0 <7.2.0'`, `deactivate`; usage errors (`activate` without/with extra arguments, invalid name), missing package (69), unsatisfiable version (65), `run` without/with unknown package, `deactivate` without package, `global`, `--help` of `global`/`activate`/`run`, `list x` | 35 | identical except (1) `+ gp 1.2.3 from git ../../pt/gp` printed with the absolute path (fixed afterwards: `p.prettyUri`'s rule) and (2) the order of `Built gp:main.` / `Built gp:other.` | **identical (616 entries)** | **27/27 identical** |
| `global1.txt` again after the fix (`batch2.sh`) | 35 | **identical** (this time the VM also finished `main` first) | identical (616 entries) | 27/27 identical |
| `global2.txt`: path activation; `-x gp -x nope` (65, `Unknown executable nope.` after installing `gp`); `--no-executables -x gp` (64); `--git-ref` without `--source git` (64); `--source bogus` (64); `cache repair` with an activated path package ("Reactivating gp 1.2.3...", re-resolution, stubs, "Reactivated 1 package."); `list`; `run gp:other`; `run --enable-asserts gp:other`; `run gp:sub/dir` (64); `deactivate gp extra` (64); `deactivate gp`; `cache repair` | 13 | identical except (1) `option "--source"` (DartForge wrote `"source"`) and (2) `run --enable-asserts` did not re-resolve the path package; both fixed afterwards | identical (48 entries) | 5/5 identical |
| `global2.txt` again after the fix | 13 | **identical** | identical (48 entries) | 5/5 identical |

Differences that remain by design:

* **What is cached.** Pub writes kernel snapshots
  (`global_packages/<pkg>/bin/<script>.dart-3.6.2.snapshot`,
  `<path package>/.dart_tool/pub/bin/<pkg>/<script>.dart-3.6.2.snapshot`); DartForge
  writes native executables at the same place without the `.dart-3.6.2.snapshot`
  suffix. The tree comparison above excludes nothing: the snapshot files and the
  native files have different names, and the trees were equal because the scenario
  deactivates every package before it ends.
* **Bin stubs.** Same header (`# Package:`, `# Version:`, `# Executable:`,
  `# Script:`, which `deactivate` and `cache repair` read), but the body runs the native
  executable, or `<dartforge> pub global run <pkg>:<script>` when it is missing,
  instead of `dart <snapshot>` / `dart pub global run`.
* **Order of `Built` lines.** Pub compiles the executables in parallel and prints each
  line when its compilation ends; DartForge compiles them one after the other in
  alphabetical order.

## `pub token`, `logout`, `uploader` (second round)

`homescen.sh tk token1.txt` runs `probehome.sh` with each tool: `HOME`, `PUB_CACHE` and
the working directory are fresh per tool, and the directories have names of the same
length, because the help text wraps the path of `pub-tokens.json` at 80 columns. After
every command the transcript includes the contents of `pub-tokens.json`. A line may
feed a token on stdin (`stdin=<token> token add ...`).

`token1.txt`: `token list` (empty), `token add` without and with extra arguments,
`http://` and a relative URL (64), a token from stdin, an invalid token (65),
`--env-var` (undefined variable, lower-case name, `=` in the name, empty name), a URL
with a final slash and `https://pub.dartlang.org` (both normalized), `token list`,
`token remove` without, with extra arguments, unknown (65) and known, `--all` twice
(file deleted, then missing), `token` without subcommand, the four `--help` texts,
`logout` without credentials, `logout x` (64), `uploader` and `uploader add a@b.c` (1):
**31/31 commands identical, including every state of `pub-tokens.json`.**

`login` is not implemented: it is the OAuth2 flow with a browser and Google's servers,
which cannot be run or compared offline.

## Suite executable cache (second round)

`suitecache.sh` (not a speed benchmark: one run each, machine shared with other agents)
empties `.dart_tool/dartforge/test-suites` and runs `dartforge test -r json -j 1` on the
fixture three times, comparing each event stream with `dart test -r json -j 1`:

| Run | Wall time | Events against `dart test` | Cache entries |
|---|---|---|---|
| cold (also compiles the test host, new binary) | 176 s | identical | 3 |
| warm (every suite from the cache) | **1 s** | identical | 3 |
| after `touch test/b_test.dart` (every key changes) | 108 s | identical | 6 |

The first version of the key looked for the first `import` of the bootstrap
(`dart:isolate`), never produced a key and therefore never cached (0 entries in a
first measurement); fixed in `Find the suite import of the bootstrap for the cache key`.

## Unit and integration tests

`cargo test --release --locked -p dartforge-cli --no-fail-fast`: all pass (30 + 4 + 2
tests in the binaries that have tests), including the new ones:

* `pub_get::tests::cache_add_list_repair_and_preload_follow_pub_layout` — against the
  local registry: `cache add` (best version, `--all` with a constraint), the hash, the
  `.cache/<name>-versions.json` listing with `_fetchedAt`, the README, the JSON of
  `cache list`, `repair` reinstalling a corrupted package, `_directoryToUrl` of a
  loopback server, `preload` with its hash and zlib's error text for a non-gzip file;
* `pub_get::tests::global_activation_writes_pub_lockfile_config_and_bin_stubs` —
  hosted activation (lockfile kinds and `sdks`, package configuration without the
  placeholder root, stub header and executable bit), path activation with
  `--no-executables` (absolute path, `dart: any`), `global list`, `deactivate` and its
  error code;
* `test_runner::tests::*` — the split between DartForge's options and package:test's,
  option checks, the compile arguments and the host template (engine, reporters,
  native bootstrap, `deserializeSuite`).

`cargo clippy --locked -p dartforge-cli --all-targets -- -D warnings`: clean.
`cargo fmt -p dartforge-cli`: applied.
