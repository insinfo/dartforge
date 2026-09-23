extension type A<T>(int it) {}
augment extension type A<T extends num> {}
//                                 ^^^
// [diag.augmentationTypeParameterBound] The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.
