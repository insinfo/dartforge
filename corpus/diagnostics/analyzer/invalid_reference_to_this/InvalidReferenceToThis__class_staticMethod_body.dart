class A {
  static void foo() {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
