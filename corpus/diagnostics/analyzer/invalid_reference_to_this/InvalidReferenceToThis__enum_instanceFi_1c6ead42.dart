enum E {
  v;
  final f = this;
//          ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
