void f(int x) {
  switch (x) {
    case 1 || final a:
//       ^
// [diag.missingVariablePattern] Variable pattern 'a' is missing in this branch of the logical-or pattern.
//                  ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      return;
  }
}
