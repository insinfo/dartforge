# Bounded inventory integrity I/O — 2026-10-20

Base `e95c74ff`. Cache verification previously serialized the complete inventory
to a String and formatted a second String with its trailing LF before comparing
against the verified read. Publication built those same full-size strings before
writing and hashing them. Artifact hashing also read each whole file into memory.
The default path already restricts full inventory consumption to experimental
closed-class analysis; integrity verification still protects ordinary cache hits.

Canonical output now flows through a 64 KiB buffer. Verification compares those
bytes directly against the same read bound to the ready marker, while publication
hashes exactly the bytes successfully written and flushed. The ready marker is
written only after that operation succeeds. Artifact digests use BLAKE3 1.8.7's
`update_reader`, whose implementation uses a 65,536-byte buffer and retries
interrupted reads. No path-only memoization or integrity check was removed.

The JSON format remains compact canonical serialization plus exactly one LF.
Duplicate keys, reordered keys, whitespace, alternate number spelling, missing LF
and extra trailing bytes still fail. Short writes update the digest only for the
accepted prefix; write and flush errors return no successful publication digest.
The payload and typed JSON tree still occupy their existing memory: this change
bounds output/hash scratch, not the entire inventory consumer.

## Verification

Four new tests compare exact bytes and hashes with the old serializer across
null, escaped/Unicode data and multi-buffer output; reject noncanonical and
duplicate input; hash empty/large/missing files; and exercise partial writes and
write/flush errors. Existing artifact/runtime/profile/key/corruption checks remain.
The complete native library suite passed **1,030/1,030**, including ignored tests,
in 300.14 seconds. Directed tests overlap that total. Strict workspace/all-target
Clippy, explicit embedded-LLVM Clippy, warnings-denied private documentation and
the JIT CLI build passed. Scoped rustfmt passes; global formatting remains open,
with its full output size/digest and normalized excerpt retained.

Actual production SDK `inventory` payloads are identical before/after for macOS
ARM64, Linux ARM64 and Windows x64. The nine-class two-isolate fixture matches
Dart 3.6.2 in before/after macOS AOT production, development and disabled pruning,
and after macOS JIT, under normal/stressed GC. Updated cross AOT executables pass
both conditions in Ubuntu ARM64 and Windows 11 ARM64's x64 compatibility mode,
with copied executable hashes checked and empty stderr. All five complete assembled
IR comparisons are byte-identical. Both VMs are suspended again.

The unchanged foreign-backend absence gate was rerun and still fails (exit 101);
both platform reproductions retain six foreign definitions. Whole-workspace test
approval and global formatting remain incomplete. No runtime source, target
bundle, ABI, schema, exposure permission or pruning decision changed. This is
not a new explorer GUI/guest JIT matrix or older-macOS compatibility certificate;
the existing LLVM compression-object deployment warning is retained.

## Isolated memory observation

The retained optimized standalone harness uses the actual production inventory,
parses the same input into `serde_json::Value`, and runs the old expressions or
the new helper. Comparison checks exact bytes; writing uses a sink and checks the
digest, isolating serialization/hash rather than filesystem publication. Input
bytes remain alive in both variants. It therefore measures these routines in a
separate process, not whole-compiler memory or end-to-end compilation speed.

After all compiler work finished and both VMs were suspended, each stage ran a
warmup per variant and three ABBA cycles: six measured samples per variant/stage.
`/usr/bin/time -l` reports macOS maximum RSS in bytes. Median results:

| Stage | Previous RSS | Updated RSS | Reduction |
| --- | ---: | ---: | ---: |
| Canonical comparison | 902,955,008 B | 760,020,992 B | 136.31 MiB |
| Serialization/hash | 903,118,848 B | 760,217,600 B | 136.28 MiB |

Comparison-stage medians were 0.06355 / 0.06233 seconds; serialization/hash medians
were 1.69339 / 1.68443 seconds. The small timing differences do not establish a
general speedup. All samples passed exact output/digest assertions. Raw samples,
RSS streams, the harness source/build command, input size/hash and summaries are
retained. No universal binary-size or compilation-time gain is claimed.

Source bindings identify the final three Rust files. Log bindings preserve
original and normalized records. Large compiler, executable, IR and inventory
artifacts remain under ignored target storage with hashes. Compiler publication
uses fresh inodes and atomic rename so active executables are not overwritten.
