mixin M {
  set foo(int _);
}

enum E with M {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter M.foo'.
  v;
}
