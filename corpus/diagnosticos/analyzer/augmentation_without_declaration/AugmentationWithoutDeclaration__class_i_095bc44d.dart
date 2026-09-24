class A {
  static int get foo => 0;
}
augment class A {
  augment set foo(int _) {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
