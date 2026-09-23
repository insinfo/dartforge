void f<T>() {}
augment void f<T, U>();
//                ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.

void g() {
  f<int>();
}
