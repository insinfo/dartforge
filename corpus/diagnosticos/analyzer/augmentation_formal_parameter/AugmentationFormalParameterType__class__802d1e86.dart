class A {
  A([int? p1]);
}
class B extends A {
  B([int? p1]);
//        ^^
// [context 1] The formal parameter is here.
  augment B([String? super.p1]);
//           ^^^^^^^
// [diag.augmentationFormalParameterTypeMismatch][context 1] The augmentation's formal parameter type 'String?' must be the same as the declaration's formal parameter type 'int?'.
}
