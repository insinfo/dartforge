void f({required int n1});
//                   ^^
// [context 1] The formal parameter is here.
augment void f({required int n1(int a)}) {}
//                       ^^^
// [diag.augmentationFormalParameterTypeMismatch][context 1] The augmentation's formal parameter type 'int Function(int)' must be the same as the declaration's formal parameter type 'int'.
