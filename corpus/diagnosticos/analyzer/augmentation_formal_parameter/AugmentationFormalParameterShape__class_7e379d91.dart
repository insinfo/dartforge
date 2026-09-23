abstract class A {
  void foo({int? n1});
//     ^^^
// [context 1] The declaration being augmented.
  augment void foo({int? n1, int? n2}) {}
//                                ^^
// [diag.augmentationNamedFormalParameterExtra][context 1] The augmentation has a named formal parameter 'n2', but the declaration doesn't.
}
