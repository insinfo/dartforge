class A {}

extension E1 on A {
  int operator [](int i) => 0;
}

extension E2 on A {
  int operator [](int i) => 0;
}

int f(A a) => a[0];
//            ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named '[]' is defined in 'extension E1 on A' and 'extension E2 on A', and neither is more specific.
