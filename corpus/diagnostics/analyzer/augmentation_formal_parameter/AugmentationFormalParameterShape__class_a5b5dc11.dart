class A {
  A(int? p1, [int? p2]);
//^
// [context 1] The declaration being augmented.
  augment A(int? p1) {}
//                 ^
// [diag.augmentationOptionalPositionalFormalParameterCount][context 1] The augmentation has 0 optional positional formal parameters, but the declaration has 1.
}
