enum A<T> {v}
augment enum A<T, U> {}
//                ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
