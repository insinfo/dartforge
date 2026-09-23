class A {
  void foo() {}
}
mixin M on A {
  static void foo() {}
//            ^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
