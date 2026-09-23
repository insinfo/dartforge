class A {
  int foo = 0;
//    ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a field with a method.
}
