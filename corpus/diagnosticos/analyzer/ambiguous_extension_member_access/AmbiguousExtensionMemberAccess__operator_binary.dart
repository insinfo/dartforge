class A {}

extension E1 on A {
  A operator +(A a) => a;
}

extension E2 on A {
  A operator +(A a) => a;
}

A f(A a) => a + a;
//          ^^^^^
// [diag.ambiguousExtensionMemberAccessTwo] A member named '+' is defined in 'extension E1 on A' and 'extension E2 on A', and neither is more specific.
