mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  void foo() {}
//     ^^^
// [diag.conflictingMethodAndField] Class 'E' can't define method 'foo' and have field 'M.foo' with the same name.
}
