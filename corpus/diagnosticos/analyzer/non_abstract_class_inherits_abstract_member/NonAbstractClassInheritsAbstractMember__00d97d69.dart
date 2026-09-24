mixin M {
  void foo();
}

enum E with M {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'M.foo'.
  v;
}
