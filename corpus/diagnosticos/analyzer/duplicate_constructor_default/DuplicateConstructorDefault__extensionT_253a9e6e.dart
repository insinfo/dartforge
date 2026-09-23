extension type A.new(int it) {
  A.new(this.it);
//^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
