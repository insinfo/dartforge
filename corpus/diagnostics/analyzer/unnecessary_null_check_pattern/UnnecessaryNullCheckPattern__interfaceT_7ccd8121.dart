void f(int? x) {
  if (x case var a?) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
