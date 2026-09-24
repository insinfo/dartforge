class C {
  static int get foo => 0;
//               ^^^
// [diag.conflictingStaticAndInstance] Class 'C' can't define static member 'foo' and have instance member 'C.foo' with the same name.
  void foo() {}
}
