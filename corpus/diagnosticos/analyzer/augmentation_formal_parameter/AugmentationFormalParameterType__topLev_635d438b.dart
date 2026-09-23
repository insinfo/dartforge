void f(dynamic p1);
//             ^^
// [context 1] The formal parameter is here.
augment void f(Object? p1) {}
//             ^^^^^^^
// [diag.augmentationFormalParameterTypeMismatch][context 1] The augmentation's formal parameter type 'Object?' must be the same as the declaration's formal parameter type 'dynamic'.
