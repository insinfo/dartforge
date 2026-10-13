extension type E(int it) {
  static void foo([Object p = this]) {}
//                            ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
