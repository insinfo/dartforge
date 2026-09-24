void f(num x) {
  if (x case 1 || final int a || 3) {}
//           ^
// [diag.missingVariablePattern] Variable pattern 'a' is missing in this branch of the logical-or pattern.
//                          ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                               ^
// [diag.missingVariablePattern] Variable pattern 'a' is missing in this branch of the logical-or pattern.
}
