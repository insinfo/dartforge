void f(int x) {
  if (x case final a) {
//                 ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    if (x case final b) {}
//                   ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
  }
}
