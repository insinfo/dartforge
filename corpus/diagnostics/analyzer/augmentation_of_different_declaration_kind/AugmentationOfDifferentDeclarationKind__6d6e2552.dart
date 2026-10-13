class A {
  void foo() {}
//     ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment set foo(int _) {}
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a method with a setter.
}
