extension type A(int it) {
  static int get foo => 0;
//               ^^^
// [diag.conflictingStaticAndInstance] Class 'A' can't define static member 'foo' and have instance member 'A.foo' with the same name.
  int get foo => 0;
}
