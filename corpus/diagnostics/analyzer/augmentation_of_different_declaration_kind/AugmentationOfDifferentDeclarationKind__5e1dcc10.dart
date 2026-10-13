extension A on int {}
//        ^
// [context 1] The declaration being augmented.
augment class A {}
// [diag.augmentationOfDifferentDeclarationKind][column 1][length 7][context 1] Can't augment a extension with a class.
