# Preserve LLVM literals when rewriting context calls — 2026-10-20

Base `d067a845`. The AOT context transformation previously used a global text
replacement of `call ptr @dartforge_context()`. That sequence is also valid
program data. Compiling the actual Dart fixture `context_literal/main.dart`
with the previous compiler fails in Clang: a 32-byte padded string initializer
is shortened to 25 bytes without changing its declared type. The original
failure and compiler digest are retained in `runtime/before-runtime.json`.

The transformation now scans LLVM tokens, skipping semicolon comments, quoted
strings, inline assembly strings and quoted/unquoted identifiers. Embedded
hexadecimal escapes and quoted names remain intact. Only the existing exact
instruction spelling is replaced; this does not widen the supported call ABI.
The scanner handles inline function bodies and keeps function-body labels from
being mistaken for module-level definitions.

An existing definition of `df.context`, including quoted/hex-escaped names,
preserves the entire module and its first configuration. In particular, a native
TLS helper's slow-path runtime call cannot become recursive or cause another
helper definition to be appended on reapplication. Later appended runtime calls
remain valid runtime calls rather than being redirected to an unverified helper.
The native TLS and macOS TSD helper templates themselves are unchanged.

## Regressions and execution

Four directed tests cover protected bytes and comments, existing helpers with a
slow-path call, inline function bodies, idempotence and misleading definition
words in function bodies. Two tests fail against the previous public transform;
all four pass after the change. The initial lexical fixture declared 29 bytes
for a 30-byte NUL-terminated constant; this independent fixture typo was fixed
before final validation. The earlier failures are retained, and do not serve as
LLVM syntax validation. The separate Dart program reproduces the real compiler
failure and supplies valid generated LLVM IR.

The Dart fixture prints the original literal, its length (29), and a quoted
interpolation with Unicode. Updated execution matches Dart 3.6.2 exactly, with
empty stderr, under both normal and stressed GC:

| Execution | Result |
| --- | --- |
| macOS ARM64 AOT production | pass / pass |
| macOS ARM64 AOT development | pass / pass |
| macOS ARM64 AOT without table pruning | pass / pass |
| macOS ARM64 JIT | pass / pass |
| Linux ARM64 cross AOT, executed in Ubuntu | pass / pass |
| Windows x64 cross AOT, executed in Windows 11 ARM64 compatibility | pass / pass |

Cross executables are built on macOS against the existing source-matched target
bundles. Guest copies are hash-checked before execution. No runtime sources or
bundles changed; both VMs were restored to their initial suspended state.
This is a focused compiler regression, not a new explorer GUI or guest JIT matrix.

## Verification and limits

The complete native library suite passed **1,010/1,010**, including ignored tests,
in 246.32 seconds. The four directed tests overlap that count. Strict workspace
all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied private docs,
and the JIT compiler build passed. Scoped rustfmt passes. Global formatting
still fails; its full output size/digest and normalized excerpt are retained.
Whole-workspace test approval and foreign-backend pruning remain open. No new
class-pruning permission, allocation certificate, ABI or performance claim is
introduced by this correction.

`source-bindings.json` identifies the final sources. `log-bindings.json` binds
original and normalized logs/scripts, including the before-fix failures.
`artifact-bindings.json` identifies preserved compilers, executables and retained
IR artifacts outside Git. The macOS compiler is published through a fresh inode
and atomic rename, avoiding the earlier in-place-copy signature problem.
