class A {
  set foo(_) {}
}
mixin M on A {
  static set foo(_) {}
//           ^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
