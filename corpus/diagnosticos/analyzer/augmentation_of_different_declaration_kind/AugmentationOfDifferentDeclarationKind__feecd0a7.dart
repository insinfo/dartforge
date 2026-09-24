void foo() {}
//   ^^^
// [context 1] The declaration being augmented.
augment class foo {}
// [diag.augmentationOfDifferentDeclarationKind][column 1][length 7][context 1] Can't augment a function with a class.
