class A {
  int get foo => 0;
//        ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a getter with a method.
}
