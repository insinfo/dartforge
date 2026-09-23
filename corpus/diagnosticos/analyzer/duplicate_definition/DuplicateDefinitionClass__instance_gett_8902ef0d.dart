class C {
  int get foo;
//        ^^^
// [context 1] The corresponding getter is declared here.
}

augment class C {
  augment int foo = 0;
//            ^^^
// [diag.augmentationWithoutSetterDeclaration][context 1] This augmentation induces a setter, but no setter declaration named 'foo' exists to augment.
}
