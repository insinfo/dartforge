class A {
  A._();
  factory A(int? p1);
//        ^
// [context 1] The declaration being augmented.
  augment factory A(int? p1, int? p2) => A._();
//                                ^^
// [diag.augmentationRequiredPositionalFormalParameterCount][context 1] The augmentation has 2 required positional formal parameters, but the declaration has 1.
}
