class A {
  A(int? p1);
//^
// [context 1] The declaration being augmented.
  augment A(int? p1, [int? p2]) {}
//                   ^
// [diag.augmentationOptionalPositionalFormalParameterCount][context 1] The augmentation has 1 optional positional formal parameters, but the declaration has 0.
}
