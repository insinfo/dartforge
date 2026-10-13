mixin M {
  static int x = 0;
}

f() {
  M?.x;
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
