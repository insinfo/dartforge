void f(int p1(int a));
//         ^^
// [context 1] The formal parameter is here.
augment void f(int p1(String a)) {}
//             ^^^
// [diag.augmentationFormalParameterTypeMismatch][context 1] The augmentation's formal parameter type 'int Function(String)' must be the same as the declaration's formal parameter type 'int Function(int)'.
