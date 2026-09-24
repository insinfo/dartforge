enum A<T, U> {v}
augment enum A<T> {}
//              ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
