extension E on int {
  void set foo(int _) {}
//         ^^^
// [context 1] The corresponding setter is declared here.
}

augment extension E {
  augment abstract int foo;
//                     ^^^
// [diag.augmentationWithoutGetterDeclaration][context 1] This augmentation induces a getter, but no getter declaration named 'foo' exists to augment.
}
