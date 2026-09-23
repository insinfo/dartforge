mixin M {
  static int get foo => 0;
//               ^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'foo' and have instance member 'M.foo' with the same name.
  set foo(_) {}
}
