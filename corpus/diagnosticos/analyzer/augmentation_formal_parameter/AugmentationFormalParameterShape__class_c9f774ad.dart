class A {
  static void foo(int? p1, int? p2);
//            ^^^
// [context 1] The declaration being augmented.
  augment static void foo(int? p1, [int? p2]) {}
//                                 ^
// [diag.augmentationRequiredPositionalFormalParameterCount][context 1] The augmentation has 1 required positional formal parameters, but the declaration has 2.
}
