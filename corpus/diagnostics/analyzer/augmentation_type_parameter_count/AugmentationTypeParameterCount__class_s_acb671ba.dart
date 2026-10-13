class A {
  static void foo<T, U>() {}
}
augment class A {
  augment static void foo<T>();
//                         ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
}
