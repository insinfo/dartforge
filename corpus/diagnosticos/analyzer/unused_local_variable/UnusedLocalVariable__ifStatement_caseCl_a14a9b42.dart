void f(Object? x) {
  if (x case int a || [int a]) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
