extension A on int {}

augment extension A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
