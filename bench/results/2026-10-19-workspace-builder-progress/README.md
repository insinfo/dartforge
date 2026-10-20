# Broad workspace run: native builder corpus passed — 2026-10-19

The original configured workspace command has advanced beyond the native builder
executor corpus. Its completed test reports **one passed, zero failed**, in
2,428.12 seconds (about 40.5 minutes). `native-corpus.log` retains the complete
test section extracted from the still-running workspace log.

The test calls the native builder executor, requires complete oracle outputs
(no pending/different entries), and checks incremental edits against clean runs.
The running Cargo parent's selected environment was inspected without printing
unrelated variables: `DARTFORGE_CASES` and `DARTFORGE_CORPUS_BUILDERS` are absent,
so no case subset was requested. The observed planned inventory contains ten
cases; plan, oracle manifest and package configuration hashes were collected
after the test, as explicitly marked in `progress.json`.

This run uses the preserved reference compiler snapshot, SHA-256
`0945d0980bc86c875156713f4b00c28f1063d40d2442a602730a05c76ffcfbcd`, not the later
compiler changes. Its hash was checked again. The test binary/source and case
selection helper hashes are recorded separately. This is evidence for that
reference corpus run, not approval of every current compiler mode or package.

The same workspace process now runs `ng_transparency`. Three tests passed:
direct generator/oracle equality, dependency generation/oracle equality, and
incremental/clean equality. The per-Dart-file edit test remains active at this
observation. There is no complete workspace result yet; the command was not
restarted or stopped. The full live log remains at
`/tmp/forge-workspace-20261019-configured.log`.

The later cache/rooting changes have their own native library, Clippy,
documentation and focused execution checks in the neighboring evidence folders.
They do not turn this partial workspace run into a global pass.
