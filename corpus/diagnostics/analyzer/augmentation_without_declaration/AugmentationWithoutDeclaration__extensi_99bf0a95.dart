extension type A(int it) {}

augment extension type A {
  augment int get foo => 0;
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
