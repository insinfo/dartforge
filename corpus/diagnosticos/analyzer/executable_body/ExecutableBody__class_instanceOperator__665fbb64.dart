class A {
  A operator +(A _) => this;
//           ^
// [context 1] The complete declaration is here.
  augment A operator +(A _) => this;
//^^^^^^^
// [diag.functionAlreadyComplete][context 1] The augmentation can't provide a body because the function or member is already complete.
}
