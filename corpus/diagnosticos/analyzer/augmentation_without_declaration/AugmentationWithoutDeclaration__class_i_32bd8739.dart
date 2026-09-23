class A {
  static set foo(int _) {}
}
augment class A {
  augment set foo(int _);
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
