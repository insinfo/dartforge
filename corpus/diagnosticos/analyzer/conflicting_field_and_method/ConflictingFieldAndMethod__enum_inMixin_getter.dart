mixin M {
  void foo() {}
}

enum E with M {
  v;
  int get foo => 0;
//        ^^^
// [diag.conflictingFieldAndMethod] Class 'E' can't define field 'foo' and have method 'M.foo' with the same name.
}
