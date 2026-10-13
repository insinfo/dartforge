extension E on String {
  set x(int value) {}
}

void f() {
  'a'?.x = 0;
//   ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
