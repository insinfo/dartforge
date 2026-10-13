class A {
  static A f = this;
//             ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
