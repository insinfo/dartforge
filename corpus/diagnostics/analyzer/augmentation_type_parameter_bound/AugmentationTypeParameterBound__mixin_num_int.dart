mixin A<T extends num> {}
augment mixin A<T extends int> {}
//                        ^^^
// [diag.augmentationTypeParameterBound] The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.
