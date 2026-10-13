void f(int _);
augment void f(int x);
//                 ^
// [context 1] The preceding declaration is here.
augment void f(int y) {}
//                 ^
// [diag.augmentationPositionalFormalParameterName][context 1] The parameter name 'y' must either match the name 'x' from a preceding declaration or be '_'.
