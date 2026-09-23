class A {
  var v;
  A() : v = f;
//          ^
// [diag.implicitThisReferenceInInitializer] The instance member 'f' can't be accessed in an initializer.
  var f;
}
