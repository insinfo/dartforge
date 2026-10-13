void f(int? p1);
//   ^
// [context 1] The declaration being augmented.
augment void f(int? p1, [int? p2, int? p3]) {}
//                      ^
// [diag.augmentationOptionalPositionalFormalParameterCount][context 1] The augmentation has 2 optional positional formal parameters, but the declaration has 0.
