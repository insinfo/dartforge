class A {
  int get f => 0;
}

class B extends A {
  B() : assert(f != 0);
//             ^
// [diag.implicitThisReferenceInInitializer] The instance member 'f' can't be accessed in an initializer.
}
