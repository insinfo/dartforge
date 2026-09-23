extension type E(int it) {
  E.named() : this(this.hashCode);
//                 ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
