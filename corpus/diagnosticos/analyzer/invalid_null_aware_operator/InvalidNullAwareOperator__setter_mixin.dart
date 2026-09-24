mixin M {
  static int x = 0;
}

f() {
  M?.x = 0;
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
