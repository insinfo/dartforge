extension E on int {
  static void set foo(_) {}
//                ^^^
// [context 1] The corresponding setter is declared here.
}

augment extension E {
  augment static abstract int foo;
//                            ^^^
// [diag.augmentationWithoutGetterDeclaration][context 1] This augmentation induces a getter, but no getter declaration named 'foo' exists to augment.
}
