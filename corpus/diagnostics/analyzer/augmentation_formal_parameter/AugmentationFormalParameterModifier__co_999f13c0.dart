abstract class A {
  void foo(int x);
//             ^
// [context 1] The formal parameter is here.
  augment void foo(covariant int x) {}
//                 ^^^^^^^^^
// [diag.augmentationFormalParameterModifierExtra][context 1] The augmentation has the 'covariant' modifier on this formal parameter, but the declaration doesn't.
}
