# Borrow canonical JSON keys — 2026-10-20

Base `96bf4c78`. The streaming canonical verifier previously deserialized every
object key into an owned String. Its slice reader can instead lend unescaped
keys from the immutable input. A small Deserialize visitor now uses Cow: ordinary
keys are borrowed, while escaped keys retain their decoded owned copy. The
previous decoded key survives nested value parsing, including parser scratch
reuse, until the next key's ordering/duplicate check.

Canonical ordering is still over decoded strings; serialization, final LF/EOF,
recursion limits, publication hashes and envelope checks are unchanged. No schema,
ABI, runtime source, pruning authority or cache policy changed. One new test
covers mixed borrowed/escaped Unicode/control keys and nested scratch reuse,
with duplicates and ordering violations checked against the existing Value
oracle. Existing numeric, malformed input, buffer boundary and envelope tests
remain active.

## Measurement

A preliminary component observation on the real 70,536,168-byte SDK inventory
measured about 0.630 s for canonical comparison and 0.447 s for envelope
projection (three samples each after a warmup). It identified both traversals;
this change addresses avoidable key copies in the first traversal only.

The retained optimized standalone harness compares the full streaming
canonical/envelope stage before/after. The harness uses rustc -O and the same
preserved Cargo debug dependency rlibs in both variants; this is not a release
compiler benchmark. Compiler work had not started, and both
VMs were suspended. Two warmups and three ABBA cycles produced six measured
samples per variant using macOS time -l maximum RSS in bytes:

| Median | Before | Borrowed keys |
| --- | ---: | ---: |
| Stage time | 1.08573 s | 1.03803 s |
| Maximum RSS | 72,941,568 B | 72,925,184 B |

That is about **4.4% less time** in this isolated stage, with effectively the
same peak memory. This does not establish a whole-compiler speedup or recover
all of the latency increase from streaming validation. Input, helper snapshots,
baseline revision, build commands and raw observations are retained. Both
binaries ran the same assertions and envelope checks.

## Fully optimized dependency check

Both helper snapshots were also compiled with rustc -O against a separate Cargo
release dependency build. Serde 1.0.229, serde_json 1.0.151 and BLAKE3 1.8.7 match
the workspace versions; the standalone lockfile/build command/log are retained.
These runs happened after compiler/tests/execution finished, with VMs suspended.
Two warmups and three ABBA cycles again provide six samples per variant:

| Median | Previous streaming | Borrowed keys |
| --- | ---: | ---: |
| Stage time | 0.20487 s | 0.16620 s |
| Maximum RSS | 72,826,880 B | 72,794,112 B |

The key change reduces this optimized stage's time by **18.9%**, with effectively
unchanged peak memory. A separate same-build ABBA control compares the prior
full-Value format/envelope expressions with the final streaming implementation:
0.33913 / 0.16486 s and 759,930,880 / 72,794,112 B. That control combines the
streaming and key-borrowing changes; its gain is not attributable to this key
change alone. It also shows why the earlier debug-dependency time tradeoff must
not be generalized to release builds. No whole-compiler timing is measured.

## Final verification

All **1,034 native library tests**, including ignored tests, passed in 276.24 s.
Strict workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private workspace documentation and the JIT CLI build passed. Scoped rustfmt and
git diff checks passed; global formatting remains open with size/digest/excerpt.
The new key test and all previous canonical/envelope tests pass.

The two-isolate fixture matches Dart 3.6.2 in macOS production, development and
unpruned AOT plus CLI JIT, normally and under actual GC stress. Three complete
assembled IRs match the preserved baseline byte-for-byte; baseline artifacts were
reused with hashes rather than recompiled. The full production SDK inventory
payload is unchanged. This is not a new guest AOT/JIT or explorer GUI matrix;
both VMs remained suspended. Runtime source, target bundles and schema did not
change. The LLVM compression-object deployment warning is retained.

The original foreign-backend absence gate was rerun and still fails (exit 101).
Full-workspace approval, six foreign definitions and global formatting remain
open. No assertion or pruning permission was weakened.
