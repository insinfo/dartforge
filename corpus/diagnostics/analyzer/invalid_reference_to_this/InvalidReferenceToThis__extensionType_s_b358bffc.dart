extension type E(int it) {
  E.named([Object p = this]) : it = 0;
//                    ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
