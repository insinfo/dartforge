extension type A(int it) {
  static void foo() {}
//            ^^^
// [diag.conflictingStaticAndInstance] Class 'A' can't define static member 'foo' and have instance member 'A.foo' with the same name.
  set foo(_) {}
}
