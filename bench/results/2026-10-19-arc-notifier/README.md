# Late generic notifier ARC regression

Production ARC code and the Dart fixture are unchanged. The old test required
`Owner.update` to fail conservative field preparation and expected a scalar
read guard in that function. The lowered body already uses the late-Ref
protocol, and the scalar `holds` reads currently stay in `Derived.notify`.

The updated test follows actual scalar reads in either body, verifies the
original late-Ref reads survive, and checks each scalar result's receiver,
index, type, layout vector, declared Derived shape and i64 representation.
Removing all admissible scalar layouts from the real body must still fail.
The existing class-case negative tests continue to reject contradictions.

`python3 bench/results/2026-10-19-arc-notifier/run.py --compiler <dartforge>
--sdk <SDK root> --llvm <LLVM root> --out <output directory>` reproduces the
native comparison. The frozen producer is from the production implementation
in ad52385d; no engine fix is claimed. Both tracing and ARC match official VM
output in normal/stress runs. ARC uses ARC_CHECK=1 and ARC_NURSERY=0.
SDK 3.6.2, LLVM 22.1.8, macOS ARM64; no performance or complete ARC safety claim.

The full native suite was followed by the final test placement refinement and
14 passing focused tests. The remaining known failure is the color kernel
specialization test. General workspace checks remain non-green as recorded in
checks.json. Executables remain in ignored target/notifier-checked-validation.
