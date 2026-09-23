abstract class A {
  void foo();
}

enum B implements A {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.foo'.
  v;
}
