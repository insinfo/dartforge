# Empty event-entry domains in fixed-image reachability

Base: `eef476c3`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.
Previously even a fixed image without any event producer retained the generated
Ref-input event bridge and could not close its class universe. The new positive
HIR test failed against that implementation (`before.log`).

The proof now defers only the emitter's automatic loop edge in a temporary
summary. The local bridge must exist with Ref(Ref) ABI. Source roots controlled
by native code and FFI trampolines disallow this base case, even with scalar
ABIs, because their native ingress and prior work remain unproved. Other roots,
actual calls, function addresses, protocol tables and SDK references stay in
the graph. If they reach the bridge, the image is rejected.

All remaining reachable bodies must then satisfy the existing complete class
proof. Every admitted native API is reviewed as unable to enqueue Dart work;
imports require verified allocation-free, exposure-free closures. Microtask,
timer, port dispatcher, native dispatcher and finalizer registration APIs are
not admitted. Unknown native work also fails coverage. Thus only complete
success establishes an empty event-input domain for this closed fixed image.
No input set for nonempty queues or external/native event sources is proved.

Only after all checks succeed is `call_dart` removed. The returned graph lets
HIR pruning delete the unreachable bridge; LLVM no longer emits its loop call.
A failed probe keeps the original bridge metadata and broad registration. The
open/JIT image path lacks the required fixed-ancestry premise. No runtime export,
source lowering, dependency or SDK summary schema is changed.

All 46 class reachability tests pass. They cover the positive empty image,
native queue/finalizer sources, native-controlled scalar roots, direct bridge
calls, source roots, table exposure, function addresses and open ancestry. An
imported queueing startup remains rejected. A compiled regression compares eager
and closed executables: both exit successfully with empty output, normally and
under GC stress; only the closed IR loses the loop call and bridge definition.
Existing compiled selector/closure and Type factory regressions also pass.

Strict workspace Clippy and private docs pass. Global fmt still fails existing
drift; its excerpt and full original digest are retained. Full native/workspace
suites and the three-OS desktop matrix were not repeated. No size or performance
gain is claimed. The CLI retains existing macOS deployment-target linker warnings.

The rebuilt final compiler passed six real commands with the experimental flag
and trace enabled: official Dart reference, optimized AOT build, AOT normal and
GC stress, JIT normal and GC stress. The program schedules a microtask and timer;
all four executions exactly match the reference. AOT preserves the loop because
its imported SDK initialization is unproved; JIT has no fixed-image premise.
Compiler hash, commands, exits and stream hashes are in `live-results.json`.

The final real production regression still exits 101: it now stops at
`df.register.dart$3a_internal`, an unproved SDK registration startup. No empty
domain or class set is published for that failing case. Full foreign-backend
pruning, SDK registration certificates and nonempty callback domains remain open.
Text logs normalize trailing whitespace and final blank lines; raw program
stdout/stderr remain unchanged. Original/stored log digests are recorded.
