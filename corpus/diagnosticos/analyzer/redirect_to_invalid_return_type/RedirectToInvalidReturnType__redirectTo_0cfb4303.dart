class A {
  A() {}
}
class B {
  factory B() = A;
//              ^
// [diag.redirectToInvalidReturnType] The return type 'A' of the redirected constructor isn't a subtype of 'B'.
}
