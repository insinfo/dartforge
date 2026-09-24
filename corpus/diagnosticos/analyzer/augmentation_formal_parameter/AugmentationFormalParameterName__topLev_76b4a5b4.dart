void f(int x, [int? y]);
//                  ^
// [context 1] The preceding declaration is here.
augment void f(int x, [int? z]) {}
//                          ^
// [diag.augmentationPositionalFormalParameterName][context 1] The parameter name 'z' must either match the name 'y' from a preceding declaration or be '_'.
