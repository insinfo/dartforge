class A {}
//    ^
// [context 1] The declaration being augmented.
augment int A = 0;
//          ^
// [diag.augmentationOfDifferentDeclarationKind][context 1] Can't augment a class with a top level variable.
