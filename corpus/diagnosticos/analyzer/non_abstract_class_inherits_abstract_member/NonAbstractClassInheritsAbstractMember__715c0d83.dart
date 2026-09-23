class A {
  void foo() {}
}

enum E implements A {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.foo'.
  v;
}
