int? bar = 0;

augment abstract int? foo, bar;
//                    ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
