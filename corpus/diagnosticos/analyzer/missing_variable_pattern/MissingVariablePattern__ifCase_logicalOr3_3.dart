void f(int x) {
  if (x case 1 || 2 || final a) {}
//           ^^^^^^
// [diag.missingVariablePattern] Variable pattern 'a' is missing in this branch of the logical-or pattern.
//                           ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
