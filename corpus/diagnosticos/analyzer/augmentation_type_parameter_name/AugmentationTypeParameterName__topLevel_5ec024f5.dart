void foo<T>() {}
augment void foo<U>();
//               ^
// [diag.augmentationTypeParameterName] The augmentation type parameter must have the same name as the corresponding type parameter of the declaration.
