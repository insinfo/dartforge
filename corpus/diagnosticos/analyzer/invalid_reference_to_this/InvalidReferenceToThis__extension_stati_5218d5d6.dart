extension E on int {
  static void foo([Object p = this]) {}
//                            ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
