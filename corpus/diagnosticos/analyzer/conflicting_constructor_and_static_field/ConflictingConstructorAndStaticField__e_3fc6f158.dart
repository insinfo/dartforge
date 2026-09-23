extension type A.foo(int it) {
//               ^^^
// [diag.conflictingConstructorAndStaticField] 'foo' can't be used to name both a constructor and a static field in this class.
  static int foo = 0;
}
