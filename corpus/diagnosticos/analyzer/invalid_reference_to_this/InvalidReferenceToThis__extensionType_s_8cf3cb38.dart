extension type E(int it) {
  factory E.named() {
    return this;
//         ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
