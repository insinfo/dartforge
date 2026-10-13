class A {
  void foo<T>() {}
}
augment class A {
  augment void foo<T, U>();
//                    ^
// [diag.augmentationTypeParameterCount] The augmentation must have the same number of type parameters as the declaration.
}

void f(A a) {
  a.foo<int>();
}
