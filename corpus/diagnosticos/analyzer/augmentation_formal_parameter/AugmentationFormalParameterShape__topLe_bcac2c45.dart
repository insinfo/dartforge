void f({int? n1, int? n2, int? n3});
//                    ^^
// [context 1] The formal parameter is here.
//                             ^^
// [context 2] The formal parameter is here.
augment void f({int? n1}) {}
//                      ^
// [diag.augmentationNamedFormalParameterMissing][context 1] The augmentation is missing the named formal parameter 'n2' from the declaration.
// [diag.augmentationNamedFormalParameterMissing][context 2] The augmentation is missing the named formal parameter 'n3' from the declaration.
