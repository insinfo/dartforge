# String length reuse before native emission — 2026-10-19

The new HIR pass recognizes only `df.string_len(Ref) -> I64` on an SSA
reference with a checked result representation. It reuses dominating reads of
the same logical immutable string, with separate dominator scopes for sibling
branches. A natural loop's leading read prefix can move to its unique outside
predecessor only when that predecessor branches unconditionally to the header
and the receiver is already available outside the loop. No intervening checks,
effects or conditional entry are crossed. A loop-carried receiver stays inside.

The pass reuses the bounded structural SSA witness (512 blocks/8,192
instructions), checks duplicate definitions/result representations separately,
and skips debug bodies and ownership-prepared modules. Constants passed directly
to the helper are excluded because JIT materialization can allocate. Hashes,
code units, list lengths and arbitrary runtime calls are not generalized into
this rule. No LLVM `invariant.load` promise was added; the emitted length helper
is byte-identical across all stored controls. `DARTFORGE_NO_STRING_LENGTH_REUSE=1`
disables this pass; the existing compiled SDK key includes these ablation flags.

## Checks and functional observations

- Eight new tests passed. An independent bounded HIR interpreter compares return
  values and observable call sequences for zero/nonzero loops, changing string
  identities, conditional null paths, and branches. Additional controls cover
  Phi edge replacement, malformed SSA/ABI, unreachable duplicate definitions,
  oversized bodies, constants, other helpers and ownership preparation.
- In that interpreter, 39 iterations require 79 length reads before and one
  afterwards. This is an operation count in a semantic test, not a timing result.
- All **930 native library tests**, including ignored tests, passed. Strict
  workspace/all-target Clippy, explicit embedded-LLVM Clippy, and private
  workspace documentation with warnings denied passed. New files pass rustfmt;
  the repository-wide formatting gate remains unresolved.
- The Dart fixture combines Latin-1/UTF-16, empty strings, an opaque no-inline
  call, a receiver replaced during iteration, and a skipped nullable path. The
  Dart 3.6.2 VM prints `35294`. AOT production with the pass enabled/disabled,
  ARC, and development AOT all match it. Each was also run with the actual
  `DARTFORGE_GC_STRESS=1` environment setting.
- Real program IR changes from two length call sites to one in both `stable`
  and `changing`. The retained read moves outside the stable loop but remains
  inside the loop with a changing receiver. Function excerpts are stored; full
  IR digests and compiler/SDK bindings are recorded separately.
- JIT IR emitted by **`emit_reloadable_ir`**, the API used by `dartforge run`,
  runs through the retained ORCv2 executor and matches the VM normally and under
  GC stress. `emit-jit.rs`, `validate-jit-api.py` and `jit-api-results.json`
  describe this check. This is not a rebuilt CLI JIT or a new three-OS matrix.

The first AOT verifier passed `--gc-stress` as a program argument, which does not
configure this runtime's collector. Those observations are preserved under
`initial-argument-only/` and are not stress evidence. The corrected verifier was
rerun; final `execution.json` records the actual environment setting explicitly.

## Failures retained, not approved

The earlier broad Cargo continuation (which excluded the already-tested build
package) passed its 922-test native library binary and reached production profile
integrations. It then exited **101**: `platform_constant_prunes_the_other_backends`
still finds Windows backend definitions in macOS IR. The other two production
profile tests passed. `workspace-continuation.log` retains the complete result.
That binary predates this pass; no assertion was weakened. This is not a global
workspace approval, and packages after that failure remain unvalidated by it.

Directly feeding the pruned AOT development IR to `dartforge-run-ir` also
segfaults. The previous retained compiler reproduces the same failed scenario
with its own matching SDK DLL, while its AOT binary matches the VM.
`old-pruned-jit.json` and `jit-execution.json` retain both observations; the
cause is unresolved. The regular JIT API path above passes. Initial verifier
setup failures (Mach-O RPATH resolution and an AOT-copied DLL without its JIT
export manifest) are retained too; the final JIT API check uses the matching SDK
cache library and hashes its manifest. A debugger attempt was stopped and is
not used as crash-cause evidence.

No general compiler-speed gain, binary-size improvement, class-pruning
authorization or File Explorer desktop regression matrix is claimed here.
Executables, full IR and SDK artifacts remain in ignored target/scratch paths;
the checked-in fixture/scripts and recorded hashes support reproducing them.
