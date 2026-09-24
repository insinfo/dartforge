class A {
  set foo(_) {}
}
class B extends A {
  static int get foo => 0;
//               ^^^
// [diag.conflictingStaticAndInstance] Class 'B' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
