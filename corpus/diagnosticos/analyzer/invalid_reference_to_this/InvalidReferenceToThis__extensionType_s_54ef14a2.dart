extension type E(int it) {
  static int get foo {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
    return 0;
  }
}
