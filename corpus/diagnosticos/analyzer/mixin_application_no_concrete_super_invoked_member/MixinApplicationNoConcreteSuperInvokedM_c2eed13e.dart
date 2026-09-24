abstract class A {
  void foo();
}

mixin M on A {
  void foo() {
    super.foo();
  }
}

class X extends A with M {}
//                     ^
// [diag.mixinApplicationNoConcreteSuperInvokedMember] The class doesn't have a concrete implementation of the super-invoked member 'foo'.
