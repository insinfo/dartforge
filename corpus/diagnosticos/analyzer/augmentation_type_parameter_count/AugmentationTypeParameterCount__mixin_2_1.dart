mixin A<T, U> {}
augment mixin A<T> {}
//               ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
