void foo() {}
//   ^^^
// [context 1] The declaration being augmented.
augment int foo = 0;
//          ^^^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a function with a top level variable.
