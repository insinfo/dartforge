void f(num x) {
  switch (x) {
    case final double a:
//                    ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      return;
    case 2:
      return;
    default:
      return;
  }
}
