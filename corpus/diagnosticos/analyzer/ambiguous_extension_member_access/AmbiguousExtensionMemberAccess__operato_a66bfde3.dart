class A {}

extension E1 on A {
  A operator +(_) => this;
}

extension E2 on A {
  A operator +(_) => this;
}

void f(A a) {
  a += 0;
//  ^^
// [diag.ambiguousExtensionMemberAccessTwo] A member named '+' is defined in 'extension E1 on A' and 'extension E2 on A', and neither is more specific.
}
