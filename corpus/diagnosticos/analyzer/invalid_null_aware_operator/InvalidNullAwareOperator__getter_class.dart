class C {
  static int x = 0;
}

f() {
  C?.x;
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
