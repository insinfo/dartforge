class A {
  static int? foo;
}

augment class A {
  augment static abstract String? foo;
//                                ^^^
// [diag.augmentationInducedGetterReturnTypeMismatch] The getter induced by this augmentation has return type 'String?', but the getter being augmented has return type 'int?'.
}
