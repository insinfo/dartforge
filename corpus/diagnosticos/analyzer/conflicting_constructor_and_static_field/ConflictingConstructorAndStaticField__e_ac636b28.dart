enum E.foo() {
//     ^^^
// [diag.conflictingConstructorAndStaticField] 'foo' can't be used to name both a constructor and a static field in this class.
  v.foo();
  static int foo = 0;
}
