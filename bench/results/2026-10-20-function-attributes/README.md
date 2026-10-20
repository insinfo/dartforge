# Preserve LLVM syntax when adding function attributes — 2026-10-20

Base `5838ac3b`. The previous attribute insertion searched whole `define` lines
for strings such as ` comdat`, ` personality ` and ` !`. Those strings can occur
inside a quoted LLVM function name. Inserting `optsize` or `uwtable` there
changes the symbol itself. Likewise, `nounwind` in a quoted name, parameter,
string attribute, inline body or comment could incorrectly suppress the actual
function attribute. A last-brace search also confused inline body text with the
function's opening brace; multiline headers were not recognized.

A token scanner now finds the function parameter list and the attribute suffix,
while preserving comments and quoted names/data. It inserts before section,
partition, COMDAT, alignment, GC, prefix/prologue, personality or metadata clauses,
or before the opening body brace. Nested parameter/attribute parentheses are
tracked. The insertion uses the end of the previous code token, so an intervening
comment cannot swallow the new attribute. All three public transformations keep
an already explicit attribute, including `uwtable(sync)`, and are idempotent.
Attribute groups remain untouched; this is not an expansion of grouped attributes.
The implementation does not modify function bodies or declarations.

## Regressions

Four tests fail against the previous public transformations and pass after the
change. They cover quoted names/parameters/string attributes, inline bodies and
comments, multiline headers and trailing clauses, repeated transformations and
nondefinitions. A fifth test compiles both the original and transformed LLVM IR
through the configured Clang. It defines a quoted function with the misleading
words, and a main function referencing that exact symbol; both produce real
object files. This establishes LLVM validity beyond comparing expected strings.
It is not a new machine-code performance measurement.

The three `sdk_module::with*` documentation examples pass. Full checks and runtime
observations are retained under `checks/` and `runtime/`. Normalized log bindings
preserve hashes and sizes of the originals, including before-fix failures.
Source bindings identify the final Rust files; artifact bindings identify large
executables and retained IR outside Git.

## Full validation and scope

The complete native library suite passed **1,015/1,015**, including ignored tests,
in 258.82 seconds. The five directed tests overlap that total. Strict workspace
all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied private docs,
and the JIT compiler build passed. Scoped rustfmt passes for the new module;
global formatting still fails, with output size/digest and an excerpt retained.
The initial before-fix run contains four tests; the real LLVM compilation test
was added afterward. `before-initial.py` retains that harness; `before.py` pins
the base revision for subsequent reproduction instead of following a moved HEAD.

The existing `context_literal` Dart fixture matches Dart 3.6.2 before/after in
macOS AOT production, development and disabled table pruning, and after in macOS
JIT, with normal/stressed GC. Actual assembled macOS IR is byte-identical before
and after for all three profiles, as independently bound by
`runtime/ir-comparison.json`. These canonical headers therefore receive the same
attributes; the targeted tests cover the previously incorrect special headers.
Cross AOT executables also pass both GC conditions in Linux ARM64 and Windows
x64 compatibility on Windows 11 ARM64, with guest copy hashes checked.
All application stderr streams are empty. Both VMs are suspended again; the
Linux temporary executable is removed even if its assertions fail.

No runtime source, target bundle, ABI or class-pruning permission changed. This
is not a new explorer GUI/guest JIT matrix, a universal speedup or binary-size
claim. Foreign-backend pruning and whole-workspace test approval remain open.
