class A {
  int foo = 0;
}

augment class A {
  augment A.foo();
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
