enum E {
  v;
  factory E.named([Object p = this]) => throw 0;
//                            ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
