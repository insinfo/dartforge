# Flow-ordered structural dominators — 2026-10-20

Base `c3196ed8`. The structural SSA witness already used 512-block dense sets,
but visited reachable blocks by their physical vector index. A reverse-numbered
chain requires 511 sweeps at that budget, even though its dependency order is
straightforward. This increment changes analysis scheduling, not CFG layout,
the dominance equation, proof budgets, typing or pruning permissions.

## Implementation

An iterative depth-first traversal produces reverse postorder while identifying
reachable blocks. Each frame retains one active successor index; duplicate switch
edges cannot grow the pending stack beyond the number of blocks. Shape reuses
that order for the fixed point and retains its sorted reachable set for every
other existing check. No successor graph copy or recursive host-stack walk is
introduced. The dominator constructor checks unique/in-range traversal members
and entry identity, then performs the same monotone bitset intersections.

Unreachable predecessors, incoming entry edges, malformed SSA, Phi/edge checks,
the 512-block/8,192-instruction budget and all conservative rejection behavior
remain unchanged. The sweep counter exists only in unit-test builds. No new
compiler/runtime option or invariant metadata is introduced.

## Correctness and work measurement

The existing exhaustive path-removal oracle covers all 4,165 directed graphs
through four blocks without incoming entry edges. It checks every candidate
block's dominance independently by deleting that block and recomputing reachability.
Existing 512-block, cyclic and unreachable-parent cases remain active. New tests
add 48 deterministic shuffled cyclic graphs, iterative traversal with duplicate
edges at the full block budget, and numeric versus flow scheduling of the same
512-block bitmap solver.

The controlled solver comparison requires identical complete sets and exactly
**511 versus 2 sweeps**. Its timing excludes order construction and is a component
measurement, not a whole-compiler speedup. The initial test mistakenly expected
512 numeric sweeps: the deepest node already holds its final all-block set at
initialization. The assertion was corrected to the exact 511 count; full-set
comparison and the independent correctness oracle remain unchanged. The original
failure log is retained. A subsequent strict Clippy rejection of a modulo check
was corrected with `is_multiple_of`; its original log is also retained.

## Application comparison and limits

The actual File Explorer sources are from `dart_ui` revision
`15aff7042318d2fa56661ffed4360c81a41a86b8`, with a clean worktree. Baseline
compiler and source/package hashes are recorded. Its first CLI timings reported
96.68 s in HIR pruning, including SDK preparation; after warming, that phase took
27.01 s. This cold/warm difference predates the optimization and is not its gain.
The two baseline emissions produced the same 293,593,137-byte LLVM IR.

Optional macOS `sample` processes supplied no report and remained live for
minutes. Only those samplers were explicitly terminated; both compiler runs were
awaited and exited zero. Cancellation records and original subprocess statuses
are preserved. No stack profile is claimed, and the sampling controller's elapsed
times are not used as compiler performance measurements.

The updated compiler is compared against the preserved baseline using an isolated
SDK cache namespace and full IR hashes. A branch-heavy executable is also compared
against Dart VM output normally and with actual `DARTFORGE_GC_STRESS=1`. Commands,
outputs and terminal statuses are retained. This is not a new desktop AOT/JIT
matrix, a fix to foreign-backend pruning, or a full-workspace test approval.

## Final verification

The final library suite passed **962/962**, including ignored tests (227.21 s).
All seven directed dominator tests passed, including the three new tests; those
counts overlap. Strict workspace/all-targets and explicit embedded-LLVM backend
Clippy and warnings-denied private docs passed. Scoped rustfmt and
`git diff --check` passed. Global formatting still fails existing drift; its
original log digest/size and normalized excerpt are retained.

Both cold and warm updated emissions match the baseline's full File Explorer
IR, SHA-256 `0e9cff28f0f9e9a04f86aecdccdf66d4fb9c123617e02014e2bd4d5c124e212d`.
The updated warm invocation took 109.87 s, versus the sampling controller's
111.91 s baseline record. These are single observations with different prior
activity/cache preparation and an unavailable sampler; no whole-compiler speedup
is inferred. The controlled sweep-count reduction is the demonstrated work gain.

Before/after AOT branch executables both print `-86488`, matching Dart 3.6.2,
normally and under GC stress. Four further executions explicitly record whether
`DARTFORGE_GC_STRESS` was unset or `1` and bind executable hashes. All passed with
empty stderr. The fixture source is already retained in
`../2026-10-19-dense-dominators/branches.dart`.

Large IR/executables remain under ignored `target/rpo-validation` storage;
`artifact-bindings.json` records locations, sizes and hashes. An initial move to
that SSD location reported an owner/group preservation warning; the moved bytes
were independently checked against the baseline digest before use. No compiler
or executable was terminated for that warning. SDK cache schemas, runtime ABI,
closed-class acceptance and production-profile expectations are unchanged.
