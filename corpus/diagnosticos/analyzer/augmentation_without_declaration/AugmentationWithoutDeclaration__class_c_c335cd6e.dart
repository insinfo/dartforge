class A {
  void foo() {}
}

augment class A {
  augment A.foo();
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
