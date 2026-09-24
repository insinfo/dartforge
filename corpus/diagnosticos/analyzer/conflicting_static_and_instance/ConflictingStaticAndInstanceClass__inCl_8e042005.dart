class C {
  static set foo(_) {}
//           ^^^
// [diag.conflictingStaticAndInstance] Class 'C' can't define static member 'foo' and have instance member 'C.foo' with the same name.
  int get foo => 0;
}
