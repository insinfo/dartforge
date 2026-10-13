extension type E(int it) {
  E.named() : it = 0, assert(this.hashCode == 0);
//                           ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
