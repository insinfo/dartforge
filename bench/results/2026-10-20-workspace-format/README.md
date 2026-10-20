# Workspace Rust formatting — 2026-10-20

Base `3b553086`, formatting commit `1013f38d`. The pinned Rust 1.98.1 toolchain
and rustfmt 1.9.0 format the workspace with the existing defaults. No exclusion,
skip attribute, formatter configuration change or weakened check was introduced.
The original check returned 1 with 15,804,562 bytes of differences: 701 printed
paths refer to 700 physical files because runtime/build.rs is reached twice.
Two formatter applications were needed to reach a fixed point in two expressions.
The final `cargo fmt --all -- --check` and `git diff --check` return 0.

The isolated commit changes only the 700 Rust files processed by rustfmt,
including normal import/module ordering. No functional repair is mixed in.
Original Git blob IDs and before/after SHA-256 bindings are recorded per file.
The complete original formatter output remains in `/tmp/dartforge-format-current.log`;
its digest/size and a normalized excerpt are retained here.

Validation after formatting:

- Workspace/all-target Clippy with `-D warnings`: exit 0.
- Native backend/all-target Clippy with embedded LLVM and `-D warnings`: exit 0.
- Workspace/private documentation with `RUSTDOCFLAGS=-D warnings`: exit 0.
- Runtime: 324 unit tests, 25 integration tests and 88 doctests passed.
- Native backend library: 1,043 tests passed, zero failed/ignored/filtered.
- The current CLI built with JIT and was published using a fresh inode and atomic
  rename, preserving older executables. Compiler identity is recorded.
- The parent/child isolate fixture matches Dart 3.6.2 in macOS AOT production,
  development and without table pruning, plus JIT, normally and under GC stress.
  All three assembled AOT IRs are byte-identical to the pre-formatting fixtures.

Source-byte fingerprints intentionally change despite formatting-only edits.
The runtime source binding changes from `a37848c6491b54570a07e4ca916ece5310d20e4c435fc32258dd2045db587ee5`
to `e0a3c8b04d39eeb559a7e217fe3b4df86978b29ee7bf93f8ac9abf2a17e6f9be`.
Old cache entries and bundles are preserved, not relabeled. Linux ARM64 and Windows
x64 bundles were freshly exported inside their VMs from the same SHA-256-bound
source snapshot. Their declared source bindings match the host, and runtime
artifact hashes were collected. The source archive is local under target/;
its hash, export scripts and terminal export output are retained here.

The current compiler cross-compiled the isolate fixture for Windows x64 and
Linux ARM64 using the new bundles. Both executables were copied and checked
by SHA-256, then executed in their destination VMs normally and with actual
GC stress enabled. All four runs return 0, match Dart VM's `parent=45` /
`child=45` output and have empty stderr. The Linux console runs use root;
this is not interactive desktop-user coverage. The two current bundle symlinks
were atomically promoted only after these checks, preserving the previous
bundle directories and recording the old/new targets. Both VMs are suspended.

This is not a new explorer GUI or guest JIT matrix. LLVM's macOS deployment
warning remains visible, so older macOS releases are not certified. The known
foreign-backend pruning failure and complete workspace approval remain open.
The original `platform_constant_prunes_the_other_backends` gate was rerun
after formatting: it still returns 101 because WindowsBackend bodies remain
in the macOS IR. Its assertion is unchanged; the failed log/status are retained.
No performance or executable-size improvement is claimed.

Logs preserve complete diagnostics with trailing whitespace/final blank lines
normalized. manifest.json records original and saved hashes.
