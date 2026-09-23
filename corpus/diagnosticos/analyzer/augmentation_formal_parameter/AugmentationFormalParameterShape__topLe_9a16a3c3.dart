void f(int? p1);
//   ^
// [context 1] The declaration being augmented.
augment void f(int? p1, int? p2) {}
//                           ^^
// [diag.augmentationRequiredPositionalFormalParameterCount][context 1] The augmentation has 2 required positional formal parameters, but the declaration has 1.

void g() {
  f(0);
}
