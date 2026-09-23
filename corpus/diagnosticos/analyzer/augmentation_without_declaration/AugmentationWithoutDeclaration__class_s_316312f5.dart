class A {}

augment class A {
  augment static void foo() {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
