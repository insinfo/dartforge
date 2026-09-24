enum E([int p = this]) {
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'p' isn't ever given.
//              ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  v;
}
