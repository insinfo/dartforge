enum E {
  v;
  static void foo() {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
