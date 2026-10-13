extension type A<T>(int it) {}
augment extension type A<T, U> {}
//                          ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
