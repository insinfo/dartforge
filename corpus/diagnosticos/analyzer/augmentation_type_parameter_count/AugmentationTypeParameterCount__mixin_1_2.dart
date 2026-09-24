mixin A<T> {}
augment mixin A<T, U> {}
//                 ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
