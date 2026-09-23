mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  static final int foo = 0;
//                 ^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'foo' and have instance member 'E.foo' with the same name.
}
