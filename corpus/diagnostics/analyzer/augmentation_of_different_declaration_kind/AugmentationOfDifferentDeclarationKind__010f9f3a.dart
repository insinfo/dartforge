class A {
  set foo(int _) {}
//    ^^^
// [context 1] The declaration being augmented.
}
augment class A {
  augment void foo() {}
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a setter with a method.
}
