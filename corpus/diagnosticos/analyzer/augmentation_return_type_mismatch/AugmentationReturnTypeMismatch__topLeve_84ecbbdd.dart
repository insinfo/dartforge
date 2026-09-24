int? get foo => 0;

augment abstract final String? foo;
//                             ^^^
// [diag.augmentationInducedGetterReturnTypeMismatch] The getter induced by this augmentation has return type 'String?', but the getter being augmented has return type 'int?'.
