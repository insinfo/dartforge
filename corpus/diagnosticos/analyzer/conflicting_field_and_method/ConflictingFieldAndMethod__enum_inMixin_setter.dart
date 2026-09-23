mixin M {
  void foo() {}
}

enum E with M {
  v;
  set foo(int _) {}
//    ^^^
// [diag.conflictingFieldAndMethod] Class 'E' can't define field 'foo' and have method 'M.foo' with the same name.
}
