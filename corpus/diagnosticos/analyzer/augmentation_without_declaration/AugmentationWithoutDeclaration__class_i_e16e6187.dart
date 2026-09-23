class A {
  static set foo(int _) {}
}
augment class A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
