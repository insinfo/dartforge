class A {
  static int foo = 0;
//           ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment A.foo();
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a field with a constructor.
}
