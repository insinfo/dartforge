# Enum facts through lazy-cache joins

The original enum source regression failed because getter results entered phis
on checked success edges, while availability was tested against the whole join
block. The value analysis now checks phi operands on incoming edges, recognizes
warm loads only behind their audited cache initialization flag, and propagates
identical tokens through structurally valid phis. Cold calls and pending checks
remain. Cache writes/exposure in specialized clones invalidate the analysis.

The added regressions cover direct success-edge phis, block reordering, warm
loads sharing a block, cache/flag mismatches, swapped guards, unequal tokens,
unknown/cyclic operands, exception-side uses, duplicate SSA/phi edges, malformed
predecessors, uses before loads, and clone-local cache writers.

`python3 bench/results/2026-10-19-enum-phi/run.py --compiler <dartforge> --sdk <SDK root> --llvm <LLVM root>
--packages <dart_ui package_config.json> --out <output directory>` compares the
existing enum-pattern identity fixture and ordinary color conversion fixture
against official VM/AOT output with constant specialization OFF/ON and GC stress.
This validation does not claim a performance gain or enable the experiment by
default. Producer/input hashes and commands are retained in validation.json.
