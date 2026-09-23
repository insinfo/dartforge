extension type A(int it) {
  A.new(this.it);
//^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
