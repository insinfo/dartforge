mixin M1 {
  set foo(int _);
}

mixin M2 on M1 {
  void bar() {
    super.foo = 0;
  }
}

enum E with M1, M2 {
//              ^^
// [diag.mixinApplicationNoConcreteSuperInvokedSetter] The class doesn't have a concrete implementation of the super-invoked setter 'foo'.
  v;
  set foo(int _) {}
}
