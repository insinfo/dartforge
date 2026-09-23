enum E {
  v;
  factory E.named() {
    return this;
//         ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
  }
}
