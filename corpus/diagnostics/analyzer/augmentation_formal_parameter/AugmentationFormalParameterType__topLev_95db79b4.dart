void f(int p1<T>(T a));
//         ^^
// [context 1] The formal parameter is here.
augment void f(int p1<T>(String a)) {}
//             ^^^
// [diag.augmentationFormalParameterTypeMismatch][context 1] The augmentation's formal parameter type 'int Function<T>(String)' must be the same as the declaration's formal parameter type 'int Function<T>(T)'.
