void f({required int? x});
//                    ^
// [context 1] The formal parameter is here.
augment void f({int? x}) {}
//                   ^
// [diag.augmentationFormalParameterModifierMissing][context 1] The augmentation is missing the 'required' modifier on this formal parameter that the declaration has.
