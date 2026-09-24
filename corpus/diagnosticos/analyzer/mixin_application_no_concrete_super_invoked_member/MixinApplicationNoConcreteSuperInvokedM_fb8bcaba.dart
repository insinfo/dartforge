mixin M1 {
  int get foo;
}

mixin M2 on M1 {
  void bar() {
    super.foo;
  }
}

enum E with M1, M2 {
//              ^^
// [diag.mixinApplicationNoConcreteSuperInvokedMember] The class doesn't have a concrete implementation of the super-invoked member 'foo'.
  v;
  int get foo => 0;
}
