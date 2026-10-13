abstract class A {
  void foo();
}

class B extends A {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.foo'.

class C extends B {}

class D extends C {}
