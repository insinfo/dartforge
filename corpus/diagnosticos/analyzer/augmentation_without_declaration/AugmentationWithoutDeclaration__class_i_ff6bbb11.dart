class A {
  static int foo = 0;
}
augment class A {
  augment abstract int foo;
//                     ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
