abstract class A {
  void foo();
}

class B {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.foo'.

augment class B implements A {}
