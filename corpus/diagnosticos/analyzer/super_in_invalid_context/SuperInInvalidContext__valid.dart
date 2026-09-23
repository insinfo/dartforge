class A {
  m() {}
}
class B extends A {
  B() {
    var v = super.m();
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  }
  n() {
    var v = super.m();
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  }
}
