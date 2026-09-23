extension type E(int it) {
  void foo([Object p = this]) {}
//                     ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
