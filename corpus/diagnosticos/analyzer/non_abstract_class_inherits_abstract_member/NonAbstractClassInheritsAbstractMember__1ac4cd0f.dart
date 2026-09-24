mixin M {
  int get foo;
}

enum E with M {
//   ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter M.foo'.
  v;
}
