class A {
  void foo() {}
//     ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment int get foo => 0;
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a method with a getter.
}
