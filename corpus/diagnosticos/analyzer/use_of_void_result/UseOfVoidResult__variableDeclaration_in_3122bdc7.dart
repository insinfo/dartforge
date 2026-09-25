class A {
  void m() {}
  n() {
    Object a = m(), b = m();
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//             ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
//                  ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
//                      ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  }
}
