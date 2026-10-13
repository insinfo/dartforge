class A {
  set foo(int _) {}
}

enum E implements A {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter A.foo'.
  v;
}
