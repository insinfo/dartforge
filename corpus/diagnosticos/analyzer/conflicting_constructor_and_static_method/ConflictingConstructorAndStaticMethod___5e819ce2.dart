enum E.foo() {
//     ^^^
// [diag.conflictingConstructorAndStaticMethod] 'foo' can't be used to name both a constructor and a static method in this class.
  v.foo();
  static void foo() {}
}
