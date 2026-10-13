extension type E(int it) {
  static set foo(int _) {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
