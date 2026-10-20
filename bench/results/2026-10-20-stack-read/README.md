# Explicit stack-register allocation observations — 2026-10-20

Base `0396131a`. Investigating the foreign-backend retention gate found an
unsupported ABI obligation on the stack checks in the real SDK helpers. The
checks call llvm.read_register.i64 with metadata naming sp/rsp; the existing
primitive-only call parser could not represent this operation.

The reviewed contract follows LLVM LangRef's read_register semantics:
<https://llvm.org/docs/LangRef.html#llvm-read-register-llvm-read-volatile-register-llvm-write-register-and-llvm-write-volatile-register-intrinsics>.
Only the exact i64 intrinsic with one inline metadata argument naming sp or rsp
is recognized. Arbitrary metadata/registers, writes, volatile forms, indirect
callees, trailing instructions and shadowed definitions remain open. Ordinary
function ABI validation remains primitive-only. The producer preserves the full
call/metadata operand in the inventory and records machine_register_read exposure.

Allocation neutrality is separate from machine-address provenance or invocation
coverage. The consumer validates this one metadata ABI exception independently
of ordinary ABI parsing and refuses to authorize its enclosing function as an
entry/import/node even if the exposure flag is removed. No LLVM generated body,
runtime ABI, cache policy, schema or pruning permission is changed by this step.
It resolves one prerequisite; it does not complete the helper/registry proof.

## Regressions and LLVM checks

The positive regression failed on the previous implementation (null footprint
rather than empty), while the negative controls passed. Four new tests cover
exact records/exposure, unsupported forms/shadowing, consumer refusal even with
a removed exposure flag, and modified published metadata/callee/result ABI.
Clang 22.1.8 accepted the reviewed sp form for AArch64 macOS and rsp for x64
Linux as real object compilations. These checks validate LLVM syntax/lowering,
not runtime execution on those two platforms.

The first documentation run rejected a bare URL. It was converted to a link;
the failed checks and corrected final run are retained. Source bindings identify
the final six Rust files.

## Final verification and actual SDK comparison

All **1,038 native library tests**, including ignored tests, passed in 275.69 s.
Strict workspace/all-target Clippy, explicit embedded-LLVM Clippy, warnings-denied
private workspace documentation and the JIT CLI build passed. Scoped rustfmt for
the new contract/tests and git diff checks pass. Global formatting still fails;
its output size, digest and excerpt are retained.

The two-isolate fixture matches Dart 3.6.2 in macOS production, development and
unpruned AOT plus CLI JIT, normally and with actual GC stress. All three complete
assembled IRs match the preserved baseline byte-for-byte. Baseline IR reuse is
bound to paths/hashes. The LLVM compression-object deployment warning is retained;
this is not an older-macOS compatibility certificate.

The production SDK has the same 32,708 function identities/body digests, module
records and global initializers. In 27,067 functions, the formerly unsupported
stack-pointer call is now explicitly recorded with the exact ABI/metadata and
machine-register exposure. The generic unsupported-call-ABI obligation disappears
in 17,929 functions; others still contain unsupported calls. An exhaustive
record comparison checks preserved calls/dependencies, unchanged direct classes,
no unrelated obligation removal and no loss of previously known allocations.
Known allocation closures remain **649**: this increment does not grant a new
class closure, input region or import/entry certificate. No speed or size gain
is claimed.

The unchanged foreign-backend absence gate was rerun and still fails (exit 101).
Both default and experimental closed-class reproductions retain six foreign
definitions. Full-workspace approval and global formatting remain open. Both VMs
remained suspended; no new guest AOT/JIT or explorer GUI matrix is claimed. No
runtime sources or target bundles changed.
