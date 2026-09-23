extension type A(int it) {
  new(this.it);
//^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
