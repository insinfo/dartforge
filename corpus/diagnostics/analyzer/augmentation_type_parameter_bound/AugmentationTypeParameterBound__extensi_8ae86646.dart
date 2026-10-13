extension A<T extends num> on int {}
augment extension A<T extends int> {}
//                            ^^^
// [diag.augmentationTypeParameterBound] The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.
