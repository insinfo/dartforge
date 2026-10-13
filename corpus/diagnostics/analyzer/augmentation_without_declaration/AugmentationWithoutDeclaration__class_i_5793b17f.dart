class A {
  static void foo() {}
}
augment class A {
  augment int get foo => 0;
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
