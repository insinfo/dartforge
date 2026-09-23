extension type E(int it) {
  static void foo() {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
