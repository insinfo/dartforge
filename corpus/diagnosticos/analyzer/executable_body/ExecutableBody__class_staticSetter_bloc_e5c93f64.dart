class A {
  static set foo(int _) {}
//           ^^^
// [context 1] The complete declaration is here.
  augment static set foo(int _) {}
//^^^^^^^
// [diag.functionAlreadyComplete][context 1] The augmentation can't provide a body because the function or member is already complete.
}
