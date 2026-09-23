enum A<T extends num> {v}
augment enum A<T extends int> {}
//                       ^^^
// [diag.augmentationTypeParameterBound] The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.
