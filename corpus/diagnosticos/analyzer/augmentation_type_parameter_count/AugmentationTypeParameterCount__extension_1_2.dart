extension A<T> on int {}
augment extension A<T, U> {}
//                     ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
