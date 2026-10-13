enum E {
  v;
  static final int foo = 0;
//                 ^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'foo' and have instance member 'E.foo' with the same name.
  set foo(int _) {}
}
