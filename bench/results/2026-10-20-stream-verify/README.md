# Streaming inventory integrity verification — 2026-10-20

Base `e4b13415`. Ordinary SDK cache hits previously parsed the complete inventory
into `serde_json::Value` solely to check its canonical serialization and envelope.
The integrity verifier now compares canonical JSON through a 64 KiB output buffer
and deserializes a small envelope projection. Arrays of inventory facts are
visited without retaining their object trees. Hashes still bind the same immutable
payload read to its ready marker and selected compiler/runtime/artifact contents.

The full inventory consumer (`read_verified`) is unchanged and still performs
its complete validation. The projection deliberately retains the previous
integrity-only shape contract: four inventory fields, including three arrays.
Integrity does not interpret function records or grant pruning authority. The
fourth field's name/value remains the consumer's responsibility, exactly as before.
The schema, runtime ABI and cache policy are unchanged.

The canonical visitor serializes scalar values through the same serde_json
serializer, requires strictly ordered decoded object keys, rejects duplicates,
and compares exactly one final LF and EOF. The parser retains its recursion
limit. The input payload and small metadata Values remain allocated; escaped
strings and keys may require scratch proportional to their size. This is not a
constant-memory guarantee for arbitrary hostile JSON or the full compiler.

Three regression tests compare the streaming predicate with the existing Value
oracle across numeric forms, Unicode, duplicate/reordered keys, malformed input,
large strings/arrays and buffer boundaries. Fourteen envelope variants compare
both verifiers, including valid uninterpreted function records, the fourth field,
wrong array shapes, extra/missing fields and changed integrity bindings.

## Final host verification

All **1,033 native library tests**, including ignored tests, passed in 276.14 s.
Strict workspace/all-target Clippy, explicit embedded-LLVM Clippy, private
workspace documentation with warnings denied and the JIT CLI build passed.
Scoped rustfmt and git diff checks pass; global formatting still fails existing
drift. Its output size, digest and excerpt are preserved. A formatting scratch
attempt initially failed to resolve a child module; no source was changed by
that attempt. Standalone harness compilation also initially failed child-module
resolution; the successful harness uses a recorded copy with an explicit child
path and otherwise identical helper source.

The two-isolate fixture matches Dart 3.6.2 before/after in macOS production,
development and unpruned AOT, normally and under GC stress. Updated CLI JIT passes
both conditions. All three complete macOS assembled IRs are byte-identical.

## Memory/latency tradeoff

An optimized standalone harness checks the actual 70,536,168-byte production
inventory. Both variants keep the input bytes alive; the old variant builds the
Value tree and checks its canonical serialization/envelope, while the new variant
uses the streaming predicate and envelope projection. This isolates format/shape
validation; runtime/artifact hashing and the compiler process are excluded.

After the host compilation/tests finished and both VMs were confirmed suspended,
two warmups and three ABBA cycles produced six measured samples per variant:

| Measurement | Previous | Streaming |
| --- | ---: | ---: |
| Median maximum RSS | 759,996,416 B | 72,941,568 B |
| Median validation time | 0.86049 s | 1.12418 s |

That is **655.23 MiB less peak RSS (90.4%)**, with **30.6% more validation time**
in this isolated stage. The memory gain is not a speedup: the new implementation
parses once to compare canonical bytes and again to project the envelope. The
full compiler was not benchmarked, and its total memory/time cannot be inferred
from these process measurements. The rustc -O harness linked Cargo debug
dependency rlibs; this time tradeoff is not a release-build conclusion. See
`../2026-10-20-borrowed-keys/README.md` for the later controlled release-dependency
comparison. All samples completed their assertions.
Raw observations, source bindings, build command and input digest are retained.

## Cross-target validation and remaining gates

Updated cross AOT fixtures pass normal/stressed execution in Ubuntu ARM64 and
Windows 11 ARM64's x64 compatibility mode. Copied executable hashes were checked,
stdout matched the VM reference and stderr was empty. Both VMs were returned to
their initial suspended state. All five complete assembled IR comparisons are
byte-identical. The full `inventory` payloads of macOS, Linux and Windows SDKs
are unchanged (32,708 / 32,708 / 32,693 functions; 649 / 650 / 634 known footprints).
Runtime sources and target bundles were not changed. This is not a new explorer
GUI or guest JIT validation matrix.

The original foreign-backend absence gate was rerun and still fails with exit
101. Both platform reproductions retain the same six foreign definitions.
Full-workspace test approval and global formatting remain open; no test or
pruning predicate was weakened. The LLVM compression-object deployment warning
is preserved; older-macOS compatibility is not certified.
