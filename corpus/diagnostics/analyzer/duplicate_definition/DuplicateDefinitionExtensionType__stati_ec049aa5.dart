extension type E(int it) {
  static int get foo => 0;
//               ^^^
// [context 1] The corresponding getter is declared here.
}

augment extension type E {
  augment static abstract int foo;
//                            ^^^
// [diag.augmentationWithoutSetterDeclaration][context 1] This augmentation induces a setter, but no setter declaration named 'foo' exists to augment.
}
