class A {
  A(p) {}
}
class B extends A {
  B() : super(f);
//            ^
// [diag.implicitThisReferenceInInitializer] The instance member 'f' can't be accessed in an initializer.
  var f;
}
