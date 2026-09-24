extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {
  static void foo() {}
//            ^^^
// [diag.conflictingStaticAndInstance] Class 'B' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
