# Opt-in Linux RELR packing — 2026-10-19

The Linux linker accepts `DARTFORGE_PACK_RELR=1`, using LLD's
`-z pack-relative-relocs`. Unset/`0` preserves the previous arguments; invalid
values fail for Linux. Windows and macOS do not consume this policy. The feature
remains off by default. Source/class-ID and production bitcode keys stay unchanged;
only the affected Linux development shared SDK gets a separate compiled key.

## Controlled real application comparison

The real dart_ui File Explorer was compiled for Linux ARM64 with the retained
compiler identified in `build.json`. Its SHA-256 is
`d94913f8b04055a37eda0e53895cd7a1f5fbb2805f7e1ec4ca6004e02326888d`.
This was not a full frontend rebuild using this change's compiler source. The
retained compiler matched the existing target bundle; no compatibility manifest
was changed to bypass runtime-source validation.

The capture wrapper copied 47 explicit file inputs (213,612,680 bytes) before
temporary object cleanup, and rewrote the baseline command to those copies.
The packed command used the same files in the same order, with only the packing
option and output path changed. Input hashes were checked again after linking.
Commands, bindings, compiler identity and stdout/stderr are retained here.
This inventories explicit inputs, not every transitive linker dependency.

| Linux ARM64 File Explorer | Standard | Packed |
|---|---:|---:|
| Executable bytes | 37,624,936 | 36,011,472 |
| Declared relocation table bytes | 1,633,512 | 19,920 |
| Permanent strings | 14,548 | 14,548 |
| Permanent string section bytes | 854,760 | 854,760 |

Saving: **1,613,464 bytes (4.29%)**. Both string sections have BLAKE3
`9a2ee1e50eeb87245d62af10c1b049fcb3c173eb7df098ba87acf7477dc17272`.
No speed improvement is claimed; build/cache/VM timing was not controlled.
`image-audits.jsonl` was produced by the new `inspect_linked_images` example.
It checks file bytes and declared relocation writes, not post-loader memory,
runtime immutability, implicit loader writes or callback effects.

## Backend fixture and execution

The new ignored backend test calls the actual linker statement helper for both
modes. A C fixture checks all 4,096 relocated pointers and values, and includes
a canonical permanent string section. Its standard/packed files are 138,648 and
40,832 bytes; relocation tables are 98,688 and 776 bytes. String section digests
match. The fixture's source and argument vectors are retained here.

`vm-results.json` records four successful executions on Ubuntu ARM64/glibc 2.39:
both fixture modes print `relr-ok`, and both real explorer modes present two
frames through Wayland/OpenGL and exit zero. Host and guest SHA-256 hashes match.
These are short startup/rendering checks, not exhaustive GUI interaction tests.
The executable files and captured objects remain in ignored `target/` storage.

A negative fixture linked with raw `--pack-dyn-relocs=relr`, without the version
requirement, was rejected with exit 127 and
`DT_RELR without GLIBC_ABI_DT_RELR dependency`. This is an expected passing
negative control, recorded in `unversioned-control.json`. `version-info.log`
confirms the packed explorer's version requirement in `.gnu.version_r`.

The glibc RELR support floor is 2.36; other sysroot ABI requirements still apply.
LLD's chosen flag synthesizes `GLIBC_ABI_DT_RELR` when `GLIBC_2.*` dependencies
exist. See the [LLD 22.1.8 manual](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/lld/docs/ld.lld.1)
and [glibc integration](https://sourceware.org/pipermail/glibc-cvs/2022q2/078142.html).

## Checks and limits

With LLVM 22.1.8 on PATH and the matching Linux sysroot selected:

```sh
DARTFORGE_TEST_LINUX_SYSROOT="$PWD/target/cross-bundles/current/linux-arm64/sysroot" \
  cargo test --locked -p dartforge-emit-native --lib relr_ -- --include-ignored --nocapture
cargo test --locked -p dartforge-emit-native --lib selected_source_cache_tests -- --include-ignored --nocapture
```

The suites passed 5 and 6 tests respectively, with one overlapping test:
10 distinct tests, including 3 new regressions. Subprocesses test cache modes
without mutating environment variables in concurrent test threads. They prove
default/0 equality, unchanged source/production keys, a changed Linux shared SDK
key and unchanged macOS keys. The fixture was actually linked, not just printed.

`checks-results.json` records strict workspace/all-targets Clippy, explicit
embedded-LLVM backend Clippy, and workspace private documentation passing.
Documentation used `RUSTDOCFLAGS=-D warnings`; LLVM selection used
`DARTFORGE_LLVM_DIR` and `LLVM_SYS_221_PREFIX` pointing to the recorded prefix.
Global formatting still fails on existing workspace drift. Only its first 120
lines are stored, with the original full log size/hash in `fmt-original.json`.
The complete workspace test suite remains unapproved. This change does not
authorize class/SDK pruning, prove a complete SDK startup closure, or add a new
Windows/macOS/JIT execution matrix.

`retention.json` binds original/stored logs, normalizing trailing whitespace and
final blank lines. `source-manifest.json` binds the changed source and documents;
`SHA256SUMS` binds retained evidence. Tool versions and hashes, application
revision, command equality and fresh hash checks are in `verification.json`.
