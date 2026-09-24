class A {
  A([int? p1]);
//^
// [context 1] The declaration being augmented.
  augment A(int? p1) {}
//               ^^
// [diag.augmentationRequiredPositionalFormalParameterCount][context 1] The augmentation has 1 required positional formal parameters, but the declaration has 0.
}
