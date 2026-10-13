extension type E(int it) {
  static late var f = this;
//                    ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
