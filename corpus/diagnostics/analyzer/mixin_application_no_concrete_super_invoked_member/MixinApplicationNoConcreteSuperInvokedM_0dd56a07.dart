abstract class A {
  void foo();
}

mixin M1 on A {
  void foo() {
    super.foo();
  }
}

mixin M2 on A {
  void foo() {}
}

class X extends A with M1, M2 {}
//                     ^^
// [diag.mixinApplicationNoConcreteSuperInvokedMember] The class doesn't have a concrete implementation of the super-invoked member 'foo'.
