class A {
  static void foo() {}
//            ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment A.foo();
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a method with a constructor.
}
