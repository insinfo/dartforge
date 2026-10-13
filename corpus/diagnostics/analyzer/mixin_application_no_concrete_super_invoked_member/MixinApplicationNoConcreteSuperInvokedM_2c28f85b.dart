abstract class A {
  void set foo(_);
}

mixin M on A {
  void bar() {
    super.foo = 0;
  }
}

abstract class X extends A with M {}
//                              ^
// [diag.mixinApplicationNoConcreteSuperInvokedSetter] The class doesn't have a concrete implementation of the super-invoked setter 'foo'.
