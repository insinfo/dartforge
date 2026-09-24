extension type A.named(int it) {
  new(this.it);
  new(this.it);
//^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
