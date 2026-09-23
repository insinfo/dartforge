class A<T extends dynamic> {}
augment class A<T extends Object?> {}
//                        ^^^^^^^
// [diag.augmentationTypeParameterBound] The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.
