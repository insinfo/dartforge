void f({int? n1, int? n2});
//           ^^
// [context 1] The formal parameter is here.
augment void f({int? n2}) {}
//                      ^
// [diag.augmentationNamedFormalParameterMissing][context 1] The augmentation is missing the named formal parameter 'n1' from the declaration.
