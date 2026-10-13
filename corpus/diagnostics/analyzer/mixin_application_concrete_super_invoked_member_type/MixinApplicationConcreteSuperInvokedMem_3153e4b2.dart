class I {
  void foo([int? p]) {}
}

class A {
  void foo(int? p) {}
}

abstract class B extends A implements I {
  void foo([int? p]);
}

mixin M on I {
  void bar() {
    super.foo(42);
  }
}

abstract class X extends B with M {}
//                              ^
// [diag.mixinApplicationConcreteSuperInvokedMemberType] The super-invoked member 'foo' has the type 'void Function([int?])', and the concrete member in the class has the type 'void Function(int?)'.
