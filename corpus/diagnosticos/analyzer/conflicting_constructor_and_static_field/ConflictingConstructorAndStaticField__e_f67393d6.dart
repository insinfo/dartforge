extension type A.bar(int it) {
  new foo() : this.bar(0);
//    ^^^
// [diag.conflictingConstructorAndStaticField] 'foo' can't be used to name both a constructor and a static field in this class.
  static int foo = 0;
}
