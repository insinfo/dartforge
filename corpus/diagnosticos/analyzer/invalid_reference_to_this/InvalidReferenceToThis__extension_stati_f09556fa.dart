extension E on int {
  static int get foo {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
    return 0;
  }
}
