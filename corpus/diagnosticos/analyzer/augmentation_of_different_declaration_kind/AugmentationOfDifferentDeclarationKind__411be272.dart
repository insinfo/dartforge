enum A {
  foo
//^^^
// [context 1] The declaration being augmented.
}
augment enum A {;
  augment static void foo() {}
//^^^^^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a field with a method.
}
