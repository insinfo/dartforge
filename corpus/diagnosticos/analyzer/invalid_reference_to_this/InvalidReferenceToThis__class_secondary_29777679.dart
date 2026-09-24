class A {
  A() {
    var v = this;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  }
}
