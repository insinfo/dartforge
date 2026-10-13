class A {}
//    ^
// [context 1] The declaration being augmented.
augment extension type A {}
// [diag.augmentationOfDifferentDeclarationKind][column 1][length 7][context 1] Can't augment a class with a extension type.
