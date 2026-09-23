abstract class A {
  void foo();
}

class B implements A {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.foo'.

augment class B {}
