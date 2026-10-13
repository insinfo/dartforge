class A {
  void m() {}
  n() {
    for(void a = m();;) {}
//           ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
  }
}
