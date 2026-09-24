class A {
  get _g => null;
}
main() {
  var v = new A()._g;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}
