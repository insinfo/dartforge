class C {}

extension E1 on C {
  int get foo => 0;
  set foo(int value) {}
}

extension E2 on C {
  int get foo => 0;
  set foo(int value) {}
}

void f(C x) {
  x..foo += 1;
//   ^^^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'foo' is defined in 'extension E1 on C' and 'extension E2 on C', and neither is more specific.
}
