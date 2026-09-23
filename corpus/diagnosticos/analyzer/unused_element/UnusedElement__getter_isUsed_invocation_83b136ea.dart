class A {
  get _g => null;
}
void f(A a) {
  var v = a._g;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}
