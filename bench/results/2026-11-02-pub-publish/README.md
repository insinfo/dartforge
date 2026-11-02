# `dartforge pub publish` against `dart pub publish` (Pub 3.6.2) — 2026-11-02

Correctness evidence only; nothing here measures speed. macOS ARM64, Dart SDK 3.6.2
(`/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk`) as the oracle, release CLI of this
branch (`cargo build --release -p dartforge-cli`). Reference source: Pub at
`e6704cc5` (the `pub_rev` of the 3.6.2 `DEPS`), package:tar `32ceb55e`,
package:source_span `ec100b7f` and Chromium zlib `108fa50c` (all from the same
`DEPS`).

**Nothing was uploaded to pub.dev or any real server.** Every upload went to the
local fake server `fakepub.py`; the only real-server traffic is reading package
listings and archives from pub.dev for the `real` scenario (`--dry-run` and
`--to-archive` only, which never upload).

## Scripts

All paths are absolute to the session scratchpad (`$SP`), as in the previous pub
evidence.

* `fakepub.py <port> <mode> <data dir>` — a fake pub server: version listings and
  archives of the packages under `<data dir>/packages`, and the publishing protocol
  (`GET /api/packages/versions/new`, multipart `POST /upload`, `GET /finalize`).
  `<mode>` picks a failure: `ok`, `auth401`, `auth403`, `new500`, `newbad`,
  `newnourl`, `gcs` (XML storage error), `noloc`, `finerr`, `finbad`. Every request
  is logged (method, path, `Accept`, `Authorization`, content type, user agent, the
  form fields and the SHA-256 of the uploaded archive). `servers.sh` starts the ten
  modes on ports 8765–8774.
* `scen.sh <dart|df> <scenario>` — runs each line (`<package dir> <pub args>`, with
  optional `stdin=` and `env=` prefixes) with the tool's own `HOME` and `PUB_CACHE`
  and `PUB_HOSTED_URL=http://localhost:8765` (or `$HOSTED`), printing stdout+stderr
  and the exit code.
