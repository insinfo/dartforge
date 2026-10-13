extension type A(int it) {}

augment extension type A {
  augment A.named() : this(0);
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
