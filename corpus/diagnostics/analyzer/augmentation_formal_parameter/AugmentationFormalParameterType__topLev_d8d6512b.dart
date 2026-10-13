void f({int? n1});
//           ^^
// [context 1] The formal parameter is here.
augment void f({String? n1}) {}
//              ^^^^^^^
// [diag.augmentationFormalParameterTypeMismatch][context 1] The augmentation's formal parameter type 'String?' must be the same as the declaration's formal parameter type 'int?'.
