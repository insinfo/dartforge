class A<T> {}
augment class A<T, U> {}
//                 ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.

void f(A<int> a) {}
