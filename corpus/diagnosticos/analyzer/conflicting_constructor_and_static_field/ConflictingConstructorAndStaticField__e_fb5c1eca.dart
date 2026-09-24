enum E {
  v.foo();
  const E.foo();
//        ^^^
// [diag.conflictingConstructorAndStaticGetter] 'foo' can't be used to name both a constructor and a static getter in this class.
  static int get foo => 0;
}
