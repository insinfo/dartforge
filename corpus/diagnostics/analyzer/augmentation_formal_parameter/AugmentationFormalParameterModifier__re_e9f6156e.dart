void f({int? x});
//           ^
// [context 1] The formal parameter is here.
augment void f({required int? x}) {}
//              ^^^^^^^^
// [diag.augmentationFormalParameterModifierExtra][context 1] The augmentation has the 'required' modifier on this formal parameter, but the declaration doesn't.
