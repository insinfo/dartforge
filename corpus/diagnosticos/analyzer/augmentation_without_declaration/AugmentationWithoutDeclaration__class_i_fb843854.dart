class A {
  static int get foo => 0;
}
augment class A {
  augment int foo = 0;
//            ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
