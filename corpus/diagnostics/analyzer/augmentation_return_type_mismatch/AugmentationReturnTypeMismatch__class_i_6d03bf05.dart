class A {
  String? foo;
  int? bar;
}

augment class A {
  augment abstract String? foo, bar;
//                              ^^^
// [diag.augmentationInducedGetterReturnTypeMismatch] The getter induced by this augmentation has return type 'String?', but the getter being augmented has return type 'int?'.
}
