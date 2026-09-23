class A<T, U> {}
augment class A<U, T> {}
//              ^
// [diag.augmentationTypeParameterName] The augmentation type parameter must have the same name as the corresponding type parameter of the declaration.
//                 ^
// [diag.augmentationTypeParameterName] The augmentation type parameter must have the same name as the corresponding type parameter of the declaration.
