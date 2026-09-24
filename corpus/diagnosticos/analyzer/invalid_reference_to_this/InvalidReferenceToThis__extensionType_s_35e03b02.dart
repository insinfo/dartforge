extension type E(int it) {
  E.named() : it = this.hashCode;
//                 ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
