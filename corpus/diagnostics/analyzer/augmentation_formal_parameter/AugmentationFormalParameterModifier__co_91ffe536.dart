abstract class A {
  void foo(covariant int x);
//                       ^
// [context 1] The formal parameter is here.
  augment void foo(int x) {}
//                     ^
// [diag.augmentationFormalParameterModifierMissing][context 1] The augmentation is missing the 'covariant' modifier on this formal parameter that the declaration has.
}
