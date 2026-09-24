abstract class A {
  void foo();
}

enum B {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.foo'.
  v;
}

augment enum B implements A {}
