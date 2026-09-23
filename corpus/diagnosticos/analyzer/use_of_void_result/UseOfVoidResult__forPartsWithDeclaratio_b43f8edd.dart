class A {
  void m() {}
  n() {
    for(Object a = m();;) {}
//             ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                 ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  }
}