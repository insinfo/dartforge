extension type A(int it) {}

augment extension type A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
