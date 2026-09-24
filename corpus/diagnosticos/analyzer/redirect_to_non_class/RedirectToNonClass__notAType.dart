class B {
  int A = 0;
  factory B() = A;
//              ^
// [diag.redirectToNonClass] The name 'A' isn't a type and can't be used in a redirected constructor.
}
