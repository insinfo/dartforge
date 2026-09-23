class A {}

augment class A {
  augment static int get foo => 0;
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
