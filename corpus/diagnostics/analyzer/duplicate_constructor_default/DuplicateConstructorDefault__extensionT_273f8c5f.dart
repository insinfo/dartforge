extension type A.named(int it) {
  factory (int it) => A.named(it);
  factory (int it) => A.named(it);
//^^^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
