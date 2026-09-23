class A {
  A({int n1 = 0});
//^
// [context 1] The declaration being augmented.
//       ^^
// [context 2] The formal parameter is here.
  augment A(int p1) {}
//              ^^
// [diag.augmentationRequiredPositionalFormalParameterCount][context 1] The augmentation has 1 required positional formal parameters, but the declaration has 0.
//                ^
// [diag.augmentationNamedFormalParameterMissing][context 2] The augmentation is missing the named formal parameter 'n1' from the declaration.
}
