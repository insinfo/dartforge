enum E {
  v.foo();
  const E.foo();
//        ^^^
// [diag.conflictingConstructorAndStaticSetter] 'foo' can't be used to name both a constructor and a static setter in this class.
  static void set foo(_) {}
}
