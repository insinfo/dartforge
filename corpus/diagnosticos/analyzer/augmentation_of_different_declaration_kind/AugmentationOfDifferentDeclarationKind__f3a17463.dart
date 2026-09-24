class A {
  A.foo();
//  ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment static int get foo => 0;
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a constructor with a getter.
}
