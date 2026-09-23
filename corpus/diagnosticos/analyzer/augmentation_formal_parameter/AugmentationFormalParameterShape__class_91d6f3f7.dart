class A {
  A({int? n1, int? n2});
//                 ^^
// [context 1] The formal parameter is here.
  augment A({int? n1}) {}
//                   ^
// [diag.augmentationNamedFormalParameterMissing][context 1] The augmentation is missing the named formal parameter 'n2' from the declaration.
}
