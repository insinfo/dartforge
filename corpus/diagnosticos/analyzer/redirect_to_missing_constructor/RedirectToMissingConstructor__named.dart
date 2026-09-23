class A implements B{
  A() {}
}
class B {
  factory B() = A.name;
//              ^^^^^^
// [diag.redirectToMissingConstructor] The constructor 'A.name' couldn't be found in 'A'.
}