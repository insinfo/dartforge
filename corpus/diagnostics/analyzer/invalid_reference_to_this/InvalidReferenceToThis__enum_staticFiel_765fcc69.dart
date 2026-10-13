enum E {
  v;
  static late final f = this;
//                      ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
