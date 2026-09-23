class Foo {
  final field = 0;
  foo([int x = field]) {}
//             ^^^^^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'field' can't be accessed in an initializer.
}
