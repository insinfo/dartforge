class A implements B{
  A.name() {}
}
class B {
  factory B() = A;
//              ^
// [diag.redirectToMissingConstructor] The constructor 'A' couldn't be found in 'A'.
}
