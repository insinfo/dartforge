abstract class I {
  void foo([int? p]);
}

mixin M1 {
  void foo(int? p) {}
}

mixin M2 implements I {}

mixin M3 on I {
  void bar() {
    super.foo(42);
  }
}

enum E with M1, M2, M3 {
//                  ^^
// [diag.mixinApplicationConcreteSuperInvokedMemberType] The super-invoked member 'foo' has the type 'void Function([int?])', and the concrete member in the class has the type 'void Function(int?)'.
  v;
  void foo([int? p]) {}
}
