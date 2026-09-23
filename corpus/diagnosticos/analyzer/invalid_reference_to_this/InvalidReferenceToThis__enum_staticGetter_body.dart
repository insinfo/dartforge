enum E {
  v;
  static int get foo {
    this;
//  ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
    return 0;
  }
}
