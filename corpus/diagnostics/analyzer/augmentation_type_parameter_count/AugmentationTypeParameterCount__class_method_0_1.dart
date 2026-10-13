class A {
  void foo() {}
}
augment class A {
  augment void foo<T>();
//                 ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
}
