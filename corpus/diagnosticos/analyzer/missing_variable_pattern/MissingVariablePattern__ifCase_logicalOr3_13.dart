void f(num x) {
  if (x case final int a || 2 || final int a) {}
//                     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                          ^
// [diag.missingVariablePattern] Variable pattern 'a' is missing in this branch of the logical-or pattern.
//                                         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
