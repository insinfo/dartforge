enum E {
  v;
  static int get foo => 0;
//               ^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'foo' and have instance member 'E.foo' with the same name.
  set foo(_) {}
}
