extension E on int {
  static int x = 0;
}

f() {
  E?.x = 0;
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
