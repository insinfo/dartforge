class A {
  static var F = m();
//               ^
// [diag.implicitThisReferenceInInitializer] The instance member 'm' can't be accessed in an initializer.
  int m() => 0;
}
