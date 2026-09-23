class A {}

extension E1 on A {
  int operator -() => 0;
}

extension E2 on A {
  int operator -() => 0;
}

int f(A a) => -a;
//             ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'unary-' is defined in 'extension E1 on A' and 'extension E2 on A', and neither is more specific.
