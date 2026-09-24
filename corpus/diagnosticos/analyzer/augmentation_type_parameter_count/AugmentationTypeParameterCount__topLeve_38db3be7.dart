void f<T, U>() {}
augment void f<T>();
//              ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
