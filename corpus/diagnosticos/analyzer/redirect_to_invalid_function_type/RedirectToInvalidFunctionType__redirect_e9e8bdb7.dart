class A implements B {
  A(int p) {}
}
class B {
  factory B() = A;
//              ^
// [diag.redirectToInvalidFunctionType] The redirected constructor 'A Function(int)' has incompatible parameters with 'B Function()'.
}
