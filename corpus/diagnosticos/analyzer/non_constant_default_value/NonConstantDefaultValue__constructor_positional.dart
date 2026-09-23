class A {
  int y = 0;
  A([x = y]) {}
//       ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'y' can't be accessed in an initializer.
}
