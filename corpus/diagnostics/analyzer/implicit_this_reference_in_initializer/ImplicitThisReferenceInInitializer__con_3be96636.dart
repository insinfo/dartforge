class A {
  A() : assert(f != 0);
//             ^
// [diag.implicitThisReferenceInInitializer] The instance member 'f' can't be accessed in an initializer.
  int get f => 0;
}
