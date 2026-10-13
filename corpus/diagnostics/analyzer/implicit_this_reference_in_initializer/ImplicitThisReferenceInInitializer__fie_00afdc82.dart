class A {
  final x = 0;
  final y = x;
//          ^
// [diag.implicitThisReferenceInInitializer] The instance member 'x' can't be accessed in an initializer.
}
