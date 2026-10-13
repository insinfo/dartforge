class A<T> {}
augment class A<T extends num> {}
//                        ^^^
// [diag.augmentationTypeParameterBound] The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.
