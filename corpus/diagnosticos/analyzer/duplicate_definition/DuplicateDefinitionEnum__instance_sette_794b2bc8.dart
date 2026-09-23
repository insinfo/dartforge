enum E {
  v;
  void set foo(int _) {}
//         ^^^
// [context 1] The corresponding setter is declared here.
}

augment enum E {;
  augment final int foo = 0;
//                  ^^^
// [diag.augmentationWithoutGetterDeclaration][context 1] This augmentation induces a getter, but no getter declaration named 'foo' exists to augment.
}
