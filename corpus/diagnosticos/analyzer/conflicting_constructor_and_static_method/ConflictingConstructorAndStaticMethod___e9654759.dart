enum E {
  v.foo();
  const E.foo();
//        ^^^
// [diag.conflictingConstructorAndStaticMethod] 'foo' can't be used to name both a constructor and a static method in this class.
  static void foo() {}
}