* `cmp.sh <name> <scenario>` — runs the scenario with both tools (each package's
  `pubspec.lock` and `.dart_tool` removed first), diffs the transcripts (only the
  `HOME`/`PUB_CACHE` paths normalized), diffs the server request logs, and compares
  byte for byte every archive written with `--to-archive ...-@TOOL@.tgz`. Both
  tools publish the **same** package directories, so the tar headers (which carry
  each file's change time) are comparable.
* `mk-simple.sh`, `mk-fixtures.sh`, `mk-fixtures2.sh` — the crafted packages and the
  fake server's packages; `mk-real.sh` — copies twelve packages from the pub cache.
* `gztest.sh`, `mkcorpus.sh`, `czgz.c` — the gzip study below.

## gzip: the VM is not classic zlib

`gztest.sh` compresses the same 1.4 MB tar (Pub's `lib/`) with the VM's
`gzip.encode` and with classic zlib 1.2.12 at the same settings (level 6, window
15, memory level 8): **292819 vs 292929 bytes, different from byte 12 on.** The VM
links Chromium's zlib, whose `insert_string` hashes four bytes
(`(v * 66521 + 66521) >> 16` over a 15-bit table). `czgz.c` built against the
Chromium zlib of the 3.6.2 `DEPS` (portable C, no SIMD) gives the VM's bytes
exactly, so the output does not depend on CPU features. DartForge ports that deflate
path to Rust (`crates/cli/src/pub_get/publish/deflate.rs`).

`mkcorpus.sh` writes 17 inputs and the VM's `gzip.encode` of each: eight tars of
source trees, a 38 MB binary (`dart`), a 23 MB snapshot, 300 KB of random bytes,
repetitive and ramp data, and the empty, one-byte and 17-byte inputs. The ignored
test `matches_a_corpus_of_the_vm` (`DARTFORGE_GZIP_CORPUS=<dir>`) compares the port
on all of them: **17/17 identical**.

## Results

The final run of all scenarios (`all.sh`, output in `results.txt`), with the
release binary of commit `4721dbcb`:

| Scenario | Commands | Transcript | Server requests | Archives |
|---|---|---|---|---|
| `usage` (help, `lish`, every usage error, `-C` missing directory, `--from-archive` missing file) | 14 | identical | — | — |
| `basic` (`simple`: `--dry-run`, `--skip-validation --to-archive`, `--to-archive`, `-n -C .`) | 4 | identical | 3 identical | 2/2 identical |
| `crafted` (31 commands over 25 crafted packages, below) | 31 | identical | same `pub get` reads in another order (below) | 4/4 identical |
| `upload` (fake servers, below) | 30 | identical | 69 identical | 1/1 identical |
| `more` (workspace, lazy pubspec errors, big-file leak, refused connection) | 13 | identical except one line (below) | identical except 3 `pub get` listing reads | 1/1 identical |
| `real` (12 packages from the pub cache, against pub.dev) | 24 | identical | — (pub.dev) | 12/12 identical |

Total: 116 commands, 20 archives byte for byte. The request-log differences all come
from the `pub get` that `publish` runs first, not from publishing: Pub fetches
listings and archives in parallel, so their order differs, and in the workspace runs
DartForge reads a listing Pub did not ask the server for. In this run the archive
downloads of DartForge also carried ureq's default `Accept: */*`; commit `5c4fc8c7`
drops it (Pub sends none), and a rerun of `crafted` with that binary gives the same
37 requests as Pub, sorted (only the order differs).

**crafted** covers every validator that can fire without Flutter: missing LICENSE,
README, CHANGELOG, description, version and homepage; SDK constraint without upper
bound, pre-release SDK, the `<3.0.0`→`<4.0.0` hint; deprecated fields
(`author(s)`, `transformers`, `web`); non-string and non-http URLs; executables
pointing nowhere and with path separators; plural and `sample(s)` directories;
pubspec typos (three, the limit); upper-case package name and library name
mismatch; `COPYING`/`README.txt`/`changelog.txt` renames, invalid UTF-8 README,
generated dartdoc; `any`, single-version, no-lower-bound, no-upper-bound,
pre-release, path and overridden dependencies; strict dependencies in `bin/`,
`lib/`, `test/` and `tool/` (dev dependency used in `lib/`, missing packages, an
invalid `package:` URL, an annotated multi-line directive, a documented export, a
file with a syntax error that is skipped, a tab); leaks (one, `false_secrets`, four
in two files); a devtools extension without `config.yaml`; `publish_to: none`
(dry run, publish, `--to-archive`), a `publish_to` server and an invalid one; the
previous-version hints against the fake server; `dart analyze` findings; directory
and broken symlinks (errors) and a file symlink; a package inside a Git repository
with checked-in ignored files, a modified file and an untracked one (also through
`-C`); and an ignore fixture with `.gitignore`/`.pubignore` at several levels,
negations, anchored rules, an invalid pattern, dot files, `.htaccess`, a directory
named `pubspec.lock`, `pubspec_overrides.yaml`, an executable file, an empty
directory, one with only hidden files and a path longer than 100 bytes (GNU long
name in the tar).

**upload** covers `--force`, the confirmation answered `n`, `y` and `Yes` (with
the "hints.." message) and refused for a package with errors, `--skip-validation`,
`--to-archive` then `--from-archive` (with and without confirmation), `--server`
(valid and invalid), `publish_to`, tokens (`token add`, sent as `Bearer` only to
URLs below the server, also on the listing reads), 401 (the token is deleted, as
Pub does), 403, 5xx with retries (`PUB_MAX_HTTP_RETRIES`), a non-JSON answer, an
answer without `url`, the XML storage error, an upload without `location`, a
server error message on finalization and an invalid finalization, each on
`localhost` and on `127.0.0.1` where Pub's handling differs. The archive hashes in
the request logs are the same for both tools.

**more**: the one transcript difference is the refused-connection report, where
Pub names the *local* port of the attempt (`port = 59068`, `59321` and `59783` in
three runs), which changes on every run; DartForge writes the server's port there.
The lazy `false_secrets` error (not a list) stops the validation with the YAML span
of the value, as in Pub.

## Known differences (see `docs/PUB-E-TEST.md`)

* Uploading to `https://pub.dev` without a token needs Pub's OAuth2 browser login
  (as `dart pub login`, not implemented): DartForge refuses with a message.
* On an interactive terminal Pub's "Validating package..." shows the elapsed time
  after one second; DartForge prints and erases the line without the clock.
* `Analyze` relies on `dartforge analyze`, whose published diagnostics are the
  verified subset (all of the cases above matched).
* The publishing code uses POSIX paths; it was not measured on Windows.
