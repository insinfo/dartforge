class A {
  int get foo => 0;
}
mixin M implements A {
  static int get foo => 0;
//               ^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
