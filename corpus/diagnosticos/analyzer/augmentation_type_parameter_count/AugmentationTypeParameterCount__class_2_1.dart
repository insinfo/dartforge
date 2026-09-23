class A<T, U> {}
augment class A<T> {}
//               ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
