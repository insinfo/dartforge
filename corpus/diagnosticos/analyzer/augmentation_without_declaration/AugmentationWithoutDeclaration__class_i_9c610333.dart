class A {
  static int get foo => 0;
}
augment class A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
