class A {
  static int foo = 0;
}
augment class A {
  augment int foo = 0;
//            ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
