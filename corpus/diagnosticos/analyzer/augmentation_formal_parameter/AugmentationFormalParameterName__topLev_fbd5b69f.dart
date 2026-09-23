void f(int x, int y);
//         ^
// [context 1] The preceding declaration is here.
//                ^
// [context 2] The preceding declaration is here.
augment void f(int a, int b) {}
//                 ^
// [diag.augmentationPositionalFormalParameterName][context 1] The parameter name 'a' must either match the name 'x' from a preceding declaration or be '_'.
//                        ^
// [diag.augmentationPositionalFormalParameterName][context 2] The parameter name 'b' must either match the name 'y' from a preceding declaration or be '_'.
