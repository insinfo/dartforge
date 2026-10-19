# Registry call observations and erased-ABI result boundaries

Base `b697b9e1`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.
Inspection of the blocked SDK startup showed that the inventory published call
ABIs but omitted operands. Quoted LLVM section arguments also caused direct
registration calls to be recorded only as unsupported/indirect ABI obligations.
The first regression failed before the parser change.

The operand scanner now handles nested delimiters, quoted identifiers, LLVM hex
escapes and comments outside quotes. Malformed lists/escapes, indirect calls,
quoted callees and unreviewed calling conventions/attributes stay unproved.
Inventory schema 2 publishes typed `arguments` for recognized direct calls.
The reader checks their count/types and supported syntax against the ABI; this
is observational data, not proof of SSA origin, object identity, memory region,
allocation closure or authorization to mutate a registry. Prior schema snapshots
cannot supply absent operands. Re-signed malformed operand records still fail
semantic loading after passing the publication-integrity check.

A scalar quoted-SSA call can now close through a verified local scalar helper.
Allocation-neutral registry functions remain opaque. The final real SDK inventory
loads correctly and its registration startups are still unproved. Comparing
snapshots found unchanged body digests for all 15 startup functions: each now
records one `dartforge_register_image` call previously lost behind quoted section
operands. Static image contents and helper bindings still require certificates;
no startup is authorized merely by its familiar name or by collecting its args.

The review also reproduced a false class-proof approval: compatible LLVM i64
results could be requested as Ref, and allocation-free Ptr returns could be
requested as packet addresses. New eligibility checks require separate value
or region certificates for Ref/Ptr results. Allocation observations remain
available; compatible scalar and void calls retain their existing policy.
The class regression consumes the unproved result as a receiver or forwarded
argument packet. This is a HIR proof regression, not an observed application
crash or a complete Dart-level ABI conformance claim.

Final checks: 38 inventory tests (including actual production SDK publication
and loading), 47 class-proof tests (including compiled selector/closure/Type
and empty-event-domain regressions), strict workspace Clippy and private docs
pass. Global fmt still fails existing drift. Full native/workspace suites, the
production profile assertion and desktop/VM/JIT matrices were not repeated.
The last production result remains blocked at SDK registration. No complete
foreign-backend pruning or binary performance/size improvement is claimed.

Tradeoff: the sampled compiler cache JSON grew from 57,251,557 to 66,874,532
bytes (about 16.8%) to retain operands. This is compiler-owned inventory metadata,
not an executable-size measurement. The operands are needed for subsequent
registration/value/region checks; they do not by themselves provide those proofs.
Full SDK inventory files are not committed; comparison reports retain paths,
hashes, counters and unchanged startup-body digests.

Text logs normalize trailing whitespace/final blank lines; original and stored
digests are recorded. The format check retains an excerpt and full original hash.
