class A {
  void m() {}
  n() {
    var a;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    a = m();
//      ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  }
}
