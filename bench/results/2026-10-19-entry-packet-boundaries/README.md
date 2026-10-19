# External argument packet boundaries in closed class reachability

Base: `7465fee8`, macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.
Inspection of the unproved event-loop input boundary found a separate omission:
local external roots rejected Ref parameters but admitted raw Ptr parameters.
A root could forward those pointers as argument/descriptor packets without a
proof that their contents belong to the selected class universe. Scalar bits
could also be reinterpreted as Ptr in an otherwise admitted body.

Both regressions failed against the original implementation because audit
returned success (`external-input-before.log`, `scalar-cast-before.log`). These
are HIR proof regressions, not an observed crash or exploit in File Explorer.
The callback's existing Ref input is still unproved; this change does not admit
it or establish a complete runtime queue/packet provenance model.

External roots now require an explicitly reviewed scalar representation. The
checks cover main, source roots, event-loop bridge, program record, RTI startup
and local SDK record roots. Imported startup keeps the zero-argument void and
pure-closure requirements. Unknown/raw addresses cannot be introduced through
scalar/managed bitcasts or unboxing; Ptr identity casts retain existing addresses.
Unbox now requires a Ref source, matching result type and a scalar destination.
Runtime exports, source lowering and SDK schema are unchanged; proof admission
is stricter. A newly rejected proof preserves broad class registration.

All 41 class reachability tests pass, including four new regressions/controls.
The tests reject external packets at all six root categories, pointer fabrication
and managed reinterpretation. They retain scalar inputs, vector inputs, local
stack addresses and pointer identity casts. Existing compiled selector, uniform
closure and typed closure tests still match eager registration under GC stress;
the compiled runtime Type factory contract also passes. Strict workspace Clippy
and private docs pass. Global fmt still fails existing formatting drift.

Verification is scoped to the changed proof module and its compiled regression
programs. The full native/workspace suites, CLI desktop matrix and Windows/Linux
VM runs were not repeated. There is no new performance or size claim. The last
real production result still stops at `df.call_dart0`; its Ref input remains
rejected by the new scalar-only root check. No complete backend pruning or
complete closed-image/native ABI certification is claimed.

Text logs normalize trailing whitespace/final blank lines; original and stored
digests are retained. The format check retains an excerpt and full original hash.
