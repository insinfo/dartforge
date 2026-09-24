enum E {
  v;
  static set foo(_) {}
//           ^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'foo' and have instance member 'E.foo' with the same name.
  int get foo => 0;
}
