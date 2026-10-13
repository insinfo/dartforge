extension E on int {
  static late var f = this;
//                    ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
