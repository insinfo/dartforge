void f(int x) {
  switch (x) {
    case 1:
    case final a:
//             ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      return;
  }
}
