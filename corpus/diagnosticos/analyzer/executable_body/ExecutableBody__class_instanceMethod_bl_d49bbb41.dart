class A {
  void foo() {}
//     ^^^
// [context 1] The complete declaration is here.
  augment void foo() {}
//^^^^^^^
// [diag.functionAlreadyComplete][context 1] The augmentation can't provide a body because the function or member is already complete.
}
