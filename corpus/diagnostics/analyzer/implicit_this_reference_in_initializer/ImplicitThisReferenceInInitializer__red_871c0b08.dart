class A {
  A(p) {}
  A.named() : this(f);
//                 ^
// [diag.implicitThisReferenceInInitializer] The instance member 'f' can't be accessed in an initializer.
  var f;
}
