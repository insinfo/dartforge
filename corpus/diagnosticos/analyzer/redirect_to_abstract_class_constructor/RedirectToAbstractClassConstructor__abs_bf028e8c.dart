abstract class A {
  factory A() = A._;
//              ^^^
// [diag.redirectToAbstractClassConstructor] The redirecting constructor 'A' can't redirect to a constructor of the abstract class 'A'.
  A._();
}
