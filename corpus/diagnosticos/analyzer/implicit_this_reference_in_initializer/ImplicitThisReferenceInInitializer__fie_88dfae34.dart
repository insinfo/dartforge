class A {
  void x<T>() {}
  final y = x<int>;
//          ^
// [diag.implicitThisReferenceInInitializer] The instance member 'x' can't be accessed in an initializer.
}
