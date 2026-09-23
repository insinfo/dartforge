class A {
  int get foo => 0;
}

enum E implements A {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter A.foo'.
  v;
}
