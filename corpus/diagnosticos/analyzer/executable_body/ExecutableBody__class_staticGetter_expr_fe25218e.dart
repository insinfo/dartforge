class A {
  static int get foo => 0;
//               ^^^
// [context 1] The complete declaration is here.
  augment static int get foo => 1;
//^^^^^^^
// [diag.functionAlreadyComplete][context 1] The augmentation can't provide a body because the function or member is already complete.
}
