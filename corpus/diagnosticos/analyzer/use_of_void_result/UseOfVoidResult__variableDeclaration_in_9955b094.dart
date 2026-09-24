void f(void x) {
  void y = x;
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'y' isn't used.
}
