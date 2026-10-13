extension E on int {
  static void foo() {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
